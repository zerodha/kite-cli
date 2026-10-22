#[cfg(test)]
mod perf;
#[cfg(test)]
mod regressions;
#[cfg(test)]
mod test_support;

mod alerts;
mod charts;
mod dispatch;
mod events;
mod keys;
mod marketwatch;
mod modals;
mod mouse;
mod movers;
mod navigation;
mod notifications;
mod palette;
mod panes;
mod reconcile;
mod state;
mod subscriptions;

use crate::cli::StreamMode;
use crate::tui::bids;
#[cfg(test)]
use crate::tui::candles::SeriesKey;
use crate::tui::candles::{CandleRequests, CandleStore};
#[cfg(test)]
use crate::tui::data::DataEvent;
use crate::tui::data::{
    self, Alert, AuctionHolding, ChartInterval, Funds, Gtt, Holding, MfHolding, Movers,
    MoversUniverse, Order, PositionsData, Profile,
};
use crate::tui::event::AppEvent;
use crate::tui::event::WsState;
use crate::tui::instruments::InstrumentStore;
use crate::tui::notes::Pad;
use crate::tui::pane_state::{self, ChartPane};
#[cfg(test)]
use crate::tui::pane_state::{ChartBinding, ChartId};
use crate::tui::settings;
use crate::tui::split::{Split, SplitRatios};
#[cfg(test)]
use crate::tui::ticket::PendingAction;
#[cfg(test)]
use crate::tui::ticket::TicketField;
use crate::tui::ui;
use crate::tui::workspace::{PaneId, Preset, Workspaces};
use charts::now_epoch;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
#[cfg(test)]
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use kiteconnect::KiteConnect;
#[cfg(test)]
use kiteconnect::ticker::TickerEvent;
use kiteconnect::ticker::{KiteTicker, Tick};
#[cfg(test)]
use palette::build::exec_row;
use panes::alerts::alert_key;
use panes::cmp_desc;
use panes::holdings::holding_key;
use panes::positions::position_key;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

pub use modals::alert::{AlertFormFocus, AlertFormStage, AlertFormState};
pub use modals::ato::{AtoItemFocus, AtoItemFormState};
pub use modals::events::EventLogState;
pub use modals::order_detail::OrderDetailState;
pub use modals::picker::WorkspacePickerState;
pub use modals::products::{
    ORDER_TYPES, PRODUCTS, TTL_MINUTES, VALIDITIES, allowed_gtt_products, allowed_products,
    cycle_product, product_label_index, validity_params,
};
pub use modals::search::SearchState;
pub use modals::theme::ThemePickerState;
pub use modals::{AlertHistoryState, Modal, SearchPurpose, TicketPrefs};
pub use notifications::{Toast, ToastLevel};
pub use palette::menu::MenuAction;
pub use palette::state::{ExecItem, PaletteAction, PaletteRow, PaletteState, RowIdentity};
pub use panes::alerts::AlertsSort;
pub use panes::gtt::GttSort;
pub use panes::holdings::HoldingsSort;
pub use panes::mf::MfSort;
pub use panes::orders::OrdersSort;
pub use panes::positions::PositionsSort;
pub use panes::{Book, SortTarget};

