use crate::tui::bids;
use crate::tui::event::AppEvent;
use crate::tui::instruments::{self, InstrumentStore};
use crate::tui::marketwatch;
use crate::tui::settings::PollIntervals;
use kiteconnect::error::api_error_has_hint;
use kiteconnect::{AlertParams, AlertRhsType, AlertStatus, AlertType, KiteConnect, Request};
use serde_json::Value;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

mod calculations;
#[cfg(test)]
pub mod test_models;
pub use calculations::*;
mod browser;
mod candles;
mod funds;
mod movers;

pub use browser::{spawn_authorise_holdings, spawn_external_link};
pub use candles::{ChartInterval, spawn_fetch_candles};
pub use funds::{Funds, FundsSegment, fetch_funds, funds_margin_available};
pub use kiteconnect::{
    Alert, AlertBasketItem as FetchedBasketItem, AlertHistory as AlertTrigger,
    AlertHistoryMeta as TriggerQuote, AuctionInstrument as AuctionHolding, Candle,
    FullUserProfile as Profile, Gtt, Holding, MfHolding, Order, Position,
    Positions as PositionsData,
};
pub use movers::{
    MOVERS_ROWS, MoverPresets, Movers, MoversUniverse, ScreenerRow, spawn_refresh_movers,
};

pub fn fund_display_name(name: &str) -> &str {
    name.strip_suffix(" - Direct Plan").unwrap_or(name)
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MfTotals {
    pub invested: f64,
    pub current: f64,
    pub pnl: f64,
    pub day_pnl: f64,
    pub prev_value: f64,
}

impl MfTotals {
    pub fn pnl_pct(&self) -> Option<f64> {
        (self.invested != 0.0).then(|| self.pnl / self.invested * 100.0)
    }

    pub fn day_pct(&self) -> Option<f64> {
        (self.prev_value != 0.0).then(|| self.day_pnl / self.prev_value * 100.0)
    }
}

pub fn mf_totals(holdings: &[MfHolding]) -> MfTotals {
    let mut totals = MfTotals::default();
    for h in holdings {
        totals.invested += crate::tui::data::mf_invested(h);
        totals.current += crate::tui::data::mf_current(h);
        totals.pnl += h.pnl;
        totals.day_pnl += h.day_change.unwrap_or(0.0) * h.quantity;
        totals.prev_value += h.quantity * h.close_price.unwrap_or(0.0);
    }
    totals
}

#[derive(Debug, Clone)]
pub struct MarginPreview {
    pub total: f64,
    pub charges: f64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelOrderTarget {
    pub parent_order_id: Option<String>,
    pub variety: String,
    pub order_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancelOrderFailure {
    pub order_id: String,
    pub error: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CancelOrdersResult {
    pub cancelled_order_ids: Vec<String>,
    pub failures: Vec<CancelOrderFailure>,
}

impl CancelOrdersResult {
    pub fn summary(&self) -> String {
        let cancelled = self.cancelled_order_ids.len();
        let cancelled_label = if cancelled == 1 { "order" } else { "orders" };
        if self.failures.is_empty() {
            return format!(
                "{cancelled} {cancelled_label} cancelled: {}",
                self.cancelled_order_ids.join(", ")
            );
        }

        let failed = self.failures.len();
        let details = self
            .failures
            .iter()
            .map(|failure| format!("{} ({})", failure.order_id, failure.error))
            .collect::<Vec<_>>()
            .join("; ");
        format!("{cancelled} {cancelled_label} cancelled; {failed} failed: {details}")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct AlertListSequence(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertRequestId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderHistoryRequest {
    pub order_id: String,
    pub seq: u64,
}

#[derive(Debug)]
pub enum DataEvent {
    Circuits {
        token: u32,
        result: Result<Option<(f64, f64)>, String>,
    },
    Positions(Result<PositionsData, String>),
    Holdings(Result<Vec<Holding>, String>),
    Auctions(Result<Vec<AuctionHolding>, String>),
    Bids(bids::Event),
    Marketwatch(marketwatch::Event),
    MfHoldings(Result<Vec<MfHolding>, String>),
    Orders(Result<Vec<Order>, String>),
    Margins(Result<Box<Funds>, String>),
    Profile(Result<Box<Profile>, String>),
    Gtts(Result<Vec<Gtt>, String>),
    Alerts {
        sequence: AlertListSequence,
        result: Result<Vec<Alert>, String>,
    },
    AlertHistory {
        uuid: String,
        request: AlertRequestId,
        result: Result<Vec<AlertTrigger>, String>,
    },
    Movers {
        universe: MoversUniverse,
        result: Result<Box<Movers>, String>,
    },
    Instruments(Result<Arc<InstrumentStore>, String>),
    OrderPlaced(Result<String, String>),
    OrderModified(Result<String, String>),
    PositionConverted(Result<(), String>),
    HoldingsAuthorised(Result<String, String>),
    ExternalLink(Result<String, String>),
    OrdersCancelled(CancelOrdersResult),
    GttPlaced(Result<String, String>),
    GttModified(Result<String, String>),
    GttDeleted(Result<String, String>),
    AlertStatusSet {
        uuid: String,
        status: AlertStatus,
        confirmed: bool,
        result: Result<String, String>,
    },
    AlertCreated {
        request: AlertRequestId,
        result: Result<String, String>,
    },
    AlertModified {
        request: Option<AlertRequestId>,
        result: Result<String, String>,
    },
    AlertDeleted(Result<String, String>),
    MarginPreview {
        seq: u64,
        result: Result<MarginPreview, String>,
    },
    OrderHistory {
        request: OrderHistoryRequest,
        result: Result<Vec<Order>, String>,
    },
    Candles {
        token: u32,
        interval: ChartInterval,
        result: Result<Vec<Candle>, String>,
    },
}

pub(crate) fn api_err(err: impl Into<anyhow::Error>) -> String {
    let err = err.into();
    let err = match err.downcast_ref::<kiteconnect::Error>() {
        Some(kiteconnect::Error::Decode { path, source }) => {
            anyhow::anyhow!("bad {path} payload: {source}")
        }
        _ => err,
    };
    let text = crate::app::errors::format_error(&err);
    crate::app::errors::transport_error_summary(&text)
        .map(str::to_string)
        .unwrap_or(text)
}

pub(crate) fn api_write_err(err: kiteconnect::Error) -> String {
    if matches!(
        err,
        kiteconnect::Error::Transport(_)
            | kiteconnect::Error::Decode { .. }
            | kiteconnect::Error::Http {
                status: 500..=599,
                ..
            }
    ) {
        let detail = crate::app::errors::format_error(&anyhow::Error::new(err));
        format!(
            "outcome unknown; check the affected order, GTT, alert or position before retrying: {detail}"
        )
    } else {
        api_err(err)
    }
}

fn response_id(value: &Value, key: &str) -> Option<String> {
    match value.get(key)? {
        Value::String(id) if !id.trim().is_empty() => Some(id.clone()),
        Value::Number(id) if id.is_i64() || id.is_u64() => Some(id.to_string()),
        _ => None,
    }
}

pub async fn fetch_circuits(
    client: &KiteConnect,
    token: u32,
) -> Result<Option<(f64, f64)>, String> {
    let data = client.quote(&[&token.to_string()]).await.map_err(api_err)?;
    let Some(quote) = data.values().next() else {
        return Ok(None);
    };
    let lower = quote.lower_circuit_limit;
    let upper = quote.upper_circuit_limit;
    Ok(match (lower, upper) {
        (Some(lower), Some(upper)) if lower > 0.0 && upper > 0.0 => Some((lower, upper)),
        _ => None,
    })
}

pub fn spawn_fetch_circuits(client: KiteConnect, tx: mpsc::Sender<AppEvent>, token: u32) {
    tokio::spawn(async move {
        let result = fetch_circuits(&client, token).await;
        send(&tx, DataEvent::Circuits { token, result }).await;
    });
}

pub async fn fetch_positions(client: &KiteConnect) -> Result<PositionsData, String> {
    client.positions().await.map_err(api_err)
}

pub async fn fetch_holdings(client: &KiteConnect) -> Result<Vec<Holding>, String> {
    client.holdings().await.map_err(api_err)
}

pub async fn fetch_profile(client: &KiteConnect) -> Result<Profile, String> {
    client.full_profile().await.map_err(api_err)
}

pub async fn fetch_auctions(client: &KiteConnect) -> Result<Vec<AuctionHolding>, String> {
    client.auction_instruments().await.map_err(api_err)
}

pub async fn fetch_mf_holdings(client: &KiteConnect) -> Result<Vec<MfHolding>, String> {
    crate::app_api::portfolio::mf_holdings(client)
        .await
        .map_err(api_err)
}

pub async fn fetch_orders(client: &KiteConnect) -> Result<Vec<Order>, String> {
    let mut orders = client.orders().await.map_err(api_err)?;
    orders.sort_by(|a, b| {
        crate::tui::data::order_is_open(b)
            .cmp(&crate::tui::data::order_is_open(a))
            .then_with(|| b.order_timestamp.cmp(&a.order_timestamp))
    });
    Ok(orders)
}

pub async fn fetch_gtts(client: &KiteConnect) -> Result<Vec<Gtt>, String> {
    client.gtts().await.map_err(api_err)
}

pub async fn fetch_alerts(client: &KiteConnect) -> Result<Vec<Alert>, String> {
    client
        .alerts(&std::collections::HashMap::new())
        .await
        .map_err(api_err)
}

pub async fn fetch_alert_history(
    client: &KiteConnect,
    uuid: &str,
) -> Result<Vec<AlertTrigger>, String> {
    client.alert_history(uuid).await.map_err(api_err)
}

pub(crate) async fn send(tx: &mpsc::Sender<AppEvent>, event: DataEvent) -> bool {
    tx.send(AppEvent::Data(event)).await.is_ok()
}

pub fn spawn_refresh_positions(client: KiteConnect, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_positions(&client).await;
        send(&tx, DataEvent::Positions(result)).await;
    })
}

pub fn spawn_refresh_holdings(client: KiteConnect, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_holdings(&client).await;
        send(&tx, DataEvent::Holdings(result)).await;
    })
}

pub fn spawn_refresh_profile(client: KiteConnect, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_profile(&client).await;
        send(&tx, DataEvent::Profile(result.map(Box::new))).await;
    })
}

pub fn spawn_refresh_auctions(client: KiteConnect, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_auctions(&client).await;
        send(&tx, DataEvent::Auctions(result)).await;
    })
}

