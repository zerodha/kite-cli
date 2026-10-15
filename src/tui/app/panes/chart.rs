use super::super::{App, ChartPanel, ChartTarget, PaneId, ToastLevel};
use super::holdings::holding_identity;
use super::orders::order_identity;
use super::positions::position_identity;
use crate::tui::indicators::ChartIndicator;
use crate::tui::pane_state::{ChartBinding, ChartId, ChartPane};
use crate::tui::settings;

impl App {
    pub fn chart_of(&self, pane: PaneId) -> Option<&ChartPane> {
        match pane {
            PaneId::TradingChart => Some(&self.trading_chart),
            PaneId::DerivativesChart => Some(&self.derivatives_chart),

            _ if self.chart_panel_pane() == Some(pane) => Some(&self.chart_pane),
            _ => None,
        }
    }

    fn chart_instance_mut(&mut self, pane: PaneId) -> Option<&mut ChartPane> {
        match pane {
            PaneId::TradingChart => Some(&mut self.trading_chart),
            PaneId::DerivativesChart => Some(&mut self.derivatives_chart),
            _ if self.chart_panel_pane() == Some(pane) => Some(&mut self.chart_pane),
            _ => None,
        }
    }

    pub(in crate::tui::app) fn acting_chart(&self, pane: PaneId) -> Option<PaneId> {
        match pane {
            PaneId::TradingChart | PaneId::DerivativesChart => Some(pane),
            _ if self.chart_panel_pane() == Some(pane) => Some(pane),
            _ => None,
        }
    }

    pub fn chart_panel_pane(&self) -> Option<PaneId> {
        self.chart_panel.map(|panel| panel.pane)
    }

    pub(in crate::tui::app) fn open_chart(&self) -> Option<&ChartPane> {
        self.chart_panel.map(|_| &self.chart_pane)
    }

    pub(in crate::tui::app) fn chart_by_id(&self, id: ChartId) -> Option<&ChartPane> {
        [
            &self.chart_pane,
            &self.trading_chart,
            &self.derivatives_chart,
        ]
        .into_iter()
        .find(|chart| chart.id() == id)
    }

    pub(in crate::tui::app) fn chart_by_id_mut(&mut self, id: ChartId) -> Option<&mut ChartPane> {
        self.charts_mut().into_iter().find(|chart| chart.id() == id)
    }

    pub fn selected_chart_target(&self) -> Option<ChartTarget> {
        self.open_chart().and_then(|chart| chart.target().cloned())
    }

    #[cfg(test)]
    pub fn open_chart_panel_for_test(&mut self, pane: PaneId) {
        let binding = ChartBinding::of_book(pane).expect("a chart-capable book pane");
        self.chart_panel = Some(ChartPanel {
            pane,
            focused: self.focused_pane(),
            maximized: self.workspaces.maximized(self.workspace),
        });
        self.chart_pane.bind(binding);
        self.publish_chart_target();
    }

    pub(in crate::tui::app) fn move_chart_cursor(&mut self, pane: PaneId, older: bool) {
        let Some(chart) = self.acting_chart(pane) else {
            return;
        };
        let len = self.chart_series_len(chart);
        if let Some(chart) = self.chart_instance_mut(chart) {
            chart.move_cursor(older, len);
        }
    }

    fn chart_series_len(&self, pane: PaneId) -> usize {
        let Some(chart) = self.chart_of(pane) else {
            return 0;
        };
        chart
            .token()
            .and_then(|token| self.candles.candles(chart.key_for(token)))
            .map_or(0, <[_]>::len)
    }

    pub(in crate::tui::app) fn seed_chart_cursor_from_hover(&mut self, pane: PaneId) {
        let chart = match self.acting_chart(pane) {
            Some(PaneId::TradingChart) => &mut self.trading_chart,
            Some(PaneId::DerivativesChart) => &mut self.derivatives_chart,
            Some(_) => &mut self.chart_pane,
            None => return,
        };
        if let Some(hover) = chart.hover().cloned() {
            chart.seed_cursor_from_hover(&hover, &self.candles);
        }
    }