pub const CHAIN_UNDERLYINGS: [&str; 4] = ["NIFTY", "BANKNIFTY", "FINNIFTY", "MIDCPNIFTY"];
pub const FLASH_TTL: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ChartPanel {
    pane: PaneId,

    focused: PaneId,

    maximized: Option<PaneId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartTarget {
    pub exchange: String,
    pub tradingsymbol: String,
    pub instrument_token: u32,
    row_identity: String,
}

impl ChartTarget {
    pub fn label(&self) -> String {
        format!("{}:{}", self.exchange, self.tradingsymbol)
    }

    #[cfg(test)]
    pub fn for_test(exchange: &str, tradingsymbol: &str, instrument_token: u32) -> Self {
        Self {
            exchange: exchange.to_string(),
            tradingsymbol: tradingsymbol.to_string(),
            instrument_token,
            row_identity: format!("{exchange}:{tradingsymbol}:{instrument_token}"),
        }
    }

    #[cfg(test)]
    pub fn untokened_for_test(exchange: &str, tradingsymbol: &str) -> Self {
        Self {
            exchange: exchange.to_string(),
            tradingsymbol: tradingsymbol.to_string(),
            instrument_token: 0,
            row_identity: format!("{exchange}:{tradingsymbol}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    pub fn label(self) -> &'static str {
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SegmentClass {
    Equity,
    Fno,
    Commodity,
    Currency,
    Other(String),
}

pub fn segment_class(exchange: &str) -> SegmentClass {
    match exchange {
        "NSE" | "BSE" => SegmentClass::Equity,
        "NFO" | "BFO" => SegmentClass::Fno,

        "MCX" | "NCO" => SegmentClass::Commodity,
        "CDS" | "BCD" => SegmentClass::Currency,
        other => SegmentClass::Other(other.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartHover {
    pub token: u32,
    pub interval: ChartInterval,

    pub ts: String,

    pub window_ts: String,

    pub visible_n: usize,
}

#[derive(Debug, Clone, Copy)]
struct SplitDrag {
    split: Split,
    span: ratatui::layout::Rect,

    origin: u16,
    moved: bool,
}

pub const SESSION_EXPIRED: &str = "access token expired";

#[expect(
    clippy::struct_excessive_bools,
    reason = "flags govern independent session, chart, fetch and action state"
)]
pub struct App {
    workspace: Preset,

    pub workspaces: Workspaces,

    account: String,

    chart_panel: Option<ChartPanel>,
    pub should_quit: bool,

    pub should_logout: bool,

    exit_notice: Option<String>,
    pub ws_state: WsState,

    pub ws_fatal: Option<String>,

    pub started_at: Instant,
    pub ticks: HashMap<u32, Tick>,
    flash: HashMap<u32, (Instant, i8)>,

    pub watch: marketwatch::State,

    pub notes: Pad,

    #[cfg(test)]
    pub notes_path: Option<std::path::PathBuf>,
    pub ephemeral: bool,

    alerts: Option<Vec<Alert>>,

    alert_history: Option<AlertHistoryState>,

    alert_index: crate::tui::alerts::AlertIndex,

    pub movers: Option<Movers>,

    pub movers_universe: MoversUniverse,

    pub chart_maximized: bool,

    movers_inflight: bool,

    mover_presets: Arc<data::MoverPresets>,

    pub movers_failed: Option<String>,
    pub positions: Option<PositionsData>,
    pub holdings: Option<Vec<Holding>>,
    pub holdings_error: Option<String>,

    pub auctions: Option<Vec<AuctionHolding>>,
    pub bids: bids::State,
    pub mf_holdings: Option<Vec<MfHolding>>,
    pub orders: Option<Vec<Order>>,
    pub gtts: Option<Vec<Gtt>>,
    pub margin_available: Option<f64>,

    pub funds: Option<Funds>,

    pub profile: Option<Profile>,
    pub instruments: Option<Arc<InstrumentStore>>,
    pub instruments_error: Option<String>,

    instrument_master_day: String,

    pub chart_pane: ChartPane,

    default_product: Option<usize>,

    candles: CandleStore,

    pub circuits: HashMap<u32, Option<(f64, f64)>>,
    circuits_inflight: HashSet<u32>,

    candle_requests: CandleRequests,

    pub splits: SplitRatios,

    drag_split: Option<SplitDrag>,

    #[cfg(test)]
    pub settings_path: Option<std::path::PathBuf>,

    pub help_max_scroll: Cell<u16>,
    pub modals: Vec<Modal>,
    pub pending_action: bool,

    preview_seq: u64,

    alert_request_seq: u64,

    order_history_seq: u64,

    last_postback_refresh: Option<Instant>,

    pub hit: RefCell<ui::HitMap>,

    pub books: pane_state::Books,

    pub watch_pane: pane_state::WatchPane,

    pub bids_pane: pane_state::BidsPane,

    pub trading_chart: ChartPane,

    pub derivatives_chain: pane_state::ChainPane,

    pub derivatives_chart: ChartPane,

    pub funds_scroll: Cell<usize>,

    positions_gen: u64,
    orders_gen: u64,
    holdings_gen: u64,
    mf_gen: u64,
    gtts_gen: u64,
    alerts_gen: u64,
    pub(in crate::tui) alert_fetch_requests: std::sync::Arc<std::sync::atomic::AtomicU64>,
    alerts_applied: data::AlertListSequence,
    alert_status_pending: HashSet<String>,
    toasts: VecDeque<Toast>,

    error_toasts: HashMap<String, Instant>,
    client: KiteConnect,
    pub(super) uuid: String,
    events_tx: mpsc::Sender<AppEvent>,
    pub(super) ticker: Option<KiteTicker>,
    last_subs: HashMap<u32, StreamMode>,

    ticket_prefs: HashMap<SegmentClass, TicketPrefs>,
}

#[cfg(test)]
mod tests {
    use super::panes::alerts::alerts_order;
    use super::test_support::{H, W, char_key, render, server_alert, test_app};
    use super::{AlertsSort, KeyCode, KeyEvent, KeyModifiers, SegmentClass, segment_class, ui};

    #[test]
    fn a_palette_rebuild_drops_its_recorded_rows() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        render(&app, W, H);
        assert!(
            app.hit.borrow().regions.iter().any(|region| matches!(
                region.action,
                ui::HitAction::Row {
                    list: ui::HitList::Palette,
                    ..
                }
            )),
            "the menu recorded its rows"
        );
        app.on_key(char_key('q'));
        assert!(
            !app.hit.borrow().regions.iter().any(|region| matches!(
                region.action,
                ui::HitAction::Row {
                    list: ui::HitList::Palette,
                    ..
                }
            )),
            "an edit replaced the rows, so their rectangles are stale"
        );
    }

    #[test]
    fn segment_class_groups_exchanges_correctly() {
        assert_eq!(segment_class("NSE"), SegmentClass::Equity);
        assert_eq!(segment_class("BSE"), SegmentClass::Equity);
        assert_eq!(segment_class("NFO"), SegmentClass::Fno);
        assert_eq!(segment_class("BFO"), SegmentClass::Fno);

        assert_eq!(segment_class("NCO"), SegmentClass::Commodity);
        assert_eq!(segment_class("MCX"), SegmentClass::Commodity);
        assert_eq!(segment_class("CDS"), SegmentClass::Currency);
        assert_eq!(segment_class("BCD"), SegmentClass::Currency);
        assert_eq!(
            segment_class("WEIRD"),
            SegmentClass::Other("WEIRD".to_string())
        );
    }

    #[test]
    fn the_symbol_sort_ignores_case() {
        let alerts = vec![
            server_alert("zeel", "NSE", ">=", 300.0),
            server_alert("INFY", "NSE", ">=", 110.0),
            server_alert("TCS", "NSE", ">=", 3000.0),
        ];
        assert_eq!(alerts_order(&alerts, AlertsSort::Symbol), vec![1, 2, 0]);
    }
}
