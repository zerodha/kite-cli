#[cfg(test)]
use super::ChartTarget;
use super::panes::alerts::alert_key;
use super::panes::gtt::gtt_key;
use super::panes::holdings::holding_key;
use super::panes::mf::mf_key;
use super::panes::orders::order_key;
use super::panes::positions::position_key;
use super::{App, Book, ToastLevel};
use crate::tui::data;
#[cfg(test)]
use crate::tui::pane_state::ChartBinding;
use crate::tui::workspace::PaneId;

impl App {
    pub(in crate::tui::app) fn ensure_circuits(&mut self) {
        let Some(item) = self.watch_pane.selected(&self.watch) else {
            return;
        };
        let token = item.instrument_token;
        if item.is_index()
            || self.circuits.contains_key(&token)
            || self.circuits_inflight.contains(&token)
        {
            return;
        }
        self.circuits_inflight.insert(token);
        data::spawn_fetch_circuits(self.client.clone(), self.events_tx.clone(), token);
    }

    #[cfg(test)]
    pub fn link_target(&self, source: PaneId) -> Option<ChartTarget> {
        self.resolve_chart_binding(ChartBinding::SelectionOf(source))
    }

    pub fn pane_row(&self, pane: PaneId) -> usize {
        match pane {
            PaneId::TradingWatch => self.watch_pane.display_row(&self.watch),
            PaneId::TradingOrders | PaneId::OrdersBook => {
                let order = self.orders_order_in(pane);
                let orders = self.orders.as_deref().unwrap_or_default();
                self.orders_pane_of(pane).map_or(0, |instance| {
                    instance
                        .list
                        .display_row(&order, |i| orders.get(i).map(order_key))
                })
            }
            PaneId::PortfolioPositions => {
                let order = self.positions_order_in(pane);
                let net = self.positions_net();
                self.books
                    .portfolio_positions
                    .display_row(&order, |i| net.get(i).map(position_key))
            }
            PaneId::PortfolioHoldings => {
                let order = self.holdings_order_in(pane);
                let holdings = self.holdings.as_deref().unwrap_or_default();
                self.books
                    .portfolio_holdings
                    .display_row(&order, |i| holdings.get(i).map(holding_key))
            }
            PaneId::OverviewPositions => self.overview_positions_row(),
            PaneId::OverviewHoldings => {
                self.overview_holdings_row(&self.overview_holdings_display_order())
            }
            PaneId::DerivativesChain => self.derivatives_chain.display_row(),
            PaneId::OrdersGtt => self.gtts_display_row(&self.gtts_display_order()),
            PaneId::OrdersAlerts => self.alerts_display_row(&self.alerts_display_order()),
            PaneId::PortfolioMf => self.mf_display_row(&self.mf_display_order()),
            PaneId::InvestBids => self.bids_pane.display_row(&self.bids_row_keys()),
            PaneId::OverviewIndices
            | PaneId::OverviewMovers
            | PaneId::TradingChart
            | PaneId::TradingDepth
            | PaneId::PortfolioFunds
            | PaneId::DerivativesChart => 0,
        }
    }

    pub(in crate::tui::app) fn links_read(&self, source: PaneId) -> bool {
        self.workspace.panes().iter().any(|pane| match pane {
            PaneId::TradingDepth => source == PaneId::TradingWatch,
            PaneId::TradingChart | PaneId::DerivativesChart => self
                .chart_of(*pane)
                .is_some_and(|chart| chart.binding().source() == source),
            _ => false,
        })
    }

    pub(in crate::tui::app) fn reconcile_book(&mut self, book: Book) {
        match book {
            Book::Positions => {
                let portfolio = self.positions_order_in(PaneId::PortfolioPositions);
                let natural = self.natural_positions_order();
                let net = self
                    .positions
                    .as_ref()
                    .map(|positions| positions.net.as_slice())
                    .unwrap_or_default();
                self.books
                    .portfolio_positions
                    .reconcile(&portfolio, |i| net.get(i).map(position_key));
                self.books
                    .overview_positions
                    .reconcile(&natural, |i| net.get(i).map(position_key));
            }
            Book::Orders => {
                let order = self.orders_order_in(PaneId::OrdersBook);
                let trading = self.orders_order_in(PaneId::TradingOrders);
                let orders = self.orders.as_deref().unwrap_or_default();
                self.books
                    .orders
                    .list
                    .reconcile(&order, |i| orders.get(i).map(order_key));
                self.books
                    .trading_orders
                    .list
                    .reconcile(&trading, |i| orders.get(i).map(order_key));
            }
            Book::Holdings => {
                let portfolio = self.holdings_order_in(PaneId::PortfolioHoldings);
                let overview = self.overview_holdings_display_order();
                let holdings = self.holdings.as_deref().unwrap_or_default();
                self.books
                    .portfolio_holdings
                    .reconcile(&portfolio, |i| holdings.get(i).map(holding_key));
                self.books
                    .overview_holdings
                    .reconcile(&overview, |i| holdings.get(i).map(holding_key));
            }
            Book::Mf => {
                let order = self.mf_display_order();
                let mf = self.mf_holdings.as_deref().unwrap_or_default();
                self.books.mf.reconcile(&order, |i| mf.get(i).map(mf_key));
            }
            Book::Gtt => {
                let order = self.gtts_display_order();
                let gtts = self.gtts.as_deref().unwrap_or_default();
                self.books
                    .gtt
                    .reconcile(&order, |i| gtts.get(i).map(gtt_key));
            }
            Book::Alerts => {
                let order = self.alerts_display_order();
                let alerts = self.alerts.as_deref().unwrap_or_default();
                self.books
                    .alerts
                    .reconcile(&order, |i| alerts.get(i).map(alert_key));
            }
        }
    }

