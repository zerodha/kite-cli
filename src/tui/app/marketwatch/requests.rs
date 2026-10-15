use super::super::App;
use crate::tui::marketwatch;
use tokio::task::JoinHandle;

impl App {
    pub fn start_marketwatch(&mut self) -> Vec<JoinHandle<()>> {
        let Ok(client) = self.watch.client() else {
            return Vec::new();
        };
        let generation = self.watch.begin_catalog();
        let mut tasks = vec![marketwatch::spawn_catalog(
            client.clone(),
            self.events_tx.clone(),
            generation,
        )];

        if let Some(watch) = self.watch_pane.watch().cloned() {
            let generation = self.watch.open(&watch);
            tasks.push(marketwatch::spawn_detail(
                client,
                self.events_tx.clone(),
                watch,
                generation,
            ));
        }
        tasks
    }

    pub(in crate::tui::app) fn reload_catalog(&mut self) {
        if let Ok(client) = self.watch.client() {
            let generation = self.watch.begin_catalog();
            marketwatch::spawn_catalog(client, self.events_tx.clone(), generation);
        }
    }

    pub(in crate::tui::app) fn reload_watch_detail(&mut self) -> bool {
        let Some(watch) = self.watch_pane.watch().cloned() else {
            return false;
        };
        self.reload_detail(watch)
    }

    pub(in crate::tui::app) fn reload_detail(&mut self, watch: marketwatch::WatchId) -> bool {
        let Ok(client) = self.watch.client() else {
            return false;
        };
        let Some(generation) = self.watch.refresh(&watch) else {
            return false;
        };
        marketwatch::spawn_detail(client, self.events_tx.clone(), watch, generation);
        true
    }

    pub(in crate::tui::app) fn switch_watch(&mut self, watch: marketwatch::WatchId) {
        let generation = self.watch.open(&watch);
        self.watch_pane.open(watch.clone());
        self.trading_chart.set_cursor(None);
        self.sync_subscriptions();
        let Ok(client) = self.watch.client() else {
            return;
        };
        marketwatch::spawn_detail(client, self.events_tx.clone(), watch, generation);
        if let Some(path) = self.settings_file()
            && let Err(err) = self.watch.remember(&path, self.watch_pane.watch())
        {
            self.toast_error(format!("could not remember the open watchlist: {err}"));
        }
    }
}
