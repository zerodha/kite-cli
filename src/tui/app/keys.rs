use super::{App, Modal};
use crate::tui::data;
use crate::tui::workspace::{PaneId, Preset};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(in crate::tui::app) fn on_paste(&mut self, text: &str) {
        if let Some(Modal::Notes(state)) = self.modals.last_mut() {
            if let Some(editor) = state.editor.as_mut() {
                for character in text.replace("\r\n", "\n").replace('\r', "\n").chars() {
                    if !character.is_control() || matches!(character, '\n' | '\t') {
                        editor.buffer.insert(character);
                    }
                }
            }
            return;
        }
        let editable = match self.modals.last() {
            Some(Modal::Search(_) | Modal::Palette(_) | Modal::ThemePicker(_)) => true,
            Some(Modal::Ticket(state)) => !state.focused().is_choice(),
            Some(Modal::PositionConvert(state)) => {
                state.focus == crate::tui::ticket::ConversionField::Quantity
            }
            Some(Modal::Alert(state)) => {
                state.stage == super::AlertFormStage::Edit
                    && state.focus == super::AlertFormFocus::Level
            }
            Some(Modal::AtoItem(state)) => matches!(
                state.focus,
                super::AtoItemFocus::Quantity
                    | super::AtoItemFocus::Price
                    | super::AtoItemFocus::Trigger
            ),
            _ => false,
        };
        if editable {
            for character in text
                .split(['\r', '\n'])
                .next()
                .unwrap_or_default()
                .chars()
                .filter(|character| !character.is_control())
            {
                self.on_key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE));
            }
        }
    }

    pub(in crate::tui::app) fn press(&mut self, code: KeyCode) {
        self.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    pub(in crate::tui::app) fn on_key(&mut self, key: KeyEvent) {
        let key = crate::tui::event::normalize_key(key);

        if self.modals.is_empty()
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && matches!(key.code, KeyCode::Char('h' | 'l'))
        {
            self.seed_chart_cursor_from_hover(self.focused_pane());
        }
        self.clear_chart_hovers();

        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            self.should_quit = true;
            return;
        }

        if key.code == KeyCode::Char('k') && key.modifiers == KeyModifiers::CONTROL {
            if self.modals.is_empty() {
                self.open_palette();
            }
            return;
        }
        if key.code == KeyCode::Char('l') && key.modifiers == KeyModifiers::CONTROL {
            self.toggle_event_log();
            return;
        }

        if key.code == KeyCode::Char('r') && key.modifiers == KeyModifiers::CONTROL {
            self.instruments_error = None;
            data::spawn_load_instruments(self.uuid.clone(), self.events_tx.clone());
            return;
        }
        if self.modals.is_empty()
            && key.code == KeyCode::Char('a')
            && key.modifiers == KeyModifiers::CONTROL
            && matches!(
                self.focused_pane(),
                PaneId::OrdersBook | PaneId::TradingOrders
            )
        {
            self.toggle_all_open_order_marks(self.focused_pane());
            return;
        }
        match self.modals.pop() {
            Some(Modal::Help(offset)) => {
                let max = self.help_max_scroll.get();
                let offset = offset.min(max);
                let next = match key.code {
                    KeyCode::Char('?' | 'q') | KeyCode::Esc => return,
                    KeyCode::Char('j') | KeyCode::Down => offset.saturating_add(1),
                    KeyCode::Char('k') | KeyCode::Up => offset.saturating_sub(1),
                    KeyCode::PageDown => offset.saturating_add(10),
                    KeyCode::PageUp => offset.saturating_sub(10),
                    KeyCode::Char('g') | KeyCode::Home => 0,
                    KeyCode::Char('G') | KeyCode::End => max,
                    _ => offset,
                };
                self.modals.push(Modal::Help(next.min(max)));
            }

            Some(Modal::Search(state)) => self.key_search(key, state),
            Some(Modal::Palette(state)) => self.key_palette(key, state),
            Some(Modal::ThemePicker(state)) => self.key_theme_picker(key, state),
            Some(Modal::WorkspacePicker(state)) => self.key_workspace_picker(key, state),
            Some(Modal::Ticket(state)) => self.key_ticket(key, state),
            Some(Modal::PositionConvert(state)) => self.key_position_convert(key, state),
            Some(Modal::Confirm(state)) => self.key_confirm(key, state),
            Some(Modal::OrderDetail(order)) => self.key_order_detail(key, order),
            Some(Modal::Alert(state)) => self.key_alert(key, state),
            Some(Modal::AtoItem(state)) => self.key_ato_item(key, state),
            Some(Modal::EventLog(state)) => self.key_event_log(key, state),
            Some(Modal::Notes(state)) => self.key_notes(key, state),
            Some(Modal::Funds) => {
                if !matches!(key.code, KeyCode::Esc | KeyCode::Char('q' | 'F')) {
                    self.modals.push(Modal::Funds);
                }
            }
            None if matches!(key.code, KeyCode::Char(_))
                && key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) => {}
            None => self.key_view(key),
        }
    }

    fn key_view(&mut self, key: KeyEvent) {
        let pane = self.focused_pane();
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,

            KeyCode::Esc if self.chart_panel.is_some() => self.close_chart_panel(),
            KeyCode::Char('?') => self.modals.push(Modal::Help(0)),
            KeyCode::Char(':') => self.open_palette(),

            KeyCode::Char('T') => self.open_theme_picker(),

            KeyCode::Char('`') => self.open_workspace_picker(),

            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(preset) = Preset::from_key(c) {
                    self.switch_workspace(preset);
                }
            }

            KeyCode::Tab => self.cycle_pane(1),
            KeyCode::BackTab => self.cycle_pane(-1),
            KeyCode::Right => self.switch_workspace(self.workspace.cycle(1)),
            KeyCode::Left => self.switch_workspace(self.workspace.cycle(-1)),

            KeyCode::Char('z') => self.toggle_maximize(),

            KeyCode::Char('c')
                if matches!(
                    pane,
                    PaneId::TradingOrders | PaneId::OrdersBook | PaneId::PortfolioHoldings
                ) =>
            {
                self.toggle_chart_panel(pane);
            }

            KeyCode::Char('C') if self.open_chart().is_some() => {
                self.chart_maximized = !self.chart_maximized;
            }
            KeyCode::Char('i') if self.acting_chart(pane).is_some() => self.cycle_interval(pane),
            KeyCode::Char('v') if self.acting_chart(pane).is_some() => {
                self.toggle_chart_volume(pane);
            }

            KeyCode::Char('I') if self.acting_chart(pane).is_some() => {
                self.cycle_chart_indicator(pane);
            }
            KeyCode::Char('h') if self.acting_chart(pane).is_some() => {
                self.move_chart_cursor(pane, true);
            }
            KeyCode::Char('l') if self.acting_chart(pane).is_some() => {
                self.move_chart_cursor(pane, false);
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),

            KeyCode::Char('g')
                if !matches!(pane, PaneId::TradingWatch | PaneId::PortfolioHoldings) =>
            {
                self.set_selection(0);
            }
            KeyCode::Char('G') => self.set_selection(usize::MAX),

            KeyCode::Char('A') => {
                data::spawn_refresh_alerts(
                    self.client.clone(),
                    self.events_tx.clone(),
                    self.alert_fetch_requests.clone(),
                );
                self.show_pane(PaneId::OrdersAlerts);
            }

            KeyCode::Char('F') => self.modals.push(Modal::Funds),

            KeyCode::Char('n') => self.open_notes(),

            KeyCode::Char('r') if pane != PaneId::OrdersAlerts => self.refresh_active(),

            KeyCode::Char('=') => self.reset_splits(),

            KeyCode::Char('[' | ']') if self.workspace == Preset::Overview => {
                self.focus_pane(if key.code == KeyCode::Char('[') {
                    PaneId::OverviewPositions
                } else {
                    PaneId::OverviewHoldings
                });
            }
            KeyCode::Char('[' | ']') if pane == PaneId::InvestBids => {
                let delta = if key.code == KeyCode::Char('[') {
                    -1
                } else {
                    1
                };
                let section = self.bids_pane.section().cycle(delta);
                self.bids_pane.set_section(section);
            }

            KeyCode::Char('u') if pane == PaneId::OverviewMovers => self.cycle_movers_universe(),

            KeyCode::Enter
                if matches!(
                    pane,
                    PaneId::OverviewPositions | PaneId::OverviewHoldings | PaneId::TradingDepth
                ) =>
            {
                self.enter_focused_pane();
            }
            _ => self.key_pane(pane, key),
        }
    }
}

