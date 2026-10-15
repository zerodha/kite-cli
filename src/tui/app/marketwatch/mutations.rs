use super::super::{App, Modal, PaneId, ToastLevel};
use crate::tui::instruments::Instrument;
use crate::tui::marketwatch::{self, Follow, Mutation, WatchItem};
use crate::tui::palette::WatchlistCmd;
use crate::tui::ticket::{ConfirmState, PendingAction};

pub(in crate::tui::app) enum Origin {
    Pane(PaneId, Option<Follow>),

    Command,

    Queued,
}

impl App {
    pub(in crate::tui::app) fn add_watch_item(&mut self, pane: PaneId, inst: &Instrument) {
        let Some(instance) = self.watch_instance(pane) else {
            return;
        };
        let watch = instance.watch().cloned();
        let index = instance.display_row(&self.watch);

        if self
            .watch_rows_of(pane)
            .iter()
            .any(|item| item.instrument_token == inst.instrument_token)
        {
            self.toast(
                ToastLevel::Info,
                format!("{} already on watchlist", inst.label()),
            );
            return;
        }
        if let Some(rows) = self.watch.ephemeral_rows_mut() {
            rows.push(WatchItem {
                exchange: inst.exchange.clone(),
                tradingsymbol: inst.tradingsymbol.clone(),
                instrument_token: inst.instrument_token,
                segment: inst.segment.clone(),
                ..WatchItem::default()
            });
            let last = self.watch_rows_of(pane).len() - 1;
            if let Some((instance, store)) = self.watch_instance_mut(pane) {
                instance.select(last, store);
            }
            self.trading_chart.set_cursor(None);
            self.sync_subscriptions();
            self.toast(
                ToastLevel::Success,
                format!("added {} to the temporary list (not saved)", inst.label()),
            );
            return;
        }
        match self.watch.add_mutation(
            watch.as_ref(),
            index,
            &inst.exchange,
            &inst.tradingsymbol,
            inst.label(),
        ) {
            Ok(mutation) => self.send_mutation(
                mutation,
                Origin::Pane(
                    pane,
                    Some(Follow::Listing {
                        exchange: inst.exchange.clone(),
                        tradingsymbol: inst.tradingsymbol.clone(),
                    }),
                ),
            ),
            Err(err) => self.toast_error(err),
        }
    }

    pub(in crate::tui::app) fn remove_watch_row(&mut self, pane: PaneId) {
        let Some(instance) = self.watch_instance(pane) else {
            return;
        };
        let watch = instance.watch().cloned();
        let Some(index) = instance.target(&self.watch) else {
            return;
        };
        if let Some(rows) = self.watch.ephemeral_rows_mut() {
            if index >= rows.len() {
                return;
            }
            let removed = rows.remove(index);
            self.trading_chart.set_cursor(None);
            self.reconcile_watch();
            self.sync_subscriptions();
            self.toast(
                ToastLevel::Info,
                format!(
                    "removed {} from the temporary list (not saved)",
                    removed.label()
                ),
            );
            return;
        }
        let Some(row) = self.watch_rows_of(pane).get(index).cloned() else {
            return;
        };
        if self.auth_expired() {
            return;
        }

        match self.watch.writable(watch.as_ref()) {
            Ok(watch) => {
                let mutation = Mutation::Remove {
                    watch: watch.clone(),
                    item: row.id.clone(),
                    label: row.label(),
                };
                self.send_mutation(mutation, Origin::Pane(pane, None));
            }
            Err(err) => self.toast_error(err),
        }
    }

    pub(in crate::tui::app) fn select_or_add_watch(&mut self, pane: PaneId, inst: &Instrument) {
        match self
            .watch_rows_of(pane)
            .iter()
            .position(|item| item.instrument_token == inst.instrument_token)
        {
            Some(idx) => {
                if let Some((instance, store)) = self.watch_instance_mut(pane)
                    && instance.select(idx, store)
                {
                    self.trading_chart.set_cursor(None);
                }
                self.sync_subscriptions();
            }
            None => self.add_watch_item(pane, inst),
        }
    }

