use crate::{Call, KiteConnect, Request, TransactionType};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Response from the Kite GET /mf/holdings endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MfHolding {
    /// Folio number of the holding.
    pub folio: Option<String>,
    /// Name of the mutual fund.
    pub fund: Option<String>,
    /// Trading symbol of the scheme (its ISIN).
    pub tradingsymbol: String,
    /// Average purchase price per unit.
    pub average_price: f64,
    /// Last price (NAV) per unit.
    pub last_price: f64,
    /// Date of the last price.
    #[serde(default, with = "crate::time::optional_date")]
    pub last_price_date: Option<NaiveDate>,
    /// Profit and loss on the holding.
    pub pnl: f64,
    /// Number of units held.
    pub quantity: f64,
    /// Previous closing price (NAV) per unit.
    pub close_price: Option<f64>,
    /// Change in price since the previous day.
    pub day_change: Option<f64>,
    /// Percentage change in price since the previous day.
    pub day_change_percentage: Option<f64>,
}

/// Response from the Kite GET /mf/holdings/{isin} endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MfTrade {
    /// Name of the mutual fund.
    pub fund: String,
    /// Trading symbol of the scheme (its ISIN).
    pub tradingsymbol: String,
    /// Average price per unit.
    pub average_price: f64,
    /// Order variety.
    pub variety: String,
    /// Exchange timestamp of the trade.
    #[serde(default, with = "crate::time::optional_datetime")]
    pub exchange_timestamp: Option<NaiveDateTime>,
    /// Amount of the trade.
    pub amount: f64,
    /// Folio number.
    pub folio: String,
    /// Number of units.
    pub quantity: f64,
}

/// Response from the Kite `GET /mf/orders` and `GET /mf/orders/{order_id}` endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MfOrder {
    /// ID of the order.
    pub order_id: String,
    /// Exchange-side order ID, if assigned.
    pub exchange_order_id: Option<String>,
    /// Trading symbol of the scheme (its ISIN).
    pub tradingsymbol: String,
    /// Order status.
    pub status: String,
    /// Status message for the order, if any.
    pub status_message: Option<String>,
    /// Folio number.
    pub folio: Option<String>,
    /// Name of the mutual fund.
    pub fund: String,
    /// Time the order was placed.
    #[serde(with = "crate::time::datetime")]
    pub order_timestamp: NaiveDateTime,
    /// Exchange timestamp.
    #[serde(default, with = "crate::time::optional_datetime")]
    pub exchange_timestamp: Option<NaiveDateTime>,
    /// Settlement ID, if assigned.
    pub settlement_id: Option<String>,
    /// Transaction type (`BUY` or `SELL`).
    pub transaction_type: String,
    /// Order variety.
    pub variety: String,
    /// Purchase type of the order.
    pub purchase_type: String,
    /// Number of units.
    pub quantity: f64,
    /// Amount of the order.
    pub amount: f64,
    /// Last price (NAV) per unit.
    pub last_price: f64,
    /// Average price per unit.
    pub average_price: f64,
    /// ID of the user who placed the order.
    pub placed_by: String,
    /// Optional tag attached to the request.
    pub tag: Option<String>,
}

/// Response from the Kite `GET /mf/sips` and `GET /mf/sips/{sip_id}` endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MfSip {
    /// SIP ID.
    pub sip_id: String,
    /// Trading symbol of the scheme (its ISIN).
    pub tradingsymbol: String,
    /// Name of the mutual fund.
    pub fund: String,
    /// Dividend option of the scheme.
    pub dividend_type: String,
    /// Transaction type (`BUY` or `SELL`).
    pub transaction_type: String,
    /// Status of the SIP.
    pub status: String,
    /// SIP type.
    pub sip_type: String,
    /// Time the SIP was created.
    #[serde(with = "crate::time::datetime")]
    pub created: NaiveDateTime,
    /// Instalment frequency (for example `weekly` or `monthly`).
    pub frequency: String,
    /// Amount of each instalment.
    pub instalment_amount: f64,
    /// Number of instalments (-1 when open ended).
    pub instalments: i64,
    /// Time of the last instalment.
    #[serde(default, with = "crate::time::optional_datetime")]
    pub last_instalment: Option<NaiveDateTime>,
    /// Number of pending instalments.
    pub pending_instalments: i64,
    /// Day of the month on which instalments run.
    pub instalment_day: i64,
    /// Number of completed instalments.
    pub completed_instalments: i64,
    /// Date of the next instalment.
    #[serde(default, with = "crate::time::optional_date")]
    pub next_instalment: Option<NaiveDate>,
    /// Trigger price for the SIP.
    pub trigger_price: f64,
    /// Step-up percentages keyed by day and month.
    #[serde(default)]
    pub step_up: BTreeMap<String, i64>,
    /// Optional tag attached to the request.
    pub tag: Option<String>,
}

