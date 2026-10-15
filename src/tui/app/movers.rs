use super::{App, ToastLevel};
use crate::tui::data;
use crate::tui::settings;
use crate::tui::workspace::Preset;
use tokio::task::JoinHandle;

impl App {
    pub fn ensure_movers(&mut self) -> Option<JoinHandle<()>> {
        if self.workspace != Preset::Overview || self.movers.is_some() || self.movers_inflight {
            return None;
        }
        Some(self.refresh_movers())
    }

    pub(in crate::tui::app) fn refresh_movers(&mut self) -> JoinHandle<()> {
        self.movers_inflight = true;
        data::spawn_refresh_movers(
            self.client.clone(),
            self.events_tx.clone(),
            self.movers_universe,
            self.mover_presets.clone(),
        )
    }

    fn next_movers_universe(&mut self) {
        self.movers_universe = self.movers_universe.cycle();
        self.movers = None;

        self.movers_inflight = false;

        self.movers_failed = None;
    }

    pub(in crate::tui::app) fn cycle_movers_universe(&mut self) {
        self.next_movers_universe();
        let universe = self.movers_universe;
        if let Some(path) = self.settings_file()
            && let Err(err) = settings::save_movers_universe(&path, universe)
        {
            self.toast(
                ToastLevel::Error,
                format!("movers universe applied but not saved: {err}"),
            );
        }
        self.toast(ToastLevel::Info, format!("movers: {}", universe.label()));
        self.refresh_movers();
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::test_app;
    use crate::tui::data::{DataEvent, Movers, MoversUniverse, ScreenerRow};
    use crate::tui::workspace::PaneId;
    use crate::tui::workspace::Preset;

    #[tokio::test]
    async fn entering_the_overview_fetches_the_movers() {
        let mut app = test_app();
        app.show_pane(PaneId::OrdersBook);
        assert!(app.movers.is_none());
        assert!(!app.movers_inflight);

        app.switch_workspace(Preset::Overview);
        assert!(app.movers_inflight, "entering the Overview fetches");
    }

    fn movers(symbol: &str) -> Movers {
        Movers {
            gainers: vec![ScreenerRow {
                tradingsymbol: symbol.into(),
                last_price: 100.0,
                change_percent: 5.0,
                volume: 1000.0,
            }],
            losers: Vec::new(),
        }
    }

    #[tokio::test]
    async fn cycling_the_movers_universe_invalidates_the_old_rows() {
        let mut app = test_app();
        let path = std::env::temp_dir().join(format!("kite-movers-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&path);
        app.settings_path = Some(path.clone());
        assert_eq!(app.movers_universe, MoversUniverse::Nifty50);
        app.movers = Some(movers("LGEINDIA"));

        app.cycle_movers_universe();
        let saved: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).expect("read settings")).unwrap();
        assert_eq!(saved["movers_universe"], "nifty100");
        std::fs::remove_file(path).expect("remove scratch settings");
        assert_eq!(app.movers_universe, MoversUniverse::Nifty100);
        assert!(
            app.movers.is_none(),
            "rows for the previous universe must not survive the cycle"
        );
    }

    #[test]
    fn movers_reply_for_a_stale_universe_is_ignored() {
        let mut app = test_app();
        app.next_movers_universe();

        app.movers_inflight = true;
        app.on_data(DataEvent::Movers {
            universe: MoversUniverse::Nifty50,
            result: Ok(Box::new(movers("STALE"))),
        });
        assert!(
            app.movers.is_none(),
            "a reply for the abandoned universe must be dropped"
        );
        assert!(
            app.movers_inflight,
            "a stale reply must not release the interlock held by the current fetch"
        );

        app.on_data(DataEvent::Movers {
            universe: MoversUniverse::Nifty100,
            result: Ok(Box::new(movers("FRESH"))),
        });
        assert_eq!(
            app.movers
                .as_ref()
                .map(|m| m.gainers[0].tradingsymbol.as_str()),
            Some("FRESH")
        );
    }

    #[test]
    fn movers_failure_releases_the_inflight_interlock() {
        let mut app = test_app();
        app.movers_inflight = true;
        app.on_data(DataEvent::Movers {
            universe: app.movers_universe,
            result: Err("kite API error InputException (HTTP 400): bad query".to_string()),
        });
        assert!(!app.movers_inflight);
        assert!(app.movers.is_none());

        assert_eq!(
            app.movers_failed.as_deref(),
            Some("kite API error InputException (HTTP 400): bad query"),
            "the failure, and why, must outlive the toast"
        );
        assert!(
            app.visible_toast()
                .is_some_and(|t| t.text.contains("bad query")),
            "the screener failure must reach the user"
        );

        assert!(app.ws_fatal.is_none());
    }

    #[test]
    fn movers_failure_for_a_stale_universe_is_ignored() {
        let mut app = test_app();
        app.next_movers_universe();
        app.movers_inflight = true;
        app.on_data(DataEvent::Movers {
            universe: MoversUniverse::Nifty500,
            result: Err("kite API error InputException (HTTP 400): bad query".to_string()),
        });
        assert!(
            app.movers_inflight,
            "the current fetch must keep its interlock"
        );
        assert!(
            app.movers_failed.is_none(),
            "the panel must not report a stale failure"
        );
        assert!(
            app.visible_toast().is_none(),
            "no toast for an abandoned universe"
        );
    }

    #[test]
    fn cycling_the_universe_clears_a_previous_failure() {
        let mut app = test_app();
        app.movers_failed = Some("boom".to_string());
        app.next_movers_universe();
        assert!(app.movers_failed.is_none());
    }
}
