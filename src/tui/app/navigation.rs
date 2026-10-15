use super::panes::holdings::holding_key;
use super::panes::positions::position_key;
use super::{App, Modal};
use crate::tui::settings;
use crate::tui::workspace::{self, PaneId, Preset};

impl App {
    pub(in crate::tui::app) fn show_pane(&mut self, pane: PaneId) {
        self.switch_workspace(pane.preset());
        if self
            .workspaces
            .maximized(self.workspace)
            .is_some_and(|shown| shown != pane)
        {
            self.close_chart_panel();
            if self.workspaces.clear_maximize(self.workspace) {
                self.remember_workspace_focus();
            }
            self.hit.borrow_mut().invalidate();
        }
        self.focus_pane(pane);
    }

    #[cfg(test)]
    pub fn set_pane(&mut self, pane: PaneId) {
        self.set_workspace(pane.preset());
        self.workspaces.focus(self.workspace, pane);
        self.workspaces.toggle_maximize(self.workspace, pane);
    }

    pub fn workspace(&self) -> Preset {
        self.workspace
    }

    #[cfg(test)]
    pub fn set_workspace(&mut self, preset: Preset) {
        self.workspaces.clear_maximize(self.workspace);
        self.workspace = preset;

        self.refresh_linked_charts();
    }

    #[cfg(test)]
    pub fn focus_pane_for_test(&mut self, pane: PaneId) {
        self.focus_pane(pane);
    }

    pub(in crate::tui::app) fn switch_workspace(&mut self, preset: Preset) {
        if self.workspace == preset {
            return;
        }

        self.close_chart_panel();
        if self.workspaces.clear_maximize(self.workspace) {
            self.remember_workspace_focus();
        }
        self.workspace = preset;
        self.remember_workspace();
        self.clear_chart_cursors();
        self.sync_subscriptions();
        if self.draws_a_chart() {
            self.ensure_candles(true);
        }
        if preset
            .panes()
            .iter()
            .any(|pane| self.chain_pane_of(*pane).is_some())
        {
            self.ensure_chain();
        }
        if preset == Preset::Invest {
            self.refresh_bids();
        }

        let _ = self.ensure_movers();
    }

    pub fn focused_pane(&self) -> PaneId {
        self.workspaces.focused(self.workspace)
    }

    pub(in crate::tui::app) fn focus_pane(&mut self, pane: PaneId) {
        if self
            .workspaces
            .maximized(self.workspace)
            .is_some_and(|shown| shown != pane)
        {
            return;
        }
        if self.workspaces.focus(self.workspace, pane) {
            self.remember_workspace_focus();
        }
    }

    fn remember_workspace(&mut self) {
        let Some(path) = self.settings_file() else {
            return;
        };
        let workspace = self.workspace;
        if self.account.is_empty() {
            return;
        }
        if let Err(err) = settings::save_workspace_preferences(&path, &self.account, workspace) {
            self.toast_error(format!("workspace opened but not remembered: {err}"));
        }
    }

    fn remember_workspace_focus(&mut self) {
        let Some(path) = self.settings_file() else {
            return;
        };
        if self.account.is_empty() || self.chart_panel.is_some() {
            return;
        }
        let workspace = self.workspace;
        let focused = self.focused_pane();
        let maximized = self.workspaces.maximized(workspace);
        if let Err(err) =
            settings::save_workspace_focus(&path, &self.account, workspace, focused, maximized)
        {
            self.toast_error(format!("pane focused but not remembered: {err}"));
        }
    }

    pub(in crate::tui::app) fn cycle_pane(&mut self, delta: isize) {
        let focused = self.focused_pane();
        let order = if self.workspaces.maximized(self.workspace).is_some() {
            std::slice::from_ref(&focused)
        } else {
            self.workspace.panes()
        };
        let next = workspace::focus_next(order, focused, delta);
        self.focus_pane(next);
    }

    pub(in crate::tui::app) fn hidden_panes(&self) -> Vec<PaneId> {
        self.hit.borrow().hidden.clone()
    }

    pub(in crate::tui::app) fn palette_origin(&self) -> PaneId {
        match self.modals.iter().rev().find_map(|modal| match modal {
            Modal::Palette(state) => Some(state.origin),
            Modal::Search(state) => Some(state.origin),
            _ => None,
        }) {
            Some(origin) => origin,
            None => self.focused_pane(),
        }
    }

