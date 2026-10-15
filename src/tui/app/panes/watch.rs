use super::super::{App, Modal, PaneId, SearchState, Side, ToastLevel, marketwatch};
use crate::tui::marketwatch::WatchItem;
use crate::tui::pane_state;
use crossterm::event::{KeyCode, KeyEvent};
use std::cell::Cell;

impl App {
    pub fn watch_items(&self) -> &[WatchItem] {
        self.watch_pane.rows(&self.watch)
    }

    pub fn watch_view(&self) -> marketwatch::Watch<'_, '_> {
        self.watch.watch(self.watch_pane.watch())
    }

    pub fn watch_rows_of(&self, pane: PaneId) -> &[WatchItem] {
        self.watch_instance(pane)
            .map(|instance| instance.rows(&self.watch))
            .unwrap_or_default()
    }

    pub fn watch_view_of(&self, pane: PaneId) -> marketwatch::Watch<'_, '_> {
        self.watch
            .watch(self.watch_instance(pane).and_then(|pane| pane.watch()))
    }

    pub fn watch_viewport_of(&self, pane: PaneId) -> &Cell<usize> {
        self.watch_instance(pane)
            .unwrap_or(&self.watch_pane)
            .viewport()
    }

    pub(in crate::tui::app) fn watch_instance(
        &self,
        pane: PaneId,
    ) -> Option<&pane_state::WatchPane> {
        (pane == PaneId::TradingWatch).then_some(&self.watch_pane)
    }

    pub(in crate::tui::app) fn watch_instance_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<(&mut pane_state::WatchPane, &marketwatch::State)> {
        (pane == PaneId::TradingWatch).then_some((&mut self.watch_pane, &self.watch))
    }

    pub(in crate::tui::app) fn key_watch(&mut self, pane: PaneId, key: KeyEvent) {
        let selected = || -> Option<WatchItem> {
            self.watch_instance(pane)
                .and_then(|instance| instance.selected(&self.watch))
                .cloned()
        };
        match key.code {
            KeyCode::Char('/') => {
                self.modals
                    .push(Modal::Search(SearchState::from_pane(pane)));
            }
            KeyCode::Char(']') => self.cycle_watchlist(1),
            KeyCode::Char('[') => self.cycle_watchlist(-1),
            KeyCode::Char('w') => self.open_watchlist_manager(),

            KeyCode::Char('J') => self.move_watch_row(pane, 1),
            KeyCode::Char('K') => self.move_watch_row(pane, -1),
            KeyCode::Char('d') => self.remove_watch_row(pane),
            KeyCode::Char('b' | 's') => {
                let Some(item) = selected() else {
                    return;
                };
                if item.is_index() {
                    self.toast(ToastLevel::Error, "indices are not tradable");
                    return;
                }
                let side = if key.code == KeyCode::Char('b') {
                    Side::Buy
                } else {
                    Side::Sell
                };
                self.open_ticket(
                    side,
                    item.exchange,
                    item.tradingsymbol,
                    item.instrument_token,
                    0,
                    None,
                );
            }
            KeyCode::Char('g') => {
                let Some(item) = selected() else {
                    return;
                };
                if item.is_index() {
                    self.toast(ToastLevel::Error, "indices are not tradable");
                    return;
                }
                self.open_gtt_ticket(
                    Side::Buy,
                    item.exchange,
                    item.tradingsymbol,
                    item.instrument_token,
                    0,
                    None,
                    None,
                    None,
                    None,
                );
            }

            KeyCode::Char('a') => {
                if let Some(item) = selected() {
                    self.open_alert_form(
                        &item.exchange,
                        &item.tradingsymbol,
                        item.instrument_token,
                        &item.segment,
                    );
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    pub fn set_watch_for_test(&mut self, watch: marketwatch::State) {
        match watch.remembered().cloned() {
            Some(id) => self.watch_pane.open(id),
            None => self.watch_pane.close(),
        }
        self.watch = watch;
        self.reconcile_watch();
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{App, Modal};
    use crate::tui::marketwatch::{State as WatchState, WatchItem};
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::{PaneId, Preset};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use kiteconnect::KiteConnect;

    fn app(watch: WatchState) -> App {
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("token").unwrap())
            .build()
            .expect("dummy client builds");
        App::new(
            client,
            events_tx,
            watch,
            Pad::default(),
            true,
            &Settings::default(),
        )
    }

    #[test]
    fn help_bottom_navigation_uses_the_rendered_maximum() {
        let mut app = app(WatchState::test_loaded(Vec::new()));
        app.set_pane(PaneId::TradingWatch);

        app.help_max_scroll.set(5);
        app.modals.push(Modal::Help(0));
        app.on_key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE));
        assert!(matches!(app.modals.last(), Some(Modal::Help(5))));
        app.on_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert!(matches!(app.modals.last(), Some(Modal::Help(4))));
    }

    #[tokio::test]
    async fn z_maximizes_the_watch_pane_and_a_row_move_drops_the_chart_scan() {
        let mut app = app(WatchState::test_loaded(vec![
            WatchItem {
                exchange: "NSE".into(),
                tradingsymbol: "INFY".into(),
                instrument_token: 408_065,
                ..WatchItem::default()
            },
            WatchItem {
                exchange: "NSE".into(),
                tradingsymbol: "RELIANCE".into(),
                instrument_token: 738_561,
                ..WatchItem::default()
            },
        ]));
        app.set_workspace(Preset::Trading);
        app.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert_eq!(
            app.workspaces.maximized(Preset::Trading),
            Some(PaneId::TradingWatch)
        );
        app.on_key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert_eq!(app.workspaces.maximized(Preset::Trading), None);

        app.trading_chart.set_cursor(Some(50));
        app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.trading_chart.cursor(), None);
    }
}