/// Response from the Kite POST and DELETE /mf/orders endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MfOrderResponse {
    /// ID of the order.
    pub order_id: String,
}

/// Response from the Kite POST, PUT and DELETE /mf/sips endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MfSipResponse {
    /// ID of the order, when one is created.
    pub order_id: Option<String>,
    /// ID of the SIP.
    pub sip_id: String,
}

/// Response from the Kite GET /mf/instruments CSV endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct MfInstrument {
    /// Trading symbol of the scheme (its ISIN).
    pub tradingsymbol: String,
    /// Name of the scheme.
    pub name: String,
    /// Last price (NAV) per unit.
    pub last_price: f64,
    /// Asset management company of the scheme.
    pub amc: String,
    /// Whether purchases are allowed.
    #[serde(deserialize_with = "csv_bool")]
    pub purchase_allowed: bool,
    /// Whether redemptions are allowed.
    #[serde(deserialize_with = "csv_bool")]
    pub redemption_allowed: bool,
    /// Minimum amount for a first purchase.
    pub minimum_purchase_amount: f64,
    /// Multiple in which the purchase amount must be given.
    pub purchase_amount_multiplier: f64,
    /// Minimum amount for an additional purchase.
    #[serde(alias = "additional_purchase_multiple")]
    pub minimum_additional_purchase_amount: f64,
    /// Minimum units that can be redeemed.
    pub minimum_redemption_quantity: f64,
    /// Multiple in which the redemption quantity must be given.
    pub redemption_quantity_multiplier: f64,
    /// Dividend option of the scheme.
    pub dividend_type: String,
    /// Scheme type.
    pub scheme_type: String,
    /// Plan type of the scheme.
    pub plan: String,
    /// Settlement type of the scheme.
    pub settlement_type: String,
    /// Date of the last price.
    #[serde(with = "crate::time::date")]
    pub last_price_date: NaiveDate,
}

/// Form parameters for the Kite POST /mf/orders endpoint.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[must_use]
pub struct MfOrderParams {
    /// Trading symbol of the scheme.
    pub tradingsymbol: String,
    /// Transaction type (`BUY` or `SELL`).
    pub transaction_type: TransactionType,
    /// Number of units (used for sell orders).
    pub quantity: Option<f64>,
    /// Amount to invest (used for buy orders).
    pub amount: Option<f64>,
    /// Optional tag attached to the request.
    pub tag: Option<String>,
}