    pub(in crate::tui::app) fn reconcile_watch(&mut self) {
        self.watch_pane.reconcile(&self.watch);
    }

    pub(in crate::tui::app) fn reconcile_bids(&mut self) {
        let keys = self.bids_row_keys();
        self.bids_pane.reconcile(&keys);
    }

    pub(in crate::tui::app) fn refresh_active(&mut self) {
        let client = self.client.clone();
        let tx = self.events_tx.clone();

        match self.focused_pane() {
            PaneId::TradingWatch => {
                self.reload_catalog();

                if !self.reload_watch_detail()
                    && let Some(pending) = self.watch.pending()
                {
                    self.toast(
                        ToastLevel::Info,
                        format!("still {pending}, the rows re-read when it lands"),
                    );
                }
            }

            PaneId::OverviewPositions
            | PaneId::OverviewHoldings
            | PaneId::OverviewIndices
            | PaneId::OverviewMovers => {
                data::spawn_refresh_positions(client.clone(), tx.clone());
                data::spawn_refresh_holdings(client.clone(), tx.clone());
                data::spawn_refresh_orders(client.clone(), tx.clone());

                self.refresh_movers();
            }
            PaneId::PortfolioPositions => {
                data::spawn_refresh_positions(client.clone(), tx.clone());
            }
            PaneId::TradingOrders | PaneId::OrdersBook => {
                data::spawn_refresh_orders(client.clone(), tx.clone());
            }
            PaneId::PortfolioHoldings | PaneId::PortfolioFunds => {
                data::spawn_refresh_holdings(client.clone(), tx.clone());
            }
            PaneId::PortfolioMf => data::spawn_refresh_mf_holdings(client.clone(), tx.clone()),
            PaneId::OrdersGtt => data::spawn_refresh_gtts(client.clone(), tx.clone()),

            PaneId::OrdersAlerts => {
                data::spawn_refresh_alerts(
                    client.clone(),
                    tx.clone(),
                    self.alert_fetch_requests.clone(),
                );
            }
            PaneId::InvestBids => {
                self.refresh_bids();
                data::spawn_refresh_auctions(client.clone(), tx.clone());
            }

            PaneId::DerivativesChain
            | PaneId::DerivativesChart
            | PaneId::TradingChart
            | PaneId::TradingDepth => {}
        }
        data::spawn_refresh_margins(client, tx);
        self.toast(ToastLevel::Info, "refreshing…");
    }
}

#[cfg(test)]
mod tests {
    use super::super::App;
    use super::super::panes::positions::tests::positions_book_app;
    use super::super::test_support::{char_key, position};
    use crate::tui::data::DataEvent;
    use crate::tui::event::AppEvent;
    use crate::tui::marketwatch::{State as WatchState, WatchItem};
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::{PaneId, Preset};
    use kiteconnect::KiteConnect;
    use tokio::sync::mpsc;

    #[test]
    fn a_refresh_keeps_each_order_books_own_record() {
        use super::super::test_support::sample_order;
        let order = |id: &str, ts: &str| {
            let mut order = sample_order("OPEN");
            order.order_id = id.to_string();
            order.order_timestamp = ts.replace(' ', "T").parse().expect("timestamp");
            order
        };
        let mut app = positions_book_app();
        app.on_data(DataEvent::Orders(Ok(vec![
            order("A", "2026-09-25 09:15:00"),
            order("B", "2026-09-25 09:16:00"),
            order("C", "2026-09-25 09:17:00"),
        ])));
        app.set_workspace(Preset::Orders);
        app.set_selection_in(PaneId::OrdersBook, 0);
        app.set_selection_in(PaneId::TradingOrders, 2);
        let chosen = |app: &App, pane| {
            app.selected_order_in(pane)
                .map(|order| order.order_id.clone())
        };
        let (book, trading) = (
            chosen(&app, PaneId::OrdersBook),
            chosen(&app, PaneId::TradingOrders),
        );
        assert_ne!(book, trading, "two cursors on two records");

        app.on_data(DataEvent::Orders(Ok(vec![
            order("D", "2026-09-25 09:18:00"),
            order("C", "2026-09-25 09:17:00"),
            order("B", "2026-09-25 09:16:00"),
            order("A", "2026-09-25 09:15:00"),
        ])));
        assert_eq!(chosen(&app, PaneId::OrdersBook), book);
        assert_eq!(chosen(&app, PaneId::TradingOrders), trading);
    }