    pub(in crate::tui::app) fn clear_chart_hovers(&mut self) {
        for chart in self.charts_mut() {
            chart.set_hover(None);
        }
    }

    fn charts_mut(&mut self) -> [&mut ChartPane; 3] {
        [
            &mut self.chart_pane,
            &mut self.trading_chart,
            &mut self.derivatives_chart,
        ]
    }

    pub(in crate::tui::app) fn toggle_chart_panel(&mut self, pane: PaneId) {
        if self.chart_panel_pane() == Some(pane) {
            self.close_chart_panel();
            return;
        }
        let Some(binding) = ChartBinding::of_book(pane) else {
            return;
        };
        let Some(target) = self.resolve_chart_binding(binding) else {
            self.toast(ToastLevel::Error, "chart unavailable for this row");
            return;
        };
        self.close_chart_panel();

        let focused = self.focused_pane();
        let maximized = self.workspaces.maximized(self.workspace);
        if maximized != Some(pane) {
            self.workspaces.toggle_maximize(self.workspace, pane);
        }
        self.chart_panel = Some(ChartPanel {
            pane,
            focused,
            maximized,
        });
        self.chart_pane.bind(binding);
        self.chart_pane.resolve(Some(target));
        self.hit.borrow_mut().invalidate();
        self.sync_subscriptions();
        self.ensure_candles(true);
    }

    pub(in crate::tui::app) fn close_chart_panel(&mut self) {
        let Some(panel) = self.chart_panel.take() else {
            return;
        };

        self.chart_maximized = false;
        self.workspaces
            .restore(panel.pane.preset(), Some(panel.focused), panel.maximized);
        self.chart_pane.resolve(None);
        self.hit.borrow_mut().invalidate();
        self.sync_subscriptions();
    }

    pub(in crate::tui::app) fn cycle_interval(&mut self, pane: PaneId) {
        let Some(chart) = self
            .acting_chart(pane)
            .and_then(|chart| self.chart_instance_mut(chart))
        else {
            return;
        };
        chart.cycle_interval();
        self.ensure_candles(true);
    }

    pub(in crate::tui::app) fn toggle_chart_volume(&mut self, pane: PaneId) {
        if let Some(chart) = self
            .acting_chart(pane)
            .and_then(|chart| self.chart_instance_mut(chart))
        {
            chart.toggle_volume();
        }
    }

    fn next_chart_indicator(&mut self, pane: PaneId) -> Option<ChartIndicator> {
        let chart = self.acting_chart(pane)?;
        let chart = self.chart_instance_mut(chart)?;
        chart.next_indicator();
        Some(chart.indicator())
    }

    pub(in crate::tui::app) fn cycle_chart_indicator(&mut self, pane: PaneId) {
        let Some(indicator) = self.next_chart_indicator(pane) else {
            return;
        };
        if let Some(path) = self.settings_file()
            && let Err(err) = settings::save_chart_indicator(&path, indicator)
        {
            self.toast(
                ToastLevel::Error,
                format!("indicator applied but not saved: {err}"),
            );
        }
    }

    pub(in crate::tui::app) fn draws_a_chart(&self) -> bool {
        self.open_chart().is_some()
            || self
                .workspace
                .panes()
                .iter()
                .any(|pane| matches!(pane, PaneId::TradingChart | PaneId::DerivativesChart))
    }

    pub(in crate::tui::app) fn clear_chart_cursors(&mut self) {
        for chart in self.charts_mut() {
            chart.set_cursor(None);
        }
    }

