mod mutations;
mod requests;

pub(in crate::tui::app) use mutations::Origin;

pub(in crate::tui::app) use crate::tui::marketwatch::{State, Watch, WatchId};

use super::{App, ToastLevel};
use crate::tui::marketwatch::{self, Mutation};

impl App {
    pub(in crate::tui::app) fn on_marketwatch(&mut self, event: marketwatch::Event) {
        match event {
            marketwatch::Event::Catalog { generation, result } => {
                if !self.watch.accepts_catalog(generation) {
                    return;
                }
                if let Err(err) = &result {
                    self.handle_data_error(format!("marketwatch: {err}"));
                }
                self.watch.set_catalog(result);
                match self.watch.reselect(self.watch_pane.watch()) {
                    marketwatch::Reselect::Keep => {}
                    marketwatch::Reselect::Open(watch) => self.switch_watch(watch),
                    marketwatch::Reselect::Nothing => self.watch_pane.close(),
                }
                self.reconcile_watch();
            }
            marketwatch::Event::Detail {
                watch,
                generation,
                result,
            } => {
                if let Err(err) = &result {
                    self.handle_data_error(format!("marketwatch: {err}"));
                }

                if !self.watch.set_detail(&watch, generation, result) {
                    return;
                }
                if let Some(store) = self.instruments.clone() {
                    self.watch.enrich(&store);
                }

                if self.watch_pane.watch() == Some(&watch)
                    && self.watch_pane.follow_confirmed(&self.watch)
                {
                    self.trading_chart.set_cursor(None);
                }
                self.reconcile_watch();
                self.sync_subscriptions();
            }
            marketwatch::Event::Mutation { mutation, result } => {
                self.watch.finish();

                if matches!(*mutation, Mutation::Delete { .. }) {
                    self.pending_action = false;
                }
                let open = self.watch_pane.watch();
                let affects_open = open.is_some() && open == mutation.watch();
                let created = match result {
                    Ok(created) => {
                        self.toast(ToastLevel::Success, mutation.done());
                        created
                    }
                    Err(err) => {
                        self.toast(ToastLevel::Error, format!("{}: {err}", mutation.verb()));
                        if self.watch.restore_confirmed(mutation.watch()) {
                            self.reconcile_watch();
                            if self.watch_pane.watch() == mutation.watch() {
                                self.watch_pane.follow_restored(&self.watch);
                            }
                        }

                        if mutation.refetches_detail()
                            && affects_open
                            && let Some(watch) = mutation.watch().cloned()
                        {
                            self.reload_detail(watch);
                        }
                        self.reload_catalog();

                        if let Some(queued) = self.watch.take_queued_reorder(None) {
                            self.send_mutation(queued, Origin::Queued);
                        }
                        return;
                    }
                };

                let queued = self.watch.take_queued_reorder(mutation.watch());
                let follows_up = queued
                    .as_ref()
                    .is_some_and(|queued| queued.watch() == mutation.watch());
                if !follows_up
                    && mutation.refetches_detail()
                    && affects_open
                    && let Some(watch) = mutation.watch().cloned()
                {
                    self.reload_detail(watch);
                }
                if let Some(queued) = queued {
                    self.send_mutation(queued, Origin::Queued);
                }
                if let Some(id) = created {
                    self.switch_watch(id);
                }
                if mutation.changes_catalog() {
                    self.reload_catalog();
                }
            }
        }
    }
}

#[cfg(test)]
mod marketwatch_tests {
    use super::super::test_support::{master_inst, screen};
    use super::super::{App, MenuAction, Modal};
    use crate::tui::data::DataEvent;
    use crate::tui::event::AppEvent;
    use crate::tui::instruments::{Instrument, InstrumentStore};
    use crate::tui::marketwatch::{
        Catalog, Detail, Event, Mutation, State as WatchState, WatchId, WatchItem,
    };
    use crate::tui::notes::Pad;
    use crate::tui::palette::WatchlistCmd;
    use crate::tui::settings::Settings;
    use crate::tui::ticket::PendingAction;
    use crate::tui::workspace::{PaneId, Preset};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use httpmock::prelude::*;
    use kiteconnect::KiteConnect;

    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    fn watch_app(server: &MockServer) -> (App, mpsc::Receiver<AppEvent>) {
        let (events_tx, events_rx) = mpsc::channel(64);
        let client = KiteConnect::builder("key")
            .access_token(kiteconnect::AccessToken::new("token").expect("credentials"))
            .base_url(server.base_url())
            .build()
            .expect("client");
        let mut app = App::new(
            client.clone(),
            events_tx,
            WatchState::remote(Some(client), String::new(), None),
            Pad::default(),
            true,
            &Settings::default(),
        );
        app.set_pane(PaneId::TradingWatch);

        app.circuits.insert(408_065, None);
        app.circuits.insert(2_953_217, None);
        (app, events_rx)
    }

    fn catalog_body() -> serde_json::Value {
        json!([
            {"name": "Favorites", "items": [{"id": "W1", "name": "Core", "is_static": false}]},
            {"name": "Others", "items": [{"id": "W2", "name": "Swing", "is_static": false}]}
        ])
    }

    fn detail_body(rows: serde_json::Value) -> serde_json::Value {
        let mut body = json!({
            "id": "W1", "name": "Core", "is_static": false,
            "groups": [{"id": "GROUP:default", "name": "Default", "weight": 1, "items": []}]
        });
        body["groups"][0]["items"] = rows;
        body
    }

    fn row(id: &str, tradingsymbol: &str) -> serde_json::Value {
        json!({"id": id, "weight": 1.0, "tradingsymbol": tradingsymbol, "segment": "NSE",
               "exchange": "NSE", "instrument_token": 0})
    }