    pub(in crate::tui::app) fn focus_or_zoom(&mut self, pane: PaneId) {
        self.focus_pane(pane);
        if self.hidden_panes().contains(&pane) && self.workspace.panes().contains(&pane) {
            self.workspaces.toggle_maximize(self.workspace, pane);
            self.remember_workspace_focus();
        }
        self.hit.borrow_mut().invalidate();
    }

    pub(in crate::tui::app) fn workspace_context(query: &str) -> String {
        if query.is_empty() {
            return crate::tui::palette::WORKSPACE_SYNTAX.to_string();
        }
        if crate::tui::palette::workspace_matches(query)
            .next()
            .is_none()
        {
            format!("No workspace named {query}")
        } else {
            "Tab completes the name · Enter switches".to_string()
        }
    }

    pub(in crate::tui::app) fn toggle_maximize(&mut self) {
        let pane = self.focused_pane();
        self.workspaces.toggle_maximize(self.workspace, pane);
        self.remember_workspace_focus();

        self.hit.borrow_mut().invalidate();
    }

    pub(in crate::tui::app) fn enter_focused_pane(&mut self) {
        match self.focused_pane() {
            PaneId::OverviewPositions => {
                let natural = self.natural_positions_order();
                let net = self.positions_net();
                let Some(target) = self
                    .books
                    .overview_positions
                    .target(&natural, |i| net.get(i).map(position_key))
                else {
                    return;
                };
                self.show_pane(PaneId::PortfolioPositions);
                let order = self.positions_order_in(PaneId::PortfolioPositions);
                let row = order.iter().position(|&i| i == target).unwrap_or(0);
                self.set_selection(row);
            }
            PaneId::OverviewHoldings => {
                let overview = self.overview_holdings_display_order();
                let holdings = self.holdings.as_deref().unwrap_or_default();
                let Some(target) = self
                    .books
                    .overview_holdings
                    .target(&overview, |i| holdings.get(i).map(holding_key))
                else {
                    return;
                };
                self.show_pane(PaneId::PortfolioHoldings);
                let order = self.holdings_order_in(PaneId::PortfolioHoldings);
                let row = order.iter().position(|&i| i == target).unwrap_or(0);
                self.set_selection(row);
            }
            PaneId::TradingDepth => self.toggle_maximize(),

            _ => {}
        }
    }
}

#[cfg(test)]
pub(in crate::tui::app) mod tests {
    use super::super::mouse::lists::tests::overview_app;
    use super::super::test_support::{char_key, holding, position, sample_order, screen, test_app};
    use super::super::{
        App, HoldingsSort, KeyCode, KeyEvent, KeyModifiers, Modal, OrdersSort, PendingAction,
        PositionsSort, SortTarget, ui,
    };

    use crate::tui::marketwatch::{State as WatchState, WatchItem};
    use crate::tui::workspace::{PaneId, Preset};
    use kiteconnect::ticker::Tick;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[tokio::test]
    async fn arrow_keys_cycle_the_workspace_registry() {
        let mut app = test_app();
        assert_eq!(app.workspace(), Preset::Overview);

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        assert_eq!(app.workspace(), Preset::Trading);

        app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.workspace(), Preset::Overview);

        app.on_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        assert_eq!(app.workspace(), Preset::Invest);
        assert_eq!(app.focused_pane(), PaneId::InvestMf);