pub fn spawn_refresh_mf_holdings(client: KiteConnect, tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let result = fetch_mf_holdings(&client).await;
        send(&tx, DataEvent::MfHoldings(result)).await;
    });
}

pub fn spawn_refresh_orders(client: KiteConnect, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_orders(&client).await;
        send(&tx, DataEvent::Orders(result)).await;
    })
}

pub fn spawn_refresh_margins(client: KiteConnect, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_funds(&client).await.map(Box::new);
        send(&tx, DataEvent::Margins(result)).await;
    })
}

pub fn spawn_refresh_gtts(client: KiteConnect, tx: mpsc::Sender<AppEvent>) {
    tokio::spawn(async move {
        let result = fetch_gtts(&client).await;
        send(&tx, DataEvent::Gtts(result)).await;
    });
}

pub fn spawn_refresh_alerts(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    requests: Arc<AtomicU64>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let event = fetch_alert_snapshot(&client, &requests).await;
        send(&tx, event).await;
    })
}

async fn fetch_alert_snapshot(client: &KiteConnect, requests: &AtomicU64) -> DataEvent {
    let sequence = AlertListSequence(requests.fetch_add(1, Ordering::Relaxed) + 1);
    DataEvent::Alerts {
        sequence,
        result: fetch_alerts(client).await,
    }
}

pub fn spawn_alert_history(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    uuid: String,
    request: AlertRequestId,
) {
    tokio::spawn(async move {
        let result = fetch_alert_history(&client, &uuid).await;
        let event = DataEvent::AlertHistory {
            uuid,
            request,
            result,
        };
        send(&tx, event).await;
    });
}

pub fn spawn_create_alert(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    request: AlertRequestId,
    label: String,
    params: AlertParams,
) {
    tokio::spawn(async move {
        let result = client
            .create_alert(params)
            .raw()
            .await
            .map_err(api_write_err)
            .map(|_| label);
        send(&tx, DataEvent::AlertCreated { request, result }).await;
    });
}

pub fn spawn_modify_alert(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    request: AlertRequestId,
    uuid: String,
    label: String,
    params: AlertParams,
) {
    tokio::spawn(async move {
        let result = client
            .modify_alert(&uuid, params)
            .raw()
            .await
            .map_err(api_write_err)
            .map(|_| label);
        send(
            &tx,
            DataEvent::AlertModified {
                request: Some(request),
                result,
            },
        )
        .await;
    });
}

pub fn alert_status_request(uuid: &str, status: AlertStatus, alert_type: AlertType) -> Request {
    Request::put(format!("/alerts/{uuid}")).form([
        ("status", status.to_string()),
        ("type", alert_type.to_string()),
    ])
}

pub fn spawn_set_alert_status(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    uuid: String,
    label: String,
    status: AlertStatus,
    alert_type: AlertType,
    confirmed: bool,
) {
    tokio::spawn(async move {
        let result = client
            .call(alert_status_request(&uuid, status, alert_type))
            .raw()
            .await
            .map_err(api_write_err)
            .map(|_| label);
        send(
            &tx,
            DataEvent::AlertStatusSet {
                uuid,
                status,
                confirmed,
                result,
            },
        )
        .await;
    });
}

pub fn spawn_delete_alert(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    uuid: String,
    label: String,
) {
    tokio::spawn(async move {
        let result = match client.delete_alerts(&[&uuid]) {
            Ok(call) => call.raw().await.map_err(api_write_err).map(|_| label),
            Err(err) => Err(api_write_err(err)),
        };
        send(&tx, DataEvent::AlertDeleted(result)).await;
    });
}

pub fn spawn_margin_preview(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    seq: u64,
    order: serde_json::Value,
) {
    tokio::spawn(async move {
        let result = async {
            let order = serde_json::from_value::<kiteconnect::OrderMarginParam>(order)
                .map_err(|err| format!("bad margin request: {err}"))?;
            let rows = client
                .order_margins(kiteconnect::MarginParams::new(vec![order]))
                .await
                .map_err(api_err)?;
            let first = rows.first().ok_or("empty margin response")?;
            let charges = first
                .charges
                .as_ref()
                .ok_or("bad /margins/orders payload: missing charges")?;
            Ok(MarginPreview {
                total: first.total,
                charges: charges.total,
            })
        }
        .await;
        send(&tx, DataEvent::MarginPreview { seq, result }).await;
    });
}

pub fn spawn_order_history(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    request: OrderHistoryRequest,
) {
    tokio::spawn(async move {
        let result = client
            .order_history(&request.order_id)
            .await
            .map_err(api_err);
        send(&tx, DataEvent::OrderHistory { request, result }).await;
    });
}