    pub(in crate::tui::app) fn move_watch_row(&mut self, pane: PaneId, delta: isize) {
        let Some(instance) = self.watch_instance(pane) else {
            return;
        };
        let watch = instance.watch().cloned();
        let Some(index) = instance.target(&self.watch) else {
            return;
        };
        if let Some(rows) = self.watch.ephemeral_rows_mut() {
            if let Some(target) = marketwatch::move_row(rows, index, delta)
                && let Some((instance, store)) = self.watch_instance_mut(pane)
            {
                instance.select(target, store);
                self.sync_subscriptions();
            }
            return;
        }
        let Some(item) = self
            .watch_rows_of(pane)
            .get(index)
            .map(|row| row.id.clone())
        else {
            return;
        };
        match self.watch.reorder_row(watch.as_ref(), index, delta) {
            Ok(Some(moved)) => {
                if let Some((instance, store)) = self.watch_instance_mut(pane) {
                    instance.select(moved.index, store);

                    if moved.mutation.is_none() {
                        instance.expect(Some(Follow::Item(item.clone())));
                    }
                }
                if let Some(mutation) = moved.mutation {
                    self.send_mutation(mutation, Origin::Pane(pane, Some(Follow::Item(item))));
                }
            }
            Ok(None) => {}
            Err(err) => self.toast_error(err),
        }
    }

    pub(in crate::tui::app) fn send_mutation(&mut self, mutation: Mutation, origin: Origin) {
        if let Some(pending) = self.watch.pending() {
            self.toast_error(format!("still {pending}, wait for it to finish"));
            return;
        }
        let client = match self.watch.client() {
            Ok(client) => client,
            Err(err) => {
                self.toast_error(err);
                return;
            }
        };
        match origin {
            Origin::Pane(pane, follow) => {
                if let Some((instance, _)) = self.watch_instance_mut(pane) {
                    instance.expect(follow);
                }
            }
            Origin::Command => {
                if self.watch_pane.watch().is_some() && self.watch_pane.watch() == mutation.watch()
                {
                    self.watch_pane.expect(None);
                }
            }
            Origin::Queued => {}
        }
        self.watch.begin(mutation.clone());
        marketwatch::spawn_mutation(client, self.events_tx.clone(), mutation);
    }

    pub(in crate::tui::app) fn cycle_watchlist(&mut self, delta: isize) {
        if let Some(watch) = self.watch.cycle(self.watch_pane.watch(), delta) {
            self.switch_watch(watch);
            self.toast(
                ToastLevel::Info,
                format!("watchlist: {}", self.watch_view().title()),
            );
        }
    }

    pub(in crate::tui::app) fn exec_watchlist(&mut self, cmd: WatchlistCmd) {
        if self.watch.is_ephemeral() {
            self.toast_error("a --token watchlist is not saved to your account".to_string());
            return;
        }
        match cmd {
            WatchlistCmd::Switch(name) => match self.watch.resolve(&name) {
                Ok(Some(watch)) => {
                    self.switch_watch(watch);
                    self.show_pane(PaneId::TradingWatch);
                    self.toast(
                        ToastLevel::Info,
                        format!("watchlist: {}", self.watch_view().title()),
                    );
                }
                Ok(None) => {
                    let weight = self.watch.create_weight();
                    self.send_mutation(Mutation::Create { name, weight }, Origin::Command);
                    self.show_pane(PaneId::TradingWatch);
                }
                Err(err) => self.toast_error(err),
            },
            WatchlistCmd::SwitchId { id, name } => {
                self.switch_watch(marketwatch::WatchId::from(id.as_str()));
                self.show_pane(PaneId::TradingWatch);
                self.toast(ToastLevel::Info, format!("watchlist: {name}"));
            }
            WatchlistCmd::Rename(name) => {
                if name == self.watch_view().title() {
                    return;
                }
                match self.watch.writable(self.watch_pane.watch()) {
                    Ok(watch) => {
                        let mutation = Mutation::Rename {
                            watch: watch.clone(),
                            name,
                        };
                        self.send_mutation(mutation, Origin::Command);
                    }
                    Err(err) => self.toast_error(err),
                }
            }

            WatchlistCmd::Delete if self.trading_disabled() => {}
            WatchlistCmd::Delete => match self.watch.writable(self.watch_pane.watch()) {
                Ok(watch) => {
                    let watch = watch.clone();
                    let name = self.watch_view().title().to_owned();
                    let rows = self.watch_items().len();
                    self.modals
                        .push(Modal::Confirm(Box::new(ConfirmState::plain(
                            "Delete watchlist".to_string(),
                            vec![
                                ("Watchlist".to_string(), name.clone()),
                                ("Instruments".to_string(), rows.to_string()),
                            ],
                            PendingAction::DeleteWatchlist {
                                watch: watch.to_string(),
                                name,
                            },
                        ))));
                }
                Err(err) => self.toast_error(err),
            },
        }
    }
}