        for want in Preset::ALL {
            app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
            assert_eq!(app.workspace(), want);
        }
    }

    #[test]
    fn overview_brackets_cannot_focus_past_a_maximized_pane() {
        let mut app = stocked();
        app.set_workspace(Preset::Overview);
        app.on_key(char_key('['));
        app.on_key(char_key('z'));
        assert_eq!(
            app.workspaces.maximized(Preset::Overview),
            Some(PaneId::OverviewPositions)
        );

        app.on_key(char_key(']'));
        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);
        assert_eq!(
            app.workspaces.maximized(Preset::Overview),
            Some(PaneId::OverviewPositions)
        );

        app.on_key(char_key('z'));
        app.on_key(char_key(']'));
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
    }

    #[test]
    fn overview_enter_maps_positions_through_display_order() {
        let mut app = test_app();

        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("AAA", 1, 10, 100.0, 100.0, 100.0),
                position("BBB", 2, 10, -50.0, 100.0, 100.0),
                position("CCC", 3, 10, 300.0, 100.0, 100.0),
            ];
            row
        });

        app.set_selection(1);
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.focused_pane(), PaneId::PortfolioPositions);
        let sel = app.pane_row(PaneId::PortfolioPositions);
        let natural = app.positions_order_in(PaneId::PortfolioPositions)[sel];
        assert_eq!(
            app.positions.as_ref().unwrap().net[natural].tradingsymbol,
            "BBB"
        );
    }

    #[test]
    fn overview_enter_maps_holdings_through_display_order() {
        let mut app = test_app();
        app.holdings = Some(vec![
            holding("AAA", 1, 100.0, 105.0, 100.0, 10),
            holding("BBB", 2, 100.0, 500.0, 100.0, 10),
        ]);
        app.workspaces
            .focus(Preset::Overview, PaneId::OverviewHoldings);

        app.set_selection(0);
        app.books.portfolio_holdings.set_sort(HoldingsSort::Symbol);
        app.holdings_gen += 1;
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.focused_pane(), PaneId::PortfolioHoldings);
        let sel = app.pane_row(PaneId::PortfolioHoldings);
        let natural = app.holdings_order_in(PaneId::PortfolioHoldings)[sel];
        assert_eq!(app.holdings.as_ref().unwrap()[natural].tradingsymbol, "BBB");
    }

    #[tokio::test]
    async fn each_workspace_keeps_the_pane_it_was_left_on() {
        let mut app = overview_app();
        app.on_key(char_key(']'));
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);

        app.show_pane(PaneId::PortfolioPositions);
        assert_eq!(app.focused_pane(), PaneId::PortfolioPositions);

        app.on_key(char_key(']'));
        assert_eq!(app.focused_pane(), PaneId::PortfolioPositions);

        app.switch_workspace(Preset::Overview);
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
    }

    const W: u16 = 160;

    const H: u16 = 45;

    pub(in crate::tui::app) fn stocked() -> App {
        let mut app = test_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("AAA", 101, 10, 10.0, 100.0, 100.0),
                position("BBB", 102, 10, 20.0, 200.0, 200.0),
            ];
            row
        });
        app.holdings = Some(
            (0u32..5)
                .map(|i| holding(&format!("H{i}"), 200 + i, 100.0, 110.0, 100.0, 1))
                .collect(),
        );
        app.orders = Some(vec![sample_order("OPEN")]);
        app.set_watch_for_test(WatchState::test_loaded(vec![
            watch_row(408_065, "INFY"),
            watch_row(738_561, "RELIANCE"),
        ]));

        app.circuits.insert(408_065, None);
        app.circuits.insert(738_561, None);
        app
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

    fn draw(app: &App) {
        let mut terminal = Terminal::new(TestBackend::new(W, H)).expect("test terminal");
        terminal
            .draw(|frame| ui::draw(frame, app))
            .expect("frame draws");
    }

    fn click(app: &mut App, column: u16, row: u16) {
        draw(app);
        app.on_event(crate::tui::event::AppEvent::Mouse(
            crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            },
        ));
    }

    fn header_rect(app: &App, owner: PaneId, want: SortTarget) -> ratatui::layout::Rect {
        let hit = app.hit.borrow();
        hit.regions
            .iter()
            .find(|region| {
                matches!(region.action, ui::HitAction::Header { owner: got, target }
                    if got == owner && target == want)
            })
            .unwrap_or_else(|| panic!("{owner:?} recorded no {want:?} header"))
            .rect
    }

    #[test]
    fn the_same_header_in_two_panes_sorts_only_its_owner_and_focuses_it_first() {
        let mut app = stocked();

        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingWatch);
        draw(&app);

        let before = app.books.orders.list.sort();
        let rect = header_rect(
            &app,
            PaneId::TradingOrders,
            SortTarget::Orders(OrdersSort::Symbol),
        );
        click(&mut app, rect.x, rect.y);

        assert_eq!(
            app.books.trading_orders.list.sort(),
            OrdersSort::Symbol,
            "the clicked table re-sorts"
        );
        assert_eq!(
            app.books.orders.list.sort(),
            before,
            "the other instance of the same book is untouched"
        );
        assert_eq!(
            app.focused_pane(),
            PaneId::TradingOrders,
            "a header click focuses its own table first"
        );
    }

    #[test]
    fn a_modal_consumes_every_key_and_pointer_including_the_strip() {
        let mut app = stocked();
        app.set_workspace(Preset::Trading);
        draw(&app);

        let strip = app
            .hit
            .borrow()
            .regions
            .iter()
            .find(|region| matches!(region.action, ui::HitAction::Workspace(_)))
            .map(|region| region.rect)
            .expect("the strip records its labels");

        app.modals.push(Modal::Help(0));
        draw(&app);
        assert!(
            !app.hit
                .borrow()
                .regions
                .iter()
                .any(|region| matches!(region.action, ui::HitAction::Workspace(_))),
            "an open modal leaves no strip target behind it"
        );

        click(&mut app, strip.x, strip.y);
        assert_eq!(app.workspace(), Preset::Trading);
        assert_eq!(app.modals.len(), 1, "and the modal is still open");

        let focused = app.focused_pane();
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        app.on_key(char_key('3'));
        assert_eq!(app.focused_pane(), focused);
        assert_eq!(app.workspace(), Preset::Trading);
        assert_eq!(app.modals.len(), 1);

        let before = app.pane_row(PaneId::PortfolioPositions);
        app.on_event(crate::tui::event::AppEvent::Mouse(
            crossterm::event::MouseEvent {
                kind: crossterm::event::MouseEventKind::ScrollDown,
                column: W / 2,
                row: H / 2,
                modifiers: KeyModifiers::NONE,
            },
        ));
        assert_eq!(app.pane_row(PaneId::PortfolioPositions), before);
    }

    #[tokio::test]
    async fn ctrl_k_lists_and_switches_workspaces_and_tab_completes_without_executing() {
        let mut app = test_app();
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        let Some(Modal::Palette(state)) = app.modals.last() else {
            panic!("Ctrl-K opens the palette");
        };
        let listed: Vec<String> = state
            .results
            .iter()
            .filter_map(|row| match row.item() {
                Some(super::super::ExecItem::Jump(crate::tui::palette::Jump::Workspace(
                    preset,
                ))) => Some(preset.title().to_string()),
                _ => None,
            })
            .collect();
        for name in [
            "Overview",
            "Trading",
            "Portfolio",
            "Derivatives",
            "Orders",
            "Invest",
        ] {
            assert!(listed.contains(&name.to_string()), "{name} is not listed");
        }

        for c in "workspace tra".chars() {
            app.on_key(char_key(c));
        }
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        let Some(Modal::Palette(state)) = app.modals.last() else {
            panic!("Tab keeps the palette open");
        };
        assert_eq!(state.input, "workspace Trading");
        assert_eq!(
            app.workspace(),
            Preset::Overview,
            "completion writes text and nothing else"
        );

        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(app.workspace(), Preset::Trading);
        assert!(app.modals.is_empty());
    }

    #[test]
    fn a_ticket_keeps_the_target_it_was_opened_with() {
        let mut app = stocked();
        app.set_workspace(Preset::Portfolio);
        app.focus_pane_for_test(PaneId::PortfolioPositions);

        app.books.portfolio_positions.set_sort(PositionsSort::Pnl);
        app.on_key(char_key('b'));
        let Some(Modal::Ticket(ticket)) = app.modals.last() else {
            panic!("b opens a ticket on the selected position");
        };
        let opened = ticket.tradingsymbol.clone();
        assert_eq!(opened, "BBB", "the higher P&L sorts first");

        app.on_ws(kiteconnect::ticker::TickerEvent::Ticks(vec![{
            let mut value = Tick::default();
            value.instrument_token = 101;
            value.last_price = Some(9_000.0);
            value
        }]));
        let Some(Modal::Ticket(ticket)) = app.modals.last() else {
            panic!("the ticket is still open");
        };
        assert_eq!(
            ticket.tradingsymbol, opened,
            "a re-rank under an open ticket does not retarget it"
        );

        app.set_chain_underlying_to(PaneId::DerivativesChain, "BANKNIFTY".to_string());
        let Some(Modal::Ticket(ticket)) = app.modals.last() else {
            panic!("the ticket is still open");
        };
        assert_eq!(ticket.tradingsymbol, opened);
    }

    #[test]
    fn a_pending_dispatch_in_one_workspace_blocks_another() {
        let mut app = stocked();

        app.set_workspace(Preset::Trading);
        app.pending_action = true;

        app.set_workspace(Preset::Portfolio);
        app.focus_pane_for_test(PaneId::PortfolioPositions);
        app.on_key(char_key('b'));
        assert!(
            app.modals.is_empty(),
            "a pending order blocks a ticket in another workspace: {:?}",
            app.modals.last()
        );
        assert!(
            app.toasts
                .iter()
                .any(|toast| toast.text.contains("in flight")),
            "and says why"
        );

        assert_eq!(app.preview_seq, 0);
        assert!(app.pending_action);

        app.pending_action = false;
        app.on_key(char_key('b'));
        assert!(matches!(app.modals.last(), Some(Modal::Ticket(_))));
        assert_eq!(app.preview_seq, 0, "staging sends nothing");
        let _ = PendingAction::AuthoriseHoldings;
    }

    #[tokio::test]
    async fn c_charts_a_book_in_place_and_esc_restores_the_layout() {
        for maximized_first in [false, true] {
            let mut app = stocked();
            app.set_workspace(Preset::Portfolio);
            app.focus_pane_for_test(PaneId::PortfolioHoldings);
            app.move_selection(2);
            if maximized_first {
                app.on_key(char_key('z'));
            }
            let before = app.workspaces.maximized(Preset::Portfolio);
            let charted = app
                .selected_holding_in(PaneId::PortfolioHoldings)
                .cloned()
                .expect("a row");

            app.on_key(char_key('c'));
            assert_eq!(app.workspace(), Preset::Portfolio);
            assert_eq!(app.focused_pane(), PaneId::PortfolioHoldings);
            assert_eq!(app.chart_panel_pane(), Some(PaneId::PortfolioHoldings));
            assert_eq!(
                app.workspaces.maximized(Preset::Portfolio),
                Some(PaneId::PortfolioHoldings),
                "the pane has the body while its chart is open"
            );
            assert_eq!(
                app.selected_chart_target()
                    .map(|target| target.instrument_token),
                Some(charted.instrument_token),
                "the chart is of the pane's own selected record"
            );

            app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            assert_eq!(app.chart_panel_pane(), None);
            assert_eq!(app.workspace(), Preset::Portfolio);
            assert_eq!(app.focused_pane(), PaneId::PortfolioHoldings);
            assert_eq!(
                app.workspaces.maximized(Preset::Portfolio),
                before,
                "maximized first: {maximized_first}"
            );

            app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
            assert_eq!(app.workspaces.maximized(Preset::Portfolio), before);
        }
    }

    #[tokio::test]
    async fn a_workspace_switch_closes_a_chart_panel() {
        let mut app = stocked();
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingOrders);
        app.on_key(char_key('c'));
        assert_eq!(app.workspace(), Preset::Trading);
        assert_eq!(app.chart_panel_pane(), Some(PaneId::TradingOrders));

        app.on_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        let moved = app.workspace();
        assert_eq!(app.chart_panel_pane(), None, "the switch closed it");
        assert_eq!(app.workspaces.maximized(Preset::Trading), None);
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.workspace(), moved, "Esc is not a way back");
    }

    #[tokio::test]
    async fn c_on_an_empty_composite_book_opens_nothing() {
        let mut app = stocked();
        app.orders = Some(Vec::new());
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingOrders);
        app.on_key(char_key('c'));
        assert_eq!(app.workspace(), Preset::Trading);
        assert!(
            app.toasts
                .iter()
                .any(|toast| toast.text.contains("chart unavailable")),
            "{:?}",
            app.toasts
        );
    }

    #[tokio::test]
    async fn enter_and_z_on_depth_end_in_the_same_state() {
        for key in [KeyCode::Enter, KeyCode::Char('z')] {
            let mut app = stocked();
            app.set_workspace(Preset::Trading);
            app.focus_pane_for_test(PaneId::TradingWatch);
            app.move_selection(1);
            app.focus_pane_for_test(PaneId::TradingDepth);

            app.on_key(KeyEvent::new(key, KeyModifiers::NONE));
            assert_eq!(app.workspace(), Preset::Trading, "{key:?}");
            assert_eq!(app.focused_pane(), PaneId::TradingDepth, "{key:?}");
            assert_eq!(
                app.workspaces.maximized(Preset::Trading),
                Some(PaneId::TradingDepth),
                "{key:?} gives the ladder the body"
            );
            assert_eq!(
                app.watch_pane
                    .selected(&app.watch)
                    .map(|item| item.tradingsymbol.clone()),
                Some("RELIANCE".to_string()),
                "{key:?} keeps the instrument the ladder was showing"
            );

            app.on_key(KeyEvent::new(key, KeyModifiers::NONE));
            assert_eq!(
                app.workspaces.maximized(Preset::Trading),
                None,
                "a second {key:?} hands the body back"
            );
        }
    }

    #[tokio::test]
    async fn movers_owns_its_universe_key_while_it_is_focused() {
        let mut app = stocked();
        app.set_workspace(Preset::Overview);
        app.focus_pane_for_test(PaneId::OverviewPositions);
        let before = app.movers_universe;
        app.on_key(char_key('u'));
        assert_eq!(
            app.movers_universe, before,
            "the snapshot beside Movers does not change its universe"
        );

        app.focus_pane_for_test(PaneId::OverviewMovers);
        app.on_key(char_key('u'));
        assert_ne!(app.movers_universe, before, "its own pane does");
    }

    #[test]
    fn the_overview_brackets_name_a_book_rather_than_toggling() {
        let mut app = stocked();
        app.set_workspace(Preset::Overview);
        app.focus_pane_for_test(PaneId::OverviewMovers);
        app.on_key(char_key('['));
        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);
        app.on_key(char_key('['));
        assert_eq!(
            app.focused_pane(),
            PaneId::OverviewPositions,
            "not a toggle"
        );
        app.on_key(char_key(']'));
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);

        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focused_pane(), PaneId::OverviewIndices);
        app.on_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.focused_pane(), PaneId::OverviewMovers);
    }

    #[test]
    fn z_zooms_the_focused_pane_and_a_workspace_switch_cancels_it() {
        let mut app = stocked();
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingOrders);
        app.on_key(char_key('z'));
        assert_eq!(
            app.workspaces.maximized(Preset::Trading),
            Some(PaneId::TradingOrders)
        );
        draw(&app);
        assert_eq!(app.hit.borrow().hidden.len(), 3, "the others are not drawn");

        app.on_key(char_key('z'));
        assert_eq!(app.workspaces.maximized(Preset::Trading), None);

        app.on_key(char_key('z'));
        app.set_workspace(Preset::Portfolio);
        assert_eq!(app.workspaces.maximized(Preset::Trading), None);
    }

    #[tokio::test]
    async fn a_palette_pane_row_focuses_and_zooms_a_hidden_pane() {
        let mut app = stocked();
        app.set_workspace(Preset::Trading);

        let mut terminal = Terminal::new(TestBackend::new(160, 45)).expect("test terminal");
        terminal
            .draw(|frame| ui::draw(frame, &app))
            .expect("frame draws");
        app.exec_palette(super::super::ExecItem::FocusPane(PaneId::TradingOrders));
        assert_eq!(app.focused_pane(), PaneId::TradingOrders);
        assert_eq!(app.workspaces.maximized(Preset::Trading), None);

        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("test terminal");
        terminal
            .draw(|frame| ui::draw(frame, &app))
            .expect("frame draws");
        app.exec_palette(super::super::ExecItem::FocusPane(PaneId::TradingDepth));
        assert_eq!(app.focused_pane(), PaneId::TradingDepth);
        assert_eq!(
            app.workspaces.maximized(Preset::Trading),
            Some(PaneId::TradingDepth)
        );
    }

    #[test]
    fn every_composite_paints_at_every_fixture_size() {
        let mut app = stocked();
        for preset in Preset::ALL {
            app.set_workspace(preset);
            for pane in preset.panes().iter().copied() {
                app.focus_pane_for_test(pane);
                for (width, height) in [(80u16, 24u16), (120, 35), (160, 45), (20, 6), (1, 1)] {
                    let mut terminal =
                        Terminal::new(TestBackend::new(width, height)).expect("test terminal");
                    terminal
                        .draw(|frame| ui::draw(frame, &app))
                        .unwrap_or_else(|err| {
                            panic!("{preset:?}/{pane:?} at {width}x{height}: {err}")
                        });
                }
            }
        }
    }

    #[test]
    fn the_orders_and_invest_workspaces_tile_and_collapse_like_the_others() {
        let mut app = stocked();
        for preset in [Preset::Orders, Preset::Invest] {
            app.set_workspace(preset);
            for (width, height) in [(120u16, 35u16), (160, 45)] {
                let text = screen(&app, width, height);
                assert!(
                    app.hidden_panes().is_empty(),
                    "{preset:?} at {width}x{height}"
                );
                for pane in preset.panes().iter().copied() {
                    assert!(
                        text.contains(pane.title()),
                        "{preset:?} at {width}x{height} lost {pane:?}:\n{text}"
                    );
                }
            }
            for pane in preset.panes().iter().copied() {
                app.focus_pane_for_test(pane);
                screen(&app, 80, 24);
                let hidden = app.hidden_panes();
                assert_eq!(hidden.len(), preset.panes().len() - 1, "{pane:?} at 80x24");
                assert!(!hidden.contains(&pane), "the focused pane is drawn");
            }
        }
    }

    #[test]
    fn only_the_focused_pane_of_invest_and_orders_carries_the_marker() {
        let mut app = stocked();
        app.mf_holdings = Some(vec![{
            let mut row = crate::tui::data::test_models::mf();
            row.tradingsymbol = "FUND A".into();
            row
        }]);
        app.set_workspace(Preset::Invest);
        let markers = |text: &str| text.matches("┌ > ").count();

        let text = screen(&app, 160, 45);
        assert_eq!(markers(&text), 1, "{text}");
        assert!(text.contains("┌ > MF"), "{text}");

        app.focus_pane_for_test(PaneId::InvestBids);
        let text = screen(&app, 160, 45);
        assert_eq!(markers(&text), 1, "{text}");
        assert!(!text.contains("┌ > MF"), "{text}");

        app.gtts = Some(vec![{
            let mut row = crate::tui::data::test_models::gtt();
            row.id = 7;
            row.trigger_type = "single".into();
            row.status = "active".into();
            row.condition.exchange = "NSE".into();
            row.condition.tradingsymbol = "INFY".into();
            row
        }]);
        app.set_workspace(Preset::Orders);
        app.focus_pane_for_test(PaneId::OrdersGtt);
        let text = screen(&app, 160, 45);
        assert_eq!(markers(&text), 1, "{text}");
        assert!(text.contains("┌ > GTT Triggers"), "{text}");
    }

    #[test]
    fn a_collapsed_composite_counts_its_hidden_panes_in_the_title() {
        let mut app = stocked();
        app.set_workspace(Preset::Trading);
        let mut terminal = Terminal::new(TestBackend::new(100, 24)).expect("test terminal");
        terminal
            .draw(|frame| ui::draw(frame, &app))
            .expect("frame draws");
        let buffer = terminal.backend().buffer().clone();
        let title: String = (0..100).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(
            title.contains("+3 hidden"),
            "a 100-column body draws one of Trading's four panes: {title:?}"
        );
        assert_eq!(app.hit.borrow().hidden.len(), 3);

        let mut terminal = Terminal::new(TestBackend::new(160, 45)).expect("test terminal");
        terminal
            .draw(|frame| ui::draw(frame, &app))
            .expect("frame draws");
        let buffer = terminal.backend().buffer().clone();
        let title: String = (0..160).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(!title.contains("hidden"), "{title:?}");
        assert_eq!(app.hit.borrow().hidden.len(), 0);
    }

    #[test]
    fn trading_links_follow_its_watch_and_derivatives_follows_the_underlying() {
        let mut app = stocked();
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingWatch);
        app.move_selection(1);

        let linked = app
            .link_target(PaneId::TradingWatch)
            .expect("the watch row resolves");
        assert_eq!(linked.tradingsymbol, "RELIANCE");
        assert_eq!(
            app.chart_of(PaneId::TradingChart)
                .and_then(|chart| chart.target())
                .map(|target| target.tradingsymbol.clone()),
            Some("RELIANCE".to_string()),
            "the chart followed the watch row"
        );

        app.focus_pane_for_test(PaneId::TradingOrders);
        app.move_selection(1);
        assert_eq!(
            app.chart_of(PaneId::TradingChart)
                .and_then(|chart| chart.target())
                .map(|target| target.tradingsymbol.clone()),
            Some("RELIANCE".to_string()),
            "an Orders move does not retarget the linked chart"
        );
        assert_eq!(
            app.link_target(PaneId::TradingWatch)
                .map(|target| target.tradingsymbol),
            Some("RELIANCE".to_string()),
            "and the depth ladder reads the same source"
        );

        assert_eq!(
            app.chart_of(PaneId::DerivativesChart)
                .map(crate::tui::pane_state::ChartPane::binding),
            Some(crate::tui::pane_state::ChartBinding::UnderlyingOf(
                PaneId::DerivativesChain
            ))
        );
    }
}