#[cfg(test)]
pub(in crate::tui::app) mod tests {
    use super::super::modals::confirm::tests::long_review;
    use super::super::navigation::tests::stocked;
    use super::super::test_support::{
        H, W, char_key, char_key_code, click, holding, master_inst, open_and_review,
        open_ticket_state, position, render, sample_order, server_alert, test_app, text_at,
    };
    use super::super::{App, ChartHover, KeyCode, KeyEvent, KeyModifiers, Modal, SearchPurpose};
    use crate::tui::instruments::InstrumentStore;
    use crate::tui::marketwatch::WatchItem;
    use crate::tui::workspace::{PaneId, Preset};
    use kiteconnect::OrderType;
    use std::sync::Arc;

    pub(in crate::tui::app) fn trading_chart_focused() -> App {
        let mut app = stocked();
        app.switch_workspace(Preset::Trading);
        app.focus_pane(PaneId::TradingChart);
        app
    }

    #[tokio::test]
    async fn chart_keys_steer_the_focused_trading_chart_not_the_hidden_one() {
        let mut app = trading_chart_focused();
        let hidden = (app.chart_pane.interval(), app.chart_pane.volume());
        let leaf = (app.trading_chart.interval(), app.trading_chart.volume());

        app.on_key(char_key('i'));
        app.on_key(char_key('v'));
        assert_ne!(app.trading_chart.interval(), leaf.0, "i cycles the leaf");
        assert_ne!(app.trading_chart.volume(), leaf.1, "v toggles the leaf");
        assert_eq!(
            (app.chart_pane.interval(), app.chart_pane.volume()),
            hidden,
            "the book chart panel's instance is untouched"
        );

        let key = app.trading_chart.key().expect("the leaf follows INFY");
        let bars = (0..5)
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
            .collect();
        app.candles.insert(key, bars);
        app.trading_chart.set_hover(Some(ChartHover {
            token: key.token,
            interval: key.interval,
            ts: "2026-07-22 10:02:00".to_string(),
            window_ts: "2026-07-22 10:00:00".to_string(),
            visible_n: 5,
        }));
        app.on_key(char_key('h'));
        assert_eq!(app.trading_chart.cursor(), Some(3), "seeded at 2, then h");
        assert_eq!(app.trading_chart.hover(), None);
        app.on_key(char_key('l'));
        assert_eq!(app.trading_chart.cursor(), Some(2));
        assert_eq!(
            app.chart_pane.cursor(),
            None,
            "the hidden chart never scans"
        );
    }

