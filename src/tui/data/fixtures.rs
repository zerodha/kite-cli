use super::tests::{connect_client, fixture_data, reply};
use super::*;
use httpmock::{Method::GET, MockServer};

#[tokio::test]
async fn positions_fixture_reaches_the_positions_pane() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/portfolio/positions",
        fixture_data(include_str!("../../../mock_responses/positions.json")),
    )
    .await;
    let positions = fetch_positions(&connect_client(&server)).await.unwrap();
    assert_eq!(positions.net[0].tradingsymbol, "LEADMINI17DECFUT");
    assert_eq!(positions.net[0].quantity, 1);
    assert_eq!(
        position_live_pnl(&positions.net[0], None).to_bits(),
        0.0_f64.to_bits()
    );
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Positions(Ok(positions)),
        crate::tui::workspace::Preset::Portfolio,
        "LEADMINI17DECFUT",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn holdings_fixture_reaches_the_holdings_pane() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/portfolio/holdings",
        fixture_data(include_str!("../../../mock_responses/holdings.json")),
    )
    .await;
    let holdings = fetch_holdings(&connect_client(&server)).await.unwrap();
    assert_eq!(holdings[0].tradingsymbol, "AARON");
    assert_eq!(holding_total_quantity(&holdings[0]), 1001);
    assert_eq!(holding_free_quantity(&holdings[0]), 1);
    assert_eq!(holdings[0].last_price.to_bits(), 352.95_f64.to_bits());
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Holdings(Ok(holdings)),
        crate::tui::workspace::Preset::Portfolio,
        "AARON",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn orders_fixture_reaches_the_order_book() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/orders",
        fixture_data(include_str!("../../../mock_responses/orders.json")),
    )
    .await;
    let orders = fetch_orders(&connect_client(&server)).await.unwrap();
    let order = orders
        .iter()
        .find(|order| order.order_id == "100000000000000")
        .unwrap();
    assert_eq!(order.tradingsymbol, "USDINR21JUNFUT");
    assert_eq!(order.status, "CANCELLED");
    assert_eq!(order.quantity.to_bits(), 1.0_f64.to_bits());
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Orders(Ok(orders)),
        crate::tui::workspace::Preset::Orders,
        "USDINR21JUNFUT",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn gtt_fixture_reaches_the_gtt_pane() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/gtt/triggers",
        fixture_data(include_str!("../../../mock_responses/gtt_get_orders.json")),
    )
    .await;
    let gtts = fetch_gtts(&connect_client(&server)).await.unwrap();
    assert_eq!(gtts[0].id, 112_127);
    assert_eq!(gtts[0].condition.tradingsymbol, "INFY");
    assert_eq!(gtts[0].condition.trigger_values, [702.0]);
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Gtts(Ok(gtts)),
        crate::tui::workspace::Preset::Orders,
        "702.00",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn alerts_fixture_reaches_the_alerts_pane() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/alerts",
        fixture_data(include_str!("../../../mock_responses/alerts_get.json")),
    )
    .await;
    let alerts = fetch_alerts(&connect_client(&server)).await.unwrap();
    assert_eq!(alert_label(&alerts[0]), "INDICES:NIFTY 50");
    assert_eq!(alert_condition(&alerts[0]), "Last price >= 27000");
    assert!(alert_is_enabled(&alerts[0]));
    assert!(alert_is_ato(&alerts[1]));
    assert_eq!(alert_basket_len(&alerts[1]), Some(1));
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(1),
            result: Ok(alerts),
        },
        crate::tui::workspace::Preset::Orders,
        "Last price >= 27000",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn alert_history_fixture_reaches_the_history_pane() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/alerts/u1/history",
        fixture_data(include_str!("../../../mock_responses/alerts_history.json")),
    )
    .await;
    let history = fetch_alert_history(&connect_client(&server), "u1")
        .await
        .unwrap();
    assert_eq!(history[0].created_at.to_string(), "2025-02-17 09:16:46");
    assert_eq!(
        trigger_quote(&history[0]).unwrap().last_price.to_bits(),
        58288.05_f64.to_bits()
    );
    assert!(!trigger_placed_orders(&history[0]));
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::AlertHistory {
            uuid: "u1".to_string(),
            request: AlertRequestId(1),
            result: Ok(history),
        },
        crate::tui::workspace::Preset::Orders,
        "58,288.05",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn mf_fixture_reaches_the_mf_pane_without_day_nav_fields() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/portfolio/holdings/mf",
        fixture_data(include_str!("../../../mock_responses/mf_holdings.json")),
    )
    .await;
    let holdings = fetch_mf_holdings(&connect_client(&server)).await.unwrap();
    assert_eq!(holdings[0].tradingsymbol, "INF205K01NT8");
    assert_eq!(holdings[0].quantity.to_bits(), 382.488_f64.to_bits());
    assert_eq!(holdings[0].last_price.to_bits(), 84.86_f64.to_bits());
    assert_eq!(mf_day_change_pct(&holdings[0]), None);
    assert_eq!(mf_totals(&holdings).day_pnl.to_bits(), 0.0_f64.to_bits());
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::MfHoldings(Ok(holdings)),
        crate::tui::workspace::Preset::Portfolio,
        "INF205K01NT8",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn margins_fixture_reaches_the_funds_pane() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/user/margins",
        fixture_data(include_str!("../../../mock_responses/margins.json")),
    )
    .await;
    let funds = fetch_funds(&connect_client(&server)).await.unwrap();
    assert_eq!(
        funds.equity.available.cash.to_bits(),
        245_431.6_f64.to_bits()
    );
    assert_eq!(
        funds.equity.utilised.span.to_bits(),
        101_989.0_f64.to_bits()
    );
    assert!((funds_margin_available(&funds) - 99725.05).abs() < 1e-8);
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Margins(Ok(Box::new(funds))),
        crate::tui::workspace::Preset::Portfolio,
        "₹99,725",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn quote_fixture_reaches_the_ticket_circuit_limits() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/quote",
        fixture_data(include_str!("../../../mock_responses/quote.json")),
    )
    .await;
    assert_eq!(
        fetch_circuits(&connect_client(&server), 408_065)
            .await
            .unwrap(),
        Some((1250.7, 1528.6))
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn auctions_fixture_reaches_the_auction_pane() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/portfolio/holdings/auctions",
        fixture_data(include_str!("../../../mock_responses/auctions_list.json")),
    )
    .await;
    let auctions = fetch_auctions(&connect_client(&server)).await.unwrap();
    assert_eq!(auctions[0].tradingsymbol, "ASHOKLEY");
    assert_eq!(auctions[0].auction_number, "20");
    assert_eq!(auctions[0].last_price.to_bits(), 142.5_f64.to_bits());
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Auctions(Ok(auctions)),
        crate::tui::workspace::Preset::Invest,
        "ASHOKLEY",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn an_invalid_typed_positions_reply_reaches_the_pane_error() {
    let server = MockServer::start_async().await;
    let mock = reply(
        &server,
        GET,
        "/portfolio/positions",
        serde_json::json!({"net":[{"tradingsymbol":"INFY"}]}),
    )
    .await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    spawn_refresh_positions(connect_client(&server), tx)
        .await
        .unwrap();
    let DataEvent::Positions(Err(error)) = tests::next_event(&mut rx).await else {
        panic!("expected pane error")
    };
    assert!(error.contains("missing field `exchange`"), "{error}");
    crate::tui::app::App::assert_fixture_renders(
        connect_client(&server),
        DataEvent::Positions(Err(error)),
        crate::tui::workspace::Preset::Portfolio,
        "missing field `exchange`",
    );
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn margin_preview_preserves_market_protection_on_the_wire() {
    let server = MockServer::start_async().await;
    let mock = server.mock_async(|when, then| {
        when.method(httpmock::Method::POST).path("/margins/orders")
            .json_body(serde_json::json!([{
                "exchange":"NSE", "tradingsymbol":"INFY", "transaction_type":"BUY", "variety":"regular", "product":"CNC", "order_type":"MARKET", "quantity":1,
                "price":0.0, "trigger_price":0.0, "market_protection":-1.0
            }]));
        then.status(200).header("content-type", "application/json").body(include_str!("../../../mock_responses/order_margins.json"));
    }).await;
    let (tx, mut rx) = tokio::sync::mpsc::channel(1);
    spawn_margin_preview(
        connect_client(&server),
        tx,
        1,
        serde_json::json!({
            "exchange":"NSE", "tradingsymbol":"INFY", "transaction_type":"BUY", "variety":"regular", "product":"CNC", "order_type":"MARKET", "quantity":1,
            "price":0.0, "trigger_price":0.0, "market_protection":-1
        }),
    );
    let DataEvent::MarginPreview {
        seq: 1,
        result: Ok(preview),
    } = tests::next_event(&mut rx).await
    else {
        panic!("expected margin preview")
    };
    assert_eq!(preview.total.to_bits(), 1498.0_f64.to_bits());
    assert_eq!(preview.charges.to_bits(), 1.792_551_22_f64.to_bits());
    mock.assert_calls_async(1).await;
}

#[tokio::test]
async fn an_order_timeout_reports_uncertainty_without_retrying() {
    let server = MockServer::start_async().await;
    let order = server
        .mock_async(|when, then| {
            when.method(httpmock::Method::POST).path("/orders/regular");
            then.delay(std::time::Duration::from_millis(300))
                .status(200)
                .json_body(serde_json::json!({"status":"success","data":{"order_id":"accepted"}}));
        })
        .await;
    let client = KiteConnect::builder("test-key")
        .access_token(kiteconnect::AccessToken::new("test-token").unwrap())
        .base_url(server.base_url())
        .timeout(std::time::Duration::from_millis(100))
        .build()
        .unwrap();
    let (tx, mut rx) = mpsc::channel(8);
    spawn_place_order(client, tx, "regular".into(), vec![]);
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let AppEvent::Data(DataEvent::OrderPlaced(Err(message))) = event else {
        panic!("expected uncertain write outcome")
    };
    assert!(message.contains("outcome unknown"), "{message}");
    assert!(!message.contains("retrying automatically"), "{message}");
    order.assert_calls_async(1).await;
}

#[tokio::test]
async fn cancelling_a_cover_leg_sends_its_parent_id() {
    let server = MockServer::start_async().await;
    let cancel = server
        .mock_async(|when, then| {
            when.method(httpmock::Method::DELETE)
                .path("/orders/co/child")
                .query_param("parent_order_id", "parent");
            then.status(200)
                .json_body(serde_json::json!({"status":"success","data":{"order_id":"child"}}));
        })
        .await;
    let (tx, mut rx) = mpsc::channel(8);
    spawn_cancel_orders(
        connect_client(&server),
        tx,
        vec![CancelOrderTarget {
            variety: "co".into(),
            order_id: "child".into(),
            parent_order_id: Some("parent".into()),
        }],
    );
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(
        matches!(event, AppEvent::Data(DataEvent::OrdersCancelled(result)) if result.cancelled_order_ids == ["child"])
    );
    cancel.assert_calls_async(1).await;
}