    fn four_rows() -> serde_json::Value {
        json!([
            row("ITEM:a", "INFY"),
            row("ITEM:b", "TCS"),
            row("ITEM:c", "WIPRO"),
            row("ITEM:d", "SBIN")
        ])
    }

    fn two_rows() -> serde_json::Value {
        json!([
            {"id": "ITEM:a", "weight": 1.0001, "tradingsymbol": "INFY", "segment": "NSE",
             "exchange": "NSE", "instrument_token": 408_065},
            {"id": "ITEM:b", "weight": 1.0002, "tradingsymbol": "TCS", "segment": "NSE",
             "exchange": "NSE", "instrument_token": 2_953_217}
        ])
    }

    fn catalog_event(app: &mut App, result: Result<Catalog, String>) -> DataEvent {
        let generation = app.watch.begin_catalog();
        DataEvent::Marketwatch(Event::Catalog { generation, result })
    }

    fn catalog(value: serde_json::Value) -> Catalog {
        serde_json::from_value(value).expect("catalog")
    }

    fn detail(value: serde_json::Value) -> Detail {
        serde_json::from_value(value).expect("detail")
    }

    fn loaded(server: &MockServer) -> (App, mpsc::Receiver<AppEvent>) {
        let (mut app, events_rx) = watch_app(server);
        open_first_watch(&mut app, two_rows());
        (app, events_rx)
    }

    fn open_first_watch(app: &mut App, rows: serde_json::Value) {
        app.watch.set_catalog(Ok(catalog(catalog_body())));
        let watch = WatchId::from("W1");
        app.watch_pane.open(watch.clone());
        let generation = app.watch.open(&watch);
        app.watch
            .set_detail(&watch, generation, Ok(detail(detail_body(rows))));
        app.reconcile_watch();
    }

    async fn settle(app: &mut App, events_rx: &mut mpsc::Receiver<AppEvent>) {
        let event = events_rx.recv().await.expect("a request result");
        app.on_event(event);
    }

    async fn settle_until(
        app: &mut App,
        events_rx: &mut mpsc::Receiver<AppEvent>,
        is_awaited: impl Fn(&AppEvent) -> bool,
    ) {
        loop {
            let event = events_rx.recv().await.expect("a request result");
            let awaited = is_awaited(&event);
            app.on_event(event);
            if awaited {
                return;
            }
        }
    }

    fn toasts(app: &App) -> Vec<String> {
        app.toasts.iter().map(|toast| toast.text.clone()).collect()
    }

    #[test]
    fn the_account_watchlist_loads_and_drives_the_subscriptions() {
        let server = MockServer::start();
        let (app, _rx) = loaded(&server);
        assert_eq!(
            app.watch_items()
                .iter()
                .map(|row| row.tradingsymbol.as_str())
                .collect::<Vec<_>>(),
            vec!["INFY", "TCS"]
        );
        assert_eq!(app.watch.count(), 2, "both sections' watches are cycleable");
        let desired = app.desired_subscriptions();
        assert!(desired.contains_key(&408_065));
        assert!(desired.contains_key(&2_953_217));
    }

