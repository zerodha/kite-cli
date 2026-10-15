use httpmock::prelude::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use zerodha_kiteconnect::{
    Alert, AlertHistory, AlertOperator, AlertParams, AlertRhsType, AlertStatus, AlertType,
    BasketCreate, BasketItemParams, BasketOperation, BasketWrite, KiteConnect,
};

const UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
const ONE: &str = include_str!("../../../mock_responses/alerts_get_one.json");
const LIST: &str = include_str!("../../../mock_responses/alerts_get.json");
const HISTORY: &str = include_str!("../../../mock_responses/alerts_history.json");
fn client(server: &MockServer) -> KiteConnect {
    KiteConnect::builder("test")
        .base_url(server.base_url())
        .build()
        .unwrap()
}
fn params() -> AlertParams {
    AlertParams::new(
        "NIFTY 50",
        AlertType::Simple,
        "INDICES",
        "NIFTY 50",
        "LastTradedPrice",
        AlertOperator::Ge,
        AlertRhsType::Constant,
    )
    .rhs_constant(27000.0)
}
fn data(fixture: &str) -> Value {
    serde_json::from_str::<Value>(fixture).unwrap()["data"].clone()
}

#[test]
fn alert_fixtures_decode_required_fields_and_optional_baskets() {
    let alerts: Vec<Alert> = serde_json::from_value(data(LIST)).unwrap();
    assert_eq!(alerts.len(), 2);
    assert_eq!(alerts[0].uuid, UUID);
    assert_eq!(alerts[0].rhs_constant.to_bits(), f64::to_bits(27_000.0));
    assert!(alerts[0].basket.is_none());
    assert_eq!(alerts[0].created_at.to_string(), "2025-05-26 12:07:50");
    let basket = alerts[1].basket.as_ref().unwrap();
    assert!(basket.name.is_none());
    assert!(basket.basket_type.is_none());
    let item = &basket.items[0];
    assert_eq!(item.id, Some(275_218_517));
    assert_eq!(item.instrument_token, Some(3_693_569));
    assert_eq!(item.tradingsymbol, "GOLDBEES");
    assert_eq!(item.weight, 10000);
    assert_eq!(item.params["quantity"], 10000);
    assert_eq!(item.params["price"], 72.22);
    assert_eq!(item.params["transaction_type"].as_str(), Some("BUY"));
    assert_eq!(item.params["gtt"]["target"], 0.0);
    assert_eq!(item.params["gtt"]["stoploss"], 0.0);
    for fixture in [
        ONE,
        include_str!("../../../mock_responses/alerts_create.json"),
        include_str!("../../../mock_responses/alerts_modify.json"),
        include_str!("../../../mock_responses/alerts_create_ato.json"),
    ] {
        let alert: Alert = serde_json::from_value(data(fixture)).unwrap();
        assert_eq!(alert.name, "NIFTY 50");
        assert_eq!(alert.user_id, "AB1234");
        assert_eq!(alert.lhs_attribute, "LastTradedPrice");
    }
    for field in ["uuid", "rhs_constant"] {
        let mut value = data(ONE);
        value.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<Alert>(value).is_err());
    }
    let mut value = data(ONE);
    value["status"] = json!("future-status");
    assert_eq!(
        serde_json::from_value::<Alert>(value).unwrap().status,
        zerodha_kiteconnect::AlertStatus::Unknown
    );
}

#[test]
fn history_fixture_decodes_quotes_and_times() {
    let history: Vec<AlertHistory> = serde_json::from_value(data(HISTORY)).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].uuid, UUID);
    assert_eq!(
        history[0].condition,
        "LastTradedPrice(\"INDICES:NIFTY NEXT 50\") <= 58290.35"
    );
    assert!(history[0].order_meta.is_none());
    let quote = &history[0].meta[0];
    assert_eq!(quote.instrument_token, 270_857);
    assert_eq!(quote.last_price.to_bits(), f64::to_bits(58_288.05));
    assert_eq!(quote.ohlc.open.to_bits(), f64::to_bits(59_221.95));
    assert_eq!(quote.ohlc.high.to_bits(), f64::to_bits(59_241.8));
    assert_eq!(quote.ohlc.low.to_bits(), f64::to_bits(58_283.7));
    assert_eq!(quote.ohlc.close.to_bits(), f64::to_bits(59_557.95));
    assert_eq!(quote.timestamp.to_string(), "2025-02-17 09:16:46");
    assert_eq!(
        quote.last_trade_time.unwrap().to_string(),
        "0001-01-01 00:00:00"
    );
    let mut value = data(HISTORY);
    value[0]["meta"][0]["last_trade_time"] = json!("");
    assert!(
        serde_json::from_value::<Vec<AlertHistory>>(value).unwrap()[0].meta[0]
            .last_trade_time
            .is_none()
    );
}