impl MfOrderParams {
    /// Creates params with the required fields set.
    pub fn new(tradingsymbol: impl Into<String>, transaction_type: TransactionType) -> Self {
        Self {
            tradingsymbol: tradingsymbol.into(),
            transaction_type,
            quantity: None,
            amount: None,
            tag: None,
        }
    }
    /// Sets the number of units.
    pub fn quantity(mut self, quantity: f64) -> Self {
        self.quantity = Some(quantity);
        self
    }
    /// Sets the order amount.
    pub fn amount(mut self, amount: f64) -> Self {
        self.amount = Some(amount);
        self
    }
    /// Sets an optional tag for the order.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }
    fn into_form(self) -> impl Iterator<Item = (&'static str, String)> {
        [
            ("tradingsymbol", Some(self.tradingsymbol)),
            ("transaction_type", Some(self.transaction_type.to_string())),
            ("quantity", self.quantity.map(|v| v.to_string())),
            ("amount", self.amount.map(|v| v.to_string())),
            ("tag", self.tag),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
    }
}

/// Form parameters for the Kite POST /mf/sips endpoint.
#[derive(Debug, Clone)]
#[non_exhaustive]
#[must_use]
pub struct MfSipParams {
    /// Trading symbol of the scheme.
    pub tradingsymbol: String,
    /// Amount of each instalment.
    pub amount: f64,
    /// Number of instalments (-1 when open ended).
    pub instalments: i64,
    /// Instalment frequency (for example `weekly` or `monthly`).
    pub frequency: String,
    /// Day of the month on which instalments run.
    pub instalment_day: Option<i64>,
    /// Amount for the initial purchase.
    pub initial_amount: Option<f64>,
    /// Trigger price for the SIP.
    pub trigger_price: Option<f64>,
    /// Step-up setting, as the raw request string.
    pub step_up: Option<String>,
    /// SIP type.
    pub sip_type: Option<String>,
    /// Optional tag attached to the request.
    pub tag: Option<String>,
}

impl MfSipParams {
    /// Creates params with the required fields set.
    pub fn new(
        tradingsymbol: impl Into<String>,
        amount: f64,
        instalments: i64,
        frequency: impl Into<String>,
    ) -> Self {
        Self {
            tradingsymbol: tradingsymbol.into(),
            amount,
            instalments,
            frequency: frequency.into(),
            instalment_day: None,
            initial_amount: None,
            trigger_price: None,
            step_up: None,
            sip_type: None,
            tag: None,
        }
    }
    /// Sets the day of the month for instalments.
    pub fn instalment_day(mut self, instalment_day: i64) -> Self {
        self.instalment_day = Some(instalment_day);
        self
    }
    /// Sets the amount for the initial purchase.
    pub fn initial_amount(mut self, initial_amount: f64) -> Self {
        self.initial_amount = Some(initial_amount);
        self
    }
    /// Sets the trigger price.
    pub fn trigger_price(mut self, trigger_price: f64) -> Self {
        self.trigger_price = Some(trigger_price);
        self
    }
    /// Sets the step-up setting.
    pub fn step_up(mut self, step_up: impl Into<String>) -> Self {
        self.step_up = Some(step_up.into());
        self
    }
    /// Sets the SIP type.
    pub fn sip_type(mut self, sip_type: impl Into<String>) -> Self {
        self.sip_type = Some(sip_type.into());
        self
    }
    /// Sets an optional tag for the SIP.
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }
    fn into_form(self) -> impl Iterator<Item = (&'static str, String)> {
        [
            ("tradingsymbol", Some(self.tradingsymbol)),
            ("amount", Some(self.amount.to_string())),
            ("instalments", Some(self.instalments.to_string())),
            ("frequency", Some(self.frequency)),
            ("instalment_day", self.instalment_day.map(|v| v.to_string())),
            ("initial_amount", self.initial_amount.map(|v| v.to_string())),
            ("trigger_price", self.trigger_price.map(|v| v.to_string())),
            ("step_up", self.step_up),
            ("sip_type", self.sip_type),
            ("tag", self.tag),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
    }
}

/// Form parameters for the Kite `PUT /mf/sips/{sip_id}` endpoint.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
#[must_use]
pub struct MfSipModifyParams {
    /// New instalment amount.
    pub amount: Option<f64>,
    /// New instalment frequency.
    pub frequency: Option<String>,
    /// New day of the month for instalments.
    pub instalment_day: Option<i64>,
    /// New number of instalments.
    pub instalments: Option<i64>,
    /// New step-up setting, as the raw request string.
    pub step_up: Option<String>,
    /// New status of the SIP.
    pub status: Option<String>,
}

impl MfSipModifyParams {
    /// Creates params with no fields set.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets the new instalment amount.
    pub fn amount(mut self, amount: f64) -> Self {
        self.amount = Some(amount);
        self
    }
    /// Sets the new instalment frequency.
    pub fn frequency(mut self, frequency: impl Into<String>) -> Self {
        self.frequency = Some(frequency.into());
        self
    }
    /// Sets the new day of the month for instalments.
    pub fn instalment_day(mut self, instalment_day: i64) -> Self {
        self.instalment_day = Some(instalment_day);
        self
    }
    /// Sets the new number of instalments.
    pub fn instalments(mut self, instalments: i64) -> Self {
        self.instalments = Some(instalments);
        self
    }
    /// Sets the new step-up setting.
    pub fn step_up(mut self, step_up: impl Into<String>) -> Self {
        self.step_up = Some(step_up.into());
        self
    }
    /// Sets the new SIP status.
    pub fn status(mut self, status: impl Into<String>) -> Self {
        self.status = Some(status.into());
        self
    }
    fn into_form(self) -> impl Iterator<Item = (&'static str, String)> {
        [
            ("amount", self.amount.map(|v| v.to_string())),
            ("frequency", self.frequency),
            ("instalment_day", self.instalment_day.map(|v| v.to_string())),
            ("instalments", self.instalments.map(|v| v.to_string())),
            ("step_up", self.step_up),
            ("status", self.status),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
    }
}

fn csv_bool<'de, D: Deserializer<'de>>(deserializer: D) -> std::result::Result<bool, D::Error> {
    struct CsvBool;

    impl serde::de::Visitor<'_> for CsvBool {
        type Value = bool;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("a boolean or CSV flag (0, 1, false or true)")
        }

