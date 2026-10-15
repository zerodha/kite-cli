use httpmock::prelude::*;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;
use zerodha_kiteconnect::AlertParams;
use zerodha_kiteconnect::KiteConnect;
use zerodha_kiteconnect::Request;

fn mocks_dir() -> PathBuf {
    if let Ok(path) = env::var("KITECONNECT_MOCKS_DIR") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let local = manifest.join("..").join("..").join("mock_responses");
    if local.join("README.md").exists() {
        return local;
    }

    for ancestor in manifest.ancestors() {
        let candidate = ancestor
            .join("workspace")
            .join("code")
            .join("github.com")
            .join("zerodha")
            .join("kiteconnect-mocks");
        if candidate.exists() {
            return candidate;
        }
    }

    panic!(
        "kiteconnect-mocks not found. Initialize the mock_responses submodule or set KITECONNECT_MOCKS_DIR to its directory"
    );
}

fn fixture(name: &str) -> String {
    fs::read_to_string(mocks_dir().join(name)).unwrap_or_else(|err| panic!("fixture {name}: {err}"))
}

fn content_type(name: &str) -> &'static str {
    if std::path::Path::new(name)
        .extension()
        .is_some_and(|extension| extension == "csv")
    {
        "text/csv"
    } else {
        "application/json"
    }
}

fn mount(server: &MockServer, method: &str, path: &str, fixture_name: &str) {
    let body = fixture(fixture_name);
    server.mock(|when, then| {
        when.method(method).path(path);
        then.status(200)
            .header("content-type", content_type(fixture_name))
            .body(body);
    });
}

fn test_client(server: &MockServer) -> KiteConnect {
    KiteConnect::builder("test_api_key".to_string())
        .access_token(
            zerodha_kiteconnect::AccessToken::new("test_access_token".to_string())
                .expect("credentials"),
        )
        .base_url(server.base_url())
        .build()
        .expect("client")
}

fn as_array_len(value: &Value) -> usize {
    value.as_array().map_or(0, Vec::len)
}

#[tokio::test]
async fn user_profile_matches_kiteconnect_mocks() {
    let server = MockServer::start();
    mount(&server, "GET", "/user/profile", "profile.json");
    mount(&server, "GET", "/user/margins", "margins.json");

    let api = test_client(&server);
    let profile = api.profile().await.expect("profile");
    assert_eq!(profile.user_id, "AB1234");
    assert_eq!(profile.user_shortname, "AxAx");

    let margins = api.margins().raw().await.expect("margins");
    assert!(margins.get("equity").is_some());
}

#[tokio::test]
async fn market_historical_and_raw_request_use_mock_contracts() {
    let server = MockServer::start();
    mount(&server, "GET", "/instruments", "instruments_all.csv");
    mount(&server, "GET", "/quote", "quote.json");
    mount(&server, "GET", "/quote/ltp", "ltp.json");
    mount(
        &server,
        "GET",
        "/instruments/historical/123/minute",
        "historical_minute.json",
    );
    mount(&server, "GET", "/user/profile", "profile.json");

    let api = test_client(&server);

    let quote = api.quote(&["NSE:INFY"]).await.expect("market quote");
    assert_eq!(quote["NSE:INFY"].instrument_token, 408_065);

    let ltp = api.ltp(&["NSE:INFY"]).await.expect("ltp");
    assert!(ltp.contains_key("NSE:INFY"));

    let call = api.instruments();
    let endpoint = call.request().path().to_string();
    let csv = call.response().await.expect("instruments csv");
    assert_eq!(endpoint, "/instruments");
    assert!(
        std::str::from_utf8(csv.body())
            .unwrap()
            .contains("instrument_token")
    );

    let candles = api
        .historical_data(
            123,
            "minute",
            chrono::NaiveDate::from_ymd_opt(2021, 1, 1)
                .unwrap()
                .and_hms_opt(9, 15, 0)
                .unwrap(),
            chrono::NaiveDate::from_ymd_opt(2021, 1, 1)
                .unwrap()
                .and_hms_opt(15, 30, 0)
                .unwrap(),
            false,
            false,
        )
        .await
        .expect("historical candles");
    assert_eq!(candles[0].open.to_bits(), f64::to_bits(1_704.5));

    let raw = api
        .call(Request::get("/user/profile"))
        .response()
        .await
        .expect("raw get");
    assert_eq!(raw.status(), 200);
    let body: Value = serde_json::from_slice(raw.body()).unwrap();
    assert_eq!(body["status"], "success");
}

#[tokio::test]
async fn alerts_use_mock_contracts() {
    const UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
    let server = MockServer::start();
    mount(&server, "GET", "/alerts", "alerts_get.json");
    mount(
        &server,
        "GET",
        &format!("/alerts/{UUID}"),
        "alerts_get_one.json",
    );
    mount(
        &server,
        "GET",
        &format!("/alerts/{UUID}/history"),
        "alerts_history.json",
    );
    mount(&server, "POST", "/alerts", "alerts_create.json");
    mount(
        &server,
        "PUT",
        &format!("/alerts/{UUID}"),
        "alerts_modify.json",
    );
    let delete_by_query = server.mock(|when, then| {
        when.method(DELETE)
            .path("/alerts")
            .query_param("uuid", UUID);
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("alerts_delete.json"));
    });
    let delete_by_path = server.mock(|when, then| {
        when.method(DELETE).path(format!("/alerts/{UUID}"));
        then.status(200)
            .header("content-type", "application/json")
            .body(fixture("alerts_delete.json"));
    });

    let client = test_client(&server);

    let listed = client
        .alerts(&std::collections::HashMap::new())
        .raw()
        .await
        .expect("alerts list");
    assert_eq!(as_array_len(&listed), 2);
    assert_eq!(listed[0]["uuid"], UUID);
    assert_eq!(listed[1]["type"], "ato");

    let one = client.alert(UUID).raw().await.expect("alert get");
    assert_eq!(one["uuid"], UUID);
    assert_eq!(one["lhs_attribute"], "LastTradedPrice");

    let history = client
        .alert_history(UUID)
        .raw()
        .await
        .expect("alert history");
    assert!(as_array_len(&history) > 0);
    assert!(
        history[0]["condition"]
            .as_str()
            .expect("condition string")
            .contains("LastTradedPrice")
    );

    let params = AlertParams::new(
        "NIFTY 50",
        zerodha_kiteconnect::AlertType::Simple,
        "INDICES",
        "NIFTY 50",
        "LastTradedPrice",
        zerodha_kiteconnect::AlertOperator::Ge,
        zerodha_kiteconnect::AlertRhsType::Constant,
    )
    .rhs_constant(27000.0)
    .status(zerodha_kiteconnect::AlertStatus::Enabled);

    let created = client
        .create_alert(params.clone())
        .raw()
        .await
        .expect("alert create");
    assert_eq!(created["uuid"], UUID);
    assert_eq!(created["status"], "enabled");

    let modified = client
        .modify_alert(UUID, params.rhs_constant(27500.0))
        .raw()
        .await
        .expect("alert modify");
    assert_eq!(modified["rhs_constant"], 27500.0);

    let deleted = client
        .delete_alerts(&[UUID])
        .unwrap()
        .raw()
        .await
        .expect("alert delete");
    assert_eq!(deleted, Value::Null);
    delete_by_query.assert_calls(1);
    delete_by_path.assert_calls(0);
}
