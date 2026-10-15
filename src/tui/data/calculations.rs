use super::{
    Alert, AlertParams, AlertRhsType, AlertStatus, AlertTrigger, AlertType, Candle, Holding,
    MfHolding, Order, Position, Profile, TriggerQuote, Value,
};
use crate::tui::alerts::Attribute;

#[expect(
    clippy::cast_precision_loss,
    reason = "display P&L retains existing f64 quantity rounding"
)]
pub fn position_live_pnl(row: &Position, live_ltp: Option<f64>) -> f64 {
    match live_ltp {
        Some(ltp) => {
            row.pnl
                + (ltp - row.last_price) * row.quantity as f64 * position_effective_multiplier(row)
        }
        None => row.pnl,
    }
}

pub fn position_effective_multiplier(row: &Position) -> f64 {
    if row.multiplier == 0.0 {
        1.0
    } else {
        row.multiplier
    }
}

pub fn holding_total_quantity(row: &Holding) -> i64 {
    row.quantity
        + row.t1_quantity
        + row.collateral_quantity
        + row.mtf.as_ref().map_or(0, |mtf| mtf.quantity)
}

pub fn holding_free_quantity(row: &Holding) -> i64 {
    row.quantity + row.t1_quantity
}

pub fn holding_mark(row: &Holding, live_ltp: Option<f64>) -> f64 {
    live_ltp.unwrap_or(row.last_price)
}

pub fn holding_has_pnl(row: &Holding, live_ltp: Option<f64>) -> bool {
    !row.discrepancy && holding_mark(row, live_ltp) > 0.0
}

#[expect(
    clippy::cast_precision_loss,
    reason = "display P&L retains existing f64 quantity rounding"
)]
pub fn holding_live_pnl(row: &Holding, live_ltp: Option<f64>) -> f64 {
    (holding_mark(row, live_ltp) - row.average_price) * holding_total_quantity(row) as f64
}

#[expect(
    clippy::cast_precision_loss,
    reason = "display P&L retains existing f64 quantity rounding"
)]
pub fn holding_live_day_pnl(row: &Holding, live_ltp: Option<f64>) -> f64 {
    let mark = holding_mark(row, live_ltp);
    let change = if mark != 0.0 && row.close_price > 0.0 {
        mark - row.close_price
    } else {
        row.day_change.unwrap_or(0.0)
    };
    change * holding_total_quantity(row) as f64
}

pub fn mf_invested(row: &MfHolding) -> f64 {
    row.quantity * row.average_price
}

pub fn mf_current(row: &MfHolding) -> f64 {
    row.quantity * row.last_price
}

pub fn mf_return_pct(row: &MfHolding) -> Option<f64> {
    (row.average_price != 0.0)
        .then(|| (row.last_price - row.average_price) / row.average_price * 100.0)
}

pub fn mf_day_change_pct(row: &MfHolding) -> Option<f64> {
    row.close_price
        .is_some_and(|price| price != 0.0)
        .then(|| row.day_change_percentage.unwrap_or(0.0))
}

pub fn order_is_open(row: &Order) -> bool {
    matches!(row.status.as_str(), "OPEN" | "TRIGGER PENDING")
        || row.status.ends_with("PENDING")
        || row.status.ends_with("RECEIVED")
}

pub fn order_validity_label(row: &Order) -> String {
    match (row.validity.as_str(), row.validity_ttl) {
        ("TTL", Some(minutes)) if minutes > 0 => format!("TTL {minutes}m"),
        _ => row.validity.clone(),
    }
}

pub fn order_kind_label(row: &Order) -> &'static str {
    if row.variety == "iceberg" {
        "ICEBERG"
    } else if row
        .tags
        .iter()
        .any(|tag| tag == "autoslice" || tag.starts_with("autoslice:"))
    {
        "AUTOSLICE"
    } else {
        match row.variety.as_str() {
            "amo" => "AMO",
            "co" => "CO",
            _ => "",
        }
    }
}

pub fn alert_label(row: &Alert) -> String {
    crate::tui::format::instrument_label(&row.lhs_exchange, &row.lhs_tradingsymbol)
}

pub fn alert_is_enabled(row: &Alert) -> bool {
    row.status == AlertStatus::Enabled
}

pub fn alert_is_ato(row: &Alert) -> bool {
    row.alert_type == AlertType::Ato
}

pub fn alert_wire_type(row: &Alert) -> AlertType {
    if alert_is_ato(row) {
        AlertType::Ato
    } else {
        AlertType::Simple
    }
}

pub fn alert_basket_len(row: &Alert) -> Option<usize> {
    row.basket.as_ref().map(|basket| basket.items.len())
}

pub fn alert_condition(row: &Alert) -> String {
    let rhs = if row.rhs_type == "instrument" {
        format!(
            "{}(\"{}:{}\")",
            row.rhs_attribute.as_deref().unwrap_or(""),
            row.rhs_exchange.as_deref().unwrap_or(""),
            row.rhs_tradingsymbol.as_deref().unwrap_or("")
        )
    } else {
        row.rhs_constant.to_string()
    };
    format!(
        "{} {} {}",
        Attribute::label_for_wire(&row.lhs_attribute),
        row.operator,
        rhs
    )
}

#[expect(
    clippy::float_cmp,
    reason = "duplicate alerts require exactly equal user levels"
)]
pub fn alert_matches_params(row: &Alert, params: &AlertParams) -> bool {
    row.alert_type == AlertType::Simple
        && row.status == AlertStatus::Enabled
        && row.rhs_type == AlertRhsType::Constant.to_string()
        && row.lhs_exchange.eq_ignore_ascii_case(&params.lhs_exchange)
        && row
            .lhs_tradingsymbol
            .eq_ignore_ascii_case(&params.lhs_tradingsymbol)
        && row.lhs_attribute == params.lhs_attribute
        && row.operator == params.operator.to_string()
        && row.rhs_constant == params.rhs_constant
}

pub fn trigger_placed_orders(row: &AlertTrigger) -> bool {
    !matches!(row.order_meta, None | Some(Value::Null))
}

pub fn trigger_quote(row: &AlertTrigger) -> Option<&TriggerQuote> {
    row.meta.first()
}

pub fn profile_requires_holdings_authorisation(row: &Profile) -> bool {
    row.meta.poa.as_deref() == Some("consent")
}

pub fn candle_timestamp(candle: &Candle) -> String {
    candle.date.format("%Y-%m-%d %H:%M:%S").to_string()
}
