use chrono::NaiveDate;
use httpmock::prelude::*;
use serde_json::{Value, json};
use zerodha_kiteconnect::{Error, KiteConnect, Quote, QuoteLtp, QuoteOhlc};

const QUOTE: &str = include_str!("../../../mock_responses/quote.json");
const LTP: &str = include_str!("../../../mock_responses/ltp.json");
const OHLC: &str = include_str!("../../../mock_responses/ohlc.json");
const INSTRUMENTS: &str = include_str!("../../../mock_responses/instruments_all.csv");
const NSE_INSTRUMENTS: &str = include_str!("../../../mock_responses/instruments_nse.csv");
const MINUTE: &str = include_str!("../../../mock_responses/historical_minute.json");
const OI: &str = include_str!("../../../mock_responses/historical_oi.json");

fn client(server: &MockServer) -> KiteConnect {
    KiteConnect::builder("fixture-key")
        .base_url(server.base_url())
        .build()
        .unwrap()
}

const INSTRUMENT_IDS: &[&str] = &["NSE:INFY", "BSE:SENSEX"];

#[tokio::test]
async fn get_quote_decodes_quote_and_depth_and_preserves_raw() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/quote")
            .query_param("i", "NSE:INFY")
            .query_param("i", "BSE:SENSEX");
        then.status(200).body(QUOTE);
    });
    let api = client(&server);
    let quotes = api.quote(INSTRUMENT_IDS).await.unwrap();
    let quote = &quotes["NSE:INFY"];
    assert_eq!(quote.instrument_token, 408_065);
    assert_eq!(quote.last_price.to_bits(), f64::to_bits(1_412.95));
    assert_eq!(quote.volume, Some(7_360_198));
    assert_eq!(quote.last_quantity, Some(5));
    assert_eq!(quote.timestamp.to_string(), "2021-06-08 15:45:56");
    assert_eq!(
        quote.last_trade_time.unwrap().to_string(),
        "2021-06-08 15:45:52"
    );
    assert_eq!(quote.ohlc.open.to_bits(), f64::to_bits(1_396.0));
    assert_eq!(quote.ohlc.high.to_bits(), f64::to_bits(1_421.75));
    let depth = quote.depth.as_ref().unwrap();
    assert_eq!(depth.buy.len(), 5);
    assert_eq!(depth.sell.len(), 5);
    assert_eq!(depth.sell[0].price.to_bits(), f64::to_bits(1_412.95));
    assert_eq!(depth.sell[0].quantity, 5191);
    assert_eq!(depth.sell[0].orders, 13);
    let expected: Value = serde_json::from_str(QUOTE).unwrap();
    assert_eq!(
        api.quote(INSTRUMENT_IDS).raw().await.unwrap(),
        expected["data"]
    );
    mock.assert_calls(2);
}

#[tokio::test]
async fn get_ltp_decodes_fixture() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/quote/ltp")
            .query_param("i", "NSE:INFY")
            .query_param("i", "BSE:SENSEX");
        then.status(200).body(LTP);
    });
    let quotes = client(&server).ltp(INSTRUMENT_IDS).await.unwrap();
    assert_eq!(quotes["NSE:INFY"].instrument_token, 408_065);
    assert_eq!(
        quotes["NSE:INFY"].last_price.to_bits(),
        f64::to_bits(1_074.35)
    );
    assert!(!quotes.contains_key("BSE:SENSEX"));
    mock.assert();
}

#[tokio::test]
async fn get_ohlc_decodes_fixture() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/quote/ohlc")
            .query_param("i", "NSE:INFY")
            .query_param("i", "BSE:SENSEX");
        then.status(200).body(OHLC);
    });
    let quotes = client(&server).ohlc(INSTRUMENT_IDS).await.unwrap();
    let quote = &quotes["NSE:INFY"];
    assert_eq!(quote.instrument_token, 408_065);
    assert_eq!(quote.last_price.to_bits(), f64::to_bits(1_075.0));
    assert_eq!(quote.ohlc.open.to_bits(), f64::to_bits(1_085.8));
    assert_eq!(quote.ohlc.low.to_bits(), f64::to_bits(1_070.9));
    assert_eq!(quote.ohlc.close.to_bits(), f64::to_bits(1_075.8));
    mock.assert();
}

