use crate::{
    Call, Exchange, KiteConnect, OrderType, Product, Request, TransactionType, Validity, Variety,
};
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Order from the Kite orders, history, and order-update endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Order {
    /// Account ID the order belongs to, when returned.
    pub account_id: Option<String>,
    /// User ID that placed the order.
    pub placed_by: String,
    /// Unique order ID.
    pub order_id: String,
    /// Order ID assigned by the exchange.
    pub exchange_order_id: Option<String>,
    /// Order ID of the parent order, if any.
    pub parent_order_id: Option<String>,
    /// Order status, such as `COMPLETE` or `REJECTED`.
    pub status: String,
    /// Human-readable status message, if any.
    pub status_message: Option<String>,
    /// Raw status message, if any.
    pub status_message_raw: Option<String>,
    /// Time the order was placed.
    #[serde(with = "crate::time::datetime")]
    pub order_timestamp: NaiveDateTime,
    /// Time of the last update received from the exchange.
    #[serde(default, with = "crate::time::optional_datetime")]
    pub exchange_update_timestamp: Option<NaiveDateTime>,
    /// Time the exchange recorded the order.
    #[serde(default, with = "crate::time::optional_datetime")]
    pub exchange_timestamp: Option<NaiveDateTime>,
    /// Order variety, such as `regular` or `amo`.
    pub variety: String,
    /// Whether the order was modified.
    pub modified: Option<bool>,
    /// Extra order metadata, such as iceberg details.
    pub meta: Option<BTreeMap<String, Value>>,
    /// Exchange the instrument trades on.
    pub exchange: String,
    /// Trading symbol of the instrument.
    pub tradingsymbol: String,
    /// Numeric instrument token.
    pub instrument_token: u32,
    /// Order type, such as `MARKET` or `LIMIT`.
    pub order_type: String,
    /// Transaction side, `BUY` or `SELL`.
    pub transaction_type: String,
    /// Order validity, such as `DAY` or `IOC`.
    pub validity: String,
    /// Validity time-to-live in minutes for `TTL` orders.
    pub validity_ttl: Option<i64>,
    /// Product, such as `CNC` or `MIS`.
    pub product: String,
    /// Order quantity.
    pub quantity: f64,
    /// Quantity disclosed to the market.
    pub disclosed_quantity: f64,
    /// Order price.
    pub price: f64,
    /// Trigger price for stop-loss orders.
    pub trigger_price: f64,
    /// Average fill price.
    pub average_price: f64,
    /// Quantity filled so far.
    pub filled_quantity: f64,
    /// Quantity still pending.
    pub pending_quantity: f64,
    /// Quantity cancelled.
    pub cancelled_quantity: f64,
    /// Auction number for auction orders.
    pub auction_number: Option<String>,
    /// Market protection setting for market and stop-loss market orders.
    pub market_protection: Option<f64>,
    /// Tag attached to the order.
    pub tag: Option<String>,
    /// Tags attached to the order.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Unique identifier of the order, if returned.
    pub guid: Option<String>,
}

/// Trade from the Kite trades and order-trades endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Trade {
    /// Average price of the trade.
    pub average_price: f64,
    /// Traded quantity.
    pub quantity: f64,
    /// Unique trade ID.
    pub trade_id: String,
    /// Product of the trade, such as `CNC` or `MIS`.
    pub product: String,
    /// Time the trade was filled.
    #[serde(with = "crate::time::datetime")]
    pub fill_timestamp: NaiveDateTime,
    /// Time the exchange recorded the trade.
    #[serde(with = "crate::time::datetime")]
    pub exchange_timestamp: NaiveDateTime,
    /// Exchange order ID of the order the trade belongs to.
    pub exchange_order_id: String,
    /// ID of the order the trade belongs to.
    pub order_id: String,
    /// Transaction side of the trade, `BUY` or `SELL`.
    pub transaction_type: String,
    /// Trading symbol of the instrument.
    pub tradingsymbol: String,
    /// Exchange the instrument trades on.
    pub exchange: String,
    /// Numeric instrument token.
    pub instrument_token: u32,
}

