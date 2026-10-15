use super::super::{App, ExecItem, MenuAction, Modal, PaneId, SearchState, ToastLevel};
use crate::tui::palette::Jump;
use crate::tui::settings;
use crate::tui::ticket::{ConfirmState, PendingAction};
use kiteconnect::OrderType;

impl App {
    pub(in crate::tui::app) fn exec_palette(&mut self, item: ExecItem) {
        match item {
            ExecItem::Quit => self.should_quit = true,
            ExecItem::Jump(Jump::Workspace(preset)) => self.switch_workspace(preset),
            ExecItem::Jump(Jump::Pane(pane)) => self.show_pane(pane),
            ExecItem::FocusPane(pane) => self.focus_or_zoom(pane),
            ExecItem::Watchlist(cmd) => self.exec_watchlist(cmd),
            ExecItem::Theme(name) => self.apply_theme(&name),
            ExecItem::Action(action) => self.exec_menu_action(action),
            ExecItem::Instrument(inst) => {
                self.select_or_add_watch(PaneId::TradingWatch, &inst);
                self.show_pane(PaneId::TradingWatch);
                self.ensure_candles(true);
            }
            ExecItem::Alert(inst) => self.open_alert_form(
                &inst.exchange,
                &inst.tradingsymbol,
                inst.instrument_token,
                &inst.segment,
            ),
            ExecItem::Order {
                side,
                instrument,
                qty,
                price,
            } => {
                if instrument.is_index() {
                    self.toast(ToastLevel::Error, "indices are not tradable");
                    return;
                }

                let token = instrument.instrument_token;
                if !self
                    .watch_items()
                    .iter()
                    .any(|item| item.instrument_token == token)
                {
                    self.add_watch_item(PaneId::TradingWatch, &instrument);
                }
                self.open_ticket(
                    side,
                    instrument.exchange.clone(),
                    instrument.tradingsymbol.clone(),
                    token,
                    qty.map_or(0, i64::from),
                    None,
                );

                if let Some(price) = price
                    && let Some(Modal::Ticket(ticket)) = self.modals.last_mut()
                {
                    ticket.price = format!("{price}");
                    ticket.order_type = OrderType::Limit;
                }
            }
        }
    }

    pub(in crate::tui::app) fn apply_theme(&mut self, name: &str) {
        let Some(theme) = settings::preset(name) else {
            let names: Vec<&str> = settings::preset_names().collect();
            self.toast(
                ToastLevel::Error,
                format!("unknown theme: {name} (try {})", names.join(", ")),
            );
            return;
        };

        let canonical = settings::preset_names()
            .find(|candidate| settings::preset(candidate) == Some(theme))
            .unwrap_or(name);
        settings::set_theme(theme);
        if let Some(path) = self.settings_file()
            && let Err(err) = settings::save_theme_preset(&path, canonical)
        {
            self.toast(
                ToastLevel::Error,
                format!("theme applied but not saved: {err}"),
            );
            return;
        }
        self.toast(ToastLevel::Success, format!("theme: {canonical}"));
    }

    pub(in crate::tui::app) fn exec_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::ToggleChart => self.toggle_chart_panel(self.palette_origin()),
            MenuAction::CycleInterval => self.cycle_interval(self.palette_origin()),
            MenuAction::ToggleVolume => self.toggle_chart_volume(self.palette_origin()),
            MenuAction::CycleIndicator => self.cycle_chart_indicator(self.palette_origin()),
            MenuAction::CycleMoversUniverse => self.cycle_movers_universe(),
            MenuAction::NewAlert => self.open_new_alert_search(),
            MenuAction::AlertHistory => {
                if let Some(alert) = self.selected_alert().cloned() {
                    self.open_alert_history(&alert);
                }
            }
            MenuAction::NewSearch => {
                let origin = self.palette_origin();
                self.modals
                    .push(Modal::Search(SearchState::from_pane(origin)));
            }
            MenuAction::ManageWatchlists => self.open_watchlist_manager(),
            MenuAction::SwitchTheme => self.open_theme_picker(),
            MenuAction::RefreshAll => self.refresh_active(),
            MenuAction::Help => self.modals.push(Modal::Help(0)),
            MenuAction::Notes => self.open_notes(),
            MenuAction::SettingsPath => {
                let path = settings::config_file_path().map_or_else(
                    || "no config directory".to_string(),
                    |path| path.display().to_string(),
                );
                self.toast(ToastLevel::Info, format!("settings: {path}"));
            }
            MenuAction::Logout => self
                .modals
                .push(Modal::Confirm(Box::new(ConfirmState::plain(
                    "LOGOUT".to_string(),
                    vec![(
                        "Session".to_string(),
                        "clear saved Kite credentials and exit".to_string(),
                    )],
                    PendingAction::Logout,
                )))),
            MenuAction::Quit => self.should_quit = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::keys::tests::trading_chart_focused;
    use super::super::super::test_support::{H, W, render, test_app};
    use super::super::super::{
        ExecItem, KeyCode, KeyEvent, KeyModifiers, MenuAction, Modal, SearchPurpose,
    };
    use super::super::tests::{app_with_master, open, press, state, tabbed};
    use crate::tui::workspace::PaneId;

