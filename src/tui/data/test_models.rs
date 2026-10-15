use super::*;
use kiteconnect::ticker::DepthEntry;
use kiteconnect::{
    AlertBasket as FetchedBasket, AlertOhlc as TriggerOhlc, FullUserMeta as ProfileMeta,
    GttCondition, GttOrder, MtfHolding,
};

pub fn depth_entry(quantity: u32, price: f64, orders: u16) -> DepthEntry {
    let mut entry = DepthEntry::default();
    entry.quantity = quantity;
    entry.price = price;
    entry.orders = orders;
    entry
}

pub(super) fn fixture<T: serde::de::DeserializeOwned>(text: &str, pointer: &str) -> T {
    let mut envelope: Value = serde_json::from_str(text).expect("fixture");
    serde_json::from_value(envelope.pointer_mut(pointer).expect("fixture path").take())
        .expect("fixture data")
}

pub fn holding() -> Holding {
    fixture(
        include_str!("../../../mock_responses/holdings.json"),
        "/data/0",
    )
}
pub fn position() -> Position {
    fixture(
        include_str!("../../../mock_responses/positions.json"),
        "/data/net/0",
    )
}
pub fn positions() -> PositionsData {
    fixture(
        include_str!("../../../mock_responses/positions.json"),
        "/data",
    )
}
pub fn mtf() -> MtfHolding {
    serde_json::from_value(serde_json::json!({"quantity":0,"used_quantity":0,"average_price":0,"value":0,"initial_margin":0})).unwrap()
}
pub fn order() -> Order {
    fixture(
        include_str!("../../../mock_responses/orders.json"),
        "/data/0",
    )
}
pub fn gtt() -> Gtt {
    fixture(
        include_str!("../../../mock_responses/gtt_get_orders.json"),
        "/data/0",
    )
}
pub fn gtt_condition() -> GttCondition {
    fixture(
        include_str!("../../../mock_responses/gtt_get_orders.json"),
        "/data/0/condition",
    )
}
pub fn gtt_order() -> GttOrder {
    fixture(
        include_str!("../../../mock_responses/gtt_get_orders.json"),
        "/data/0/orders/0",
    )
}
pub fn mf() -> MfHolding {
    serde_json::from_value(serde_json::json!({"tradingsymbol":"", "quantity":0, "average_price":0, "last_price":0, "close_price":0,"pnl":0,"day_change":0,"day_change_percentage":0})).expect("mf")
}
pub fn alert() -> Alert {
    fixture(
        include_str!("../../../mock_responses/alerts_get.json"),
        "/data/0",
    )
}
pub fn basket() -> FetchedBasket {
    fixture(
        include_str!("../../../mock_responses/alerts_get.json"),
        "/data/1/basket",
    )
}
pub fn basket_item() -> FetchedBasketItem {
    fixture(
        include_str!("../../../mock_responses/alerts_get.json"),
        "/data/1/basket/items/0",
    )
}
pub fn history() -> AlertTrigger {
    fixture(
        include_str!("../../../mock_responses/alerts_history.json"),
        "/data/0",
    )
}
pub fn trigger_quote() -> TriggerQuote {
    fixture(
        include_str!("../../../mock_responses/alerts_history.json"),
        "/data/0/meta/0",
    )
}
pub fn ohlc() -> TriggerOhlc {
    fixture(
        include_str!("../../../mock_responses/alerts_history.json"),
        "/data/0/meta/0/ohlc",
    )
}
pub fn profile() -> Profile {
    fixture(
        include_str!("../../../mock_responses/full_profile.json"),
        "/data",
    )
}
pub fn profile_meta() -> ProfileMeta {
    fixture(
        include_str!("../../../mock_responses/full_profile.json"),
        "/data/meta",
    )
}
pub fn funds() -> Funds {
    fixture(
        include_str!("../../../mock_responses/margins.json"),
        "/data",
    )
}
pub fn candle() -> Candle {
    Candle::new("2024-01-01T09:15:00+05:30".parse().expect("date"), 0.0)
}
pub fn auction() -> AuctionHolding {
    fixture(
        include_str!("../../../mock_responses/auctions_list.json"),
        "/data/0",
    )
}

pub fn candle_at(ts: &str, open: f64, high: f64, low: f64, close: f64, volume: u64) -> Candle {
    let text = ts.replace(' ', "T");
    let text = if text.len() == 10 {
        format!("{text}T00:00:00")
    } else {
        text
    };
    let date = text
        .parse()
        .or_else(|_| format!("{text}+05:30").parse())
        .expect("date");
    let mut candle = Candle::new(date, open);
    candle.high = high;
    candle.low = low;
    candle.close = close;
    candle.volume = volume;
    candle
}

pub(super) fn alerts_get_fixture() -> serde_json::Value {
    serde_json::json!([
        {
            "type": "simple",
            "user_id": "AB1234",
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "name": "NIFTY 50",
            "status": "enabled",
            "disabled_reason": "",
            "lhs_attribute": "LastTradedPrice",
            "lhs_exchange": "INDICES",
            "lhs_tradingsymbol": "NIFTY 50",
            "operator": ">=",
            "rhs_type": "constant",
            "rhs_attribute": "",
            "rhs_exchange": "",
            "rhs_tradingsymbol": "",
            "rhs_constant": 27000,
            "alert_count": 0,
            "created_at": "2025-05-26 12:07:50",
            "updated_at": "2025-05-26 12:07:50"
        },
        {
            "type": "ato",
            "user_id": "AB1234",
            "uuid": "e888ed4a-6801-406f-bdc2-002db5a8411d",
            "name": "buy gold",
            "status": "disabled",
            "disabled_reason": "",
            "basket": { "items": [ { "id": 275_218_517, "tradingsymbol": "GOLDBEES",
                "exchange": "NSE", "instrument_token": 3_693_569, "weight": 10000,
                "params": { "order_type": "LIMIT", "quantity": 10000, "price": 72.22 } } ] },
            "lhs_attribute": "LastTradedPrice",
            "lhs_exchange": "NSE",
            "lhs_tradingsymbol": "GOLDBEES",
            "operator": "<=",
            "rhs_type": "constant",
            "rhs_attribute": "",
            "rhs_exchange": "",
            "rhs_tradingsymbol": "",
            "rhs_constant": 71.8,
            "alert_count": 1,
            "created_at": "2025-02-17 08:24:10",
            "updated_at": "2025-02-17 09:15:20"
        }
    ])
}

pub(super) fn alerts_history_fixture() -> serde_json::Value {
    serde_json::json!([
        {
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "type": "simple",
            "meta": [
                {
                    "instrument_token": 270_857,
                    "tradingsymbol": "NIFTY NEXT 50",
                    "timestamp": "2025-02-17 09:16:46",
                    "last_price": 58288.05,
                    "ohlc": { "open": 59221.95, "high": 59241.8, "low": 58283.7, "close": 59557.95 },
                    "net_change": -1269.9,
                    "exchange": "INDICES",
                    "last_trade_time": "0001-01-01 00:00:00",
                    "last_quantity": 0,
                    "buy_quantity": 0,
                    "sell_quantity": 0,
                    "volume": 0,
                    "volume_tick": 0,
                    "average_price": 0,
                    "oi": 0,
                    "oi_day_high": 0,
                    "oi_day_low": 0,
                    "lower_circuit_limit": 0,
                    "upper_circuit_limit": 0
                }
            ],
            "condition": "LastTradedPrice(\"INDICES:NIFTY NEXT 50\") <= 58290.35",
            "created_at": "2025-02-17 09:16:46",
            "order_meta": null
        }
    ])
}