/// Result from the Kite place, modify, cancel, and exit order endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct OrderResponse {
    /// ID of the order.
    pub order_id: String,
    /// Child orders from autoslice, empty otherwise.
    #[serde(default)]
    pub children: Vec<OrderChild>,
}

/// Child order from the Kite autoslice order endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct OrderChild {
    /// ID of the child order, if it was placed.
    pub order_id: Option<String>,
    /// Error for the child order, if it failed.
    pub error: Option<OrderChildError>,
}

/// Child error from the Kite autoslice order endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct OrderChildError {
    /// Numeric error code.
    pub code: i64,
    /// Error type name.
    pub error_type: String,
    /// Error message.
    pub message: String,
    /// Extra error data, if any.
    pub data: Option<Value>,
}

/// Form parameters for the Kite place and modify order endpoints.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
#[must_use]
pub struct OrderParams {
    /// Exchange the instrument trades on.
    pub exchange: Option<Exchange>,
    /// Trading symbol of the instrument.
    pub tradingsymbol: Option<String>,
    /// Order validity, such as `DAY` or `IOC`.
    pub validity: Option<Validity>,
    /// Validity time-to-live in minutes for `TTL` orders.
    pub validity_ttl: Option<i64>,
    /// Product, such as `CNC` or `MIS`.
    pub product: Option<Product>,
    /// Order type, such as `MARKET` or `LIMIT`.
    pub order_type: Option<OrderType>,
    /// Transaction side, `BUY` or `SELL`.
    pub transaction_type: Option<TransactionType>,
    /// Order quantity.
    pub quantity: Option<i64>,
    /// Quantity disclosed to the market.
    pub disclosed_quantity: Option<i64>,
    /// Order price.
    pub price: Option<f64>,
    /// Trigger price for stop-loss orders.
    pub trigger_price: Option<f64>,
    /// Square-off target for bracket orders.
    pub squareoff: Option<f64>,
    /// Stop-loss value for bracket orders.
    pub stoploss: Option<f64>,
    /// Trailing stop-loss value for bracket orders.
    pub trailing_stoploss: Option<f64>,
    /// Number of legs for iceberg orders.
    pub iceberg_legs: Option<i64>,
    /// Quantity per leg for iceberg orders.
    pub iceberg_quantity: Option<i64>,
    /// Auction number for auction orders.
    pub auction_number: Option<String>,
    /// Market protection setting for market and stop-loss market orders.
    pub market_protection: Option<f64>,
    /// Whether to slice the order automatically when it exceeds the freeze quantity.
    pub autoslice: Option<bool>,
    /// Algo ID for the order.
    pub algo_id: Option<String>,
    /// Tag attached to the order.
    pub tag: Option<String>,
}