        fn visit_bool<E: serde::de::Error>(self, value: bool) -> std::result::Result<bool, E> {
            Ok(value)
        }

        fn visit_u64<E: serde::de::Error>(self, value: u64) -> std::result::Result<bool, E> {
            self.visit_i64(i64::try_from(value).unwrap_or(2))
        }

        fn visit_i64<E: serde::de::Error>(self, value: i64) -> std::result::Result<bool, E> {
            match value {
                0 => Ok(false),
                1 => Ok(true),
                _ => Err(E::custom("expected a CSV boolean (0 or 1)")),
            }
        }

        fn visit_str<E: serde::de::Error>(self, value: &str) -> std::result::Result<bool, E> {
            match value {
                "1" | "true" => Ok(true),
                "0" | "false" => Ok(false),
                _ => Err(E::custom("expected a CSV boolean (0, 1, false or true)")),
            }
        }
    }

    deserializer.deserialize_any(CsvBool)
}

impl KiteConnect {
    /// `GET /mf/orders`.
    pub fn mf_orders(&self) -> Call<Vec<MfOrder>> {
        Call::new(self.clone(), Request::get("/mf/orders"))
    }
    /// `GET /mf/orders?from=&to=`.
    /// Orders placed between two dates.
    pub fn mf_orders_by_date(&self, from: NaiveDate, to: NaiveDate) -> Call<Vec<MfOrder>> {
        Call::new(
            self.clone(),
            Request::get("/mf/orders").query([
                ("from", from.format("%Y-%m-%d").to_string()),
                ("to", to.format("%Y-%m-%d").to_string()),
            ]),
        )
    }
    /// `GET /mf/orders/{order_id}`.
    pub fn mf_order_info(&self, order_id: &str) -> Call<MfOrder> {
        let order_id = crate::client::path_segment(order_id);
        Call::new(self.clone(), Request::get(format!("/mf/orders/{order_id}")))
    }
    /// `POST /mf/orders`.
    pub fn place_mf_order(&self, params: MfOrderParams) -> Call<MfOrderResponse> {
        Call::new(
            self.clone(),
            Request::post("/mf/orders").form(params.into_form()),
        )
    }
    /// `DELETE /mf/orders/{order_id}`.
    pub fn cancel_mf_order(&self, order_id: &str) -> Call<MfOrderResponse> {
        let order_id = crate::client::path_segment(order_id);
        Call::new(
            self.clone(),
            Request::delete(format!("/mf/orders/{order_id}")),
        )
    }
    /// `GET /mf/sips`.
    pub fn mf_sips(&self) -> Call<Vec<MfSip>> {
        Call::new(self.clone(), Request::get("/mf/sips"))
    }
    /// `GET /mf/sips/{sip_id}`.
    pub fn mf_sip_info(&self, sip_id: &str) -> Call<MfSip> {
        let sip_id = crate::client::path_segment(sip_id);
        Call::new(self.clone(), Request::get(format!("/mf/sips/{sip_id}")))
    }
    /// `POST /mf/sips`.
    pub fn place_mf_sip(&self, params: MfSipParams) -> Call<MfSipResponse> {
        Call::new(
            self.clone(),
            Request::post("/mf/sips").form(params.into_form()),
        )
    }
    /// `PUT /mf/sips/{sip_id}`.
    pub fn modify_mf_sip(&self, sip_id: &str, params: MfSipModifyParams) -> Call<MfSipResponse> {
        let sip_id = crate::client::path_segment(sip_id);
        Call::new(
            self.clone(),
            Request::put(format!("/mf/sips/{sip_id}")).form(params.into_form()),
        )
    }
    /// `DELETE /mf/sips/{sip_id}`.
    pub fn cancel_mf_sip(&self, sip_id: &str) -> Call<MfSipResponse> {
        let sip_id = crate::client::path_segment(sip_id);
        Call::new(self.clone(), Request::delete(format!("/mf/sips/{sip_id}")))
    }
    /// `GET /mf/holdings`.
    pub fn mf_holdings(&self) -> Call<Vec<MfHolding>> {
        Call::new(self.clone(), Request::get("/mf/holdings"))
    }
    /// `GET /mf/holdings/{isin}`.
    pub fn mf_holding_info(&self, isin: &str) -> Call<Vec<MfTrade>> {
        let isin = crate::client::path_segment(isin);
        Call::new(self.clone(), Request::get(format!("/mf/holdings/{isin}")))
    }
    /// `GET /mf/allotments`.
    pub fn mf_allotted_isins(&self) -> Call<Vec<String>> {
        Call::new(self.clone(), Request::get("/mf/allotments"))
    }
    /// `GET /mf/instruments`.
    /// Decoded from CSV.
    pub fn mf_instruments(&self) -> Call<Vec<MfInstrument>> {
        Call::with_decoder(
            self.clone(),
            Request::get("/mf/instruments"),
            crate::call::decode_csv,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Body, Error, RawResponse};
    use httpmock::{
        Method::{DELETE, GET, POST, PUT},
        MockServer,
    };
    use serde::de::DeserializeOwned;
    use serde_json::Value;

    const ORDERS: &str = include_str!("../../../mock_responses/mf_orders.json");
    const ORDER: &str = include_str!("../../../mock_responses/mf_orders_info.json");
    const SIPS: &str = include_str!("../../../mock_responses/mf_sips.json");
    const SIP: &str = include_str!("../../../mock_responses/mf_sip_info.json");
    const HOLDINGS: &str = include_str!("../../../mock_responses/mf_holdings.json");
    const INSTRUMENTS: &str = include_str!("../../../mock_responses/mf_instruments.csv");
    const ORDER_RESPONSE: &str = include_str!("../../../mock_responses/mf_order_response.json");
    const ORDER_CANCEL: &str = include_str!("../../../mock_responses/mf_order_cancel.json");
    const SIP_PLACE: &str = include_str!("../../../mock_responses/mf_sip_place.json");
    const SIP_MODIFY: &str = include_str!("../../../mock_responses/mf_sip_modify.json");
    const SIP_CANCEL: &str = include_str!("../../../mock_responses/mf_sip_cancel.json");
    const TRADE: &str = r#"{"fund":"Example Fund","tradingsymbol":"INF209K01157","average_price":106.8,"variety":"regular","exchange_timestamp":"2021-06-30 08:33:07","amount":1000,"folio":"3108290884","quantity":9.363}"#;

    fn fixture<T: DeserializeOwned>(body: &str) -> T {
        let envelope: Value = serde_json::from_str(body).unwrap();
        serde_json::from_value(envelope["data"].clone()).unwrap()
    }

    fn client(server: &MockServer) -> KiteConnect {
        KiteConnect::builder("key")
            .base_url(server.base_url())
            .build()
            .unwrap()
    }

    #[test]
    fn mf_order_fixtures_decode() {
        let orders: Vec<MfOrder> = fixture(ORDERS);
        assert_eq!(orders.len(), 5);
        assert_eq!(orders[0].order_id, "271989e0-a64e-4cf3-b4e4-afb8f38dd203");
        assert_eq!(orders[0].amount.to_bits(), f64::to_bits(1_000.0));
        assert_eq!(
            orders[0].exchange_timestamp.unwrap().to_string(),
            "2021-06-30 00:00:00"
        );
        assert_eq!(orders[1].exchange_order_id, None);
        let order: MfOrder = fixture(ORDER);
        assert_eq!(order.tradingsymbol, "INF761K01EE1");
        assert_eq!(order.amount.to_bits(), f64::to_bits(5_000.0));
        assert_eq!(order.order_timestamp.to_string(), "2021-06-29 12:20:28");
        assert_eq!(order.folio, None);
        for body in [ORDER_RESPONSE, ORDER_CANCEL] {
            let response: MfOrderResponse = fixture(body);
            assert_eq!(response.order_id, "3bb085d1-5038-450e-a807-6543fef6c9ae");
        }
    }

    #[test]
    fn mf_sip_fixtures_decode() {
        let sips: Vec<MfSip> = fixture(SIPS);
        assert_eq!(sips.len(), 5);
        assert_eq!(sips[0].sip_id, "892741486820670");
        assert_eq!(sips[0].instalments, -1);
        assert_eq!(sips[0].pending_instalments, -1);
        assert_eq!(sips[0].instalment_amount.to_bits(), f64::to_bits(500.0));
        assert_eq!(sips[0].step_up["05-05"], 10);
        let sip: MfSip = fixture(SIP);
        assert_eq!(sip.sip_id, "181635213661372");
        assert_eq!(sip.instalment_amount.to_bits(), f64::to_bits(5_000.0));
        assert_eq!(sip.next_instalment.unwrap().to_string(), "2022-02-22");
        for body in [SIP_PLACE, SIP_MODIFY, SIP_CANCEL] {
            let response: MfSipResponse = fixture(body);
            assert_eq!(response.sip_id, "986124545877922");
            assert_eq!(response.order_id, None);
        }
    }

    #[test]
    fn mf_holdings_fixture_decodes() {
        let holdings: Vec<MfHolding> = fixture(HOLDINGS);
        assert_eq!(holdings.len(), 3);
        assert_eq!(holdings[0].folio.as_deref(), Some("3108290884"));
        assert_eq!(holdings[0].quantity.to_bits(), f64::to_bits(382.488));
        assert_eq!(holdings[0].last_price.to_bits(), f64::to_bits(84.86));
        assert_eq!(holdings[0].last_price_date, None);
    }

    #[test]
    fn mf_instruments_fixture_decodes() {
        let response = RawResponse {
            status: 200,
            content_type: Some("text/csv".into()),
            body: INSTRUMENTS.as_bytes().to_vec(),
        };
        let instruments: Vec<MfInstrument> = crate::call::decode_csv(
            &KiteConnect::builder("k").build().unwrap(),
            &response,
            "/mf/instruments",
        )
        .unwrap();
        assert_eq!(instruments.len(), 99);
        let instrument = &instruments[0];
        assert_eq!(instrument.tradingsymbol, "INF209K01157");
        assert_eq!(instrument.name, "Aditya Birla Sun Life Advantage Fund");
        assert_eq!(instrument.last_price.to_bits(), f64::to_bits(106.8));
        assert_eq!(
            instrument.minimum_additional_purchase_amount.to_bits(),
            f64::to_bits(1_000.0)
        );
        assert!(instrument.purchase_allowed);
        assert!(instrument.redemption_allowed);
        assert_eq!(instrument.last_price_date.to_string(), "2017-11-23");
        let roundtrip: MfInstrument =
            serde_json::from_value(serde_json::to_value(instrument).unwrap()).unwrap();
        assert!(roundtrip.purchase_allowed);
    }

    #[test]
    fn mf_trade_decodes_without_an_upstream_fixture() {
        let trade: MfTrade = serde_json::from_str(TRADE).unwrap();
        assert_eq!(trade.tradingsymbol, "INF209K01157");
        assert_eq!(trade.quantity.to_bits(), f64::to_bits(9.363));
        assert_eq!(
            trade.exchange_timestamp.unwrap().to_string(),
            "2021-06-30 08:33:07"
        );
    }

    #[test]
    fn mf_required_identifiers_and_numbers_cannot_disappear() {
        let order: Value = serde_json::from_str(ORDER).unwrap();
        for key in [
            "order_id",
            "amount",
            "quantity",
            "last_price",
            "average_price",
        ] {
            let mut data = order["data"].clone();
            data.as_object_mut().unwrap().remove(key);
            assert!(serde_json::from_value::<MfOrder>(data).is_err(), "{key}");
        }
        assert!(serde_json::from_str::<MfOrderResponse>("{}").is_err());
        assert!(serde_json::from_str::<MfSipResponse>("{}").is_err());
    }

    #[test]
    fn mf_params_omit_unset_fields_and_preserve_field_order() {
        let order = MfOrderParams::new("INF209K01157", TransactionType::Sell);
        assert_eq!(
            order.clone().into_form().collect::<Vec<_>>(),
            vec![
                ("tradingsymbol", "INF209K01157".into()),
                ("transaction_type", "SELL".into())
            ]
        );
        assert_eq!(
            MfSipModifyParams::new().into_form().collect::<Vec<_>>(),
            Vec::<(&str, String)>::new()
        );
        assert_eq!(
            MfSipModifyParams::new()
                .instalments(0)
                .into_form()
                .collect::<Vec<_>>(),
            vec![("instalments", "0".into())]
        );
        let sip = MfSipParams::new("INF209K01157", 500.0, -1, "monthly");
        assert_eq!(
            sip.into_form().collect::<Vec<_>>(),
            vec![
                ("tradingsymbol", "INF209K01157".into()),
                ("amount", "500".into()),
                ("instalments", "-1".into()),
                ("frequency", "monthly".into())
            ]
        );
        let server = MockServer::start();
        match client(&server)
            .place_mf_order(order.clone())
            .request()
            .body()
        {
            Body::Form(fields) => assert_eq!(
                fields,
                &vec![
                    ("tradingsymbol".to_string(), "INF209K01157".to_string()),
                    ("transaction_type".to_string(), "SELL".to_string()),
                ]
            ),
            _ => panic!("expected form body"),
        }
    }

    #[test]
    fn mf_sips_fixture_missing_status_is_reported_without_relaxing_envelope() {
        let response = RawResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: SIPS.as_bytes().to_vec(),
        };
        assert!(matches!(
            crate::call::envelope_data(&response, "/mf/sips"),
            Err(Error::Decode { .. })
        ));
    }