    pub(in crate::tui::app) fn reconcile_chart_target(&mut self) {
        let Some(pane) = self.chart_panel_pane() else {
            return;
        };
        let Some(identity) = self
            .chart_pane
            .target()
            .map(|target| target.row_identity.clone())
        else {
            return;
        };
        let still_there = match pane {
            PaneId::PortfolioPositions => self
                .positions_net()
                .iter()
                .any(|position| position_identity(position) == identity),
            PaneId::TradingOrders | PaneId::OrdersBook => self
                .orders
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|order| order_identity(order) == identity),
            PaneId::PortfolioHoldings => self
                .holdings
                .as_deref()
                .unwrap_or_default()
                .iter()
                .any(|holding| holding_identity(holding) == identity),
            _ => false,
        };
        if !still_there {
            self.publish_chart_target();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::navigation::tests::stocked;
    use super::super::super::test_support::{char_key, test_app};
    use super::super::super::{App, PaneId, Preset, settings};
    use crate::tui::candles::SeriesKey;

    use crate::tui::indicators::ChartIndicator;
    use crate::tui::marketwatch::{State as WatchState, WatchItem};
    use crate::tui::notes::Pad;
    use kiteconnect::KiteConnect;

    use tokio::sync::mpsc;

    #[test]
    fn chart_scan_starts_one_candle_older_and_returns_live() {
        let mut app = test_app();
        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: 408_065,
            segment: String::new(),
            ..WatchItem::default()
        });
        app.set_workspace(Preset::Trading);
        app.candles.insert(
            SeriesKey::new(408_065, app.trading_chart.interval()),
            (0..3)
                .map(|i| {
                    let mut row = crate::tui::data::test_models::candle();
                    row.date = format!(
                        "{}+05:30",
                        (format!("2026-07-22 10:0{i}:00")).replace(' ', "T")
                    )
                    .parse()
                    .expect("date");
                    row.open = 100.0;
                    row.high = 101.0;
                    row.low = 99.0;
                    row.close = 100.0;
                    row.volume = 10;
                    row
                })
                .collect(),
        );

