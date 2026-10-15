use super::panes::chain::today_ist;
use super::{App, marketwatch};
use crate::tui::alerts;
use crate::tui::bids;
use crate::tui::candles::{CandleRequests, CandleStore};
use crate::tui::data::{self, ChartInterval};
use crate::tui::event::AppEvent;
use crate::tui::event::WsState;
use crate::tui::notes::Pad;
use crate::tui::pane_state::{self, ChartBinding, ChartId, ChartPane};
use crate::tui::settings::{self, Settings};
use crate::tui::ui;
use crate::tui::workspace::{PaneId, Preset, Workspaces};
use kiteconnect::KiteConnect;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;

impl App {
    #[expect(
        clippy::too_many_lines,
        reason = "constructor initializes independent session fields together"
    )]
    pub fn new(
        client: KiteConnect,
        events_tx: mpsc::Sender<AppEvent>,
        watch: marketwatch::State,
        notes: Pad,
        ephemeral: bool,
        settings: &Settings,
    ) -> Self {
        let watch_pane = pane_state::WatchPane::opened(watch.remembered().cloned());

        let account = watch.account().to_string();
        let remembered = settings.workspace_preferences(&account);
        let workspace = remembered.active.unwrap_or(Preset::Overview);
        let mut workspaces = Workspaces::default();
        for preset in Preset::ALL {
            let focused = remembered
                .focused
                .iter()
                .find(|(saved, _)| *saved == preset)
                .map(|(_, pane)| *pane);
            let maximized = remembered
                .maximized
                .iter()
                .find(|(saved, _)| *saved == preset)
                .map(|(_, pane)| *pane);
            workspaces.restore(preset, focused, maximized);
        }
        let (interval, volume, indicator) = (
            settings.chart_interval().unwrap_or(ChartInterval::M5),
            settings.chart_volume.unwrap_or(true),
            settings.chart_indicator().unwrap_or_default(),
        );
        let chart =
            |id, binding| ChartPane::new(ChartId::new(id), binding, interval, volume, indicator);
        Self {
            workspace,
            workspaces,
            account,
            chart_panel: None,
            should_quit: false,
            should_logout: false,
            exit_notice: None,
            ws_state: WsState::Connecting,
            ws_fatal: None,
            started_at: Instant::now(),
            ticks: HashMap::new(),
            flash: HashMap::new(),
            watch,
            notes,
            #[cfg(test)]
            notes_path: None,
            ephemeral,
            alerts: None,
            alert_history: None,
            alert_index: alerts::AlertIndex::default(),
            movers: None,
            movers_universe: settings.movers_universe().unwrap_or_default(),
            chart_maximized: false,
            movers_inflight: false,
            mover_presets: Arc::new(data::MoverPresets::default()),
            movers_failed: None,
            positions: None,
            holdings: None,
            auctions: None,
            bids: bids::State::default(),
            mf_holdings: None,
            orders: None,
            gtts: None,
            margin_available: None,
            funds: None,
            profile: None,
            instruments: None,
            instruments_error: None,
            instrument_master_day: today_ist(),
            chart_pane: chart(0, ChartBinding::SelectionOf(PaneId::OrdersBook)),
            default_product: settings.default_product_index(),
            candles: CandleStore::default(),
            circuits: HashMap::new(),
            circuits_inflight: HashSet::new(),
            candle_requests: CandleRequests::default(),
            splits: settings.split_ratios(),
            drag_split: None,
            #[cfg(test)]
            settings_path: None,
            help_max_scroll: Cell::new(0),
            modals: Vec::new(),
            pending_action: false,
            preview_seq: 0,
            alert_request_seq: 0,
            order_history_seq: 0,
            last_postback_refresh: None,
            hit: RefCell::new(ui::HitMap::default()),
            books: pane_state::Books::default(),
            watch_pane,
            bids_pane: pane_state::BidsPane::default(),
            trading_chart: chart(1, ChartBinding::SelectionOf(PaneId::TradingWatch)),
            derivatives_chain: pane_state::ChainPane::default(),
            derivatives_chart: chart(2, ChartBinding::UnderlyingOf(PaneId::DerivativesChain)),
            funds_scroll: Cell::new(0),
            positions_gen: 0,
            orders_gen: 0,
            holdings_gen: 0,
            mf_gen: 0,
            gtts_gen: 0,
            alerts_gen: 0,
            alert_fetch_requests: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            alerts_applied: data::AlertListSequence::default(),
            alert_status_pending: HashSet::new(),
            toasts: VecDeque::new(),
            error_toasts: HashMap::new(),
            client,
            events_tx,
            ticker: None,
            uuid: String::new(),
            last_subs: HashMap::new(),
            ticket_prefs: HashMap::new(),
        }
    }

    #[cfg_attr(
        not(test),
        expect(
            clippy::unused_self,
            reason = "test builds use the per-app settings path"
        )
    )]
    pub(in crate::tui::app) fn settings_file(&self) -> Option<std::path::PathBuf> {
        #[cfg(test)]
        {
            self.settings_path.clone()
        }
        #[cfg(not(test))]
        {
            settings::config_file_path()
        }
    }
}

