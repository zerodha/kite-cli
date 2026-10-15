use httpmock::prelude::*;
use kite::cli::{ConfirmArgs, GttWriteArgs, JsonArrayInput, OrderPlaceArgs};
use kiteconnect::{KiteConnect, Request};
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;

fn mocks_dir() -> PathBuf {
    if let Ok(path) = env::var("KITECONNECT_MOCKS_DIR") {
        let path = PathBuf::from(path);
        if path.exists() {
            return path;
        }
    }

    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let local = manifest.join("mock_responses");
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
    let is_csv = std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext == "csv");
    if is_csv {
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
            kiteconnect::AccessToken::new("test_access_token".to_string()).expect("credentials"),
        )
        .base_url(server.base_url())
        .build()
        .expect("client")
}

fn order_args(variety: &str) -> OrderPlaceArgs {
    OrderPlaceArgs {
        variety: variety.to_string(),
        exchange: "NSE".to_string(),
        tradingsymbol: "INFY".to_string(),
        transaction_type: "BUY".to_string(),
        order_type: "MARKET".to_string(),
        quantity: 1,
        product: "CNC".to_string(),
        price: None,
        trigger_price: None,
        disclosed_quantity: None,
        validity: Some("DAY".to_string()),
        validity_ttl: None,
        iceberg_legs: None,
        iceberg_quantity: None,
        auction_number: None,
        market_protection: None,
        autoslice: false,
        tag: None,
        confirm: confirm(),
    }
}

const SWITCH_TO_AMO: &str = r#"{"status":"error","data":{"hints":["switch_to_amo"]},"error_type":"InputException","message":"convert"}"#;

fn confirm() -> ConfirmArgs {
    ConfirmArgs {
        dry_run: false,
        confirm: true,
    }
}

fn as_array_len(value: &Value) -> usize {
    value.as_array().map_or(0, std::vec::Vec::len)
}

#[tokio::test]
async fn orders_and_portfolio_use_mock_contracts() {
    let server = MockServer::start();
    mount(&server, "GET", "/orders", "orders.json");
    mount(&server, "GET", "/orders/100000000000000", "order_info.json");
    mount(
        &server,
        "GET",
        "/orders/100000000000000/trades",
        "order_trades.json",
    );
    mount(&server, "POST", "/orders/regular", "order_response.json");
    mount(
        &server,
        "DELETE",
        "/orders/regular/100000000000000",
        "order_cancel.json",
    );
    mount(&server, "GET", "/portfolio/holdings", "holdings.json");
    mount(&server, "GET", "/portfolio/positions", "positions.json");
    mount(
        &server,
        "PUT",
        "/portfolio/positions",
        "convert_position.json",
    );

    let api = test_client(&server);

    let orders = api.orders().raw().await.expect("orders list");
    assert!(as_array_len(&orders) > 0);

    let order = api
        .order_history("100000000000000")
        .raw()
        .await
        .expect("order get");
    assert!(order.is_array());

    let trades = api
        .order_trades("100000000000000")
        .raw()
        .await
        .expect("order trades");
    assert!(as_array_len(&trades) > 0);

    let placed = api
        .place_order(
            kiteconnect::Variety::Regular,
            kite::util::order_place_params(&order_args("regular")).unwrap(),
        )
        .await
        .expect("place order");
    assert_eq!(placed.order_id, "151220000000000");

    let cancelled = api
        .cancel_order(kiteconnect::Variety::Regular, "100000000000000", None)
        .raw()
        .await
        .expect("cancel order");
    assert!(cancelled.get("order_id").is_some());

    let holdings = api.holdings().raw().await.expect("holdings");
    assert!(holdings.is_array());

    let positions = api.positions().raw().await.expect("positions");
    assert!(positions.get("net").is_some());

    let converted = api
        .convert_position(kiteconnect::ConvertPositionParams::new(
            kiteconnect::Exchange::Nse,
            "INFY",
            kiteconnect::Product::Mis,
            kiteconnect::Product::Cnc,
            kiteconnect::PositionType::Day,
            kiteconnect::TransactionType::Buy,
            1,
        ))
        .raw()
        .await
        .expect("convert position");
    assert_eq!(converted, Value::Bool(true));
}

