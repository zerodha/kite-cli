mod results;
mod websocket;

use super::panes::chain::today_ist;
use super::{App, Book, Modal, ToastLevel};
use crate::tui::bids;
use crate::tui::candles::SeriesKey;
use crate::tui::data::{self, DataEvent};
use crate::tui::event::AppEvent;
use crate::tui::workspace::PaneId;

impl App {
    pub fn on_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.on_key(key),
            AppEvent::Paste(text) => self.on_paste(&text),
            AppEvent::Shutdown => self.should_quit = true,
            AppEvent::Mouse(mouse) => self.on_mouse(mouse),
            AppEvent::Timer => {
                self.tick_candles();
                self.recenter_chain();
                let today = today_ist();
                if today != self.instrument_master_day {
                    self.instrument_master_day = today;
                    data::spawn_load_instruments(self.uuid.clone(), self.events_tx.clone());
                }
            }

            AppEvent::Resize => {
                self.clear_chart_hovers();
                self.hit.borrow_mut().invalidate();
                self.drag_split = None;
            }
            AppEvent::Ws(event) => self.on_ws(event),
            AppEvent::Data(event) => self.on_data(event),
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive event dispatch keeps each variant and its effects together"
    )]
    pub(in crate::tui::app) fn on_data(&mut self, event: DataEvent) {
        match event {
            DataEvent::Circuits { token, result } => match result {
                Ok(limits) => {
                    self.circuits_inflight.remove(&token);
                    self.circuits.insert(token, limits);
                }

                Err(err) => self.handle_data_error(err),
            },
            DataEvent::Positions(Ok(positions)) => {
                self.positions = Some(positions);
                self.positions_gen += 1;
                self.reconcile_book(Book::Positions);
                self.reconcile_chart_target();
                self.sync_subscriptions();
            }
            DataEvent::Auctions(Ok(auctions)) => {
                self.auctions = Some(auctions);
                self.bids.auctions_error = None;
                self.reconcile_bids();
                self.sync_subscriptions();
            }
            DataEvent::Bids(bids::Event::Instruments { section, result }) => {
                if let Err(error) = &result {
                    self.handle_data_error(format!("{}: {error}", section.title()));
                }
                self.bids.set_instruments(section, result);
                self.reconcile_bids();
            }
            DataEvent::Marketwatch(event) => self.on_marketwatch(event),
            DataEvent::Bids(bids::Event::Applications(result)) => {
                if let Err(error) = &result {
                    self.handle_data_error(format!("IPO applications: {error}"));
                }
                self.bids.set_applications(result);
                self.reconcile_bids();
            }
            DataEvent::ExternalLink(Ok(url)) => {
                self.toast(ToastLevel::Info, format!("browser launch started: {url}"));
            }
            DataEvent::ExternalLink(Err(error)) => self.toast_error(error),
            DataEvent::Holdings(Ok(holdings)) => {
                self.holdings = Some(holdings);
                self.holdings_gen += 1;
                self.reconcile_book(Book::Holdings);
                self.reconcile_chart_target();
                self.sync_subscriptions();
            }

            DataEvent::MfHoldings(Ok(mf_holdings)) => {
                self.mf_holdings = Some(mf_holdings);
                self.mf_gen += 1;
                self.reconcile_book(Book::Mf);
            }
            DataEvent::Orders(Ok(orders)) => {
                self.reconcile_marked_orders(&orders);
                self.orders = Some(orders);
                self.orders_gen += 1;
                self.reconcile_book(Book::Orders);
                self.reconcile_chart_target();
                self.sync_subscriptions();
            }
            DataEvent::Margins(Ok(funds)) => {
                self.margin_available = Some(data::funds_margin_available(&funds));
                self.funds = Some(*funds);
            }
            DataEvent::Profile(Ok(profile)) => {
                self.profile = Some(*profile);
            }
            DataEvent::Gtts(Ok(gtts)) => {
                self.gtts = Some(gtts);
                self.gtts_gen += 1;
                self.reconcile_book(Book::Gtt);
            }

            DataEvent::Alerts {
                sequence,
                result: Ok(alerts),
            } => {
                if sequence <= self.alerts_applied {
                    return;
                }
                self.alerts_applied = sequence;

                let triggered = self.triggers_since_last_fetch(&alerts);
                self.set_alerts(alerts);
                self.reconcile_book(Book::Alerts);
                for message in triggered {
                    self.toast(ToastLevel::Success, message);

                    eprint!("\x07");
                }

                if let Some(state) = self.alert_history.as_ref()
                    && !self
                        .alert_list()
                        .iter()
                        .any(|alert| alert.uuid == state.uuid)
                {
                    self.alert_history = None;
                }
            }
            DataEvent::AlertHistory {
                uuid,
                request,
                result,
            } => self.on_alert_history(&uuid, request, result),

            DataEvent::Movers { universe, result } => {
                if universe == self.movers_universe {
                    self.movers_inflight = false;
                    match result {
                        Ok(movers) => {
                            self.movers_failed = None;
                            self.movers = Some(*movers);
                        }

                        Err(err) => {
                            self.movers_failed = Some(err.clone());
                            self.handle_data_error(err);
                        }
                    }
                }
            }
            DataEvent::AlertStatusSet {
                uuid,
                status,
                confirmed,
                result,
            } => {
                if confirmed {
                    self.pending_action = false;
                }
                self.alert_status_pending.remove(&uuid);
                if result.is_ok() {
                    let mut alerts = self.alert_list().to_vec();
                    if let Some(alert) = alerts.iter_mut().find(|alert| alert.uuid == uuid) {
                        alert.status = status;
                    }
                    self.set_alerts(alerts);
                    self.reconcile_book(Book::Alerts);
                }
                self.on_alert_result(
                    if status == kiteconnect::AlertStatus::Enabled {
                        "enabled"
                    } else {
                        "disabled"
                    },
                    None,
                    result,
                );
            }
            DataEvent::AlertCreated { request, result } => {
                self.on_alert_result("created", Some(request), result);
            }
            DataEvent::AlertModified { request, result } => {
                self.on_alert_result("updated", request, result);
            }

            DataEvent::AlertDeleted(result) => {
                self.pending_action = false;
                self.on_alert_result("deleted", None, result);
            }
            DataEvent::MarginPreview { seq, result } => {
                if let Some(Modal::Confirm(state)) = self.modals.last_mut()
                    && state.preview_seq == Some(seq)
                {
                    state.margin = Some(result);
                }
            }

            DataEvent::OrderHistory { request, result } => {
                if let Some(Modal::OrderDetail(state)) = self.modals.last_mut()
                    && state.request == request
                {
                    state.history = Some(result);
                }
            }
            DataEvent::GttDeleted(result) => {
                self.pending_action = false;
                match result {
                    Ok(trigger_id) => {
                        self.toast(ToastLevel::Success, format!("GTT deleted: {trigger_id}"));
                        data::spawn_refresh_gtts(self.client.clone(), self.events_tx.clone());
                    }
                    Err(err) => {
                        self.toast(
                            ToastLevel::Error,
                            format!("GTT deletion request failed: {err}"),
                        );
                        data::spawn_refresh_gtts(self.client.clone(), self.events_tx.clone());
                    }
                }
            }
            DataEvent::Instruments(Ok(store)) => {
                self.instruments_error = None;

                self.watch.enrich(&store);
                self.refresh_search_results(Some(&store));
                self.toast(
                    ToastLevel::Info,
                    format!("instrument master loaded ({} instruments)", store.len()),
                );
                self.instruments = Some(store);
                self.rebuild_chain(PaneId::DerivativesChain);

                if matches!(self.modals.last(), Some(Modal::Palette(_)))
                    && let Some(Modal::Palette(mut state)) = self.modals.pop()
                {
                    let identity = state.selected_identity();
                    self.rebuild_palette(&mut state, identity);
                    self.modals.push(Modal::Palette(state));
                }
            }
            DataEvent::Instruments(Err(err)) => {
                self.instruments_error = Some(err.clone());
                self.handle_data_error(err);
            }
            DataEvent::Alerts {
                sequence,
                result: Err(err),
            } => {
                if sequence > self.alerts_applied {
                    self.alerts_applied = sequence;
                    self.handle_data_error(err);
                }
            }
            DataEvent::Positions(Err(err))
            | DataEvent::Holdings(Err(err))
            | DataEvent::MfHoldings(Err(err))
            | DataEvent::Orders(Err(err))
            | DataEvent::Margins(Err(err))
            | DataEvent::Profile(Err(err))
            | DataEvent::Gtts(Err(err)) => self.handle_data_error(err),
            DataEvent::Auctions(Err(err)) => {
                self.bids.auctions_error = Some(err.clone());
                self.handle_data_error(err);
            }
            DataEvent::Candles {
                token,
                interval,
                result,
            } => {
                let key = SeriesKey::new(token, interval);
                self.candle_requests.finish(key, result.is_ok());
                match result {
                    Ok(candles) => self.candles.insert(key, candles),
                    Err(err) => self.handle_data_error(err),
                }

                self.pump_candles();
            }
            DataEvent::OrderPlaced(result) => self.on_order_result("placed", result),
            DataEvent::OrderModified(result) => self.on_order_result("modified", result),
            DataEvent::PositionConverted(result) => self.on_position_conversion(result),
            DataEvent::HoldingsAuthorised(result) => self.on_holdings_authorised(result),
            DataEvent::OrdersCancelled(result) => self.on_orders_cancelled(&result),
            DataEvent::GttPlaced(result) => self.on_gtt_result("placed", result),
            DataEvent::GttModified(result) => self.on_gtt_result("modified", result),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        char_key, char_key_code, holding, master_inst, sample_order, test_app,
    };
    use super::super::{KeyCode, KeyEvent, KeyModifiers, Modal, Side};
    use crate::tui::data::DataEvent;
    use crate::tui::event::AppEvent;
    use crate::tui::instruments::InstrumentStore;
    use crate::tui::workspace::PaneId;
    use kiteconnect::OrderType;
    use kiteconnect::ticker::TickerEvent;
    use std::sync::Arc;

    #[tokio::test]
    async fn a_new_day_always_dispatches_an_instrument_reload() {
        if std::env::var_os("KITE_TEST_ISOLATED_RELOAD").is_none() {
            let directory =
                std::env::temp_dir().join(format!("kite-reload-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&directory).unwrap();
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .env_clear()
                .env("KITE_TEST_ISOLATED_RELOAD", "1")
                .env("HOME", &directory)
                .env("USERPROFILE", &directory)
                .env("LOCALAPPDATA", &directory)
                .env("APPDATA", &directory)
                .env("XDG_CACHE_HOME", &directory)
                .env("XDG_CONFIG_HOME", &directory)
                .env("XDG_DATA_HOME", &directory)
                .env("XDG_STATE_HOME", &directory)
                .env("CARGO_HOME", directory.join("cargo"))
                .env("RUSTUP_HOME", directory.join("rustup"))
                .env("MISE_DATA_DIR", directory.join("mise"))
                .args([
                    "--exact",
                    "tui::app::events::tests::a_new_day_always_dispatches_an_instrument_reload",
                    "--nocapture",
                ])
                .output()
                .unwrap();
            std::fs::remove_dir_all(&directory).unwrap();
            assert!(
                output.status.success(),
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }
        let mut app = test_app();
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        app.events_tx = tx;

        app.uuid = "invalid\n".into();
        app.instrument_master_day = "previous day".into();

        app.on_event(AppEvent::Timer);
        assert_eq!(app.instrument_master_day, super::today_ist());
        let event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
            .await
            .expect("the reload must produce a result")
            .unwrap();
        assert!(matches!(event, AppEvent::Data(DataEvent::Instruments(_))));
        app.on_event(event);

        app.on_event(AppEvent::Timer);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), rx.recv())
                .await
                .is_err(),
            "the same day must not dispatch another reload"
        );
    }

    #[tokio::test]
    async fn a_late_order_history_reply_does_not_fill_another_order() {
        let mut app = test_app();
        let mut first = sample_order("COMPLETE");
        first.order_id = "FIRST".into();
        let mut second = sample_order("COMPLETE");
        second.order_id = "SECOND".into();
        app.orders = Some(vec![first.clone(), second.clone()]);
        app.show_pane(PaneId::OrdersBook);
        let open = |app: &mut super::super::App, order_id: &str| {
            let row = app
                .orders_order_in(PaneId::OrdersBook)
                .iter()
                .position(|&i| app.orders.as_ref().unwrap()[i].order_id == order_id)
                .expect("the order is listed");
            app.set_selection(row);
            app.on_key(char_key_code(KeyCode::Enter));
            let Some(Modal::OrderDetail(state)) = app.modals.last() else {
                panic!("Enter opens the order detail");
            };
            assert_eq!(state.order.order_id, order_id);
            state.request.clone()
        };

        let first_request = open(&mut app, "FIRST");
        app.on_key(char_key_code(KeyCode::Esc));
        let second_request = open(&mut app, "SECOND");

        app.on_data(DataEvent::OrderHistory {
            request: first_request,
            result: Ok(vec![first]),
        });
        let Some(Modal::OrderDetail(state)) = app.modals.last() else {
            panic!("SECOND is still open");
        };
        assert!(state.history.is_none(), "FIRST's fills landed on SECOND");

        app.on_data(DataEvent::OrderHistory {
            request: second_request,
            result: Ok(vec![second]),
        });
        let Some(Modal::OrderDetail(state)) = app.modals.last() else {
            panic!("SECOND is still open");
        };
        assert!(matches!(&state.history, Some(Ok(rows)) if rows[0].order_id == "SECOND"));
    }

    #[test]
    fn auction_data_clamps_selection_and_opens_a_fixed_sell_ticket() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "INFY", 408_065,
        )])));
        app.set_pane(PaneId::InvestBids);
        app.bids_pane
            .set_section(crate::tui::bids::Section::Auctions);
        app.on_data(DataEvent::Auctions(Ok(vec![{
            let mut row = crate::tui::data::test_models::auction();
            let holding = holding("INFY", 408_066, 1500.0, 1523.4, 1510.0, 7);
            row.tradingsymbol = holding.tradingsymbol;
            row.exchange = holding.exchange;
            row.instrument_token = holding.instrument_token;
            row.quantity = holding.quantity;
            row.t1_quantity = holding.t1_quantity;
            row.collateral_quantity = holding.collateral_quantity;
            row.average_price = holding.average_price;
            row.last_price = holding.last_price;
            row.close_price = holding.close_price;
            row.discrepancy = holding.discrepancy;
            row.auction_number = "AUC-42".to_string();
            row
        }])));

        app.press(KeyCode::Char('G'));
        assert_eq!(app.bids_row_keys().len(), 1);
        assert_eq!(app.pane_row(PaneId::InvestBids), 0);

        app.on_key(char_key('s'));
        let Some(Modal::Ticket(ticket)) = app.modals.last() else {
            panic!("auction sell must open an order ticket");
        };
        assert!(ticket.is_auction());
        assert_eq!(ticket.side, Side::Sell);
        assert_eq!(ticket.instrument_token, 408_066);
        assert_eq!(ticket.qty, "7");
        assert_eq!(ticket.order_type, OrderType::Limit);
        assert_eq!(ticket.product, 0);
        assert_eq!(ticket.validity, 0);
    }

    #[test]
    fn ipo_application_navigation_reaches_the_oldest_application() {
        let mut app = test_app();
        app.set_pane(PaneId::InvestBids);
        app.bids_pane.set_section(crate::tui::bids::Section::Ipo);
        app.bids_pane.focus_applications();
        app.on_data(DataEvent::Bids(crate::tui::bids::Event::Applications(Ok(
            (0..4)
                .map(|index| crate::tui::bids::Application {
                    symbol: format!("IPO{index}"),
                    ..Default::default()
                })
                .collect(),
        ))));

        for _ in 0..4 {
            app.on_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        assert_eq!(app.bids_row_keys().len(), 4);
        assert_eq!(app.pane_row(PaneId::InvestBids), 3);
    }

    #[test]
    fn rest_quote_caches_circuits_on_success_and_retries_failures_after_reconnect() {
        let mut app = test_app();
        app.circuits_inflight.insert(408_065);
        app.on_event(AppEvent::Data(DataEvent::Circuits {
            token: 408_065,
            result: Ok(Some((100.0, 200.0))),
        }));
        assert_eq!(app.circuits.get(&408_065), Some(&Some((100.0, 200.0))));
        assert!(
            !app.circuits_inflight.contains(&408_065),
            "token must be cleared from the inflight set once its fetch lands"
        );

        app.circuits_inflight.insert(999);
        app.on_event(AppEvent::Data(DataEvent::Circuits {
            token: 999,
            result: Ok(None),
        }));
        assert_eq!(app.circuits.get(&999), Some(&None));
        assert!(!app.circuits_inflight.contains(&999));
        app.circuits_inflight.insert(1000);
        app.on_event(AppEvent::Data(DataEvent::Circuits {
            token: 1000,
            result: Err("quote request failed".into()),
        }));
        assert!(!app.circuits.contains_key(&1000), "a failure is not cached");
        assert!(app.circuits_inflight.contains(&1000));
        app.on_event(AppEvent::Ws(TickerEvent::Connected));
        assert!(
            app.circuits_inflight.is_empty(),
            "a reconnect clears failed fetches so they are retried"
        );
    }
}