#[tokio::test]
async fn get_instruments_decodes_csv_and_preserves_response() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/instruments")
            .query_param_count(".*", ".*", 0);
        then.status(200)
            .header("content-type", "text/csv")
            .body(INSTRUMENTS);
    });
    let api = client(&server);
    let instruments = api.instruments().await.unwrap();
    assert_eq!(instruments[0].instrument_token, 3_813_889);
    assert_eq!(instruments[0].exchange_token, 14898);
    assert_eq!(instruments[0].tradingsymbol, "CENTRALBK-BE");
    assert_eq!(instruments[0].name, "CENTRAL BANK OF INDIA");
    assert_eq!(instruments[0].expiry, None);
    assert_eq!(instruments[0].tick_size.to_bits(), f64::to_bits(0.05));
    let option = instruments
        .iter()
        .find(|i| i.instrument_token == 12_073_986)
        .unwrap();
    assert_eq!(option.expiry, NaiveDate::from_ymd_opt(2018, 1, 25));
    assert_eq!(option.strike.to_bits(), f64::to_bits(23_500.0));
    assert_eq!(option.lot_size.to_bits(), f64::to_bits(40.0));
    assert_eq!(option.segment, "NFO-OPT");
    let response = api.instruments().response().await.unwrap();
    assert_eq!(response.body(), INSTRUMENTS.as_bytes());
    mock.assert_calls(2);
}

#[tokio::test]
async fn get_instruments_by_exchange_decodes_csv() {
    let server = MockServer::start_async().await;
    let mock = server.mock(|when, then| {
        when.method(GET)
            .path("/instruments/NSE")
            .query_param_count(".*", ".*", 0);
        then.status(200).body(NSE_INSTRUMENTS);
    });
    let instruments = client(&server)
        .instruments_by_exchange("NSE")
        .await
        .unwrap();
    assert_eq!(instruments[0].instrument_token, 3_813_889);
    assert_eq!(instruments[0].last_price.to_bits(), f64::to_bits(0.0));
    assert_eq!(instruments[0].exchange, "NSE");
    assert_eq!(instruments[0].instrument_type, "EQ");
    mock.assert();
}

#[tokio::test]
async fn get_historical_data_decodes_both_fixtures_and_flags() {
    let server = MockServer::start_async().await;
    let from = NaiveDate::from_ymd_opt(2017, 12, 15)
        .unwrap()
        .and_hms_opt(9, 15, 0)
        .unwrap();
    let to = NaiveDate::from_ymd_opt(2017, 12, 15)
        .unwrap()
        .and_hms_opt(15, 30, 0)
        .unwrap();
    let api = client(&server);
    for (continuous, oi, fixture) in [(false, false, MINUTE), (true, true, OI)] {
        let mock = server.mock(|when, then| {
            when.method(GET)
                .path("/instruments/historical/408065/minute")
                .query_param("from", "2017-12-15 09:15:00")
                .query_param("to", "2017-12-15 15:30:00")
                .query_param("continuous", if continuous { "1" } else { "0" })
                .query_param("oi", if oi { "1" } else { "0" })
                .query_param("instrument_token", "408065")
                .query_param("interval", "minute");
            then.status(200).body(fixture);
        });
        let candles = api
            .historical_data(408_065, "minute", from, to, continuous, oi)
            .await
            .unwrap();
        let candle = &candles[0];
        assert_eq!(candle.date.offset().local_minus_utc(), 19800);
        if oi {
            assert_eq!(candle.date.to_rfc3339(), "2019-12-04T09:15:00+05:30");
            assert_eq!(candle.open.to_bits(), f64::to_bits(12_009.9));
            assert_eq!(candle.volume, 163_275);
            assert_eq!(candle.oi, Some(13_667_775));
        } else {
            assert_eq!(candle.date.to_rfc3339(), "2017-12-15T09:15:00+05:30");
            assert_eq!(candle.open.to_bits(), f64::to_bits(1_704.5));
            assert_eq!(candle.high.to_bits(), f64::to_bits(1_705.0));
            assert_eq!(candle.low.to_bits(), f64::to_bits(1_699.25));
            assert_eq!(candle.close.to_bits(), f64::to_bits(1_702.8));
            assert_eq!(candle.volume, 2499);
            assert_eq!(candle.oi, None);
        }
        let expected: Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(
            api.historical_data(408_065, "minute", from, to, continuous, oi)
                .raw()
                .await
                .unwrap(),
            expected["data"]
        );
        let serialized = serde_json::to_value(candle).unwrap();
        let round_trip: zerodha_kiteconnect::Candle = serde_json::from_value(serialized).unwrap();
        assert_eq!(round_trip.date, candle.date);
        mock.assert_calls(2);
    }
}