    #[test]
    fn a_refresh_keeps_the_selected_record_not_its_row() {
        let mut app = positions_book_app();

        app.set_selection(2);
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("BBB".to_string())
        );

        app.on_data(DataEvent::Positions(Ok({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("AAA", 1, 10, 100.0, 100.0, 100.0),
                position("BBB", 2, 10, 900.0, 100.0, 100.0),
                position("CCC", 3, 10, 300.0, 100.0, 100.0),
            ];
            row
        })));
        assert_eq!(
            app.pane_row(PaneId::PortfolioPositions),
            0,
            "BBB moved to the top"
        );
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("BBB".to_string()),
            "the record, not row 2"
        );
    }

    #[test]
    fn a_vanished_record_clears_the_action_selection() {
        let mut app = positions_book_app();

        app.set_selection(0);
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("CCC".to_string())
        );

        app.on_data(DataEvent::Positions(Ok({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("AAA", 1, 10, 100.0, 100.0, 100.0),
                position("BBB", 2, 10, -50.0, 100.0, 100.0),
            ];
            row
        })));
        assert_eq!(
            app.positions_order_in(PaneId::PortfolioPositions),
            vec![0, 1],
            "AAA, BBB"
        );
        assert!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .is_none(),
            "the pane must not adopt AAA just because it took the row"
        );
        app.on_key(char_key('s'));
        assert!(app.modals.is_empty(), "and no key may open a ticket for it");

        assert_eq!(app.pane_row(PaneId::PortfolioPositions), 0);
        app.on_key(char_key('s'));
        assert!(app.modals.is_empty(), "still nothing chosen");
        app.set_selection(0);
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("AAA".to_string()),
            "an explicit row choice brings the pane back"
        );
    }

    #[tokio::test]
    async fn tab_reaches_mf_in_portfolio_and_r_refreshes_its_holdings() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/portfolio/holdings/mf");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(include_str!("../../../mock_responses/mf_holdings.json"));
            })
            .await;
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .base_url(server.base_url())
            .build()
            .unwrap();
        let (events_tx, mut events_rx) = mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            WatchState::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        app.switch_workspace(Preset::Portfolio);
        let tab = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Tab,
            crossterm::event::KeyModifiers::NONE,
        );
        let mut order = vec![app.focused_pane()];
        for _ in 0..4 {
            app.on_key(tab);
            order.push(app.focused_pane());
        }
        assert_eq!(
            order,
            [
                PaneId::PortfolioPositions,
                PaneId::PortfolioMf,
                PaneId::PortfolioHoldings,
                PaneId::PortfolioFunds,
                PaneId::PortfolioPositions,
            ]
        );

        app.on_key(tab);
        app.on_key(char_key('r'));
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let event = tokio::time::timeout_at(deadline, events_rx.recv())
                .await
                .expect("MF holdings arrive within the deadline")
                .unwrap();
            if matches!(event, AppEvent::Data(DataEvent::MfHoldings(_))) {
                app.on_event(event);
                break;
            }
        }
        assert_eq!(app.mf_holdings.as_ref().map(Vec::len), Some(3));
        mock.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn ensure_circuits_marks_inflight_and_dedupes_refetch() {
        let server = httpmock::MockServer::start_async().await;
        let mock = server
            .mock_async(|when, then| {
                when.method(httpmock::Method::GET)
                    .path("/quote")
                    .query_param("i", "408065");
                then.status(200).json_body(serde_json::json!({
                "status": "success",
                "data": {"NSE:INFY": {"instrument_token":408_065,"timestamp":"2026-10-13 10:00:00","last_price":150.0,"ohlc":{"open":150.0,"high":160.0,"low":140.0,"close":145.0},"net_change":5.0,"lower_circuit_limit":100.0,"upper_circuit_limit":200.0}}
            }));
            })
            .await;
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .base_url(server.base_url())
            .build()
            .unwrap();
        let (events_tx, mut events_rx) = mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            WatchState::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: 408_065,
            ..WatchItem::default()
        });
        app.ensure_circuits();
        app.ensure_circuits();
        let event = tokio::time::timeout(std::time::Duration::from_secs(5), events_rx.recv())
            .await
            .unwrap()
            .unwrap();
        app.on_event(event);
        assert_eq!(app.circuits.get(&408_065), Some(&Some((100.0, 200.0))));
        app.ensure_circuits();

        drop(app);
        let rest = tokio::time::timeout(std::time::Duration::from_secs(5), events_rx.recv())
            .await
            .unwrap();
        assert!(rest.is_none(), "no second fetch was spawned");
        mock.assert_calls_async(1).await;
    }
}