pub fn spawn_place_gtt(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    form: Vec<(String, String)>,
) {
    tokio::spawn(async move {
        let result = client
            .call(Request::post("/gtt/triggers").form(form))
            .raw()
            .await
            .map_err(api_write_err)
            .map(|value| {
                response_id(&value, "trigger_id").unwrap_or_else(|| "unknown".to_string())
            });
        send(&tx, DataEvent::GttPlaced(result)).await;
    });
}

pub fn spawn_modify_gtt(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    trigger_id: i64,
    form: Vec<(String, String)>,
) {
    tokio::spawn(async move {
        let result = client
            .call(Request::put(format!("/gtt/triggers/{trigger_id}")).form(form))
            .raw()
            .await
            .map_err(api_write_err)
            .map(|value| {
                response_id(&value, "trigger_id").unwrap_or_else(|| trigger_id.to_string())
            });
        send(&tx, DataEvent::GttModified(result)).await;
    });
}

pub fn spawn_delete_gtt(client: KiteConnect, tx: mpsc::Sender<AppEvent>, trigger_id: i64) {
    tokio::spawn(async move {
        let result = client
            .delete_gtt(trigger_id)
            .raw()
            .await
            .map_err(api_write_err)
            .map(|value| {
                response_id(&value, "trigger_id").unwrap_or_else(|| trigger_id.to_string())
            });
        send(&tx, DataEvent::GttDeleted(result)).await;
    });
}

pub fn spawn_load_instruments(app_uuid: String, tx: mpsc::Sender<AppEvent>) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = instruments::load(&app_uuid).await.map(Arc::new);
        send(&tx, DataEvent::Instruments(result)).await;
    })
}

pub fn spawn_pollers(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    intervals: PollIntervals,
    alert_requests: Arc<AtomicU64>,
) -> Vec<JoinHandle<()>> {
    vec![
        poll(
            client.clone(),
            tx.clone(),
            intervals.orders,
            |client| async move { DataEvent::Orders(fetch_orders(&client).await) },
        ),
        poll(
            client.clone(),
            tx.clone(),
            intervals.positions,
            |client| async move { DataEvent::Positions(fetch_positions(&client).await) },
        ),
        poll(
            client.clone(),
            tx.clone(),
            intervals.holdings,
            |client| async move { DataEvent::Holdings(fetch_holdings(&client).await) },
        ),
        poll(
            client.clone(),
            tx.clone(),
            intervals.holdings,
            |client| async move { DataEvent::Auctions(fetch_auctions(&client).await) },
        ),
        poll(
            client.clone(),
            tx.clone(),
            intervals.holdings,
            |client| async move { DataEvent::MfHoldings(fetch_mf_holdings(&client).await) },
        ),
        poll(
            client.clone(),
            tx.clone(),
            intervals.margins,
            |client| async move { DataEvent::Margins(fetch_funds(&client).await.map(Box::new)) },
        ),
        poll(
            client.clone(),
            tx.clone(),
            intervals.gtt,
            |client| async move { DataEvent::Gtts(fetch_gtts(&client).await) },
        ),
        poll(client, tx, intervals.alerts, move |client| {
            let requests = alert_requests.clone();
            async move { fetch_alert_snapshot(&client, &requests).await }
        }),
    ]
}

fn poll<Fut>(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    every: Duration,
    fetch: impl Fn(KiteConnect) -> Fut + Send + 'static,
) -> JoinHandle<()>
where
    Fut: Future<Output = DataEvent> + Send + 'static,
{
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(every);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let event = fetch(client.clone()).await;
            if !send(&tx, event).await {
                return;
            }
        }
    })
}

async fn place_with_amo_fallback(
    client: &KiteConnect,
    variety: &str,
    form: Vec<(String, String)>,
) -> kiteconnect::Result<Value> {
    match client
        .call(Request::post(format!("/orders/{variety}")).form(form.clone()))
        .raw()
        .await
    {
        Ok(value) => Ok(value),
        Err(err) if variety == "regular" && api_error_has_hint(&err, 400, "switch_to_amo") => {
            let mut amo_form = form;
            amo_form.push(("tag".to_string(), "switch_to_amo".to_string()));
            client
                .call(Request::post("/orders/amo").form(amo_form))
                .raw()
                .await
        }
        Err(err) => Err(err),
    }
}

pub fn spawn_place_order(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    variety: String,
    form: Vec<(String, String)>,
) {
    tokio::spawn(async move {
        let result = place_with_amo_fallback(&client, &variety, form)
            .await
            .map_err(api_write_err)
            .map(|value| response_id(&value, "order_id").unwrap_or_else(|| "unknown".to_string()));
        send(&tx, DataEvent::OrderPlaced(result)).await;
    });
}

pub fn spawn_modify_order(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    variety: String,
    order_id: String,
    form: Vec<(String, String)>,
) {
    tokio::spawn(async move {
        let result = client
            .call(Request::put(format!("/orders/{variety}/{order_id}")).form(form))
            .raw()
            .await
            .map_err(api_write_err)
            .map(|value| response_id(&value, "order_id").unwrap_or(order_id));
        send(&tx, DataEvent::OrderModified(result)).await;
    });
}

pub fn spawn_convert_position(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    form: Vec<(String, String)>,
) {
    tokio::spawn(async move {
        let result = client
            .call(Request::put("/portfolio/positions").form(form))
            .raw()
            .await
            .map_err(api_write_err)
            .map(|_| ());
        send(&tx, DataEvent::PositionConverted(result)).await;
    });
}