#[cfg(test)]
mod workspace_persistence_tests {

    use super::super::test_support::char_key;
    use super::super::{KeyCode, KeyEvent, KeyModifiers};
    use super::{App, AppEvent, Pad, settings};
    use crate::tui::KiteConnect;
    use crate::tui::marketwatch::State as WatchState;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::{PaneId, Preset};
    use std::path::PathBuf;

    use tokio::sync::mpsc;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kite-workspaces-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        dir.join("tui.json")
    }

    fn app_for(account: &str, settings: &Settings) -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = mpsc::channel::<AppEvent>(8);
        App::new(
            client,
            events_tx,
            WatchState::remote(None, account.to_string(), None),
            Pad::default(),
            false,
            settings,
        )
    }

    #[test]
    fn startup_opens_the_remembered_workspace_focus_and_maximize() {
        let settings = settings::from_json(
            r#"{"workspaces":{"version":2,"accounts":{"AB1234":{
                "active":"trading",
                "focused":{"trading":"trading.orders","portfolio":"portfolio.funds"},
                "maximized":{"trading":"trading.orders"}
            }}}}"#,
        );
        let app = app_for("AB1234", &settings);
        assert_eq!(app.workspace(), Preset::Trading);
        assert_eq!(app.focused_pane(), PaneId::TradingOrders);
        assert_eq!(
            app.workspaces.maximized(Preset::Trading),
            Some(PaneId::TradingOrders)
        );
        assert_eq!(
            app.workspaces.focused(Preset::Portfolio),
            PaneId::PortfolioFunds,
            "a workspace not on screen still restores its own focus"
        );
        assert_eq!(app.workspaces.maximized(Preset::Portfolio), None);
    }

    #[test]
    fn a_remembered_single_view_opens_the_workspace_that_holds_it() {
        let settings = settings::from_json(
            r#"{"workspaces":{"version":1,"accounts":{"AB1234":{"active":"alerts"}}}}"#,
        );
        let app = app_for("AB1234", &settings);
        assert_eq!(app.workspace(), Preset::Orders);
        assert_eq!(app.focused_pane(), PaneId::OrdersAlerts);
        assert_eq!(app.workspaces.maximized(Preset::Orders), None);
    }

    #[test]
    fn a_stale_remembered_workspace_falls_back_to_overview_without_writing() {
        let path = scratch("stale");
        let original = r#"{"workspaces":{"version":2,"accounts":{"AB1234":{"active":"atlantis","focused":{"overview":"trading.watch"}}}}}"#;
        std::fs::write(&path, original).expect("write");

        let settings = settings::from_json(original);
        let app = app_for("AB1234", &settings);
        assert_eq!(app.workspace(), Preset::Overview);
        assert_eq!(
            app.focused_pane(),
            PaneId::OverviewPositions,
            "a pane the Overview does not draw is not its focus"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("read"),
            original,
            "startup corrects nothing on disk"
        );
    }

    #[test]
    fn one_account_neither_reads_nor_overwrites_anothers_workspace() {
        let path = scratch("accounts");
        std::fs::write(
            &path,
            r#"{"workspaces":{"version":2,"accounts":{"CD5678":{"active":"derivatives"}}}}"#,
        )
        .expect("write");
        let settings = settings::from_json(&std::fs::read_to_string(&path).expect("read"));

        let mut app = app_for("AB1234", &settings);
        assert_eq!(
            app.workspace(),
            Preset::Overview,
            "AB1234 has no entry of its own"
        );
        app.settings_path = Some(path.clone());
        app.on_key(char_key('3'));

        let saved = settings::from_json(&std::fs::read_to_string(&path).expect("read"));
        assert_eq!(
            saved.workspace_preferences("AB1234").active,
            Some(Preset::Portfolio)
        );
        assert_eq!(
            saved.workspace_preferences("CD5678").active,
            Some(Preset::Derivatives),
            "the other account's workspace is untouched"
        );
    }

    #[test]
    fn an_ephemeral_session_saves_no_workspace() {
        let path = scratch("ephemeral");
        std::fs::write(&path, "{}").expect("write");
        let settings = Settings::default();
        let mut app = app_for("", &settings);
        app.settings_path = Some(path.clone());

        app.on_key(char_key('3'));
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), "{}");
    }

    #[tokio::test]
    async fn a_restart_lands_on_the_workspace_focus_zoom_and_seam_the_session_left() {
        use crate::tui::marketwatch::WatchItem;
        use crate::tui::split::Split;
        use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};

        const W: u16 = 160;
        const H: u16 = 40;
        let render = |app: &App| {
            let mut terminal =
                ratatui::Terminal::new(ratatui::backend::TestBackend::new(W, H)).expect("terminal");
            terminal
                .draw(|frame| crate::tui::ui::draw(frame, app))
                .expect("frame draws");
        };
        let mouse = |kind: MouseEventKind, column: u16, row: u16| MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        };

        let path = scratch("round-trip");
        let mut app = app_for("AB1234", &Settings::default());
        app.settings_path = Some(path.clone());
        app.orders = Some(vec![super::super::test_support::sample_order("OPEN")]);
        app.set_watch_for_test(crate::tui::marketwatch::State::test_loaded(vec![
            WatchItem {
                exchange: "NSE".into(),
                tradingsymbol: "INFY".into(),
                instrument_token: 408_065,
                ..WatchItem::default()
            },
        ]));

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.workspace(), Preset::Trading);

        render(&app);
        let (strip, span) = {
            let hit = app.hit.borrow();
            hit.regions
                .iter()
                .find_map(|region| match region.action {
                    crate::tui::ui::HitAction::Divider {
                        split: Split::TradingRoot,
                        span,
                    } => Some((region.rect, span)),
                    _ => None,
                })
                .expect("the seam recorded its divider")
        };
        let want = span.x + 49;
        app.on_mouse(mouse(
            MouseEventKind::Down(MouseButton::Left),
            strip.x,
            strip.y,
        ));
        app.on_mouse(mouse(MouseEventKind::Drag(MouseButton::Left), want, 10));
        app.on_mouse(mouse(MouseEventKind::Up(MouseButton::Left), want, 10));

        for _ in 0..3 {
            app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        }
        assert_eq!(app.focused_pane(), PaneId::TradingOrders);
        app.on_key(char_key('z'));
        assert_eq!(
            app.workspaces.maximized(Preset::Trading),
            Some(PaneId::TradingOrders)
        );

        let settings = settings::from_json(&std::fs::read_to_string(&path).expect("read"));
        let restarted = app_for("AB1234", &settings);
        assert_eq!(restarted.workspace(), Preset::Trading);
        assert_eq!(restarted.focused_pane(), PaneId::TradingOrders);
        assert_eq!(
            restarted.workspaces.maximized(Preset::Trading),
            Some(PaneId::TradingOrders)
        );
        assert_eq!(
            restarted.splits.get(Split::TradingRoot),
            app.splits.get(Split::TradingRoot),
            "the seam comes back where the drag left it"
        );
    }

    #[tokio::test]
    async fn navigation_writes_the_choice_and_selection_does_not() {
        let path = scratch("writes");
        let settings = Settings::default();
        let mut app = app_for("AB1234", &settings);
        app.settings_path = Some(path.clone());

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.workspace(), Preset::Trading);
        let saved = settings::from_json(&std::fs::read_to_string(&path).expect("read"));
        assert_eq!(
            saved.workspace_preferences("AB1234").active,
            Some(Preset::Trading)
        );

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focused_pane(), PaneId::TradingChart);
        let saved = settings::from_json(&std::fs::read_to_string(&path).expect("read"));
        assert_eq!(
            saved.workspace_preferences("AB1234").focused,
            vec![(Preset::Trading, PaneId::TradingChart)]
        );

        let before = std::fs::read_to_string(&path).expect("read");
        app.on_key(char_key('j'));
        app.on_key(char_key('k'));
        assert_eq!(std::fs::read_to_string(&path).expect("read"), before);
    }
}