/// `kite orders place` is one direct POST on the token client.
#[tokio::test]
async fn cli_place_never_switches_to_amo() {
    let server = MockServer::start();
    let regular = server.mock(|when, then| {
        when.method(POST).path("/orders/regular");
        then.status(400)
            .header("content-type", "application/json")
            .body(SWITCH_TO_AMO);
    });
    let amo = server.mock(|when, then| {
        when.method(POST).path("/orders/amo");
        then.status(200)
            .header("content-type", "application/json")
            .body(r#"{"status":"success","data":{"order_id":"unexpected"}}"#);
    });

    let err = test_client(&server)
        .place_order(
            kiteconnect::Variety::Regular,
            kite::util::order_place_params(&order_args("regular")).unwrap(),
        )
        .await
        .expect_err("the CLI returns the rejection");

    assert!(err.to_string().contains("convert"), "{err}");
    regular.assert_calls(1);
    amo.assert_calls(0);
}

#[tokio::test]
async fn margins_gtt_and_mutual_funds_use_mock_contracts() {
    let server = MockServer::start();
    mount(&server, "POST", "/margins/orders", "order_margins.json");
    mount(&server, "POST", "/margins/basket", "basket_margins.json");
    mount(
        &server,
        "POST",
        "/charges/orders",
        "virtual_contract_note.json",
    );
    mount(&server, "GET", "/gtt/triggers", "gtt_get_orders.json");
    mount(&server, "GET", "/gtt/triggers/123", "gtt_get_order.json");
    mount(&server, "POST", "/gtt/triggers", "gtt_place_order.json");
    mount(&server, "GET", "/mf/orders", "mf_orders.json");
    mount(&server, "GET", "/mf/holdings", "mf_holdings.json");
    mount(&server, "GET", "/mf/instruments", "mf_instruments.csv");

    let api = test_client(&server);

    let order_margins = api
        .call(Request::post("/margins/orders").json(kite::util::read_json_array(&JsonArrayInput {
                file: None,
                input_json: Some(
                    r#"[{"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","variety":"regular","product":"CNC","order_type":"MARKET","quantity":1,"price":0,"trigger_price":0}]"#
                        .to_string(),
                ),
                order: vec![],
            }).unwrap()))
        .raw().await
        .expect("order margins");
    assert!(order_margins.is_array());

    let basket = api
        .call(Request::post("/margins/basket").json(kite::util::read_json_array(&JsonArrayInput {
                file: None,
                input_json: Some(
                    r#"[{"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","variety":"regular","product":"CNC","order_type":"MARKET","quantity":1,"price":0,"trigger_price":0}]"#
                        .to_string(),
                ),
                order: vec![],
            }).unwrap()))
        .raw().await
        .expect("basket margins");
    assert!(basket.get("initial").is_some());

    let charges = api
        .call(Request::post("/charges/orders").json(kite::util::read_json_array(&JsonArrayInput {
                file: None,
                input_json: Some(
                    r#"[{"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","product":"CNC","order_type":"MARKET","quantity":1,"price":0}]"#
                        .to_string(),
                ),
                order: vec![],
            }).unwrap()))
        .raw().await
        .expect("charges");
    assert!(charges.is_array());

    let gtts = api.gtts().raw().await.expect("gtt list");
    assert!(gtts.is_array());

    let gtt = api.gtt(123).raw().await.expect("gtt get");
    assert!(gtt.get("id").is_some() || gtt.get("trigger_id").is_some());

    let (condition, orders) = kite::util::gtt_payload(&GttWriteArgs {
            trigger_type: Some("single".to_string()),
            trailing_points: None,
            condition_json: Some(
                r#"{"exchange":"NSE","tradingsymbol":"INFY","trigger_values":[702.0],"last_price":798.0}"#
                    .to_string(),
            ),
            condition_file: None,
            orders_json: Some(
                r#"[{"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","quantity":1,"order_type":"LIMIT","product":"CNC","price":702.5}]"#
                    .to_string(),
            ),
            orders_file: None,
            confirm: confirm(),
        }).unwrap();
    let placed = api
        .call(Request::post("/gtt/triggers").form([
            ("type", "single".to_string()),
            ("condition", condition.to_string()),
            ("orders", orders.to_string()),
        ]))
        .raw()
        .await
        .expect("gtt place");
    assert!(placed.get("trigger_id").is_some());

    let mf_orders = api.mf_orders().raw().await.expect("mf orders");
    assert!(mf_orders.is_array());

    let mf_holdings = api.mf_holdings().raw().await.expect("mf holdings");
    assert!(mf_holdings.is_array());

    let response = api.mf_instruments().response().await.expect("mf csv");
    let mf_csv = std::str::from_utf8(response.body()).expect("mf CSV UTF-8");
    assert!(mf_csv.contains("tradingsymbol") || mf_csv.contains("plan"));
}

#[test]
fn config_show_resolves_environment_config_and_build_values() {
    let path = env::temp_dir().join(format!("kite-build-values-{}.json", std::process::id()));
    let build_root = option_env!("KITE_API_ROOT").filter(|value| !value.trim().is_empty());
    let build_key = option_env!("KITE_DEFAULT_API_KEY").filter(|value| !value.trim().is_empty());
    for (config_root, config_key, env_root, env_key, expected_root, expected_key) in [
        (None, None, None, None, build_root, build_key),
        (
            Some(" "),
            Some(" "),
            Some(" "),
            Some(" "),
            build_root,
            build_key,
        ),
        (
            Some("https://config.example.invalid"),
            Some("config-key"),
            None,
            None,
            Some("https://config.example.invalid"),
            Some("config-key"),
        ),
        (
            Some("https://config.example.invalid"),
            Some("config-key"),
            Some(" "),
            Some(" "),
            Some("https://config.example.invalid"),
            Some("config-key"),
        ),
        (
            Some("https://config.example.invalid"),
            Some("config-key"),
            Some("https://env.example.invalid"),
            Some("env-key"),
            Some("https://env.example.invalid"),
            Some("env-key"),
        ),
    ] {
        fs::write(
            &path,
            serde_json::json!({"api_root":config_root,"api_key":config_key}).to_string(),
        )
        .unwrap();
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
        command.args([
            "--config",
            path.to_str().unwrap(),
            "--json",
            "config",
            "show",
        ]);
        for (name, _) in
            env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
        {
            command.env_remove(name);
        }
        if let Some(value) = env_root {
            command.env("KITE_API_ROOT", value);
        }
        if let Some(value) = env_key {
            command.env("KITE_API_KEY", value);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let data: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            data["effective"]["api_root"],
            serde_json::json!(expected_root)
        );
        assert_eq!(
            data["effective"]["api_key"],
            serde_json::json!(expected_key)
        );
        assert_eq!(data["effective"]["access_token"], Value::Null);
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn stream_ticks_uses_the_configured_url_and_requires_login_on_auth_rejection() {
    assert_stream_rejection(
        r#"{"status":"error","error_type":"TokenException","message":"Invalid `access_token` or `api_key`."}"#,
        true,
    );
}

#[test]
fn stream_ticks_reports_unsupported_auth_without_requesting_login() {
    assert_stream_rejection(
        r#"{"status":"error","message":"Invalid auth token."}"#,
        false,
    );
}

fn assert_stream_rejection(body: &'static str, session_expired: bool) {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut buffer = [0; 1024];
        while !bytes.ends_with(b"\r\n\r\n") {
            let count = stream.read(&mut buffer).unwrap();
            assert_ne!(count, 0);
            bytes.extend_from_slice(&buffer[..count]);
        }
        let request = String::from_utf8(bytes).unwrap().to_ascii_lowercase();
        assert!(
            request
                .starts_with("get /socket?api_key=dummy-key&access_token=dummy-token http/1.1\r\n")
        );
        assert!(request.contains(&format!(
            "user-agent: {}",
            kite::app::http::user_agent().to_ascii_lowercase()
        )));
        assert!(request.contains("x-kite-app-uuid: 5b697354-64d2-49c4-a080-6b25475555e3\r\n"));
        let response = format!(
            "HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
    });
    let path = env::temp_dir().join(format!(
        "kite-stream-rejected-{}-{session_expired}.json",
        std::process::id()
    ));
    fs::write(&path, serde_json::json!({
        "api_root":"https://example.invalid", "api_key":"dummy-key", "access_token":"dummy-token",
        "ticker_url":format!("ws://{address}/socket"), "app_uuid":"5b697354-64d2-49c4-a080-6b25475555e3"
    }).to_string()).unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
    command.env_clear();
    #[cfg(windows)]
    if let Some(system_root) = env::var_os("SYSTEMROOT") {
        command.env("SYSTEMROOT", system_root);
    }
    command.args([
        "--config",
        path.to_str().unwrap(),
        "--json",
        "stream",
        "ticks",
        "--token",
        "408065",
        "--timeout-seconds",
        "5",
    ]);
    let output = command.output().unwrap();
    assert!(!output.status.success());
    let data: Value = serde_json::from_slice(&output.stdout).unwrap();
    let message = data["error"]["message"].as_str().unwrap();
    assert_eq!(
        message.contains("run `kite auth login`"),
        session_expired,
        "unexpected stream rejection: {message}"
    );
    if !session_expired {
        assert!(
            message.contains("Check ticker authentication settings"),
            "unexpected stream rejection: {message}"
        );
    }
    let saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved["access_token"], "dummy-token");
    server.join().unwrap();
    fs::remove_file(path).unwrap();
}

#[test]
fn config_show_resolves_ticker_url_and_shows_app_uuid() {
    let path = env::temp_dir().join(format!("kite-ticker-values-{}.json", std::process::id()));
    let build_url = option_env!("KITE_TICKER_URL").filter(|value| !value.trim().is_empty());
    for (config_url, env_url, expected) in [
        (None, None, build_url),
        (Some(" "), Some(" "), build_url),
        (
            Some("wss://config.example.invalid"),
            None,
            Some("wss://config.example.invalid"),
        ),
        (
            Some("wss://config.example.invalid"),
            Some("wss://env.example.invalid"),
            Some("wss://env.example.invalid"),
        ),
    ] {
        fs::write(
            &path,
            serde_json::json!({"ticker_url":config_url,
            "app_uuid":"5b697354-64d2-49c4-a080-6b25475555e3"})
            .to_string(),
        )
        .unwrap();
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
        command.args([
            "--config",
            path.to_str().unwrap(),
            "--json",
            "config",
            "show",
        ]);
        for (name, _) in
            env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
        {
            command.env_remove(name);
        }
        if let Some(value) = env_url {
            command.env("KITE_TICKER_URL", value);
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        let data: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(data["effective"]["ticker_url"], serde_json::json!(expected));
        assert_eq!(
            data["effective"]["app_uuid"],
            "5b697354-64d2-49c4-a080-6b25475555e3"
        );
    }
    fs::remove_file(path).unwrap();
}

#[test]
fn missing_build_values_fail_before_interactive_login_or_network_access() {
    let path = env::temp_dir().join(format!("kite-missing-values-{}.json", std::process::id()));
    for (build_value, config, expected) in [
        (
            option_env!("KITE_API_ROOT"),
            serde_json::json!({"api_key":"test-key"}),
            "no API root configured; set KITE_API_ROOT or \"api_root\" in config.json",
        ),
        (
            option_env!("KITE_DEFAULT_API_KEY"),
            serde_json::json!({"api_root":"http://127.0.0.1:1"}),
            "no API key configured; set KITE_API_KEY or \"api_key\" in config.json",
        ),
    ] {
        if build_value.is_some_and(|value| !value.trim().is_empty()) {
            continue;
        }
        fs::write(&path, config.to_string()).unwrap();
        for args in [["auth", "login"], ["user", "profile"]] {
            let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
            command
                .args(["--config", path.to_str().unwrap(), "--json"])
                .args(args);
            for (name, _) in
                env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
            {
                command.env_remove(name);
            }
            let output = command.output().unwrap();
            assert!(!output.status.success());
            let error: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(error["error"]["message"], expected);
        }
    }
    if path.exists() {
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn mf_instrument_commands_reach_network_without_config() {
    let dir = env::temp_dir().join(format!("kite-mf-no-config-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("config.json");
    for args in [vec!["mf", "instruments"], vec!["mf", "search", "equity"]] {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
        command
            .args(["--config", path.to_str().unwrap(), "--json"])
            .args(&args)
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .env_remove("NO_PROXY")
            .env_remove("no_proxy");
        for (name, _) in
            env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
        {
            command.env_remove(name);
        }
        let output = command.output().unwrap();
        assert!(!output.status.success(), "{args:?} unexpectedly succeeded");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            !text.contains("no API root configured"),
            "{args:?} required API root configuration"
        );
        assert!(
            !text.contains("no API key configured"),
            "{args:?} required API key configuration"
        );
        let error: Value = serde_json::from_slice(&output.stdout).unwrap();
        let message = error["error"]["message"].as_str().unwrap();
        assert!(message.starts_with("request failed:"), "{args:?}");
        assert!(message.to_lowercase().contains("connect"), "{args:?}");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn logout_revokes_saved_tokens_and_clears_local_fields_even_on_failure() {
    for status in [200, 403] {
        let server = MockServer::start();
        let mock = server.mock(|when, then| {
            when.method(DELETE)
                .path("/session/token")
                .query_param("api_key", "saved-key")
                .query_param("access_token", "SENTINEL-TOKEN")
                .header("x-kite-app-uuid", "5b697354-64d2-49c4-a080-6b25475555e3")
                .header_exists("user-agent");
            if status == 200 {
                then.status(200)
                    .json_body(serde_json::json!({"status":"success","data":true}));
            } else {
                then.status(403).json_body(serde_json::json!({
                    "status":"error", "error_type":"TokenException", "message":"Already invalidated"
                }));
            }
        });
        let path = env::temp_dir().join(format!(
            "kite-logout-revoke-{}-{status}.json",
            std::process::id()
        ));
        fs::write(
            &path,
            serde_json::json!({
                "api_root":server.base_url(), "api_key":"saved-key", "user_id":"AB1234",
                "access_token":"SENTINEL-TOKEN", "refresh_token":"SENTINEL-REFRESH",
                "app_uuid":"5b697354-64d2-49c4-a080-6b25475555e3"
            })
            .to_string(),
        )
        .unwrap();
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
        command.args([
            "--config",
            path.to_str().unwrap(),
            "--json",
            "auth",
            "logout",
            "--confirm",
        ]);
        for (name, _) in
            env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
        {
            command.env_remove(name);
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            serde_json::json!({
                "logged_out":true, "revoked":status == 200,
                "cleared_config_fields":["access_token"], "path":path,
            })
        );
        assert_eq!(
            serde_json::from_str::<Value>(&fs::read_to_string(&path).unwrap()).unwrap(),
            serde_json::json!({
                "api_root":server.base_url(), "api_key":"saved-key", "user_id":"AB1234",
                "access_token":null, "app_uuid":"5b697354-64d2-49c4-a080-6b25475555e3"
            })
        );
        mock.assert();
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn logout_clears_saved_session_when_api_values_are_missing() {
    let path = env::temp_dir().join(format!("kite-logout-missing-{}.json", std::process::id()));
    for (build_value, mut config, warning) in [
        (
            option_env!("KITE_API_ROOT"),
            serde_json::json!({"api_key":null}),
            "no API root configured",
        ),
        (
            option_env!("KITE_DEFAULT_API_KEY"),
            serde_json::json!({"api_root":"http://127.0.0.1:1", "api_key":null}),
            "no API key configured",
        ),
    ] {
        if build_value.is_some_and(|value| !value.trim().is_empty()) {
            continue;
        }
        config["user_id"] = serde_json::json!("AB1234");
        config["access_token"] = serde_json::json!("SENTINEL-TOKEN");
        config["refresh_token"] = serde_json::json!("SENTINEL-REFRESH");
        config["app_uuid"] = serde_json::json!("5b697354-64d2-49c4-a080-6b25475555e3");
        fs::write(&path, config.to_string()).unwrap();
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
        command.args([
            "--config",
            path.to_str().unwrap(),
            "--json",
            "auth",
            "logout",
            "--confirm",
        ]);
        for (name, _) in
            env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
        {
            command.env_remove(name);
        }
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            serde_json::json!({
                "logged_out":true, "revoked":false,
                "cleared_config_fields":["access_token"], "path":path,
            })
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains(warning));
        config["access_token"] = Value::Null;
        config.as_object_mut().unwrap().remove("refresh_token");
        config
            .as_object_mut()
            .unwrap()
            .retain(|_, value| !value.is_null());
        config["access_token"] = Value::Null;
        assert_eq!(
            serde_json::from_str::<Value>(&fs::read_to_string(&path).unwrap()).unwrap(),
            config
        );
    }
    if path.exists() {
        fs::remove_file(path).unwrap();
    }
}

#[tokio::test]
async fn runtime_client_uses_the_configured_api_root_and_key() {
    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.method(GET)
                .path("/user/profile")
                .header("Authorization", "token test-key:test-token");
            then.status(200)
                .json_body(serde_json::json!({"status":"success", "data":{"user_id":"AB1234"}}));
        })
        .await;
    let path = env::temp_dir().join(format!("kite-configured-root-{}.json", std::process::id()));
    fs::write(&path, serde_json::json!({"api_root":server.base_url(), "api_key":"test-key", "access_token":"test-token"}).to_string()).unwrap();
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_kite"));
    command.args([
        "--config",
        path.to_str().unwrap(),
        "--json",
        "request",
        "get",
        "/user/profile",
    ]);
    for (name, _) in env::vars_os().filter(|(name, _)| name.to_string_lossy().starts_with("KITE_"))
    {
        command.env_remove(name);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let data: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        data["response"],
        serde_json::json!({"status":"success", "data":{"user_id":"AB1234"}})
    );
    mock.assert_calls_async(1).await;
    fs::remove_file(path).unwrap();
}