pub fn spawn_cancel_orders(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    targets: Vec<CancelOrderTarget>,
) {
    tokio::spawn(async move {
        let mut result = CancelOrdersResult::default();
        let mut targets = targets.into_iter().peekable();
        while let Some(CancelOrderTarget {
            variety,
            order_id,
            parent_order_id,
        }) = targets.next()
        {
            match client
                .call(
                    Request::delete(format!("/orders/{variety}/{order_id}"))
                        .query(parent_order_id.map(|id| ("parent_order_id", id))),
                )
                .raw()
                .await
                .map_err(api_write_err)
            {
                Ok(_) => result.cancelled_order_ids.push(order_id),
                Err(error) => result.failures.push(CancelOrderFailure { order_id, error }),
            }
            if targets.peek().is_some() {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        send(&tx, DataEvent::OrdersCancelled(result)).await;
    });
}

#[cfg(test)]
mod tests {
    use super::test_models::{alerts_get_fixture, alerts_history_fixture};
    use super::{
        Alert, AlertTrigger, DataEvent, KiteConnect, MfHolding, api_err, fund_display_name,
        mf_totals, place_with_amo_fallback,
    };
    use crate::tui::event::AppEvent;
    use httpmock::Method::{DELETE, GET, POST, PUT};
    use httpmock::{Mock, MockServer};
    use kiteconnect::error::ApiError;
    use kiteconnect::{AlertType, Error};

    use proptest::prelude::*;
    use serde_json::{Value, json};

    use tokio::sync::mpsc;

    #[test]
    fn alerts_parse_the_list_payload_including_ato() {
        let parsed: Vec<Alert> =
            serde_json::from_value(alerts_get_fixture()).expect("alerts parse");
        assert_eq!(parsed.len(), 2);

        let simple = &parsed[0];
        assert_eq!(simple.alert_type, AlertType::Simple);
        assert!(!crate::tui::data::alert_is_ato(simple));
        assert_eq!(simple.uuid, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(simple.name, "NIFTY 50");
        assert!(crate::tui::data::alert_is_enabled(simple));
        assert_eq!(simple.disabled_reason.as_deref(), Some(""));
        assert_eq!(crate::tui::data::alert_label(simple), "INDICES:NIFTY 50");
        assert_eq!(
            crate::tui::data::alert_condition(simple),
            "Last price >= 27000"
        );
        assert_eq!(simple.alert_count, 0);

        let ato = &parsed[1];
        assert!(crate::tui::data::alert_is_ato(ato));
        assert_eq!(crate::tui::data::alert_wire_type(ato), AlertType::Ato);
        let basket = ato.basket.as_ref().expect("an ATO carries its basket");
        assert_eq!(crate::tui::data::alert_basket_len(ato), Some(1));
        assert_eq!(basket.items.len(), 1);
        let item = &basket.items[0];
        assert_eq!(item.id, Some(275_218_517));
        assert_eq!(item.exchange, "NSE");
        assert_eq!(item.tradingsymbol, "GOLDBEES");
        assert_eq!(item.instrument_token, Some(3_693_569));
        assert_eq!(item.weight, 10_000, "fetched weights are not always 1..n");
        assert_eq!(
            item.params,
            serde_json::json!({"order_type": "LIMIT", "quantity": 10000, "price": 72.22}),
            "params must survive as the exact object the service sent"
        );
        assert_eq!(crate::tui::data::alert_basket_len(simple), None);
        assert_eq!(crate::tui::data::alert_wire_type(simple), AlertType::Simple);
        assert!(!crate::tui::data::alert_is_enabled(ato));
        assert_eq!(crate::tui::data::alert_label(ato), "NSE:GOLDBEES");
        assert_eq!(crate::tui::data::alert_condition(ato), "Last price <= 71.8");
        assert_eq!(ato.alert_count, 1);

        assert_eq!(simple.created_at.to_string(), "2025-05-26 12:07:50");
        assert_eq!(ato.created_at.to_string(), "2025-02-17 08:24:10");
    }

    #[test]
    fn order_kind_identifies_iceberg_and_autoslice() {
        let order = |variety: &str, tags: &[&str]| {
            let mut order = crate::tui::data::test_models::order();
            order.variety = variety.into();
            order.tags = tags.iter().map(|tag| (*tag).into()).collect();
            order
        };
        assert_eq!(
            crate::tui::data::order_kind_label(&order("iceberg", &[])),
            "ICEBERG"
        );
        assert_eq!(
            crate::tui::data::order_kind_label(&order("regular", &["autoslice:abc"])),
            "AUTOSLICE"
        );
        assert_eq!(
            crate::tui::data::order_kind_label(&order("amo", &[])),
            "AMO"
        );
    }

    #[test]
    fn alert_history_parses_the_trigger_payload() {
        let parsed: Vec<AlertTrigger> =
            serde_json::from_value(alerts_history_fixture()).expect("history parse");
        assert_eq!(parsed.len(), 1);

        let trigger = &parsed[0];
        assert_eq!(trigger.created_at.to_string(), "2025-02-17 09:16:46");
        assert_eq!(
            trigger.condition,
            "LastTradedPrice(\"INDICES:NIFTY NEXT 50\") <= 58290.35"
        );
        assert!(!crate::tui::data::trigger_placed_orders(trigger));

        let quote = crate::tui::data::trigger_quote(trigger).expect("trigger quote");
        assert_eq!(quote.last_price.to_bits(), 58288.05_f64.to_bits());
        assert_eq!(quote.net_change.to_bits(), (-1269.9_f64).to_bits());
        assert_eq!(quote.ohlc.open.to_bits(), 59221.95_f64.to_bits());
        assert_eq!(quote.ohlc.high.to_bits(), 59241.8_f64.to_bits());
        assert_eq!(quote.ohlc.low.to_bits(), 58283.7_f64.to_bits());
        assert_eq!(quote.ohlc.close.to_bits(), 59557.95_f64.to_bits());
        assert_eq!(quote.volume, 0);
    }

    #[test]
    fn an_ato_trigger_with_order_meta_parses_and_reports_its_orders() {
        let parsed: Vec<AlertTrigger> = serde_json::from_value(serde_json::json!([
            {
                "uuid": "e888ed4a-6801-406f-bdc2-002db5a8411d",
                "type": "ato",
                "meta": [
                    {
                        "instrument_token": 3_693_569,
                        "tradingsymbol": "GOLDBEES",
                        "timestamp": "2025-02-17 09:15:20",
                        "exchange": "NSE",
                        "last_quantity": 0,
                        "buy_quantity": 0,
                        "sell_quantity": 0,
                        "volume_tick": 0,
                        "average_price": 0,
                        "oi": 0,
                        "oi_day_high": 0,
                        "oi_day_low": 0,
                        "lower_circuit_limit": 0,
                        "upper_circuit_limit": 0,
                        "last_price": 71.8,
                        "net_change": -0.42,
                        "ohlc": { "open": 72.3, "high": 72.4, "low": 71.75, "close": 72.22 },
                        "volume": 4_823_910
                    }
                ],
                "condition": "LastTradedPrice(\"NSE:GOLDBEES\") <= 71.80",
                "created_at": "2025-02-17 09:15:20",
                "order_meta": { "orders": [{ "order_id": "250217000123456" }] }
            }
        ]))
        .expect("ato history parse");
        let trigger = &parsed[0];
        assert!(crate::tui::data::trigger_placed_orders(trigger));
        assert_eq!(
            (crate::tui::data::trigger_quote(trigger)
                .expect("quote")
                .last_price)
                .to_bits(),
            71.8_f64.to_bits()
        );
        assert_eq!(
            crate::tui::data::trigger_quote(trigger)
                .expect("quote")
                .volume,
            4_823_910
        );
    }

    #[test]
    fn an_alert_that_never_fired_parses_as_an_empty_history() {
        let parsed: Vec<AlertTrigger> =
            serde_json::from_value(serde_json::json!([])).expect("empty history parse");
        assert!(parsed.is_empty());
    }

    #[test]
    fn instrument_rhs_renders_the_instrument_not_a_zero_constant() {
        let mut alert = crate::tui::data::test_models::alert();
        alert.lhs_attribute = "LastTradedPrice".into();
        alert.operator = ">=".into();
        alert.rhs_type = "instrument".into();
        alert.rhs_attribute = Some("LastTradedPrice".into());
        alert.rhs_exchange = Some("NSE".into());
        alert.rhs_tradingsymbol = Some("TCS".into());
        alert.rhs_constant = 0.0;
        assert_eq!(
            crate::tui::data::alert_condition(&alert),
            "Last price >= LastTradedPrice(\"NSE:TCS\")"
        );
    }

    #[test]
    fn list_condition_includes_lhs_attribute_label_and_exact_operator() {
        let alert = {
            let mut row = crate::tui::data::test_models::alert();
            row.lhs_attribute = "HighPrice".into();
            row.operator = ">".into();
            row.rhs_type = "constant".into();
            row.rhs_constant = 123.45;
            row
        };
        assert_eq!(
            crate::tui::data::alert_condition(&alert),
            "High price > 123.45"
        );

        let unknown = {
            let mut row = crate::tui::data::test_models::alert();
            row.lhs_attribute = "FutureAttribute".into();
            row.operator = "!=".into();
            row.rhs_type = "constant".into();
            row.rhs_constant = 5.0;
            row
        };
        assert_eq!(
            crate::tui::data::alert_condition(&unknown),
            "FutureAttribute != 5"
        );
    }

    #[test]
    fn duplicate_detection_only_rejects_an_identical_condition() {
        let existing: Vec<Alert> =
            serde_json::from_value(alerts_get_fixture()).expect("alerts parse");
        let same = crate::tui::alerts::params(
            "NIFTY 50",
            "INDICES",
            "NIFTY 50",
            crate::tui::alerts::Attribute::LastTradedPrice,
            crate::tui::alerts::Operator::GreaterThanOrEqual,
            27000.0,
            true,
        );
        assert!(
            existing
                .iter()
                .any(|a| crate::tui::data::alert_matches_params(a, &same))
        );

        let other_side = crate::tui::alerts::params(
            "NIFTY 50",
            "INDICES",
            "NIFTY 50",
            crate::tui::alerts::Attribute::LastTradedPrice,
            crate::tui::alerts::Operator::LessThanOrEqual,
            27000.0,
            true,
        );
        assert!(
            !existing
                .iter()
                .any(|a| crate::tui::data::alert_matches_params(a, &other_side))
        );

        let other_level = crate::tui::alerts::params(
            "NIFTY 50",
            "INDICES",
            "NIFTY 50",
            crate::tui::alerts::Attribute::LastTradedPrice,
            crate::tui::alerts::Operator::GreaterThanOrEqual,
            27500.0,
            true,
        );
        assert!(
            !existing
                .iter()
                .any(|a| crate::tui::data::alert_matches_params(a, &other_level))
        );

        let renamed = crate::tui::alerts::params(
            "my nifty alert",
            "INDICES",
            "NIFTY 50",
            crate::tui::alerts::Attribute::LastTradedPrice,
            crate::tui::alerts::Operator::GreaterThanOrEqual,
            27000.0,
            true,
        );
        assert!(
            existing
                .iter()
                .any(|a| crate::tui::data::alert_matches_params(a, &renamed))
        );
    }

    #[test]
    fn fully_pledged_holding_counts_collateral() {
        let h = {
            let mut row = crate::tui::data::test_models::holding();
            row.tradingsymbol = "INFY".into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.quantity = 0;
            row.t1_quantity = 0;
            row.collateral_quantity = 50;
            row.mtf = Some(crate::tui::data::test_models::mtf());
            row.average_price = 1400.0;
            row.last_price = 1600.0;
            row.close_price = 1550.0;
            row.day_change = None;
            row.discrepancy = false;
            row
        };
        let qty = crate::tui::data::holding_total_quantity(&h);
        assert_eq!(qty, 50);
        #[expect(
            clippy::cast_precision_loss,
            reason = "fixture quantity 50 is exactly representable in f64"
        )]
        let value = h.last_price * qty as f64;
        #[expect(
            clippy::cast_precision_loss,
            reason = "fixture quantity 50 is exactly representable in f64"
        )]
        let pnl = (h.last_price - h.average_price) * qty as f64;
        assert_eq!(value.to_bits(), 80_000.0_f64.to_bits());
        assert_eq!(pnl.to_bits(), 10_000.0_f64.to_bits());
    }

    #[test]
    fn free_quantity_excludes_pledged_collateral() {
        let h = {
            let mut row = crate::tui::data::test_models::holding();
            row.tradingsymbol = "INFY".into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.quantity = 5;
            row.t1_quantity = 2;
            row.collateral_quantity = 50;
            row.mtf = Some(crate::tui::data::test_models::mtf());
            row.average_price = 1400.0;
            row.last_price = 1600.0;
            row.close_price = 1550.0;
            row.day_change = None;
            row.discrepancy = false;
            row
        };
        assert_eq!(crate::tui::data::holding_free_quantity(&h), 7);
        assert_eq!(crate::tui::data::holding_total_quantity(&h), 57);
    }

    #[test]
    fn total_quantity_includes_nested_mtf_quantity() {
        let mut h = {
            let mut row = crate::tui::data::test_models::holding();
            row.tradingsymbol = "INFY".into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.quantity = 5;
            row.t1_quantity = 2;
            row.collateral_quantity = 3;
            row.mtf = Some({
                let mut row = crate::tui::data::test_models::mtf();
                row.quantity = 11;
                row
            });
            row.average_price = 1400.0;
            row.last_price = 1600.0;
            row.close_price = 1550.0;
            row.day_change = None;
            row.discrepancy = false;
            row
        };
        assert_eq!(crate::tui::data::holding_total_quantity(&h), 21);
        assert_eq!(
            crate::tui::data::holding_live_day_pnl(&h, None).to_bits(),
            ((1600.0_f64 - 1550.0_f64) * 21.0_f64).to_bits()
        );

        assert_eq!(crate::tui::data::holding_free_quantity(&h), 7);
        h.mtf.as_mut().expect("mtf").quantity = 0;
        assert_eq!(crate::tui::data::holding_total_quantity(&h), 10);
    }

    #[test]
    fn holding_pnl_recomputes_over_pledged_quantity_like_kite_web() {
        let h = {
            let mut row = crate::tui::data::test_models::holding();
            row.tradingsymbol = "INFY".into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.quantity = 10;
            row.t1_quantity = 0;
            row.collateral_quantity = 5;
            row.mtf = Some(crate::tui::data::test_models::mtf());
            row.average_price = 20.0;
            row.last_price = 100.0;
            row.close_price = 98.0;
            row.day_change = Some(2.0);
            row.discrepancy = false;
            row
        };

        assert_eq!(
            crate::tui::data::holding_live_pnl(&h, None).to_bits(),
            1_200.0_f64.to_bits()
        );
        assert_eq!(
            crate::tui::data::holding_live_day_pnl(&h, None).to_bits(),
            30.0_f64.to_bits()
        );
        assert_eq!(
            crate::tui::data::holding_live_pnl(&h, Some(101.0)).to_bits(),
            1_215.0_f64.to_bits()
        );
        assert_eq!(
            crate::tui::data::holding_live_day_pnl(&h, Some(101.0)).to_bits(),
            45.0_f64.to_bits()
        );

        let flagged = {
            let mut row = h.clone();
            row.discrepancy = true;
            row
        };
        assert!(!crate::tui::data::holding_has_pnl(&flagged, Some(101.0)));
        assert!(crate::tui::data::holding_has_pnl(&h, Some(101.0)));
        let unpriced = {
            let mut row = h.clone();
            row.last_price = 0.0;
            row
        };
        assert!(!crate::tui::data::holding_has_pnl(&unpriced, None));
        assert!(crate::tui::data::holding_has_pnl(&unpriced, Some(101.0)));

        let no_close = {
            let mut row = h.clone();
            row.close_price = 0.0;
            row
        };
        assert_eq!(
            crate::tui::data::holding_live_day_pnl(&no_close, None).to_bits(),
            (2.0_f64 * 15.0_f64).to_bits()
        );
    }

    fn mf(qty: f64, avg: f64, nav: f64, close: f64, pnl: f64, day_change: f64) -> MfHolding {
        {
            let mut row = crate::tui::data::test_models::mf();
            row.tradingsymbol = "Fund".into();
            row.quantity = qty;
            row.average_price = avg;
            row.last_price = nav;
            row.close_price = Some(close);
            row.pnl = pnl;
            row.day_change = Some(day_change);
            row.day_change_percentage = Some(0.0);
            row
        }
    }

    #[test]
    fn mf_day_change_defaults_to_zero_when_percentage_is_absent() {
        let mut holding = crate::tui::data::test_models::mf();
        holding.close_price = Some(84.0);
        holding.day_change_percentage = None;
        assert_eq!(super::mf_day_change_pct(&holding), Some(0.0));
    }

    #[test]
    fn api_err_sanitizes_secrets() {
        let err = anyhow::anyhow!(r#"request failed: {{"access_token":"SECRETVAL123"}}"#);
        let out = api_err(err);
        assert!(!out.contains("SECRETVAL123"), "access_token leaked: {out}");
        assert!(out.contains("<redacted>"));
    }

    #[test]
    fn fund_display_name_strips_direct_plan_suffix() {
        assert_eq!(
            fund_display_name("DSP Ultra Short Fund - Direct Plan"),
            "DSP Ultra Short Fund"
        );
        assert_eq!(
            fund_display_name("DSP Ultra Short Fund"),
            "DSP Ultra Short Fund"
        );
        assert_eq!(
            fund_display_name("Direct Plan - Growth"),
            "Direct Plan - Growth"
        );
    }

    #[test]
    fn return_pct_none_only_when_no_cost_basis() {
        assert_eq!(
            crate::tui::data::mf_return_pct(&mf(0.0, 0.0, 10.0, 9.0, 0.0, 0.0)),
            None
        );
        assert_eq!(
            crate::tui::data::mf_return_pct(&mf(5.0, 100.0, 110.0, 100.0, 50.0, 10.0)),
            Some(10.0)
        );
    }

    fn arb_mf() -> impl Strategy<Value = MfHolding> {
        let qty = prop_oneof![Just(0.0), 1e-3..1e6];
        let avg = prop_oneof![Just(0.0), 1e-3..1e5];
        let nav = prop_oneof![Just(0.0), 1e-3..1e5];
        let close = prop_oneof![Just(0.0), 1e-3..1e5];
        let pnl = -1e7..1e7;
        let day = -1e5..1e5;
        (qty, avg, nav, close, pnl, day)
            .prop_map(|(qty, avg, nav, close, pnl, day)| mf(qty, avg, nav, close, pnl, day))
    }

    proptest! {
        #[test]
        fn mf_row_math_holds(h in arb_mf()) {
            prop_assert_eq!(crate::tui::data::mf_invested(&h).to_bits(), (h.quantity * h.average_price).to_bits());
            prop_assert_eq!(crate::tui::data::mf_current(&h).to_bits(), (h.quantity * h.last_price).to_bits());
            prop_assert!(crate::tui::data::mf_invested(&h) >= 0.0, "invested negative for non-negative inputs");
            prop_assert!(crate::tui::data::mf_current(&h) >= 0.0, "current negative for non-negative inputs");

            match crate::tui::data::mf_return_pct(&h) {
                None => prop_assert_eq!(h.average_price.to_bits(), 0.0_f64.to_bits()),
                Some(pct) => {
                    prop_assert!(h.average_price != 0.0);
                    prop_assert!(pct.is_finite(), "return_pct is not finite");
                    prop_assert_eq!(
                        pct.partial_cmp(&0.0),
                        (h.last_price - h.average_price).partial_cmp(&0.0),
                        "return_pct sign disagrees with nav - avg"
                    );
                }
            }
        }

        #[test]
        fn mf_totals_match_naive_sum(hs in prop::collection::vec(arb_mf(), 0..40)) {
            let totals = mf_totals(&hs);

            let mut invested = 0.0;
            let mut current = 0.0;
            let mut pnl = 0.0;
            let mut day_pnl = 0.0;
            let mut prev_value = 0.0;
            for h in &hs {
                invested += h.quantity * h.average_price;
                current += h.quantity * h.last_price;
                pnl += h.pnl;
                day_pnl += h.day_change.unwrap_or(0.0) * h.quantity;
                prev_value += h.quantity * h.close_price.unwrap_or(0.0);
            }
            prop_assert_eq!(totals.invested.to_bits(), invested.to_bits());
            prop_assert_eq!(totals.current.to_bits(), current.to_bits());
            prop_assert_eq!(totals.pnl.to_bits(), pnl.to_bits());
            prop_assert_eq!(totals.day_pnl.to_bits(), day_pnl.to_bits());
            prop_assert_eq!(totals.prev_value.to_bits(), prev_value.to_bits());

            prop_assert!(totals.invested.is_finite());
            prop_assert!(totals.current.is_finite());
            prop_assert_eq!(totals.pnl_pct().is_none(), totals.invested == 0.0);
            prop_assert_eq!(totals.day_pct().is_none(), totals.prev_value == 0.0);
            if let Some(pct) = totals.pnl_pct() {
                prop_assert!(pct.is_finite(), "pnl_pct not finite");
                prop_assert_eq!(
                    pct.partial_cmp(&0.0),
                    totals.pnl.partial_cmp(&0.0),
                    "pnl_pct sign disagrees with total pnl"
                );
            }
            if let Some(pct) = totals.day_pct() {
                prop_assert!(pct.is_finite(), "day_pct not finite");
                prop_assert_eq!(
                    pct.partial_cmp(&0.0),
                    totals.day_pnl.partial_cmp(&0.0),
                    "day_pct sign disagrees with total day_pnl"
                );
            }
        }

        #[test]
        fn mf_totals_are_order_independent(
            hs in prop::collection::vec(arb_mf_exact(), 0..40)
        ) {
            let base = mf_totals(&hs);

            let mut reversed = hs.clone();
            reversed.reverse();
            prop_assert_eq!(base, mf_totals(&reversed));

            if !hs.is_empty() {
                let mut rotated = hs.clone();
                rotated.rotate_left(1);
                prop_assert_eq!(base, mf_totals(&rotated));
            }
        }
    }

    fn arb_mf_exact() -> impl Strategy<Value = MfHolding> {
        let ints = || (0i32..1000).prop_map(f64::from);
        let signed = || (-1000i32..1000).prop_map(f64::from);
        (ints(), ints(), ints(), ints(), signed(), signed())
            .prop_map(|(qty, avg, nav, close, pnl, day)| mf(qty, avg, nav, close, pnl, day))
    }

    #[test]
    fn cancel_orders_summary_names_partial_failures() {
        let result = super::CancelOrdersResult {
            cancelled_order_ids: vec!["OID1".to_string()],
            failures: vec![
                super::CancelOrderFailure {
                    order_id: "OID2".to_string(),
                    error: "request timed out".to_string(),
                },
                super::CancelOrderFailure {
                    order_id: "OID3".to_string(),
                    error: "Kite rejected cancellation".to_string(),
                },
            ],
        };

        assert_eq!(
            result.summary(),
            "1 order cancelled; 2 failed: OID2 (request timed out); OID3 (Kite rejected cancellation)"
        );
    }

    #[test]
    fn full_profile_only_requires_cdsl_for_consent_accounts() {
        for (poa, requires) in [("consent", true), ("ddpi", false), ("physical", false)] {
            let mut profile = crate::tui::data::test_models::profile();
            profile.meta.poa = Some(poa.into());
            assert_eq!(
                crate::tui::data::profile_requires_holdings_authorisation(&profile),
                requires,
                "{poa}"
            );
        }
    }

    pub(super) fn connect_client(server: &MockServer) -> KiteConnect {
        KiteConnect::builder("key".to_string())
            .access_token(
                kiteconnect::AccessToken::new("token".to_string()).expect("connect credentials"),
            )
            .base_url(server.base_url())
            .build()
            .expect("connect client")
    }

    pub(super) async fn reply<'a>(
        server: &'a MockServer,
        method: httpmock::Method,
        path: &str,
        data: Value,
    ) -> Mock<'a> {
        let body = json!({ "status": "success", "data": data }).to_string();
        server
            .mock_async(|when, then| {
                when.method(method).path(path);
                then.status(200)
                    .header("content-type", "application/json")
                    .body(body);
            })
            .await
    }

    pub(super) async fn next_event(rx: &mut mpsc::Receiver<AppEvent>) -> DataEvent {
        match rx.recv().await {
            Some(AppEvent::Data(event)) => event,
            _ => panic!("the task ended without sending a data event"),
        }
    }

    pub(super) fn fixture_data(fixture: &str) -> Value {
        super::test_models::fixture(fixture, "/data")
    }

    #[tokio::test]
    async fn connect_keeps_documented_shared_reads() {
        let server = MockServer::start_async().await;
        let auctions = reply(
            &server,
            GET,
            "/portfolio/holdings/auctions",
            fixture_data(include_str!("../../mock_responses/auctions_list.json")),
        )
        .await;

        let rows = super::fetch_auctions(&connect_client(&server))
            .await
            .expect("auctions");

        assert_eq!(rows.len(), 3);
        auctions.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn profile_and_mf_reads_use_token_headers_without_a_prefix() {
        let server = MockServer::start_async().await;
        let token_get = |path: &'static str, fixture: &'static str| {
            let body = fixture.to_string();
            server.mock_async(move |when, then| {
                when.method(GET)
                    .path(path)
                    .header("authorization", "token key:token")
                    .header("x-kite-version", "3");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(body);
            })
        };
        let profile_mock = token_get(
            "/user/profile/full",
            include_str!("../../mock_responses/full_profile.json"),
        )
        .await;
        let mf_mock = token_get(
            "/portfolio/holdings/mf",
            include_str!("../../mock_responses/mf_holdings.json"),
        )
        .await;
        let client = connect_client(&server);

        let profile = super::fetch_profile(&client).await.expect("profile");
        let funds = super::fetch_mf_holdings(&client)
            .await
            .expect("mf holdings");

        assert_eq!(profile.meta.poa.as_deref(), Some("physical"));
        assert_eq!(funds.len(), 3);
        assert_eq!(funds[0].tradingsymbol, "INF205K01NT8");
        profile_mock.assert_calls_async(1).await;
        mf_mock.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn conversion_writer_sends_form_and_reports_completion() {
        let server = MockServer::start_async().await;
        let mock = server.mock_async(|when, then| {
            when.method(httpmock::Method::PUT).path("/portfolio/positions")
                .body("exchange=NSE&tradingsymbol=INFY&transaction_type=BUY&position_type=day&quantity=1&old_product=MIS&new_product=CNC");
            then.status(200).header("content-type", "application/json")
                .body(include_str!("../../mock_responses/convert_position.json"));
        }).await;
        let form = vec![
            ("exchange".to_string(), "NSE".to_string()),
            ("tradingsymbol".to_string(), "INFY".to_string()),
            ("transaction_type".to_string(), "BUY".to_string()),
            ("position_type".to_string(), "day".to_string()),
            ("quantity".to_string(), "1".to_string()),
            ("old_product".to_string(), "MIS".to_string()),
            ("new_product".to_string(), "CNC".to_string()),
        ];
        let (tx, mut rx) = mpsc::channel(1);
        super::spawn_convert_position(connect_client(&server), tx, form);
        match next_event(&mut rx).await {
            DataEvent::PositionConverted(result) => result.unwrap(),
            _ => panic!("expected conversion completion"),
        }
        mock.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn success_without_data_is_a_payload_error() {
        let server = MockServer::start_async().await;
        server
            .mock_async(|when, then| {
                when.method(GET).path("/portfolio/positions");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(r#"{"status":"success"}"#);
            })
            .await;
        let (tx, mut rx) = mpsc::channel(1);

        super::spawn_refresh_positions(connect_client(&server), tx)
            .await
            .expect("positions task");

        match next_event(&mut rx).await {
            DataEvent::Positions(Err(err)) => {
                assert!(err.starts_with("bad /portfolio/positions payload"), "{err}");
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_placed_order_reports_a_readable_id_or_unknown() {
        for (data, expected) in [
            (json!({ "order_id": "151220000000000" }), "151220000000000"),
            (json!({ "order_id": 123 }), "123"),
            (json!({}), "unknown"),
            (Value::Null, "unknown"),
            (json!({ "order_id": "" }), "unknown"),
            (json!({ "order_id": 1.5 }), "unknown"),
            (json!([{ "order_id": "1" }, { "order_id": "2" }]), "unknown"),
        ] {
            let server = MockServer::start_async().await;
            let regular = reply(&server, POST, "/orders/regular", data.clone()).await;
            let amo = reply(&server, POST, "/orders/amo", json!({})).await;
            let (tx, mut rx) = mpsc::channel(1);

            super::spawn_place_order(
                connect_client(&server),
                tx,
                "regular".to_string(),
                vec![("tradingsymbol".to_string(), "INFY".to_string())],
            );

            match next_event(&mut rx).await {
                DataEvent::OrderPlaced(Ok(id)) => assert_eq!(id, expected, "{data}"),
                other => panic!("{data}: unexpected {other:?}"),
            }
            regular.assert_calls_async(1).await;
            amo.assert_calls_async(0).await;
        }
    }

    #[tokio::test]
    async fn a_modified_order_without_an_echoed_id_reports_its_own_id() {
        let server = MockServer::start_async().await;
        let put = reply(&server, PUT, "/orders/regular/OID-7", json!({})).await;
        let (tx, mut rx) = mpsc::channel(1);

        super::spawn_modify_order(
            connect_client(&server),
            tx,
            "regular".to_string(),
            "OID-7".to_string(),
            vec![("price".to_string(), "10".to_string())],
        );

        match next_event(&mut rx).await {
            DataEvent::OrderModified(Ok(id)) => assert_eq!(id, "OID-7"),
            other => panic!("unexpected {other:?}"),
        }
        put.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn gtt_writes_report_ids_without_json_quotes() {
        let form = || {
            vec![
                ("type".to_string(), "single".to_string()),
                ("condition".to_string(), "{}".to_string()),
                ("orders".to_string(), "[]".to_string()),
            ]
        };
        for (data, expected) in [
            (json!({ "trigger_id": 123 }), "123"),
            (json!({ "trigger_id": "123" }), "123"),
            (json!({}), "unknown"),
        ] {
            let server = MockServer::start_async().await;
            let post = reply(&server, POST, "/gtt/triggers", data.clone()).await;
            let (tx, mut rx) = mpsc::channel(1);
            super::spawn_place_gtt(connect_client(&server), tx, form());
            match next_event(&mut rx).await {
                DataEvent::GttPlaced(Ok(id)) => assert_eq!(id, expected, "{data}"),
                other => panic!("{data}: unexpected {other:?}"),
            }
            post.assert_calls_async(1).await;
        }

        for (data, expected) in [(json!({}), "42"), (json!({ "trigger_id": "77" }), "77")] {
            let server = MockServer::start_async().await;
            let put = reply(&server, PUT, "/gtt/triggers/42", data.clone()).await;
            let delete = reply(&server, DELETE, "/gtt/triggers/42", data.clone()).await;
            let (tx, mut rx) = mpsc::channel(2);
            super::spawn_modify_gtt(connect_client(&server), tx.clone(), 42, form());
            match next_event(&mut rx).await {
                DataEvent::GttModified(Ok(id)) => assert_eq!(id, expected, "{data}"),
                other => panic!("{data}: unexpected {other:?}"),
            }
            super::spawn_delete_gtt(connect_client(&server), tx, 42);
            match next_event(&mut rx).await {
                DataEvent::GttDeleted(Ok(id)) => assert_eq!(id, expected, "{data}"),
                other => panic!("{data}: unexpected {other:?}"),
            }
            put.assert_calls_async(1).await;
            delete.assert_calls_async(1).await;
        }
    }

    #[tokio::test]
    async fn the_margin_preview_requires_both_totals() {
        let fixture = fixture_data(include_str!("../../mock_responses/order_margins.json"));
        let mut zero = fixture.clone();
        zero[0]["total"] = json!(0);
        zero[0]["charges"]["total"] = json!(0);
        let cases = [
            (fixture, Ok((1498.0, 1.792_551_22))),
            (zero, Ok((0.0, 0.0))),
            (json!([{ "total": 1 }]), Err("bad /margins/orders payload")),
            (
                json!([{ "charges": { "total": 1 } }]),
                Err("bad /margins/orders payload"),
            ),
            (
                json!([{ "total": "1", "charges": { "total": 1 } }]),
                Err("bad /margins/orders payload"),
            ),
            (json!([]), Err("empty margin response")),
        ];
        for (seq, (data, expected)) in (1u64..).zip(cases) {
            let server = MockServer::start_async().await;
            let post = reply(&server, POST, "/margins/orders", data.clone()).await;
            let (tx, mut rx) = mpsc::channel(1);

            super::spawn_margin_preview(
                connect_client(&server),
                tx,
                seq,
                json!({"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","variety":"regular","product":"CNC","order_type":"LIMIT","quantity":1}),
            );

            let DataEvent::MarginPreview { seq: got, result } = next_event(&mut rx).await else {
                panic!("{data}: not a margin preview");
            };
            assert_eq!(got, seq);
            match (result, expected) {
                (Ok(preview), Ok((total, charges))) => {
                    assert_eq!((preview.total, preview.charges), (total, charges), "{data}");
                }
                (Err(err), Err(prefix)) => assert!(err.starts_with(prefix), "{data}: {err}"),
                (result, _) => panic!("{data}: unexpected {result:?}"),
            }
            post.assert_calls_async(1).await;
        }
    }

    const SWITCH_TO_AMO: &str = r#"{"status":"error","data":{"hints":["switch_to_amo"]},"error_type":"InputException","message":"convert"}"#;

    #[tokio::test]
    async fn regular_order_switches_to_amo_once_and_preserves_form_and_tags() {
        let server = MockServer::start();
        let regular = server.mock(|when, then| {
            when.method(POST)
                .path("/orders/regular")
                .header("authorization", "token key:token")
                .body_includes("exchange=NSE")
                .body_includes("quantity=1")
                .body_includes("tag=desk-order");
            then.status(400)
                .header("content-type", "application/json")
                .body(SWITCH_TO_AMO);
        });
        let amo = server.mock(|when, then| {
            when.method(POST)
                .path("/orders/amo")
                .header("authorization", "token key:token")
                .body_includes("exchange=NSE")
                .body_includes("quantity=1")
                .body_includes("tag=desk-order")
                .body_includes("tag=switch_to_amo");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"status":"success","data":{"order_id":"amo-1"}}"#);
        });

        let order = place_with_amo_fallback(
            &connect_client(&server),
            "regular",
            vec![
                ("exchange".to_string(), "NSE".to_string()),
                ("quantity".to_string(), "1".to_string()),
                ("tag".to_string(), "desk-order".to_string()),
            ],
        )
        .await
        .expect("converted order");

        assert_eq!(order["order_id"], "amo-1");
        regular.assert_calls(1);
        amo.assert_calls(1);
    }

    #[tokio::test]
    async fn amo_retry_failure_is_returned() {
        let server = MockServer::start();
        let regular = server.mock(|when, then| {
            when.method(POST).path("/orders/regular");
            then.status(400)
                .header("content-type", "application/json")
                .body(SWITCH_TO_AMO);
        });
        let amo = server.mock(|when, then| {
            when.method(POST).path("/orders/amo");
            then.status(400)
                .header("content-type", "application/json")
                .body(
                    r#"{"status":"error","error_type":"InputException","message":"amo window closed"}"#,
                );
        });

        let err = place_with_amo_fallback(&connect_client(&server), "regular", Vec::new())
            .await
            .expect_err("a failed AMO retry must propagate");

        assert!(err.to_string().contains("amo window closed"), "{err}");
        regular.assert_calls(1);
        amo.assert_calls(1);
    }

    #[tokio::test]
    async fn regular_order_propagates_rejection_without_switch_hint() {
        let server = MockServer::start();
        let regular = server.mock(|when, then| {
            when.method(POST).path("/orders/regular");
            then.status(400)
                .header("content-type", "application/json")
                .body(r#"{"status":"error","data":{"hints":["other_hint"]},"error_type":"InputException","message":"reject me"}"#);
        });
        let amo = server.mock(|when, then| {
            when.method(POST).path("/orders/amo");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"status":"success","data":{"order_id":"unexpected"}}"#);
        });

        let err = place_with_amo_fallback(&connect_client(&server), "regular", Vec::new())
            .await
            .expect_err("unrelated hint must reject");

        assert!(err.to_string().contains("reject me"));
        regular.assert_calls(1);
        amo.assert_calls(0);
    }

    #[tokio::test]
    async fn regular_order_does_not_retry_after_transport_failure() {
        let server = MockServer::start();
        let regular = server.mock(|when, then| {
            when.method(POST).path("/orders/regular");
            then.status(307)
                .header("location", "http://127.0.0.1:1/orders/regular");
        });
        let amo = server.mock(|when, then| {
            when.method(POST).path("/orders/amo");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"status":"success","data":{"order_id":"unexpected"}}"#);
        });

        let err = place_with_amo_fallback(&connect_client(&server), "regular", Vec::new())
            .await
            .expect_err("transport failure must propagate");

        assert!(err.to_string().contains("request failed"));
        regular.assert_calls(1);
        amo.assert_calls(0);
    }

    #[tokio::test]
    async fn amo_order_does_not_retry_even_when_server_returns_switch_hint() {
        let server = MockServer::start();
        let amo = server.mock(|when, then| {
            when.method(POST).path("/orders/amo");
            then.status(400)
                .header("content-type", "application/json")
                .body(r#"{"status":"error","data":{"hints":["switch_to_amo"]},"error_type":"InputException","message":"reject amo"}"#);
        });

        let err = place_with_amo_fallback(&connect_client(&server), "amo", Vec::new())
            .await
            .expect_err("AMO rejection must propagate");

        assert!(err.to_string().contains("reject amo"));
        amo.assert_calls(1);
    }

    #[tokio::test]
    async fn amo_switch_requires_http_400_and_the_exact_hint() {
        const EXTRA_HINT: &str = r#"{"status":"error","data":{"hints":["switch_to_amo_extra"]},"error_type":"InputException","message":"convert"}"#;
        for (status, body, hint) in [
            (409, SWITCH_TO_AMO, "switch_to_amo"),
            (400, EXTRA_HINT, "switch_to_amo_extra"),
        ] {
            let server = MockServer::start();
            let regular = server.mock(|when, then| {
                when.method(POST).path("/orders/regular");
                then.status(status)
                    .header("content-type", "application/json")
                    .body(body);
            });
            let amo = server.mock(|when, then| {
                when.method(POST).path("/orders/amo");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(r#"{"status":"success","data":{"order_id":"unexpected"}}"#);
            });

            let err = place_with_amo_fallback(&connect_client(&server), "regular", Vec::new())
                .await
                .expect_err("the original rejection must propagate");

            match &err {
                Error::Api(ApiError {
                    kind,
                    message,
                    status: http_status,
                    data,
                    ..
                }) => {
                    assert_eq!(kind.as_str(), "InputException");
                    assert_eq!(message, "convert");
                    assert_eq!(*http_status, status);
                    assert_eq!(data, &Some(serde_json::json!({ "hints": [hint] })));
                }
                other => panic!("expected the original ApiError for HTTP {status}, got {other:?}"),
            }
            regular.assert_calls(1);
            amo.assert_calls(0);
        }
    }
}

#[cfg(test)]
mod fixtures;