    #[tokio::test]
    async fn local_key_exceptions_follow_the_focused_pane() {
        let mut app = stocked();
        app.switch_workspace(Preset::Overview);
        app.switch_workspace(Preset::Trading);
        app.focus_pane(PaneId::TradingWatch);
        app.on_key(char_key('g'));
        assert!(
            matches!(app.modals.last(), Some(Modal::Ticket(_))),
            "g on Trading's Watch opens a GTT ticket: {:?}",
            app.modals.last()
        );

        let mut app = stocked();
        app.show_pane(PaneId::OrdersAlerts);
        app.switch_workspace(Preset::Portfolio);
        app.focus_pane(PaneId::PortfolioHoldings);
        app.on_key(char_key('r'));
        assert!(
            app.toasts().iter().any(|t| t.text.contains("refreshing")),
            "r on Portfolio's Holdings refreshes"
        );
    }

    #[tokio::test]
    async fn holdings_chart_opens_in_place_follows_selection_and_toggles_closed() {
        let mut app = test_app();
        app.holdings = Some(vec![
            holding("INFY", 408_065, 100.0, 110.0, 105.0, 10),
            holding("RELIANCE", 738_561, 100.0, 105.0, 100.0, 5),
        ]);
        app.set_workspace(Preset::Portfolio);
        app.focus_pane(PaneId::PortfolioHoldings);

        app.on_key(char_key('c'));
        assert_eq!(app.focused_pane(), PaneId::PortfolioHoldings);
        assert_eq!(app.chart_panel_pane(), Some(PaneId::PortfolioHoldings));
        assert_eq!(
            app.workspaces.maximized(Preset::Portfolio),
            Some(PaneId::PortfolioHoldings),
            "the pane takes the body while its chart is open"
        );
        assert_eq!(
            app.selected_chart_target()
                .map(|target| target.instrument_token),
            Some(408_065)
        );

        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(
            app.selected_chart_target()
                .map(|target| target.instrument_token),
            Some(738_561)
        );

        app.chart_pane.set_cursor(Some(20));
        app.on_key(char_key('5'));
        assert_eq!(app.focused_pane(), PaneId::OrdersBook);
        assert_eq!(app.chart_panel_pane(), None, "a switch closes the panel");
        assert_eq!(app.chart_pane.cursor(), None);
        app.on_key(char_key('3'));
        assert_eq!(app.focused_pane(), PaneId::PortfolioHoldings);
        assert_eq!(app.workspaces.maximized(Preset::Portfolio), None);

        app.on_key(char_key('c'));
        assert_eq!(app.chart_panel_pane(), Some(PaneId::PortfolioHoldings));
        app.on_key(char_key('c'));
        assert_eq!(app.focused_pane(), PaneId::PortfolioHoldings);
        assert_eq!(app.chart_panel_pane(), None);
        assert_eq!(app.chart_pane.target(), None);
        assert_eq!(
            app.workspaces.maximized(Preset::Portfolio),
            None,
            "closing the chart hands the body back"
        );
    }