#[test]
fn quotes_require_ids_and_prices_and_accept_unknown_fields() {
    for fixture in [QUOTE, LTP, OHLC] {
        let envelope: Value = serde_json::from_str(fixture).unwrap();
        for field in ["instrument_token", "last_price"] {
            let mut value = envelope["data"]["NSE:INFY"].clone();
            value.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<QuoteLtp>(value).is_err());
        }
    }
    let mut quote: Value = serde_json::from_str(QUOTE).unwrap();
    quote["data"]["NSE:INFY"]["future_field"] = json!(true);
    quote["data"]["NSE:INFY"]["last_trade_time"] = json!("");
    let quote: Quote = serde_json::from_value(quote["data"]["NSE:INFY"].clone()).unwrap();
    assert_eq!(quote.last_trade_time, None);
    assert_eq!(quote.last_price.to_bits(), f64::to_bits(1_412.95));
    assert!(
        serde_json::from_value::<QuoteOhlc>(json!({"instrument_token": 1, "last_price": 1}))
            .is_err()
    );
}

#[tokio::test]
async fn malformed_csv_and_candles_return_decode_errors() {
    let server = MockServer::start_async().await;
    let csv = server.mock(|when, then| {
        when.method(GET).path("/instruments");
        then.status(200)
            .body(INSTRUMENTS.replacen("3813889", "invalid", 1));
    });
    assert!(
        matches!(client(&server).instruments().await, Err(Error::Decode { path, .. }) if path == "/instruments")
    );
    csv.assert();
    let mut error = server.mock(|when, then| {
        when.method(GET).path("/instruments/NSE");
        then.status(200)
            .body(r#"{"status":"error","error_type":"InputException","message":"bad exchange"}"#);
    });
    assert!(matches!(
        client(&server).instruments_by_exchange("NSE").await,
        Err(Error::Api(error)) if error.message == "bad exchange"
    ));
    error.assert();
    error.delete();
    let from = NaiveDate::from_ymd_opt(2021, 1, 1)
        .unwrap()
        .and_hms_opt(9, 15, 0)
        .unwrap();
    for row in [
        json!(["invalid", 1, 2, 3, 4, 5]),
        json!(["2021-01-01T09:15:00+0530", 1, 2, 3, 4]),
        json!(["2021-01-01T09:15:00+0530", 1, 2, 3, 4, "invalid"]),
        json!(["2021-01-01T09:15:00+0530", 1, 2, 3, 4, 5, "invalid-oi"]),
        json!(["2021-01-01T09:15:00+0530", 1, null, 3, 4, 5]),
    ] {
        let mut mock = server.mock(|when, then| {
            when.method(GET).path("/instruments/historical/1/minute");
            then.status(200)
                .json_body(json!({"status":"success", "data":{"candles":[row]}}));
        });
        assert!(matches!(
            client(&server)
                .historical_data(1, "minute", from, from, false, false)
                .await,
            Err(Error::Decode { .. })
        ));
        mock.assert();
        mock.delete();
    }
}