    #[tokio::test]
    async fn a_catalog_reply_reads_the_watch_it_names() {
        let server = MockServer::start();
        let items = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(two_rows())}).to_string());
        });
        let (mut app, mut events_rx) = watch_app(&server);
        let reply = catalog_event(&mut app, Ok(catalog(catalog_body())));
        app.on_data(reply);
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W1")));
        settle(&mut app, &mut events_rx).await;
        items.assert();
        assert_eq!(app.watch_items().len(), 2);
    }

    #[tokio::test]
    async fn a_failed_first_read_offers_a_retry_instead_of_an_empty_list() {
        let server = MockServer::start();
        let retry = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch");
            then.status(200)
                .body(json!({"status": "success", "data": catalog_body()}).to_string());
        });
        let (mut app, mut events_rx) = watch_app(&server);
        let reply = catalog_event(&mut app, Err("offline".to_string()));
        app.on_data(reply);
        assert_eq!(
            app.watch_view().unavailable(),
            Some(crate::tui::marketwatch::Unavailable::Failed)
        );
        assert_eq!(app.watch_items().len(), 0);

        app.exec_menu_action(MenuAction::RefreshAll);
        settle_until(&mut app, &mut events_rx, |event| {
            matches!(
                event,
                AppEvent::Data(DataEvent::Marketwatch(Event::Catalog { .. }))
            )
        })
        .await;
        retry.assert();
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W1")));
    }

    #[tokio::test]
    async fn adding_an_instrument_posts_it_and_waits_for_the_refetch() {
        let server = MockServer::start();
        let add = server.mock(|when, then| {
            when.method(POST)
                .path("/api/marketwatch/W1/items")
                .body("exchange=NSE&tradingsymbol=WIPRO&weight=1.0003&group=GROUP%3Adefault");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let mut three = two_rows();
        three.as_array_mut().unwrap().push(json!({
            "id": "ITEM:c", "weight": 1.0003, "tradingsymbol": "WIPRO", "segment": "NSE",
            "exchange": "NSE", "instrument_token": 969_473
        }));
        let refetch = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(three)}).to_string());
        });

        let (mut app, mut events_rx) = loaded(&server);
        app.add_watch_item(
            PaneId::TradingWatch,
            &Instrument {
                instrument_token: 969_473,
                tradingsymbol: "WIPRO".into(),
                name: "WIPRO".into(),
                exchange: "NSE".into(),
                segment: "NSE".into(),
                expiry: None,
                strike: None,
                tick_size: 0.05,
                lot_size: 1,
                instrument_type: "EQ".into(),
                freeze_quantity: None,
                max_order_quantity: None,
            },
        );
        assert_eq!(app.watch_items().len(), 2, "not on screen until confirmed");
        assert_eq!(app.watch.pending(), Some("adding"));

        settle(&mut app, &mut events_rx).await;
        add.assert();
        assert_eq!(app.watch.pending(), None);
        settle(&mut app, &mut events_rx).await;
        refetch.assert();
        assert_eq!(app.watch_items().len(), 3);
        assert_eq!(
            app.watch_pane.display_row(&app.watch),
            2,
            "the selection follows the added row"
        );
        assert!(toasts(&app).iter().any(|text| text == "added NSE:WIPRO"));
    }

    #[test]
    fn a_duplicate_add_is_refused_without_a_request() {
        let server = MockServer::start();
        let (mut app, _rx) = loaded(&server);
        app.add_watch_item(
            PaneId::TradingWatch,
            &Instrument {
                instrument_token: 408_065,
                tradingsymbol: "INFY".into(),
                name: "INFY".into(),
                exchange: "NSE".into(),
                segment: "NSE".into(),
                expiry: None,
                strike: None,
                tick_size: 0.05,
                lot_size: 1,
                instrument_type: "EQ".into(),
                freeze_quantity: None,
                max_order_quantity: None,
            },
        );
        assert_eq!(app.watch.pending(), None);
        assert!(
            toasts(&app)
                .iter()
                .any(|text| text.contains("already on watchlist"))
        );
    }

    fn scrip(at: u32) -> serde_json::Value {
        json!({"id": format!("ITEM:{at}"), "weight": f64::from(at), "segment": "NSE",
               "tradingsymbol": format!("SCRIP{at:02}"), "exchange": "NSE",
               "instrument_token": 1_000 + at})
    }

    fn scrip_inst(at: u32) -> Instrument {
        master_inst("NSE", "NSE", &format!("SCRIP{at:02}"), 1_000 + at)
    }

    #[tokio::test]
    async fn a_duplicate_add_selects_the_existing_row_across_groups_without_a_request() {
        let server = MockServer::start_async().await;
        let marketwatch = server
            .mock_async(|when, then| {
                when.path_includes("/api/marketwatch");
                then.status(500);
            })
            .await;
        let (mut app, _rx) = watch_app(&server);
        app.set_workspace(Preset::Trading);
        app.watch.set_catalog(Ok(catalog(catalog_body())));
        let watch = WatchId::from("W1");
        app.watch_pane.open(watch.clone());
        let generation = app.watch.open(&watch);
        let mut body = detail_body(json!((0..20).map(scrip).collect::<Vec<_>>()));
        body["groups"].as_array_mut().expect("groups").push(json!({
            "id": "GROUP:later", "name": "Later", "weight": 2,
            "items": (20..46).map(scrip).collect::<Vec<_>>()
        }));
        app.watch.set_detail(&watch, generation, Ok(detail(body)));
        app.reconcile_watch();
        assert!(
            !screen(&app, 120, 35).contains("NSE:SCRIP45"),
            "starts offscreen"
        );

        app.trading_chart.set_cursor(Some(5));
        app.add_watch_item(PaneId::TradingWatch, &scrip_inst(45));
        assert_eq!(app.watch_pane.display_row(&app.watch), 45);
        assert_eq!(
            app.trading_chart.cursor(),
            None,
            "a new row resets the chart"
        );
        assert!(screen(&app, 120, 35).contains("▶ NSE:SCRIP45"));

        app.trading_chart.set_cursor(Some(5));
        app.add_watch_item(PaneId::TradingWatch, &scrip_inst(45));
        assert_eq!(app.watch_pane.display_row(&app.watch), 45);
        assert_eq!(
            app.trading_chart.cursor(),
            Some(5),
            "re-adding the selected row keeps the chart"
        );

        app.add_watch_item(PaneId::TradingWatch, &scrip_inst(3));
        assert_eq!(app.watch_pane.display_row(&app.watch), 3);
        assert_eq!(app.watch.pending(), None);
        assert_eq!(app.watch_items().len(), 46);
        assert_eq!(
            toasts(&app)
                .iter()
                .filter(|text| text.contains("already on watchlist"))
                .count(),
            3
        );
        tokio::task::yield_now().await;
        marketwatch.assert_calls_async(0).await;
    }

    #[tokio::test]
    async fn a_duplicate_add_to_a_temporary_list_selects_the_row() {
        let server = MockServer::start_async().await;
        let (mut app, _rx) = watch_app(&server);
        app.set_watch_for_test(WatchState::ephemeral(vec![
            WatchItem::from_token(408_065),
            WatchItem::from_token(2_953_217),
        ]));
        let mut inst = scrip_inst(0);
        inst.instrument_token = 2_953_217;
        app.add_watch_item(PaneId::TradingWatch, &inst);
        assert_eq!(app.watch_pane.display_row(&app.watch), 1);
        assert_eq!(app.watch_items().len(), 2);
    }

    #[tokio::test]
    async fn removing_a_row_deletes_it_on_the_account() {
        let server = MockServer::start();
        let delete = server.mock(|when, then| {
            when.method(DELETE).path("/api/marketwatch/W1/items/ITEM:a");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let refetch = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200).body(
                json!({"status": "success", "data": detail_body(json!([
                    {"id": "ITEM:b", "weight": 1.0002, "tradingsymbol": "TCS",
                     "segment": "NSE", "exchange": "NSE", "instrument_token": 2_953_217}
                ]))})
                .to_string(),
            );
        });

        let (mut app, mut events_rx) = loaded(&server);
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.watch_items().len(), 2, "still there until confirmed");

        settle(&mut app, &mut events_rx).await;
        delete.assert();
        settle(&mut app, &mut events_rx).await;
        refetch.assert();
        assert_eq!(
            app.watch_items()
                .iter()
                .map(|row| row.tradingsymbol.as_str())
                .collect::<Vec<_>>(),
            vec!["TCS"]
        );
        assert!(toasts(&app).iter().any(|text| text == "removed NSE:INFY"));
    }

    #[tokio::test]
    async fn a_failed_write_reports_it_and_re_reads_the_watch() {
        let server = MockServer::start();
        let delete = server.mock(|when, then| {
            when.method(DELETE).path("/api/marketwatch/W1/items/ITEM:a");
            then.status(500).body(
                json!({"status": "error", "message": "Something went wrong.",
                       "error_type": "GeneralException"})
                .to_string(),
            );
        });
        let refetch = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(two_rows())}).to_string());
        });
        let catalog_reread = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch");
            then.status(200)
                .body(json!({"status": "success", "data": catalog_body()}).to_string());
        });

        let (mut app, mut events_rx) = loaded(&server);
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        settle(&mut app, &mut events_rx).await;
        delete.assert();
        assert_eq!(app.watch.pending(), None, "the interlock is released");
        assert!(
            toasts(&app)
                .iter()
                .any(|text| text.contains("Something went wrong.")),
            "{:?}",
            toasts(&app)
        );
        settle(&mut app, &mut events_rx).await;
        settle(&mut app, &mut events_rx).await;
        refetch.assert();
        catalog_reread.assert();
        assert_eq!(app.watch_items().len(), 2, "the row was never removed");
    }

    #[tokio::test]
    async fn reorder_sends_the_whole_watch_in_traversal_order() {
        let server = MockServer::start();
        let reorder = server.mock(|when, then| {
            when.method(PUT)
                .path("/api/marketwatch/W1/items")
                .body("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Aa=3");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let refetch = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200).body(
                json!({"status": "success", "data": detail_body(json!([
                    {"id": "ITEM:b", "weight": 1, "tradingsymbol": "TCS", "segment": "NSE",
                     "exchange": "NSE", "instrument_token": 2_953_217},
                    {"id": "ITEM:a", "weight": 2, "tradingsymbol": "INFY", "segment": "NSE",
                     "exchange": "NSE", "instrument_token": 408_065}
                ]))})
                .to_string(),
            );
        });

        let (mut app, mut events_rx) = loaded(&server);
        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(
            app.watch_items()[1].tradingsymbol,
            "INFY",
            "the row moves on the press, not on the round trip"
        );
        assert_eq!(
            app.watch_pane.display_row(&app.watch),
            1,
            "and the selection is on it already"
        );
        settle(&mut app, &mut events_rx).await;
        reorder.assert();
        settle(&mut app, &mut events_rx).await;
        refetch.assert();
        assert_eq!(app.watch_items()[1].tradingsymbol, "INFY");
        assert_eq!(
            app.watch_pane.display_row(&app.watch),
            1,
            "the selection follows the row it moved"
        );
    }

    #[test]
    fn a_reorder_that_would_leave_the_group_is_a_silent_no_op() {
        let server = MockServer::start();
        let put = server.mock(|when, then| {
            when.method(PUT).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let (mut app, _rx) = loaded(&server);
        app.on_key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));

        let generation = app.watch.generation(&WatchId::from("W1"));
        app.watch.set_detail(
            &WatchId::from("W1"),
            generation,
            Ok(detail(json!({
                "id": "W1", "name": "Core", "is_static": false,
                "groups": [
                    {"id": "GROUP:default", "weight": 1, "items": two_rows()},
                    {"id": "GROUP:b", "weight": 2, "items": [
                        {"id": "ITEM:c", "weight": 2.0001, "tradingsymbol": "WIPRO",
                         "segment": "NSE", "exchange": "NSE", "instrument_token": 969_473}
                    ]}
                ]
            }))),
        );
        app.watch_pane.select(1, &app.watch);
        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));

        assert_eq!(app.watch.pending(), None);
        assert_eq!(
            app.watch_items()
                .iter()
                .map(|row| row.tradingsymbol.as_str())
                .collect::<Vec<_>>(),
            vec!["INFY", "TCS", "WIPRO"],
            "the row stayed in its group"
        );
        assert_eq!(app.watch_pane.display_row(&app.watch), 1);
        assert!(toasts(&app).is_empty(), "{:?}", toasts(&app));
        assert_eq!(put.calls(), 0, "a refused move sends nothing");
    }

    #[tokio::test]
    async fn rapid_presses_coalesce_into_one_follow_up_write() {
        let server = MockServer::start();
        let reorder = |body: &'static str| {
            server.mock(move |when, then| {
                when.method(PUT)
                    .path("/api/marketwatch/W1/items")
                    .body(body);
                then.status(200)
                    .body(json!({"status": "success", "data": {}}).to_string());
            })
        };
        let first = reorder("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Aa=3&ITEM%3Ac=4&ITEM%3Ad=5");
        let middle = reorder("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Ac=3&ITEM%3Aa=4&ITEM%3Ad=5");
        let last = reorder("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Ac=3&ITEM%3Ad=4&ITEM%3Aa=5");
        let refetch = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200).body(
                json!({"status": "success", "data": detail_body(json!([
                    row("ITEM:b", "TCS"), row("ITEM:c", "WIPRO"),
                    row("ITEM:d", "SBIN"), row("ITEM:a", "INFY")
                ]))})
                .to_string(),
            );
        });

        let (mut app, mut events_rx) = watch_app(&server);
        open_first_watch(&mut app, four_rows());

        for _ in 0..3 {
            app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        }
        assert_eq!(
            app.watch_items()
                .iter()
                .map(|row| row.tradingsymbol.as_str())
                .collect::<Vec<_>>(),
            vec!["TCS", "WIPRO", "SBIN", "INFY"],
            "all three presses landed on the rows at once"
        );
        assert_eq!(app.watch_pane.display_row(&app.watch), 3);
        assert!(toasts(&app).is_empty(), "no press was refused");

        settle(&mut app, &mut events_rx).await;
        assert_eq!(first.calls(), 1);
        settle(&mut app, &mut events_rx).await;
        assert_eq!(last.calls(), 1, "the follow-up carries the final order");
        assert_eq!(middle.calls(), 0, "the step between them is never sent");

        settle(&mut app, &mut events_rx).await;
        assert_eq!(refetch.calls(), 1);
        assert_eq!(app.watch_items()[3].tradingsymbol, "INFY");
        assert_eq!(
            app.watch_pane.display_row(&app.watch),
            3,
            "the selection is still on the row that moved"
        );
    }

    #[tokio::test]
    async fn a_refresh_behind_a_reorder_is_deferred_and_keeps_the_queued_move() {
        let server = MockServer::start();
        let reorder = |body: &'static str| {
            server.mock(move |when, then| {
                when.method(PUT)
                    .path("/api/marketwatch/W1/items")
                    .body(body);
                then.status(200)
                    .body(json!({"status": "success", "data": {}}).to_string());
            })
        };
        let first = reorder("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Aa=3&ITEM%3Ac=4&ITEM%3Ad=5");
        let last = reorder("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Ac=3&ITEM%3Aa=4&ITEM%3Ad=5");
        let items = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200).body(
                json!({"status": "success", "data": detail_body(json!([
                    row("ITEM:b", "TCS"), row("ITEM:c", "WIPRO"),
                    row("ITEM:a", "INFY"), row("ITEM:d", "SBIN")
                ]))})
                .to_string(),
            );
        });

        let (mut app, mut events_rx) = watch_app(&server);
        open_first_watch(&mut app, four_rows());

        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert!(
            !app.reload_watch_detail(),
            "the read `r` asks for waits for the write"
        );
        assert_eq!(items.calls(), 0, "nothing was read behind the write");

        settle(&mut app, &mut events_rx).await;
        assert_eq!(first.calls(), 1);
        settle(&mut app, &mut events_rx).await;
        assert_eq!(last.calls(), 1, "the queued move survived the refresh");
        settle(&mut app, &mut events_rx).await;
        assert_eq!(items.calls(), 1);
        assert_eq!(
            app.watch_items()
                .iter()
                .map(|row| row.tradingsymbol.as_str())
                .collect::<Vec<_>>(),
            vec!["TCS", "WIPRO", "INFY", "SBIN"]
        );
    }

    #[tokio::test]
    async fn a_failed_reorder_restores_the_confirmed_order() {
        let server = MockServer::start();
        let reorder = server.mock(|when, then| {
            when.method(PUT).path("/api/marketwatch/W1/items");
            then.status(400).body(
                json!({"status": "error", "message": "Cannot reorder this watchlist.",
                       "error_type": "InputException"})
                .to_string(),
            );
        });

        server.mock(|when, then| {
            when.method(GET);
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(four_rows())}).to_string());
        });

        let (mut app, mut events_rx) = watch_app(&server);
        open_first_watch(&mut app, four_rows());

        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert_eq!(app.watch_items()[2].tradingsymbol, "INFY");

        settle(&mut app, &mut events_rx).await;
        reorder.assert();
        assert_eq!(
            app.watch_items()
                .iter()
                .map(|row| row.tradingsymbol.as_str())
                .collect::<Vec<_>>(),
            vec!["INFY", "TCS", "WIPRO", "SBIN"],
            "the account's confirmed order is what is on screen"
        );
        assert_eq!(
            app.watch_pane.display_row(&app.watch),
            0,
            "the selection stays on the row, back where the account has it"
        );
        assert!(
            toasts(&app)
                .iter()
                .any(|text| text.contains("Cannot reorder this watchlist.")),
            "{:?}",
            toasts(&app)
        );
    }

    #[tokio::test]
    async fn switching_reads_the_next_watch_in_the_accounts_order() {
        let server = MockServer::start();
        let items = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W2/items");
            then.status(200).body(
                json!({"status": "success", "data": {
                    "id": "W2", "name": "Swing", "is_static": false,
                    "groups": [{"id": "GROUP:s", "weight": 1, "items": [
                        {"id": "ITEM:z", "weight": 1.0001, "tradingsymbol": "WIPRO",
                         "segment": "NSE", "exchange": "NSE", "instrument_token": 969_473}
                    ]}]
                }})
                .to_string(),
            );
        });

        let (mut app, mut events_rx) = loaded(&server);
        app.on_key(KeyEvent::new(KeyCode::Char(']'), KeyModifiers::NONE));
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W2")));
        assert!(
            app.watch_items().is_empty(),
            "the old rows are not this list"
        );
        settle(&mut app, &mut events_rx).await;
        items.assert();
        assert_eq!(app.watch_items()[0].tradingsymbol, "WIPRO");
        assert_eq!(app.watch_view().position(), 1);
    }

    #[tokio::test]
    async fn wl_with_an_unknown_name_creates_it_on_the_account() {
        let server = MockServer::start();
        let create = server.mock(|when, then| {
            when.method(POST)
                .path("/api/marketwatch")
                .body("name=Long+term&weight=2");
            then.status(200)
                .body(json!({"status": "success", "data": {"id": "W9"}}).to_string());
        });

        let (mut app, mut events_rx) = loaded(&server);
        app.exec_watchlist(WatchlistCmd::Switch("Long term".to_string()));
        settle(&mut app, &mut events_rx).await;
        create.assert();
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W9")));
        assert!(
            toasts(&app)
                .iter()
                .any(|text| text == "watchlist created: Long term")
        );
    }

    #[tokio::test]
    async fn wl_with_a_known_name_switches_to_it() {
        let server = MockServer::start();
        let (mut app, _rx) = loaded(&server);
        app.exec_watchlist(WatchlistCmd::Switch("swing".to_string()));
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W2")));
    }

    #[tokio::test]
    async fn renaming_renames_it_on_the_account() {
        let server = MockServer::start();
        let rename = server.mock(|when, then| {
            when.method(PUT)
                .path("/api/marketwatch/W1")
                .body("name=Cash+only");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let (mut app, mut events_rx) = loaded(&server);
        app.exec_watchlist(WatchlistCmd::Rename("Cash only".to_string()));
        settle(&mut app, &mut events_rx).await;
        rename.assert();
    }

    #[tokio::test]
    async fn deleting_a_watchlist_is_confirmed_first_and_may_empty_the_account() {
        let server = MockServer::start();
        let delete = server.mock(|when, then| {
            when.method(DELETE).path("/api/marketwatch/W1");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let empty = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch");
            then.status(200)
                .body(json!({"status": "success", "data": []}).to_string());
        });

        let (mut app, mut events_rx) = loaded(&server);
        app.exec_watchlist(WatchlistCmd::Delete);
        let Some(Modal::Confirm(confirm)) = app.modals.last() else {
            panic!("a watchlist delete must be confirmed");
        };
        let PendingAction::DeleteWatchlist { watch, name } = &confirm.action else {
            panic!("expected a DeleteWatchlist confirmation");
        };
        assert_eq!(watch, "W1");
        assert_eq!(name, "Core");
        assert_eq!(app.watch.pending(), None, "nothing is sent yet");

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.watch.pending(), None);
        assert_eq!(delete.calls(), 0, "backing out must not delete anything");

        app.exec_watchlist(WatchlistCmd::Delete);
        super::super::test_support::render(
            &app,
            super::super::test_support::W,
            super::super::test_support::H,
        );
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        settle(&mut app, &mut events_rx).await;
        delete.assert();
        assert!(
            !app.pending_action,
            "the order interlock must not be left armed"
        );
        settle(&mut app, &mut events_rx).await;
        empty.assert();
        assert_eq!(
            app.watch_view().unavailable(),
            Some(crate::tui::marketwatch::Unavailable::Empty),
            "the last list may go, and the empty state offers a create"
        );
    }

    #[tokio::test]
    async fn an_older_catalog_reply_cannot_leave_the_watch_just_created() {
        let server = MockServer::start();
        let created = server.mock(|when, then| {
            when.method(POST).path("/api/marketwatch");
            then.status(200)
                .body(json!({"status": "success", "data": {"id": "W2"}}).to_string());
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W2/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(two_rows())}).to_string());
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch");
            then.status(200)
                .body(json!({"status": "success", "data": catalog_body()}).to_string());
        });

        let (mut app, mut events_rx) = watch_app(&server);

        let stale = app.watch.begin_catalog();
        app.send_mutation(
            Mutation::Create {
                name: "Swing".to_string(),
                weight: 1,
            },
            super::Origin::Command,
        );
        settle(&mut app, &mut events_rx).await;
        created.assert();
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W2")));

        app.on_data(DataEvent::Marketwatch(Event::Catalog {
            generation: stale,
            result: Ok(catalog(json!([
                {"name": "Favorites", "items": [{"id": "W1", "name": "Core", "is_static": false}]}
            ]))),
        }));
        assert_eq!(
            app.watch_pane.watch(),
            Some(&WatchId::from("W2")),
            "the new watch stays open"
        );
    }

    #[tokio::test]
    async fn a_curated_list_renders_but_refuses_writes() {
        let server = MockServer::start();
        let (mut app, _rx) = watch_app(&server);
        let reply = catalog_event(
            &mut app,
            Ok(catalog(json!([
                {"name": "Favorites", "items": [{"id": "W1", "name": "Nifty 50", "is_static": true}]}
            ]))),
        );
        app.on_data(reply);
        let generation = app.watch.generation(&WatchId::from("W1"));
        let mut curated = detail(detail_body(two_rows()));
        curated.is_static = true;
        app.on_data(DataEvent::Marketwatch(Event::Detail {
            watch: WatchId::from("W1"),
            generation,
            result: Ok(curated),
        }));
        assert_eq!(app.watch_items().len(), 2, "it is viewable");

        for key in ['d', 'J', 'K'] {
            app.on_key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
        }
        assert_eq!(app.watch.pending(), None);
        assert!(
            toasts(&app)
                .iter()
                .any(|text| text.contains("Kite-curated")),
            "{:?}",
            toasts(&app)
        );
    }

    #[test]
    fn a_token_session_never_calls_the_account() {
        let (events_tx, _events_rx) = mpsc::channel(64);
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("token").unwrap())
            .build()
            .expect("dummy client builds");
        let mut app = App::new(
            client,
            events_tx,
            WatchState::ephemeral(vec![WatchItem::from_token(408_065)]),
            Pad::default(),
            true,
            &Settings::default(),
        );
        app.set_pane(PaneId::TradingWatch);
        assert!(app.start_marketwatch().is_empty());
        assert_eq!(app.watch_items().len(), 1);

        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.watch_items().len(), 0);
        app.exec_watchlist(WatchlistCmd::Delete);
        assert!(
            toasts(&app).iter().any(|text| text.contains("--token")),
            "{:?}",
            toasts(&app)
        );
        assert!(app.modals.is_empty(), "and nothing is confirmed");
    }

    fn trading_on_tcs(server: &MockServer) -> (App, mpsc::Receiver<AppEvent>) {
        let (mut app, events_rx) = loaded(server);
        app.watch_pane.open(WatchId::from("W1"));
        app.watch_pane.select(1, &app.watch);
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingWatch);
        (app, events_rx)
    }

    #[tokio::test]
    async fn a_trading_watch_delete_targets_its_own_row() {
        let server = MockServer::start();
        let tcs = server.mock(|when, then| {
            when.method(DELETE).path("/api/marketwatch/W1/items/ITEM:b");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let (mut app, mut events_rx) = trading_on_tcs(&server);
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        settle(&mut app, &mut events_rx).await;
        tcs.assert();
        assert!(toasts(&app).iter().any(|text| text == "removed NSE:TCS"));
    }

    #[tokio::test]
    async fn a_trading_watch_reorder_moves_its_own_row() {
        let server = MockServer::start();
        let put = server.mock(|when, then| {
            when.method(PUT)
                .path("/api/marketwatch/W1/items")
                .body("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Aa=3");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let (mut app, mut events_rx) = trading_on_tcs(&server);
        app.on_key(KeyEvent::new(KeyCode::Char('K'), KeyModifiers::SHIFT));
        assert_eq!(app.watch.pending(), Some("reordering"), "TCS moved up");
        settle(&mut app, &mut events_rx).await;
        put.assert();
        assert_eq!(
            app.watch_pane
                .selected(&app.watch)
                .map(|row| row.tradingsymbol.as_str()),
            Some("TCS"),
        );
        assert_eq!(app.watch_pane.display_row(&app.watch), 0);
    }

    #[tokio::test]
    async fn a_trading_watch_add_follows_the_row_in_that_pane() {
        let server = MockServer::start();
        let add = server.mock(|when, then| {
            when.method(POST).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let mut three = two_rows();
        three.as_array_mut().unwrap().push(json!({
            "id": "ITEM:c", "weight": 1.0003, "tradingsymbol": "WIPRO", "segment": "NSE",
            "exchange": "NSE", "instrument_token": 969_473
        }));
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(three)}).to_string());
        });
        let (mut app, mut events_rx) = trading_on_tcs(&server);
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "WIPRO", 969_473,
        )])));
        app.on_key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for c in "WIPRO".chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::SHIFT));
        }
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        settle(&mut app, &mut events_rx).await;
        add.assert();
        settle(&mut app, &mut events_rx).await;
        assert_eq!(app.watch_pane.display_row(&app.watch), 2);
    }

    #[tokio::test]
    async fn a_catalog_reply_binds_the_watch_pane() {
        let server = MockServer::start();
        let items = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(two_rows())}).to_string());
        });
        let (mut app, mut events_rx) = watch_app(&server);
        let reply = catalog_event(&mut app, Ok(catalog(catalog_body())));
        app.on_data(reply);
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W1")));
        settle(&mut app, &mut events_rx).await;
        assert_eq!(items.calls(), 1, "one read for the watch the pane opens");
        assert_eq!(app.watch_rows_of(PaneId::TradingWatch).len(), 2);

        app.watch_pane.open(WatchId::from("W9"));
        let reply = catalog_event(&mut app, Ok(catalog(catalog_body())));
        app.on_data(reply);
        assert_eq!(app.watch_pane.watch(), Some(&WatchId::from("W1")));
        settle(&mut app, &mut events_rx).await;
        assert_eq!(items.calls(), 2, "the pane's new watch is read");
        assert_eq!(app.watch_rows_of(PaneId::TradingWatch).len(), 2);
    }

    #[tokio::test]
    async fn a_reorder_on_another_watch_outlives_its_older_read_and_is_sent() {
        let server = MockServer::start();
        let w1 = server.mock(|when, then| {
            when.method(PUT)
                .path("/api/marketwatch/W1/items")
                .body("GROUP%3Adefault=1&ITEM%3Ab=2&ITEM%3Aa=3&ITEM%3Ac=4&ITEM%3Ad=5");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let w1_refetch = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(four_rows())}).to_string());
        });
        let w2 = server.mock(|when, then| {
            when.method(PUT)
                .path("/api/marketwatch/W2/items")
                .body("GROUP%3Adefault=1&ITEM%3Ay=2&ITEM%3Ax=3");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let swing = json!([row("ITEM:y", "SBIN"), row("ITEM:x", "WIPRO")]);
        let w2_refetch = server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W2/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(swing)}).to_string());
        });

        let (mut app, mut events_rx) = watch_app(&server);
        open_first_watch(&mut app, four_rows());
        let swing = WatchId::from("W2");
        let generation = app.watch.open(&swing);
        let rows = json!([row("ITEM:x", "WIPRO"), row("ITEM:y", "SBIN")]);
        app.watch
            .set_detail(&swing, generation, Ok(detail(detail_body(rows))));
        let older_read = app.watch.refresh(&swing).expect("no write yet");

        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingWatch);
        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        app.watch_pane.open(swing.clone());
        app.reconcile_watch();
        app.on_key(KeyEvent::new(KeyCode::Char('J'), KeyModifiers::SHIFT));
        assert!(toasts(&app).is_empty(), "{:?}", toasts(&app));
        let rows = json!([row("ITEM:x", "WIPRO"), row("ITEM:y", "SBIN")]);
        app.on_data(DataEvent::Marketwatch(Event::Detail {
            watch: swing,
            generation: older_read,
            result: Ok(detail(detail_body(rows))),
        }));
        assert_eq!(
            app.watch_rows_of(PaneId::TradingWatch)[0].tradingsymbol,
            "SBIN"
        );

        settle(&mut app, &mut events_rx).await;
        w1.assert();
        assert_eq!(
            app.watch.pending(),
            Some("reordering"),
            "W2's move went out"
        );
        settle(&mut app, &mut events_rx).await;
        w2.assert();
        settle(&mut app, &mut events_rx).await;
        w2_refetch.assert();
        assert_eq!(
            w1_refetch.calls(),
            0,
            "the pane has left W1, so nothing re-reads it"
        );
        assert_eq!(app.watch.pending(), None);
        assert_eq!(
            app.watch_pane
                .selected(&app.watch)
                .map(|row| row.tradingsymbol.as_str()),
            Some("WIPRO"),
        );
    }

    #[tokio::test]
    async fn a_watch_delete_is_refused_at_entry_when_it_cannot_be_sent() {
        let server = MockServer::start();
        let remove = server.mock(|when, then| {
            when.method(DELETE).path("/api/marketwatch/W1/items/ITEM:a");
            then.status(200)
                .body(json!({"status": "success", "data": {}}).to_string());
        });
        let (mut app, mut events_rx) = loaded(&server);
        let auth_expired = |app: &App| {
            toasts(app)
                .iter()
                .filter(|text| text.starts_with("auth expired"))
                .count()
        };

        app.ws_fatal = Some("TokenException".to_string());
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.watch.pending(), None, "a row delete went out");
        assert_eq!(auth_expired(&app), 1);
        app.exec_watchlist(WatchlistCmd::Delete);
        assert!(app.modals.is_empty(), "a watchlist delete was confirmed");
        assert_eq!(auth_expired(&app), 2);
        app.ws_fatal = None;

        app.pending_action = true;
        app.exec_watchlist(WatchlistCmd::Delete);
        assert!(app.modals.is_empty(), "a watchlist delete was confirmed");
        assert!(
            toasts(&app)
                .iter()
                .any(|text| text == "an order action is in flight")
        );
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.watch.pending(), Some("removing"));
        settle(&mut app, &mut events_rx).await;
        remove.assert();
        assert!(app.pending_action, "the order still holds the interlock");
    }

    fn wipro() -> Instrument {
        Instrument {
            instrument_token: 969_473,
            tradingsymbol: "WIPRO".into(),
            name: "WIPRO".into(),
            exchange: "NSE".into(),
            segment: "NSE".into(),
            expiry: None,
            strike: None,
            tick_size: 0.05,
            lot_size: 1,
            instrument_type: "EQ".into(),
            freeze_quantity: None,
            max_order_quantity: None,
        }
    }

    fn serve_writes_then(server: &MockServer, rows: serde_json::Value) {
        let ok = json!({"status": "success", "data": {}}).to_string();
        server.mock(|when, then| {
            when.method(POST).path("/api/marketwatch/W1/items");
            then.status(200).body(ok.clone());
        });
        server.mock(|when, then| {
            when.method(DELETE).path("/api/marketwatch/W1/items/ITEM:a");
            then.status(200).body(ok.clone());
        });
        server.mock(|when, then| {
            when.method(PUT).path("/api/marketwatch/W1");
            then.status(200).body(ok.clone());
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200)
                .body(json!({"status": "success", "data": detail_body(rows)}).to_string());
        });
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch");
            then.status(200)
                .body(json!({"status": "success", "data": catalog_body()}).to_string());
        });
    }

    #[tokio::test]
    async fn a_pane_write_replaces_the_follow_its_earlier_write_left() {
        let server = MockServer::start();
        serve_writes_then(
            &server,
            json!([row("ITEM:b", "TCS"), row("ITEM:c", "WIPRO")]),
        );
        let (mut app, mut events_rx) = loaded(&server);
        app.add_watch_item(PaneId::TradingWatch, &wipro());
        settle(&mut app, &mut events_rx).await;
        app.on_key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        assert_eq!(app.watch.pending(), Some("removing"));

        for _ in 0..3 {
            settle(&mut app, &mut events_rx).await;
        }
        assert_eq!(app.watch.pending(), None);
        assert_eq!(app.watch_items().len(), 2);

        assert_eq!(
            app.watch_pane.display_row(&app.watch),
            0,
            "the delete's refetch followed the add"
        );
    }

    #[tokio::test]
    async fn a_palette_write_clears_the_follows_it_supersedes() {
        let server = MockServer::start();
        serve_writes_then(
            &server,
            json!([
                row("ITEM:a", "INFY"),
                row("ITEM:b", "TCS"),
                row("ITEM:c", "WIPRO")
            ]),
        );
        let (mut app, mut events_rx) = loaded(&server);
        app.add_watch_item(PaneId::TradingWatch, &wipro());
        settle(&mut app, &mut events_rx).await;
        app.exec_watchlist(WatchlistCmd::Rename("Cash only".to_string()));
        assert_eq!(app.watch.pending(), Some("renaming watchlist"));

        for _ in 0..4 {
            settle(&mut app, &mut events_rx).await;
        }
        assert_eq!(app.watch_items().len(), 3);
        assert_eq!(
            app.watch_pane
                .selected(&app.watch)
                .map(|row| row.tradingsymbol.as_str()),
            Some("INFY"),
            "the rename's refetch followed the add"
        );
    }
}