impl OrderParams {
    /// Creates empty order parameters.
    pub fn new() -> Self {
        Self::default()
    }
    /// Sets `exchange`.
    pub fn exchange(mut self, value: Exchange) -> Self {
        self.exchange = Some(value);
        self
    }
    /// Sets `tradingsymbol`.
    pub fn tradingsymbol(mut self, value: impl Into<String>) -> Self {
        self.tradingsymbol = Some(value.into());
        self
    }
    /// Sets `validity`.
    pub fn validity(mut self, value: Validity) -> Self {
        self.validity = Some(value);
        self
    }
    /// Sets `validity_ttl`.
    pub fn validity_ttl(mut self, value: i64) -> Self {
        self.validity_ttl = Some(value);
        self
    }
    /// Sets `product`.
    pub fn product(mut self, value: Product) -> Self {
        self.product = Some(value);
        self
    }
    /// Sets `order_type`.
    pub fn order_type(mut self, value: OrderType) -> Self {
        self.order_type = Some(value);
        self
    }
    /// Sets `transaction_type`.
    pub fn transaction_type(mut self, value: TransactionType) -> Self {
        self.transaction_type = Some(value);
        self
    }
    /// Sets `quantity`.
    pub fn quantity(mut self, value: i64) -> Self {
        self.quantity = Some(value);
        self
    }
    /// Sets `disclosed_quantity`.
    pub fn disclosed_quantity(mut self, value: i64) -> Self {
        self.disclosed_quantity = Some(value);
        self
    }
    /// Sets `price`.
    pub fn price(mut self, value: f64) -> Self {
        self.price = Some(value);
        self
    }
    /// Sets `trigger_price`.
    pub fn trigger_price(mut self, value: f64) -> Self {
        self.trigger_price = Some(value);
        self
    }
    /// Sets `squareoff`.
    pub fn squareoff(mut self, value: f64) -> Self {
        self.squareoff = Some(value);
        self
    }
    /// Sets `stoploss`.
    pub fn stoploss(mut self, value: f64) -> Self {
        self.stoploss = Some(value);
        self
    }
    /// Sets `trailing_stoploss`.
    pub fn trailing_stoploss(mut self, value: f64) -> Self {
        self.trailing_stoploss = Some(value);
        self
    }
    /// Sets `iceberg_legs`.
    pub fn iceberg_legs(mut self, value: i64) -> Self {
        self.iceberg_legs = Some(value);
        self
    }
    /// Sets `iceberg_quantity`.
    pub fn iceberg_quantity(mut self, value: i64) -> Self {
        self.iceberg_quantity = Some(value);
        self
    }
    /// Sets `auction_number`.
    pub fn auction_number(mut self, value: impl Into<String>) -> Self {
        self.auction_number = Some(value.into());
        self
    }
    /// Sets `market_protection`.
    pub fn market_protection(mut self, value: f64) -> Self {
        self.market_protection = Some(value);
        self
    }
    /// Sets `autoslice`.
    pub fn autoslice(mut self, value: bool) -> Self {
        self.autoslice = Some(value);
        self
    }
    /// Sets `algo_id`.
    pub fn algo_id(mut self, value: impl Into<String>) -> Self {
        self.algo_id = Some(value.into());
        self
    }
    /// Sets `tag`.
    pub fn tag(mut self, value: impl Into<String>) -> Self {
        self.tag = Some(value.into());
        self
    }
    fn into_form(self) -> impl Iterator<Item = (&'static str, String)> {
        [
            ("exchange", self.exchange.map(|v| v.to_string())),
            ("tradingsymbol", self.tradingsymbol),
            ("validity", self.validity.map(|v| v.to_string())),
            ("validity_ttl", self.validity_ttl.map(|v| v.to_string())),
            ("product", self.product.map(|v| v.to_string())),
            ("order_type", self.order_type.map(|v| v.to_string())),
            (
                "transaction_type",
                self.transaction_type.map(|v| v.to_string()),
            ),
            ("quantity", self.quantity.map(|v| v.to_string())),
            (
                "disclosed_quantity",
                self.disclosed_quantity.map(|v| v.to_string()),
            ),
            ("price", self.price.map(|v| v.to_string())),
            ("trigger_price", self.trigger_price.map(|v| v.to_string())),
            ("squareoff", self.squareoff.map(|v| v.to_string())),
            ("stoploss", self.stoploss.map(|v| v.to_string())),
            (
                "trailing_stoploss",
                self.trailing_stoploss.map(|v| v.to_string()),
            ),
            ("iceberg_legs", self.iceberg_legs.map(|v| v.to_string())),
            (
                "iceberg_quantity",
                self.iceberg_quantity.map(|v| v.to_string()),
            ),
            ("auction_number", self.auction_number),
            (
                "market_protection",
                self.market_protection.map(|v| v.to_string()),
            ),
            ("autoslice", self.autoslice.map(|v| v.to_string())),
            ("algo_id", self.algo_id),
            ("tag", self.tag),
        ]
        .into_iter()
        .filter_map(|(key, value)| Some((key, value?)))
    }
}