    #[tokio::test]
    async fn palette_chart_rows_act_on_the_chart_they_were_opened_from() {
        let mut app = trading_chart_focused();
        let hidden = (app.chart_pane.interval(), app.chart_pane.volume());
        let leaf = (app.trading_chart.interval(), app.trading_chart.volume());

        for row in ["Cycle chart interval", "Toggle chart volume"] {
            open(&mut app, row);
            press(&mut app, KeyCode::Enter);
            assert!(app.modals.is_empty(), "{row} ran");
        }
        assert_ne!(app.trading_chart.interval(), leaf.0);
        assert_ne!(app.trading_chart.volume(), leaf.1);
        assert_eq!(
            (app.chart_pane.interval(), app.chart_pane.volume()),
            hidden,
            "the hidden Watch chart is untouched"
        );
    }

    #[test]
    fn the_new_alert_row_dispatches_the_picker_off_the_alerts_view() {
        let mut app = app_with_master();
        app.set_pane(PaneId::PortfolioPositions);
        open(&mut app, "New alert");
        assert!(matches!(
            state(&app).selected_item(),
            Some(ExecItem::Action(MenuAction::NewAlert))
        ));
        assert_eq!(state(&app).enter_effect(), Some("runs"));

        press(&mut app, KeyCode::Enter);
        let Some(Modal::Search(search)) = app.modals.last() else {
            panic!(
                "the row opens the instrument picker: {:?}",
                app.modals.last()
            );
        };
        assert_eq!(search.purpose, SearchPurpose::NewAlert);
        assert_eq!(
            app.focused_pane(),
            PaneId::PortfolioPositions,
            "the row switched no view"
        );

        let mut app = app_with_master();
        app.set_pane(PaneId::PortfolioPositions);
        assert_eq!(tabbed(&mut app, "New alert"), "alert ");
        assert!(matches!(
            state(&app).selected_item(),
            Some(ExecItem::Action(MenuAction::NewAlert))
        ));
    }

    #[test]
    fn a_non_tradable_index_is_still_refused_from_the_palette() {
        let mut app = app_with_master();

        open(&mut app, "buy nse:nifty 50");
        assert!(
            matches!(state(&app).selected_item(), Some(ExecItem::Order { .. })),
            "the index resolves as an order target: {:?}",
            state(&app).results
        );

        assert!(
            !state(&app).can_complete(),
            "a whitespace identity must not be written into an order"
        );

        press(&mut app, KeyCode::Enter);
        assert!(
            !matches!(app.modals.last(), Some(Modal::Ticket(_))),
            "indices are not tradable"
        );
    }

    #[test]
    fn logout_menu_requires_confirmation_then_sets_session_exit_flag() {
        let mut app = test_app();
        app.exec_menu_action(MenuAction::Logout);
        render(&app, W, H);
        let Some(Modal::Confirm(confirm)) = app.modals.pop() else {
            panic!("logout must open a confirmation modal");
        };
        assert!(!app.should_logout);
        app.key_confirm(
            KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE),
            confirm,
        );
        assert!(app.should_logout);
        assert!(!app.pending_action);

        assert!(app.modals.is_empty());
    }
    #[tokio::test]
    async fn simplification_regression_index_segments_are_case_insensitive() {
        let mut app = app_with_master();
        let mut instrument = app
            .instruments
            .as_ref()
            .unwrap()
            .by_token(256_265)
            .unwrap()
            .clone();
        instrument.segment = "indices".to_string();
        app.exec_palette(ExecItem::Order {
            side: super::super::super::Side::Buy,
            instrument,
            qty: None,
            price: None,
        });
        assert!(app.modals.is_empty());
        assert_eq!(
            app.toasts().back().unwrap().text,
            "indices are not tradable"
        );
    }
}