    #[tokio::test]
    async fn mf_orders_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/orders");
                then.status(200).body(ORDERS);
            })
            .await;
        let result = client(&server).mf_orders().await.unwrap();
        assert_eq!(result[0].amount.to_bits(), f64::to_bits(1_000.0));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_orders_by_date_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET)
                    .path("/mf/orders")
                    .query_param("from", "2021-06-01")
                    .query_param("to", "2021-06-30");
                then.status(200).body(ORDERS);
            })
            .await;
        let result = client(&server)
            .mf_orders_by_date(
                NaiveDate::from_ymd_opt(2021, 6, 1).unwrap(),
                NaiveDate::from_ymd_opt(2021, 6, 30).unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(result.len(), 5);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_order_info_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/orders/order-id");
                then.status(200).body(ORDER);
            })
            .await;
        let result = client(&server).mf_order_info("order-id").await.unwrap();
        assert_eq!(result.amount.to_bits(), f64::to_bits(5_000.0));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn place_mf_order_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(POST).path("/mf/orders").body("tradingsymbol=INF209K01157&transaction_type=BUY&quantity=1.5&amount=1000&tag=tag");
            then.status(200).body(ORDER_RESPONSE);
        }).await;
        let result = client(&server)
            .place_mf_order(
                MfOrderParams::new("INF209K01157", TransactionType::Buy)
                    .quantity(1.5)
                    .amount(1000.0)
                    .tag("tag"),
            )
            .await
            .unwrap();
        assert_eq!(result.order_id, "3bb085d1-5038-450e-a807-6543fef6c9ae");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn cancel_mf_order_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(DELETE).path("/mf/orders/order-id").body("");
                then.status(200).body(ORDER_CANCEL);
            })
            .await;
        let result = client(&server).cancel_mf_order("order-id").await.unwrap();
        assert_eq!(result.order_id, "3bb085d1-5038-450e-a807-6543fef6c9ae");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_sips_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/sips");
                then.status(200).body(r#"{"status":"success","data":[]}"#);
            })
            .await;
        assert!(client(&server).mf_sips().await.unwrap().is_empty());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_sip_info_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/sips/sip-id");
                then.status(200).body(SIP);
            })
            .await;
        let result = client(&server).mf_sip_info("sip-id").await.unwrap();
        assert_eq!(result.instalment_amount.to_bits(), f64::to_bits(5_000.0));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn place_mf_sip_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(POST).path("/mf/sips").body("tradingsymbol=INF209K01157&amount=500&instalments=-1&frequency=monthly&instalment_day=5&initial_amount=1000&trigger_price=10&step_up=%7B%2205-05%22%3A10%7D&sip_type=sip&tag=tag");
            then.status(200).body(SIP_PLACE);
        }).await;
        let result = client(&server)
            .place_mf_sip(
                MfSipParams::new("INF209K01157", 500.0, -1, "monthly")
                    .instalment_day(5)
                    .initial_amount(1000.0)
                    .trigger_price(10.0)
                    .step_up("{\"05-05\":10}")
                    .sip_type("sip")
                    .tag("tag"),
            )
            .await
            .unwrap();
        assert_eq!(result.sip_id, "986124545877922");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn modify_mf_sip_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(PUT).path("/mf/sips/sip-id").body("amount=1000&frequency=weekly&instalment_day=5&instalments=-1&step_up=%7B%2205-05%22%3A10%7D&status=PAUSED");
            then.status(200).body(SIP_MODIFY);
        }).await;
        let result = client(&server)
            .modify_mf_sip(
                "sip-id",
                MfSipModifyParams::new()
                    .amount(1000.0)
                    .frequency("weekly")
                    .instalment_day(5)
                    .instalments(-1)
                    .step_up("{\"05-05\":10}")
                    .status("PAUSED"),
            )
            .await
            .unwrap();
        assert_eq!(result.sip_id, "986124545877922");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn cancel_mf_sip_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(DELETE).path("/mf/sips/sip-id").body("");
                then.status(200).body(SIP_CANCEL);
            })
            .await;
        let result = client(&server).cancel_mf_sip("sip-id").await.unwrap();
        assert_eq!(result.order_id, None);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_holdings_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/holdings");
                then.status(200).body(HOLDINGS);
            })
            .await;
        let result = client(&server).mf_holdings().await.unwrap();
        assert_eq!(result[0].quantity.to_bits(), f64::to_bits(382.488));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_holding_info_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/holdings/INF209K01157");
                then.status(200)
                    .body(format!("{{\"status\":\"success\",\"data\":[{TRADE}]}}"));
            })
            .await;
        let result = client(&server)
            .mf_holding_info("INF209K01157")
            .await
            .unwrap();
        assert_eq!(result[0].quantity.to_bits(), f64::to_bits(9.363));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_allotted_isins_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/allotments");
                then.status(200)
                    .body(r#"{"status":"success","data":["INF209K01157"]}"#);
            })
            .await;
        let result = client(&server).mf_allotted_isins().await.unwrap();
        assert_eq!(result, vec!["INF209K01157"]);
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_instruments_uses_go_http_contract() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/instruments");
                then.status(200).body(INSTRUMENTS);
            })
            .await;
        let result = client(&server).mf_instruments().await.unwrap();
        assert_eq!(result[0].last_price.to_bits(), f64::to_bits(106.8));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn mf_raw_preserves_original_json_fields() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/mf/orders");
                then.status(200).body(ORDERS);
            })
            .await;
        let data = client(&server).mf_orders().raw().await.unwrap();
        assert_eq!(data, serde_json::from_str::<Value>(ORDERS).unwrap()["data"]);
        assert_eq!(data[0]["last_price_date"], "2021-06-29");
        mock.assert_async().await;
    }

    #[test]
    fn mf_csv_rejects_bad_numbers_and_preserves_api_errors() {
        let bad = INSTRUMENTS.replacen(",106.8,", ",not-a-price,", 1);
        let response = RawResponse {
            status: 200,
            content_type: None,
            body: bad.into_bytes(),
        };
        assert!(matches!(
            crate::call::decode_csv::<MfInstrument>(
                &KiteConnect::builder("k").build().unwrap(),
                &response,
                "/mf/instruments"
            ),
            Err(Error::Decode { .. })
        ));
        let response = RawResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: br#"{"status":"error","error_type":"TokenException","message":"expired"}"#
                .to_vec(),
        };
        assert!(
            crate::call::decode_csv::<MfInstrument>(
                &KiteConnect::builder("k").build().unwrap(),
                &response,
                "/mf/instruments"
            )
            .unwrap_err()
            .is_session_expired()
        );
    }
}