        app.move_chart_cursor(PaneId::TradingChart, true);
        assert_eq!(app.trading_chart.cursor(), Some(1));
        app.move_chart_cursor(PaneId::TradingChart, false);
        assert_eq!(app.trading_chart.cursor(), Some(0));
        app.move_chart_cursor(PaneId::TradingChart, false);
        assert_eq!(app.trading_chart.cursor(), None);
    }

    #[tokio::test]
    async fn closing_a_chart_restores_the_layout_it_found_after_z_or_tab() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);

        let mut app = stocked();
        app.set_workspace(Preset::Orders);
        app.focus_pane_for_test(PaneId::OrdersBook);
        app.on_key(char_key('z'));
        app.on_key(char_key('c'));
        assert_eq!(app.chart_panel_pane(), Some(PaneId::OrdersBook));
        app.on_key(char_key('z'));
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.chart_panel_pane(), None);
        assert_eq!(app.focused_pane(), PaneId::OrdersBook);
        assert_eq!(
            app.workspaces.maximized(Preset::Orders),
            Some(PaneId::OrdersBook)
        );

        let dir = std::env::temp_dir().join(format!("kite-chart-layout-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("tui.json");
        let saved = |path: &std::path::Path| {
            let prefs = settings::from_json(&std::fs::read_to_string(path).expect("written"))
                .workspace_preferences("AB1234");
            let find = |slots: &[(Preset, PaneId)]| {
                slots
                    .iter()
                    .find(|(preset, _)| *preset == Preset::Orders)
                    .map(|(_, pane)| *pane)
            };
            (find(&prefs.focused), find(&prefs.maximized))
        };
        let mut app = stocked();
        app.account = "AB1234".into();
        app.settings_path = Some(path.clone());
        app.set_workspace(Preset::Orders);
        app.on_key(key(KeyCode::Tab));
        app.on_key(key(KeyCode::BackTab));
        assert_eq!(saved(&path), (Some(PaneId::OrdersBook), None));

        app.on_key(char_key('c'));
        app.on_key(char_key('z'));
        app.on_key(key(KeyCode::Tab));
        assert_eq!(app.focused_pane(), PaneId::OrdersGtt);
        assert_eq!(
            saved(&path),
            (Some(PaneId::OrdersBook), None),
            "the chart's own z and Tab are not written"
        );
        app.on_key(key(KeyCode::Esc));
        assert_eq!(app.chart_panel_pane(), None);
        assert_eq!(app.focused_pane(), PaneId::OrdersBook);
        assert_eq!(app.workspaces.maximized(Preset::Orders), None);
        assert_eq!(saved(&path), (Some(PaneId::OrdersBook), None));
    }

    #[test]
    fn the_indicator_key_is_gated_on_an_open_chart() {
        let mut app = test_app();

        for pane in [
            PaneId::OverviewPositions,
            PaneId::DerivativesChain,
            PaneId::InvestBids,
            PaneId::InvestMf,
        ] {
            app.set_pane(pane);
            assert_eq!(app.acting_chart(pane), None);
            app.on_key(char_key('I'));
            assert_eq!(
                app.chart_pane.indicator(),
                ChartIndicator::None,
                "I must be inert on {pane:?}"
            );
        }

        app.set_pane(PaneId::PortfolioPositions);
        assert_eq!(app.chart_panel_pane(), None);
        app.on_key(char_key('I'));
        assert_eq!(app.chart_pane.indicator(), ChartIndicator::None);
    }

    #[test]
    fn cycling_the_chart_indicator_keeps_exactly_one_active() {
        let mut app = test_app();
        app.set_workspace(Preset::Trading);
        assert_eq!(app.trading_chart.indicator(), ChartIndicator::None);
        let mut seen = Vec::new();
        for _ in 0..6 {
            app.next_chart_indicator(PaneId::TradingChart);
            seen.push(app.trading_chart.indicator());
        }
        assert_eq!(
            seen,
            vec![
                ChartIndicator::Sma,
                ChartIndicator::Ema,
                ChartIndicator::Bollinger,
                ChartIndicator::Rsi,
                ChartIndicator::Macd,
                ChartIndicator::None,
            ]
        );

        app.trading_chart.set_indicator(ChartIndicator::Rsi);
        assert!(app.trading_chart.indicator().is_oscillator());
        app.next_chart_indicator(PaneId::TradingChart);
        assert!(app.trading_chart.indicator().is_oscillator(), "RSI → MACD");
        app.next_chart_indicator(PaneId::TradingChart);
        assert!(!app.trading_chart.indicator().is_oscillator(), "MACD → off");
    }

    #[test]
    fn the_chart_indicator_is_seeded_from_settings() {
        let settings = settings::from_json(r#"{"chart_indicator":"ema"}"#);
        assert_eq!(settings.chart_indicator(), Some(ChartIndicator::Ema));
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = mpsc::channel(8);
        let app = App::new(
            client,
            events_tx,
            WatchState::test_loaded(Vec::new()),
            Pad::default(),
            true,
            &settings,
        );
        assert_eq!(app.chart_pane.indicator(), ChartIndicator::Ema);
        assert_eq!(test_app().chart_pane.indicator(), ChartIndicator::None);
    }
}

#[cfg(test)]
mod chart_instance_tests {
    use super::super::super::test_support::{
        holding, ltp_tick, master_inst, sample_order, test_app,
    };
    use super::super::super::{
        App, ChartBinding, ChartId, ChartPane, ChartTarget, DataEvent, PaneId, SeriesKey,
        StreamMode, TickerEvent,
    };
    use crate::tui::candles::CandleStore;
    use crate::tui::data::{Candle, ChartInterval};
    use crate::tui::indicators::ChartIndicator;
    use crate::tui::instruments::InstrumentStore;
    use crate::tui::marketwatch::WatchItem;
    use crate::tui::subscriptions::{self, Demands};
    use std::sync::Arc;

    const INFY: u32 = 408_065;
    const RELIANCE: u32 = 738_561;

    const TCS: u32 = 2_953_217;

    fn bar(ts: &str, close: f64) -> Candle {
        crate::tui::data::test_models::candle_at(ts, close, close, close, close, 0)
    }

    fn bars(n: usize) -> Vec<Candle> {
        (0..n)
            .map(|index| {
                bar(
                    &format!("2026-07-07 09:{:02}:00", 15 + index),
                    100.0 + f64::from(u32::try_from(index).expect("fixture candle index fits u32")),
                )
            })
            .collect()
    }

    fn chart(id: u32, token: u32, symbol: &str, interval: ChartInterval) -> ChartPane {
        let mut chart = ChartPane::new(
            ChartId::new(id),
            ChartBinding::SelectionOf(PaneId::TradingWatch),
            interval,
            true,
            ChartIndicator::Sma,
        );
        chart.resolve(Some(ChartTarget::for_test("NSE", symbol, token)));
        chart
    }

    fn watch_row(token: u32, symbol: &str) -> WatchItem {
        WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: symbol.into(),
            instrument_token: token,
            segment: String::new(),
            ..WatchItem::default()
        }
    }

    #[test]
    fn two_charts_share_the_candles_for_a_key_and_own_their_controls() {
        let mut store = CandleStore::default();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let day = SeriesKey::new(INFY, ChartInterval::Day);
        store.insert(five, bars(20));
        store.insert(day, bars(5));

        let mut fast = chart(1, INFY, "INFY", ChartInterval::M5);
        let mut slow = chart(2, INFY, "INFY", ChartInterval::Day);

        assert_eq!(fast.key(), Some(five));
        assert_eq!(slow.key(), Some(day));
        assert_eq!(store.candles(five).map(<[_]>::len), Some(20));
        assert_eq!(store.candles(day).map(<[_]>::len), Some(5));

        let fast_series = fast.indicator_series(INFY, &store).clone();
        let slow_series = slow.indicator_series(INFY, &store).clone();
        assert_ne!(fast_series, slow_series, "twenty bars is not five bars");
        assert_eq!(*fast.indicator_series(INFY, &store), fast_series);
        assert_eq!(*slow.indicator_series(INFY, &store), slow_series);

        for _ in 0..30 {
            fast.move_cursor(true, store.candles(five).unwrap().len());
            slow.move_cursor(true, store.candles(day).unwrap().len());
        }
        assert_eq!(fast.cursor(), Some(19), "clamped to twenty bars");
        assert_eq!(slow.cursor(), Some(4), "clamped to five");

        fast.toggle_volume();
        assert!(!fast.volume());
        assert!(
            slow.volume(),
            "one chart's v did not hide the other's volume"
        );
        fast.next_indicator();
        assert_ne!(fast.indicator(), slow.indicator());
    }

    #[test]
    fn a_hover_on_one_chart_leaves_its_siblings_readout_and_scan_alone() {
        let mut store = CandleStore::default();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        store.insert(five, bars(20));

        let mut pointed = chart(1, INFY, "INFY", ChartInterval::M5);
        let sibling = chart(2, INFY, "INFY", ChartInterval::M5);

        let hover = super::super::super::ChartHover {
            token: INFY,
            interval: ChartInterval::M5,
            ts: "2026-07-07 09:20:00".to_string(),
            window_ts: "2026-07-07 09:15:00".to_string(),
            visible_n: 10,
        };
        pointed.inspect(hover.clone(), None);

        assert!(pointed.hover_for(INFY).is_some());
        assert!(
            sibling.hover_for(INFY).is_none(),
            "the sibling draws the same key and is not being pointed at"
        );

        pointed.seed_cursor_from_hover(&hover, &store);
        assert_eq!(
            pointed.cursor(),
            Some(14),
            "09:20 is the sixth of twenty bars, so fourteen back from the newest"
        );
        assert_eq!(sibling.cursor(), None, "the sibling's scan did not move");
    }

    #[test]
    fn a_candle_reply_lands_under_its_own_key() {
        let mut app = test_app();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let day = SeriesKey::new(INFY, ChartInterval::Day);

        app.on_data(DataEvent::Candles {
            token: INFY,
            interval: ChartInterval::Day,
            result: Ok(bars(5)),
        });
        app.on_data(DataEvent::Candles {
            token: INFY,
            interval: ChartInterval::M5,
            result: Ok(bars(20)),
        });

        assert_eq!(app.candle_store().candles(day).map(<[_]>::len), Some(5));
        assert_eq!(app.candle_store().candles(five).map(<[_]>::len), Some(20));
    }

    #[test]
    fn two_wants_both_stand_and_each_key_reports_its_own_loading() {
        let mut app = test_app();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let day = SeriesKey::new(INFY, ChartInterval::Day);
        let other = SeriesKey::new(RELIANCE, ChartInterval::M5);

        app.want_candles_for_test(five);
        app.want_candles_for_test(day);

        assert!(app.candles_loading(five));
        assert!(app.candles_loading(day));
        assert!(!app.candles_loading(other), "nothing asked for this one");
    }

    #[test]
    fn a_chart_only_token_is_claimed_while_its_panel_draws_and_dropped_after() {
        let mut app = test_app();

        let mut order = sample_order("COMPLETE");
        order.tradingsymbol = "TCS".into();
        order.instrument_token = TCS;
        app.orders = Some(vec![order]);
        app.set_pane(PaneId::OrdersBook);

        app.sync_subscriptions();
        assert!(
            !app.desired_subscriptions().contains_key(&TCS),
            "no chart is drawing it, so nothing claims it"
        );

        app.open_chart_panel_for_test(PaneId::OrdersBook);
        let key = SeriesKey::new(TCS, app.chart_pane.interval());
        app.candles.insert(key, bars(20));
        app.sync_subscriptions();
        assert_eq!(
            app.desired_subscriptions().get(&TCS),
            Some(&StreamMode::Full),
            "the chart claims its instrument in full"
        );
        assert!(
            app.candle_store().contains(key),
            "the chart's own claim is what carries its series through the prune"
        );

        app.close_chart_panel();
        assert!(!app.desired_subscriptions().contains_key(&TCS));
        assert!(
            !app.candle_store().contains(key),
            "and nothing keeps a series no chart is reading"
        );
    }

    #[test]
    fn a_token_two_consumers_claim_survives_losing_one_of_them() {
        let mut app = test_app();
        app.watch.test_push(watch_row(INFY, "INFY"));
        app.set_pane(PaneId::TradingWatch);
        app.candles
            .insert(SeriesKey::new(INFY, ChartInterval::M5), bars(20));

        app.circuits.insert(INFY, None);

        app.sync_subscriptions();
        assert_eq!(
            app.desired_subscriptions().get(&INFY),
            Some(&StreamMode::Full),
            "the Watch detail column and its chart both want depth"
        );

        app.set_pane(PaneId::DerivativesChain);
        app.sync_subscriptions();
        assert_eq!(
            app.desired_subscriptions().get(&INFY),
            Some(&StreamMode::Quote),
            "the row is still a consumer, so the token is not unsubscribed"
        );
    }

    #[test]
    fn the_union_of_two_charts_demands_carries_both_their_series() {
        let mut store = CandleStore::default();
        let mine = SeriesKey::new(INFY, ChartInterval::M5);
        let theirs = SeriesKey::new(RELIANCE, ChartInterval::Day);
        let stale = SeriesKey::new(TCS, ChartInterval::M5);
        for key in [mine, theirs, stale] {
            store.insert(key, bars(3));
        }

        let mut demands = Demands::default();
        demands.absorb(chart(1, INFY, "INFY", ChartInterval::M5).demands());
        demands.absorb(chart(2, RELIANCE, "RELIANCE", ChartInterval::Day).demands());
        let desired = demands.resolve(subscriptions::TOKEN_BUDGET);

        store.retain(|key| desired.contains_key(&key.token));
        assert!(store.contains(mine), "the first chart is still drawn");
        assert!(store.contains(theirs), "and so is the second");
        assert!(!store.contains(stale), "nothing draws this one");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn one_tick_advances_every_retained_interval_of_its_token_once() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "INFY", INFY,
        )])));
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let day = SeriesKey::new(INFY, ChartInterval::Day);
        app.candles
            .insert(five, vec![bar("2026-07-07 09:55:00", 100.0)]);
        app.candles
            .insert(day, vec![bar("2026-07-07 00:00:00", 100.0)]);
        let before = (
            app.candle_store().generation(five),
            app.candle_store().generation(day),
        );

        let mut tick = ltp_tick(INFY, 105.0);
        tick.last_trade_time = Some(1_783_398_600);
        tick.volume = Some(500);
        app.on_ws(TickerEvent::Ticks(vec![tick]));

        let five_bars = app.candle_store().candles(five).unwrap();
        assert_eq!(five_bars.len(), 2, "10:00 is a new five-minute bucket");
        assert_eq!(
            crate::tui::data::candle_timestamp(&five_bars[1]),
            "2026-07-07 10:00:00"
        );
        assert_eq!(five_bars[1].close.to_bits(), 105.0_f64.to_bits());

        let day_bars = app.candle_store().candles(day).unwrap();
        assert_eq!(day_bars.len(), 1, "10:00 is still the same day");
        assert_eq!(
            day_bars[0].close.to_bits(),
            105.0_f64.to_bits(),
            "and it took the same tick"
        );
        assert_eq!(day_bars[0].high.to_bits(), 105.0_f64.to_bits());

        assert_eq!(app.candle_store().generation(five), before.0 + 1);
        assert_eq!(app.candle_store().generation(day), before.1 + 1);
    }

    #[test]
    fn a_charts_memo_follows_its_own_series_generation() {
        let mut store = CandleStore::default();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        store.insert(five, bars(20));
        let chart = chart(1, INFY, "INFY", ChartInterval::M5);

        let first = chart.indicator_series(INFY, &store).clone();

        assert_eq!(*chart.indicator_series(INFY, &store), first);

        store.insert(five, bars(30));
        assert_ne!(
            *chart.indicator_series(INFY, &store),
            first,
            "a fetch that replaced the series must recompute the line"
        );

        let refreshed = chart.indicator_series(INFY, &store).clone();
        store.insert(SeriesKey::new(INFY, ChartInterval::Day), bars(5));
        assert_eq!(*chart.indicator_series(INFY, &store), refreshed);
    }

    #[test]
    fn rebinding_a_chart_drops_the_scan_it_had_on_the_old_instrument() {
        let mut app = test_app();
        app.holdings = Some(vec![holding("INFY", INFY, 100.0, 110.0, 105.0, 10)]);
        app.orders = Some(vec![sample_order("COMPLETE")]);
        app.set_pane(PaneId::PortfolioHoldings);
        app.open_chart_panel_for_test(PaneId::PortfolioHoldings);
        app.candles
            .insert(SeriesKey::new(INFY, ChartInterval::M5), bars(20));
        app.sync_subscriptions();

        app.chart_pane.set_cursor(Some(7));
        assert_eq!(
            app.chart_pane.binding(),
            ChartBinding::SelectionOf(PaneId::PortfolioHoldings)
        );

        app.set_pane(PaneId::OrdersBook);
        app.open_chart_panel_for_test(PaneId::OrdersBook);
        assert_eq!(
            app.chart_pane.binding(),
            ChartBinding::SelectionOf(PaneId::OrdersBook)
        );
        assert_eq!(
            app.chart_pane.cursor(),
            None,
            "a scan of the holding's bars means nothing on the order's"
        );
    }

    #[test]
    fn a_chart_is_looked_up_by_the_id_a_hit_recorded() {
        let app: App = test_app();
        let id = app.chart_pane.id();
        assert!(app.chart_by_id(id).is_some());
        assert!(
            app.chart_by_id(ChartId::new(41)).is_none(),
            "a hit from another instance resolves to no chart here"
        );
    }
}