impl KiteConnect {
    /// `GET /orders`.
    pub fn orders(&self) -> Call<Vec<Order>> {
        Call::new(self.clone(), Request::get("/orders"))
    }
    /// `GET /trades`.
    pub fn trades(&self) -> Call<Vec<Trade>> {
        Call::new(self.clone(), Request::get("/trades"))
    }
    /// `GET /orders/{order_id}`.
    pub fn order_history(&self, order_id: &str) -> Call<Vec<Order>> {
        let order_id = crate::client::path_segment(order_id);
        Call::new(self.clone(), Request::get(format!("/orders/{order_id}")))
    }
    /// `GET /orders/{order_id}/trades`.
    pub fn order_trades(&self, order_id: &str) -> Call<Vec<Trade>> {
        let order_id = crate::client::path_segment(order_id);
        Call::new(
            self.clone(),
            Request::get(format!("/orders/{order_id}/trades")),
        )
    }
    /// `POST /orders/{variety}`.
    pub fn place_order(&self, variety: Variety, params: OrderParams) -> Call<OrderResponse> {
        Call::new(
            self.clone(),
            Request::post(format!("/orders/{variety}")).form(params.into_form()),
        )
    }
    /// `PUT /orders/{variety}/{order_id}`.
    pub fn modify_order(
        &self,
        variety: Variety,
        order_id: &str,
        params: OrderParams,
    ) -> Call<OrderResponse> {
        let order_id = crate::client::path_segment(order_id);
        Call::new(
            self.clone(),
            Request::put(format!("/orders/{variety}/{order_id}")).form(params.into_form()),
        )
    }
    /// `DELETE /orders/{variety}/{order_id}`.
    pub fn cancel_order(
        &self,
        variety: Variety,
        order_id: &str,
        parent_order_id: Option<&str>,
    ) -> Call<OrderResponse> {
        let order_id = crate::client::path_segment(order_id);
        let request = Request::delete(format!("/orders/{variety}/{order_id}"))
            .query(parent_order_id.map(|id| ("parent_order_id", id)));
        Call::new(self.clone(), request)
    }
    /// `DELETE /orders/{variety}/{order_id}`.
    /// The same request as `cancel_order`, kept for Go parity.
    pub fn exit_order(
        &self,
        variety: Variety,
        order_id: &str,
        parent_order_id: Option<&str>,
    ) -> Call<OrderResponse> {
        self.cancel_order(variety, order_id, parent_order_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::{Method::*, MockServer};
    use serde::de::DeserializeOwned;

    fn data<T: DeserializeOwned>(fixture: &str) -> T {
        let envelope: Value = serde_json::from_str(fixture).unwrap();
        serde_json::from_value(envelope["data"].clone()).unwrap()
    }

    #[test]
    fn orders_fixture() {
        let orders: Vec<Order> = data(include_str!("../../../mock_responses/orders.json"));
        assert_eq!(orders.len(), 10);
        assert_eq!(orders[0].order_id, "100000000000000");
        assert_eq!(orders[0].instrument_token, 412_675);
        assert_eq!(orders[1].average_price.to_bits(), f64::to_bits(109.4));
        assert_eq!(orders[3].exchange_timestamp, None);
        assert_eq!(orders[3].validity_ttl, Some(2));
        assert_eq!(orders[3].meta.as_ref().unwrap()["iceberg"]["legs"], 5);
        assert_eq!(orders[6].auction_number.as_deref(), Some("22"));
        assert_eq!(orders[7].product, "MTF");
        assert_eq!(orders[8].tags, ["mktp", "mktp:2.00"]);
        assert_eq!(orders[0].order_timestamp.to_string(), "2021-05-31 09:18:57");
    }

    #[test]
    fn history_fixture() {
        let history: Vec<Order> = data(include_str!("../../../mock_responses/order_info.json"));
        assert_eq!(history.len(), 8);
        assert_eq!(history[0].order_id, "171229000724687");
        assert_eq!(history[0].status, "PUT ORDER REQ RECEIVED");
        assert_eq!(history[7].price.to_bits(), f64::to_bits(300.1));
        assert_eq!(history[0].exchange_update_timestamp, None);
    }

    #[test]
    fn postback_fixture() {
        let order: Order =
            serde_json::from_str(include_str!("../../../mock_responses/postback.json")).unwrap();
        assert_eq!(order.order_id, "220303000308932");
        assert_eq!(order.average_price.to_bits(), f64::to_bits(470.0));
        assert_eq!(order.filled_quantity.to_bits(), f64::to_bits(1.0));
        assert_eq!(order.modified, None);
        assert_eq!(
            order.exchange_timestamp.unwrap().to_string(),
            "2022-03-03 09:24:25"
        );
    }

    #[test]
    fn trades_fixtures() {
        let trades: Vec<Trade> = data(include_str!("../../../mock_responses/trades.json"));
        assert_eq!(trades.len(), 4);
        assert_eq!(trades[0].trade_id, "10000000");
        assert_eq!(trades[0].average_price.to_bits(), f64::to_bits(420.65));
        assert_eq!(trades[0].fill_timestamp.to_string(), "2021-05-31 09:16:39");
        let trades: Vec<Trade> = data(include_str!("../../../mock_responses/order_trades.json"));
        assert_eq!(trades.len(), 1);
        assert_eq!(trades[0].order_id, "200000000000000");
        assert_eq!(trades[0].quantity.to_bits(), f64::to_bits(1.0));
        assert_eq!(
            trades[0].exchange_timestamp.to_string(),
            "2021-05-31 16:00:36"
        );
    }

    #[test]
    fn order_response_fixtures() {
        for fixture in [
            include_str!("../../../mock_responses/order_response.json"),
            include_str!("../../../mock_responses/order_modify.json"),
            include_str!("../../../mock_responses/order_cancel.json"),
        ] {
            let response: OrderResponse = data(fixture);
            assert_eq!(response.order_id, "151220000000000");
            assert!(response.children.is_empty());
        }
        let response: OrderResponse = data(include_str!(
            "../../../mock_responses/autoslice_response.json"
        ));
        assert_eq!(response.order_id, "1914227164488687616");
        assert_eq!(response.children.len(), 4);
        assert_eq!(
            response.children[0].order_id.as_deref(),
            Some("1914227164534824960")
        );
        let error = response.children[2].error.as_ref().unwrap();
        assert_eq!(error.code, 400);
        assert_eq!(error.error_type, "MarginException");
        assert_eq!(
            error.message,
            "Insufficient funds. Required margin is 228365.92 but available margin is 228358.50."
        );
        assert_eq!(error.data, None);
    }

    #[test]
    fn missing_required_values_fail() {
        let fixture: Value =
            serde_json::from_str(include_str!("../../../mock_responses/orders.json")).unwrap();
        for field in [
            "order_id",
            "instrument_token",
            "price",
            "average_price",
            "quantity",
            "filled_quantity",
        ] {
            let mut order = fixture["data"][0].clone();
            order.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<Order>(order).is_err(), "{field}");
        }
        assert!(serde_json::from_value::<OrderResponse>(serde_json::json!({})).is_err());
    }

    fn client(server: &MockServer) -> KiteConnect {
        KiteConnect::builder("key")
            .base_url(server.base_url())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn orders_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/orders");
                then.body(include_str!("../../../mock_responses/orders.json"));
            })
            .await;
        let orders = client(&server).orders().await.unwrap();
        assert_eq!(orders[0].order_id, "100000000000000");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn trades_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/trades");
                then.body(include_str!("../../../mock_responses/trades.json"));
            })
            .await;
        let trades = client(&server).trades().await.unwrap();
        assert_eq!(trades[0].average_price.to_bits(), f64::to_bits(420.65));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn history_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/orders/171229000724687");
                then.body(include_str!("../../../mock_responses/order_info.json"));
            })
            .await;
        let history = client(&server)
            .order_history("171229000724687")
            .await
            .unwrap();
        assert_eq!(history[7].price.to_bits(), f64::to_bits(300.1));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn order_trades_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/orders/200000000000000/trades");
                then.body(include_str!("../../../mock_responses/order_trades.json"));
            })
            .await;
        let trades = client(&server)
            .order_trades("200000000000000")
            .await
            .unwrap();
        assert_eq!(trades[0].average_price.to_bits(), f64::to_bits(4_852.0));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn place_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(POST).path("/orders/iceberg")
                .body("exchange=NSE&tradingsymbol=SBIN&validity=TTL&validity_ttl=2&product=CNC&order_type=LIMIT&transaction_type=BUY&quantity=1000&disclosed_quantity=0&price=463&trigger_price=0&squareoff=1&stoploss=2&trailing_stoploss=3&iceberg_legs=5&iceberg_quantity=200&auction_number=22&market_protection=-1&autoslice=true&algo_id=algo-1&tag=desk");
            then.body(include_str!("../../../mock_responses/order_response.json"));
        }).await;
        let params = OrderParams::new()
            .exchange(Exchange::Nse)
            .tradingsymbol("SBIN")
            .validity(Validity::Ttl)
            .validity_ttl(2)
            .product(Product::Cnc)
            .order_type(OrderType::Limit)
            .transaction_type(TransactionType::Buy)
            .quantity(1000)
            .disclosed_quantity(0)
            .price(463.0)
            .trigger_price(0.0)
            .squareoff(1.0)
            .stoploss(2.0)
            .trailing_stoploss(3.0)
            .iceberg_legs(5)
            .iceberg_quantity(200)
            .auction_number("22")
            .market_protection(-1.0)
            .autoslice(true)
            .algo_id("algo-1")
            .tag("desk");
        let response = client(&server)
            .place_order(Variety::Iceberg, params)
            .await
            .unwrap();
        assert_eq!(response.order_id, "151220000000000");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn modify_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(PUT)
                    .path("/orders/regular/order-1")
                    .body("price=0&trigger_price=0");
                then.body(include_str!("../../../mock_responses/order_modify.json"));
            })
            .await;
        let response = client(&server)
            .modify_order(
                Variety::Regular,
                "order-1",
                OrderParams::new().price(0.0).trigger_price(0.0),
            )
            .await
            .unwrap();
        assert_eq!(response.order_id, "151220000000000");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn cancel_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(DELETE)
                    .path("/orders/co/order-1")
                    .query_param("parent_order_id", "parent-1")
                    .body("");
                then.body(include_str!("../../../mock_responses/order_cancel.json"));
            })
            .await;
        let response = client(&server)
            .cancel_order(Variety::Co, "order-1", Some("parent-1"))
            .await
            .unwrap();
        assert_eq!(response.order_id, "151220000000000");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn exit_endpoint() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(DELETE)
                    .path("/orders/co/order-1")
                    .query_param("parent_order_id", "parent-1")
                    .body("");
                then.body(include_str!("../../../mock_responses/order_cancel.json"));
            })
            .await;
        let response = client(&server)
            .exit_order(Variety::Co, "order-1", Some("parent-1"))
            .await
            .unwrap();
        assert_eq!(response.order_id, "151220000000000");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn raw_bypasses_required_fields() {
        let server = MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/orders");
                then.json_body(
                    serde_json::json!({"status":"success","data":[{"order_id":"order-1"}]}),
                );
            })
            .await;
        assert_eq!(
            client(&server).orders().raw().await.unwrap(),
            serde_json::json!([{"order_id":"order-1"}])
        );
        assert!(client(&server).orders().await.is_err());
        mock.assert_calls_async(2).await;
    }
}