#[tokio::test]
async fn alerts_sends_go_filter_map() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/alerts")
            .query_param("status", "enabled")
            .query_param("type", "ato");
        then.status(200).body(LIST);
    });
    let alerts = client(&server)
        .alerts(&HashMap::from([
            ("status".into(), "enabled".into()),
            ("type".into(), "ato".into()),
        ]))
        .await
        .unwrap();
    assert_eq!(alerts[1].alert_type, AlertType::Ato);
    mock.assert();
}

#[tokio::test]
async fn alert_reads_one() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path(format!("/alerts/{UUID}"));
        then.status(200).body(ONE);
    });
    assert_eq!(client(&server).alert(UUID).await.unwrap().uuid, UUID);
    mock.assert();
}

#[tokio::test]
async fn alert_history_reads_triggers() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path(format!("/alerts/{UUID}/history"));
        then.status(200).body(HISTORY);
    });
    assert_eq!(
        client(&server).alert_history(UUID).await.unwrap()[0].meta[0]
            .last_price
            .to_bits(),
        f64::to_bits(58_288.05)
    );
    mock.assert();
}

#[tokio::test]
async fn create_alert_sends_constant_in_go_field_order() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(POST).path("/alerts").header("content-type", "application/x-www-form-urlencoded").body("name=NIFTY+50&type=simple&lhs_exchange=INDICES&lhs_tradingsymbol=NIFTY+50&lhs_attribute=LastTradedPrice&operator=%3E%3D&rhs_type=constant&rhs_constant=27000");
        then.status(200).body(include_str!("../../../mock_responses/alerts_create.json"));
    });
    assert_eq!(
        client(&server)
            .create_alert(params())
            .await
            .unwrap()
            .rhs_constant
            .to_bits(),
        f64::to_bits(27_000.0)
    );
    mock.assert();
}

#[tokio::test]
async fn modify_alert_sends_instrument_rhs_and_preserves_status() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(PUT).path(format!("/alerts/{UUID}")).body("name=NIFTY+50&type=simple&lhs_exchange=INDICES&lhs_tradingsymbol=NIFTY+50&lhs_attribute=LastTradedPrice&operator=%3E%3D&rhs_type=instrument&rhs_exchange=NSE&rhs_tradingsymbol=INFY&rhs_attribute=LastTradedPrice&status=disabled");
        then.status(200).body(include_str!("../../../mock_responses/alerts_modify.json"));
    });
    assert_eq!(
        client(&server)
            .modify_alert(
                UUID,
                params()
                    .rhs_instrument("NSE", "INFY", "LastTradedPrice")
                    .status(AlertStatus::Disabled)
            )
            .await
            .unwrap()
            .rhs_constant
            .to_bits(),
        f64::to_bits(27_500.0)
    );
    mock.assert();
}

#[tokio::test]
async fn create_ato_sends_basket_json() {
    let server = MockServer::start();
    let basket = BasketCreate::new(
        "alerts-basket",
        vec![BasketOperation::Insert {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            weight: 1,
            params: BasketItemParams::Preserved(
                json!({"order_type": "LIMIT", "quantity": 10, "price": 1500.0}),
            ),
        }],
    );
    let mock = server.mock(|when, then| {
        when.method(POST)
            .path("/alerts")
            .form_urlencoded_tuple("type", "ato")
            .form_urlencoded_tuple("basket", r#"{"name":"alerts-basket","type":"alert","tags":[],"items":[{"type":"insert","exchange":"NSE","tradingsymbol":"INFY","weight":1,"params":{"order_type":"LIMIT","price":1500.0,"quantity":10}}]}"#);
        then.status(200).body(include_str!(
            "../../../mock_responses/alerts_create_ato.json"
        ));
    });
    let mut params = params();
    params.alert_type = AlertType::Ato;
    assert_eq!(
        client(&server)
            .create_alert(params.basket(BasketWrite::Create(basket)))
            .await
            .unwrap()
            .alert_type,
        AlertType::Ato
    );
    mock.assert();
}

#[tokio::test]
async fn delete_alerts_repeats_uuid_query_and_decodes_null_success() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(DELETE)
            .path("/alerts")
            .query_param("uuid", UUID)
            .query_param("uuid", "another");
        then.status(200)
            .body(include_str!("../../../mock_responses/alerts_delete.json"));
    });
    assert!(
        client(&server)
            .delete_alerts(&[UUID, "another"])
            .unwrap()
            .await
            .unwrap()
    );
    mock.assert();
}

#[test]
fn delete_alerts_rejects_empty_input_before_sending() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(DELETE).path("/alerts");
        then.status(200);
    });
    assert!(matches!(
        client(&server).delete_alerts(&[]),
        Err(zerodha_kiteconnect::Error::InvalidInput(
            "at least one uuid must be provided"
        ))
    ));
    mock.assert_calls(0);
}