    #[tokio::test]
    async fn tokenless_order_opens_unavailable_panel_that_can_close() {
        let mut app = test_app();
        let mut tokenless = sample_order("REJECTED");
        tokenless.instrument_token = 0;
        app.orders = Some(vec![tokenless]);
        app.set_pane(PaneId::OrdersBook);

        app.on_key(char_key('c'));
        assert_eq!(app.chart_panel_pane(), Some(PaneId::OrdersBook));
        assert_eq!(
            app.selected_chart_target()
                .map(|target| target.instrument_token),
            Some(0)
        );

        app.on_key(char_key('c'));
        assert_eq!(app.chart_panel_pane(), None);
        assert_eq!(app.chart_pane.target(), None);
    }

    #[tokio::test]
    async fn backing_out_of_a_review_returns_the_unchanged_draft() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, |state| {
            state.order_type = OrderType::Market;
            state.qty = "7".to_string();
        });
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));

        for back in [KeyCode::Char('n'), KeyCode::Esc] {
            let Some(Modal::Ticket(_)) = ({
                app.on_key(char_key_code(back));
                app.modals.last()
            }) else {
                panic!("{back:?} did not return the draft");
            };
            let state = open_ticket_state(&app);
            assert_eq!(state.qty, "7", "{back:?} changed the draft");
            assert_eq!(state.order_type, OrderType::Market);
            app.on_key(char_key_code(KeyCode::Enter));
        }
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));
    }

    #[test]
    fn a_review_answers_to_its_own_keys_alone() {
        let mut app = test_app();
        app.modals.push(Modal::Confirm(long_review()));
        for ignored in [KeyCode::Char('x'), KeyCode::Down, KeyCode::Tab] {
            app.on_key(char_key_code(ignored));
            assert!(
                matches!(app.modals.last(), Some(Modal::Confirm(_))),
                "{ignored:?} closed the review"
            );
        }
        app.on_key(char_key_code(KeyCode::Char('n')));
        assert!(app.modals.is_empty(), "n backs out");

        app.modals.push(Modal::Confirm(long_review()));
        app.on_key(char_key_code(KeyCode::Esc));
        assert!(app.modals.is_empty(), "Esc backs out");

        app.notes = crate::tui::notes::Pad::default();
        app.modals.push(Modal::Confirm(long_review()));
        render(&app, W, H);
        app.on_key(char_key_code(KeyCode::Char('y')));
        assert!(app.modals.is_empty());
        assert!(!app.pending_action, "a local delete arms no request");
    }

    #[test]
    fn every_instrument_row_that_creates_a_gtt_also_creates_an_alert() {
        for (view, seed, alert_key) in [
            (PaneId::TradingWatch, 0u8, 'a'),
            (PaneId::PortfolioHoldings, 1, 'B'),
            (PaneId::PortfolioPositions, 2, 'a'),
        ] {
            let mut app = test_app();
            app.ephemeral = true;
            app.set_pane(view);
            match seed {
                0 => app.watch.test_push(WatchItem {
                    exchange: "NSE".into(),
                    tradingsymbol: "INFY".into(),
                    instrument_token: 408_065,
                    segment: "NSE".into(),
                    ..WatchItem::default()
                }),
                1 => {
                    app.holdings = Some(vec![holding("INFY", 408_065, 100.0, 110.0, 100.0, 5)]);
                }
                _ => {
                    app.positions = Some({
                        let mut row = crate::tui::data::test_models::positions();
                        row.net = vec![position("INFY", 408_065, 5, 10.0, 110.0, 100.0)];
                        row
                    });
                }
            }

            app.on_key(char_key(alert_key));
            let Some(Modal::Alert(state)) = app.modals.last() else {
                panic!("{view:?}: {alert_key} must open the alert form");
            };
            assert_eq!(state.tradingsymbol, "INFY", "{view:?}");
            assert_eq!(state.exchange, "NSE", "{view:?}");
            assert_eq!(state.instrument_token, 408_065, "{view:?}");
            assert!(
                !state.is_edit(),
                "{view:?}: {alert_key} creates, it does not edit"
            );
        }
    }

    #[test]
    fn holdings_footer_hints_press_the_alert_and_cdsl_keys() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.holdings = Some(vec![holding("INFY", 408_065, 100.0, 110.0, 100.0, 5)]);
        app.profile = Some({
            let mut row = crate::tui::data::test_models::profile();
            row.meta = {
                let mut row = crate::tui::data::test_models::profile_meta();
                row.poa = Some("consent".to_string());
                row
            };
            row
        });

        let x = text_at(&app, W, H, H - 1, "B:alert");
        click(&mut app, W, H, x, H - 1);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("`B:alert` must open the alert form");
        };
        assert_eq!(state.tradingsymbol, "INFY");
        app.modals.clear();

        let x = text_at(&app, W, H, H - 1, "a:authorise");
        click(&mut app, W, H, x, H - 1);
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "`a:authorise` must open the authorisation confirm"
        );
    }

    #[test]
    fn the_alerts_view_creates_an_alert_through_the_instrument_picker() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "INFY", 408_065,
        )])));
        app.set_pane(PaneId::OrdersAlerts);

        app.on_key(char_key('a'));
        let Some(Modal::Search(state)) = app.modals.last() else {
            panic!("a must open the instrument picker");
        };
        assert_eq!(state.purpose, SearchPurpose::NewAlert);

        for ch in "INFY".chars() {
            app.on_key(char_key(ch));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("picking an instrument must open the alert form");
        };
        assert_eq!(state.tradingsymbol, "INFY");
        assert_eq!(state.instrument_token, 408_065);
    }

    #[tokio::test]
    async fn the_number_keys_select_the_six_workspaces_and_no_others() {
        let mut app = test_app();
        for (key, preset) in ['1', '2', '3', '4', '5', '6'].into_iter().zip(Preset::ALL) {
            app.on_key(char_key(key));
            assert_eq!(app.workspace(), preset, "{key}");
        }
        for key in ['7', '8', '9', '0'] {
            app.on_key(char_key(key));
            assert_eq!(app.workspace(), Preset::Invest, "{key} moved the workspace");
            assert!(app.modals.is_empty(), "{key} opened something");
        }
    }

    #[tokio::test]
    async fn the_alerts_pane_is_reachable_by_its_workspace_key_and_by_a() {
        let mut app = test_app();
        app.on_key(char_key(Preset::Orders.key()));
        assert_eq!(app.workspace(), Preset::Orders);
        assert_eq!(
            Preset::Orders.panes(),
            [PaneId::OrdersBook, PaneId::OrdersGtt, PaneId::OrdersAlerts],
            "Alerts sits below GTT, beside the order book"
        );

        app.on_key(char_key('1'));
        assert_eq!(app.workspace(), Preset::Overview);

        app.on_key(char_key('A'));
        assert_eq!(
            app.focused_pane(),
            PaneId::OrdersAlerts,
            "A must switch to the alerts pane"
        );
        assert!(
            app.modals.is_empty(),
            "A must no longer open a modal: {:?}",
            app.modals.last()
        );
    }

    #[tokio::test]
    async fn shift_c_maximizes_an_open_chart_and_never_outlives_it() {
        let mut app = test_app();
        app.orders = Some(vec![sample_order("OPEN")]);
        app.set_workspace(Preset::Orders);
        assert!(!app.chart_maximized);

        app.on_key(char_key('C'));
        assert!(
            !app.chart_maximized,
            "C must not maximize when there is no chart to maximize"
        );

        app.on_key(char_key('c'));
        assert_eq!(
            app.chart_panel_pane(),
            Some(PaneId::OrdersBook),
            "c opens the chart"
        );
        app.on_key(char_key('C'));
        assert!(app.chart_maximized, "C maximizes the open chart");
        app.on_key(char_key('C'));
        assert!(!app.chart_maximized, "C toggles back to the split");

        app.on_key(char_key('C'));
        assert!(app.chart_maximized);
        app.on_key(char_key('c'));
        assert_eq!(app.chart_panel_pane(), None, "c closes the chart");
        assert!(
            !app.chart_maximized,
            "closing the chart must clear maximize"
        );
    }

    #[tokio::test]
    async fn r_toggles_status_on_the_alerts_view_while_r_shift_refreshes() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        app.set_pane(PaneId::OrdersAlerts);

        app.on_key(char_key('r'));
        assert!(
            !app.toasts().iter().any(|t| t.text.contains("refreshing")),
            "r must not refresh on the alerts view, it toggles the alert"
        );
        app.on_key(char_key('R'));
        assert!(
            app.toasts().iter().any(|t| t.text.contains("refreshing")),
            "R must refresh the alert list"
        );

        let mut app = test_app();
        app.set_pane(PaneId::OrdersGtt);
        app.on_key(char_key('r'));
        assert!(app.toasts().iter().any(|t| t.text.contains("refreshing")));
    }
}
