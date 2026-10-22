pub(super) mod alerts;
pub(super) mod bids;
pub(super) mod chain;
pub(super) mod chart;
pub(super) mod gtt;
pub(super) mod holdings;
pub(super) mod mf;
pub(super) mod orders;
pub(super) mod positions;
pub(super) mod watch;

use super::{
    AlertsSort, App, GttSort, HoldingsSort, KeyCode, KeyEvent, MfSort, OrdersSort, PositionsSort,
};
use crate::tui::pane_state;
use crate::tui::workspace::PaneId;
use alerts::alert_key;
use gtt::gtt_key;
use holdings::holding_key;
use mf::mf_key;
use orders::order_key;
use positions::position_key;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortTarget {
    Positions(PositionsSort),
    Orders(OrdersSort),
    Holdings(HoldingsSort),
    Mf(MfSort),
    Gtt(GttSort),
    Alerts(AlertsSort),
}

impl SortTarget {
    fn book(self) -> Book {
        match self {
            SortTarget::Positions(_) => Book::Positions,
            SortTarget::Orders(_) => Book::Orders,
            SortTarget::Holdings(_) => Book::Holdings,
            SortTarget::Mf(_) => Book::Mf,
            SortTarget::Gtt(_) => Book::Gtt,
            SortTarget::Alerts(_) => Book::Alerts,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Book {
    Positions,
    Orders,
    Holdings,
    Mf,
    Gtt,
    Alerts,
}

fn book_of(pane: PaneId) -> Option<Book> {
    match pane {
        PaneId::PortfolioPositions | PaneId::OverviewPositions => Some(Book::Positions),
        PaneId::OrdersBook | PaneId::TradingOrders => Some(Book::Orders),
        PaneId::PortfolioHoldings | PaneId::OverviewHoldings => Some(Book::Holdings),
        PaneId::PortfolioMf => Some(Book::Mf),
        PaneId::OrdersGtt => Some(Book::Gtt),
        PaneId::OrdersAlerts => Some(Book::Alerts),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RowStep {
    By(isize),

    To(usize),
}

impl RowStep {
    fn apply<K: PartialEq, S>(
        self,
        pane: &mut pane_state::ListPane<K, S>,
        order: &[usize],
        key: impl Fn(usize) -> Option<K>,
    ) -> Option<bool> {
        if order.is_empty() {
            return None;
        }
        Some(match self {
            Self::By(delta) => pane.step(delta, order, key),
            Self::To(row) => pane.select(row, order, key),
        })
    }
}

pub(in crate::tui::app) fn cmp_desc(a: f64, b: f64) -> std::cmp::Ordering {
    b.partial_cmp(&a).unwrap_or(std::cmp::Ordering::Equal)
}
impl App {
    pub(in crate::tui::app) fn key_pane(&mut self, pane: PaneId, key: KeyEvent) {
        if key.code == KeyCode::Char('S')
            && matches!(
                pane,
                PaneId::PortfolioPositions
                    | PaneId::PortfolioHoldings
                    | PaneId::TradingOrders
                    | PaneId::OrdersBook
                    | PaneId::PortfolioMf
                    | PaneId::OrdersGtt
                    | PaneId::OrdersAlerts
            )
        {
            self.cycle_sort(pane);
            return;
        }
        match pane {
            PaneId::TradingWatch => self.key_watch(pane, key),
            PaneId::PortfolioPositions => self.key_positions(pane, key),
            PaneId::TradingOrders | PaneId::OrdersBook => self.key_orders(pane, key),
            PaneId::PortfolioHoldings => self.key_holdings(pane, key),
            PaneId::OrdersGtt => self.key_gtt(key),
            PaneId::OrdersAlerts => self.key_alerts(key),
            PaneId::DerivativesChain => self.key_chain(pane, key),
            PaneId::InvestBids => self.key_bids(key),
            _ => {}
        }
    }

    fn move_cursor(&mut self, pane: PaneId, step: RowStep) -> Option<bool> {
        match pane {
            PaneId::PortfolioPositions => {
                let order = self.positions_order_in(pane);
                let net = self
                    .positions
                    .as_ref()
                    .map(|positions| positions.net.as_slice())
                    .unwrap_or_default();
                step.apply(&mut self.books.portfolio_positions, &order, |i| {
                    net.get(i).map(position_key)
                })
            }
            PaneId::OverviewPositions => {
                let order = self.natural_positions_order();
                let net = self
                    .positions
                    .as_ref()
                    .map(|positions| positions.net.as_slice())
                    .unwrap_or_default();
                step.apply(&mut self.books.overview_positions, &order, |i| {
                    net.get(i).map(position_key)
                })
            }
            PaneId::OrdersBook | PaneId::TradingOrders => {
                let order = self.orders_order_in(pane);
                let orders = self.orders.as_deref().unwrap_or_default();
                let instance = match pane {
                    PaneId::TradingOrders => &mut self.books.trading_orders.list,
                    _ => &mut self.books.orders.list,
                };
                step.apply(instance, &order, |i| orders.get(i).map(order_key))
            }
            PaneId::PortfolioHoldings => {
                let order = self.holdings_order_in(pane);
                let holdings = self.holdings.as_deref().unwrap_or_default();
                step.apply(&mut self.books.portfolio_holdings, &order, |i| {
                    holdings.get(i).map(holding_key)
                })
            }
            PaneId::OverviewHoldings => {
                let order = self.overview_holdings_display_order();
                let holdings = self.holdings.as_deref().unwrap_or_default();
                step.apply(&mut self.books.overview_holdings, &order, |i| {
                    holdings.get(i).map(holding_key)
                })
            }
            PaneId::PortfolioMf => {
                let order = self.mf_display_order();
                let mf = self.mf_holdings.as_deref().unwrap_or_default();
                step.apply(&mut self.books.mf, &order, |i| mf.get(i).map(mf_key))
            }
            PaneId::OrdersGtt => {
                let order = self.gtts_display_order();
                let gtts = self.gtts.as_deref().unwrap_or_default();
                step.apply(&mut self.books.gtt, &order, |i| gtts.get(i).map(gtt_key))
            }
            PaneId::OrdersAlerts => {
                let order = self.alerts_display_order();
                let alerts = self.alerts.as_deref().unwrap_or_default();
                step.apply(&mut self.books.alerts, &order, |i| {
                    alerts.get(i).map(alert_key)
                })
            }
            PaneId::TradingWatch => {
                if self.watch_pane.rows(&self.watch).is_empty() {
                    return None;
                }
                Some(match step {
                    RowStep::By(delta) => self.watch_pane.step(delta, &self.watch),
                    RowStep::To(row) => self.watch_pane.select(row, &self.watch),
                })
            }
            PaneId::DerivativesChain => {
                if self
                    .chain_pane_of(pane)
                    .is_none_or(|instance| instance.rows().is_empty())
                {
                    return None;
                }
                let instance = self.chain_instance_mut(pane)?;
                Some(match step {
                    RowStep::By(delta) => instance.step(delta),
                    RowStep::To(row) => instance.select(row),
                })
            }
            PaneId::InvestBids => {
                let keys = self.bids_row_keys();
                if keys.is_empty() {
                    return None;
                }
                Some(match step {
                    RowStep::By(delta) => self.bids_pane.step(delta, &keys),
                    RowStep::To(row) => self.bids_pane.select(row, &keys),
                })
            }

            _ => None,
        }
    }

    pub(in crate::tui::app) fn move_selection(&mut self, delta: isize) {
        self.move_selection_in(self.focused_pane(), delta);
    }

    pub(in crate::tui::app) fn move_selection_in(&mut self, pane: PaneId, delta: isize) {
        let Some(changed) = self.move_cursor(pane, RowStep::By(delta)) else {
            return;
        };
        self.after_selection_change(pane, changed);
    }

    pub(in crate::tui::app) fn set_selection(&mut self, target: usize) {
        self.set_selection_in(self.focused_pane(), target);
    }

    pub(in crate::tui::app) fn set_selection_in(&mut self, pane: PaneId, target: usize) {
        let Some(changed) = self.move_cursor(pane, RowStep::To(target)) else {
            return;
        };
        self.after_selection_change(pane, changed);
    }

    fn after_selection_change(&mut self, pane: PaneId, changed: bool) {
        if changed && self.links_read(pane) {
            self.sync_subscriptions();
            return;
        }
        if changed && self.chart_panel_pane() == Some(pane) {
            self.publish_chart_target();
            self.chart_pane.set_cursor(None);
            self.sync_subscriptions();
        }
    }

    fn resort(&mut self, book: Book, apply: impl FnOnce(&mut Self)) {
        apply(self);
        self.reconcile_book(book);
        self.hit.borrow_mut().invalidate();
    }

    pub(in crate::tui::app) fn cycle_sort(&mut self, pane: PaneId) {
        let Some(book) = book_of(pane) else {
            return;
        };
        self.resort(book, |app| match pane {
            PaneId::PortfolioPositions => {
                app.books
                    .portfolio_positions
                    .set_sort(app.books.portfolio_positions.sort().next());
            }
            PaneId::OrdersBook => {
                app.books
                    .orders
                    .list
                    .set_sort(app.books.orders.list.sort().next());
            }
            PaneId::TradingOrders => {
                app.books
                    .trading_orders
                    .list
                    .set_sort(app.books.trading_orders.list.sort().next());
            }
            PaneId::PortfolioHoldings => {
                app.books
                    .portfolio_holdings
                    .set_sort(app.books.portfolio_holdings.sort().next());
            }
            PaneId::PortfolioMf => {
                app.books.mf.set_sort(app.books.mf.sort().next());
            }
            PaneId::OrdersGtt => {
                app.books.gtt.set_sort(app.books.gtt.sort().next());
            }
            PaneId::OrdersAlerts => {
                app.books.alerts.set_sort(app.books.alerts.sort().next());
            }

            _ => {}
        });
    }

    pub(in crate::tui::app) fn sort_by_header(&mut self, owner: PaneId, target: SortTarget) {
        let Some(book) = book_of(owner) else {
            return;
        };

        if target.book() != book {
            return;
        }
        self.resort(book, |app| {
            fn apply<K, S: Copy + PartialEq>(
                instance: &mut pane_state::ListPane<K, S>,
                mode: S,
                next: impl FnOnce(S) -> S,
            ) {
                let sort = instance.sort();
                instance.set_sort(if sort == mode { next(mode) } else { mode });
            }
            match (owner, target) {
                (PaneId::PortfolioPositions, SortTarget::Positions(mode)) => apply(
                    &mut app.books.portfolio_positions,
                    mode,
                    PositionsSort::next,
                ),
                (PaneId::OrdersBook, SortTarget::Orders(mode)) => {
                    apply(&mut app.books.orders.list, mode, OrdersSort::next);
                }
                (PaneId::TradingOrders, SortTarget::Orders(mode)) => {
                    apply(&mut app.books.trading_orders.list, mode, OrdersSort::next);
                }
                (PaneId::PortfolioHoldings, SortTarget::Holdings(mode)) => {
                    apply(&mut app.books.portfolio_holdings, mode, HoldingsSort::next);
                }
                (PaneId::PortfolioMf, SortTarget::Mf(mode)) => {
                    apply(&mut app.books.mf, mode, MfSort::next);
                }
                (PaneId::OrdersGtt, SortTarget::Gtt(mode)) => {
                    apply(&mut app.books.gtt, mode, GttSort::next);
                }
                (PaneId::OrdersAlerts, SortTarget::Alerts(mode)) => {
                    apply(&mut app.books.alerts, mode, AlertsSort::next);
                }

                _ => {}
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        char_key, holding, position, render, sample_order, server_alert, test_app,
    };
    use super::super::{
        AlertsSort, GttSort, HoldingsSort, MfSort, Modal, OrdersSort, PositionsSort,
    };
    use super::positions::tests::positions_book_app;
    use crate::tui::data::DataEvent;
    use crate::tui::ticket::PendingAction;
    use crate::tui::workspace::{PaneId, Preset};

    #[tokio::test]
    async fn every_former_single_view_keeps_its_keys_as_a_pane() {
        let mut app = test_app();
        app.orders = Some(vec![sample_order("OPEN")]);
        app.gtts = Some(vec![{
            let mut row = crate::tui::data::test_models::gtt();
            row.id = 7;
            row.trigger_type = "single".into();
            row.status = "active".into();
            row.condition.exchange = "NSE".into();
            row.condition.tradingsymbol = "INFY".into();
            row.condition.trigger_values = vec![100.0];
            row.condition.last_price = 95.0;
            row
        }]);
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        app.mf_holdings = Some(vec![{
            let mut row = crate::tui::data::test_models::mf();
            row.tradingsymbol = "FUND A".into();
            row
        }]);

        app.set_workspace(Preset::Orders);
        render(&app, 160, 40);
        assert_eq!(app.hidden_panes(), [], "the workspace tiles at 160x40");
        assert_eq!(app.focused_pane(), PaneId::OrdersBook);
        let trading = app.books.trading_orders.list.sort();
        app.on_key(char_key('S'));
        assert_ne!(app.books.orders.list.sort(), OrdersSort::default());
        assert_eq!(app.books.trading_orders.list.sort(), trading);
        app.on_key(char_key('x'));
        let Some(Modal::Confirm(confirm)) = app.modals.pop() else {
            panic!("x on an open order confirms its cancel");
        };
        assert!(matches!(confirm.action, PendingAction::CancelOrders { .. }));
        app.on_key(char_key('c'));
        assert_eq!(app.chart_panel_pane(), Some(PaneId::OrdersBook));
        app.on_key(char_key('c'));

        app.focus_pane_for_test(PaneId::OrdersGtt);
        app.on_key(char_key('S'));
        assert_ne!(app.books.gtt.sort(), GttSort::default());
        app.on_key(char_key('x'));
        let Some(Modal::Confirm(confirm)) = app.modals.pop() else {
            panic!("x on a GTT confirms its delete");
        };
        assert!(matches!(
            confirm.action,
            PendingAction::DeleteGtt { trigger_id: 7 }
        ));

        app.focus_pane_for_test(PaneId::OrdersAlerts);
        app.on_key(char_key('S'));
        assert_ne!(app.books.alerts.sort(), AlertsSort::default());
        app.on_key(char_key('d'));
        let Some(Modal::Confirm(confirm)) = app.modals.pop() else {
            panic!("d on an alert confirms its delete");
        };
        assert!(matches!(confirm.action, PendingAction::DeleteAlert { .. }));

        app.set_workspace(Preset::Portfolio);
        app.focus_pane_for_test(PaneId::PortfolioMf);
        app.on_key(char_key('S'));
        assert_ne!(app.books.mf.sort(), MfSort::default());
        app.set_workspace(Preset::Invest);
        assert_eq!(app.focused_pane(), PaneId::InvestBids);
        let section = app.bids_pane.section();
        app.on_key(char_key(']'));
        assert_eq!(app.bids_pane.section(), section.cycle(1));
        app.on_key(char_key('['));
        assert_eq!(app.bids_pane.section(), section);
    }

    #[tokio::test]
    async fn reselecting_a_lost_rows_replacement_republishes_the_chart() {
        let mut app = test_app();
        let mut first = sample_order("OPEN");
        first.order_id = "FIRST".into();
        let mut second = sample_order("OPEN");
        second.order_id = "SECOND".into();
        second.tradingsymbol = "TCS".into();
        second.instrument_token = 2_953_217;
        app.on_data(DataEvent::Orders(Ok(vec![first.clone(), second.clone()])));
        app.show_pane(PaneId::OrdersBook);
        app.set_selection(0);
        app.on_key(char_key('c'));
        let opened = app.chart_pane.target().map(|t| t.instrument_token);
        let survivor = if opened == Some(first.instrument_token) {
            second
        } else {
            first
        };

        app.on_data(DataEvent::Orders(Ok(vec![survivor.clone()])));
        assert_eq!(app.chart_pane.target(), None, "the chart's order left");

        app.set_selection(0);
        assert_eq!(
            app.chart_pane.target().map(|t| t.instrument_token),
            Some(survivor.instrument_token),
            "the replacement at the same row is the chart's new target"
        );
    }

    #[test]
    fn overview_movement_is_bounded_by_the_focused_book() {
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("AAA", 1, 10, 0.0, 100.0, 100.0),
                position("BBB", 2, 10, 0.0, 100.0, 100.0),
            ];
            row
        });
        app.holdings = Some(vec![holding("CCC", 3, 100.0, 100.0, 100.0, 5)]);

        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);
        app.on_key(char_key('G'));
        assert_eq!(app.pane_row(app.focused_pane()), 1, "two positions");

        app.workspaces
            .focus(Preset::Overview, PaneId::OverviewHoldings);
        app.on_key(char_key('G'));
        assert_eq!(app.pane_row(app.focused_pane()), 0, "one holding");
    }

    #[test]
    fn overview_books_keep_independent_selections() {
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("AAA", 1, 10, 0.0, 100.0, 100.0),
                position("BBB", 2, 10, 0.0, 100.0, 100.0),
            ];
            row
        });
        app.holdings = Some(vec![
            holding("CCC", 3, 100.0, 100.0, 100.0, 5),
            holding("DDD", 4, 100.0, 100.0, 100.0, 5),
        ]);
        app.set_selection(1);
        assert_eq!(app.pane_row(app.focused_pane()), 1);

        app.on_key(char_key(']'));
        assert_eq!(app.focused_pane(), PaneId::OverviewHoldings);
        assert_eq!(app.pane_row(app.focused_pane()), 0);
        app.set_selection(1);

        app.on_key(char_key('['));
        assert_eq!(app.focused_pane(), PaneId::OverviewPositions);
        assert_eq!(app.pane_row(app.focused_pane()), 1);
        app.on_key(char_key(']'));
        assert_eq!(app.pane_row(app.focused_pane()), 1);
    }

    #[test]
    fn a_resort_keeps_the_selected_record_not_its_row() {
        let mut app = positions_book_app();
        app.set_selection(2);
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("BBB".to_string())
        );

        app.on_key(char_key('S'));
        assert_eq!(app.books.portfolio_positions.sort(), PositionsSort::Qty);
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("BBB".to_string()),
            "the sort moved the rows, not the selection"
        );
    }

    #[test]
    fn every_sort_enum_lists_its_whole_cycle() {
        macro_rules! assert_cycle {
            ($enum:ty) => {{
                let all = <$enum>::ALL;
                let mut mode = <$enum>::default();
                let start = all
                    .iter()
                    .position(|listed| *listed == mode)
                    .expect("the default mode is listed");
                for step in 0..all.len() {
                    assert_eq!(
                        mode,
                        all[(start + step) % all.len()],
                        "{} step {step} is not the mode ALL lists",
                        stringify!($enum)
                    );
                    mode = mode.next();
                }
                assert_eq!(
                    mode,
                    <$enum>::default(),
                    "{} does not return to its default after one cycle",
                    stringify!($enum)
                );
            }};
        }
        assert_cycle!(PositionsSort);
        assert_cycle!(OrdersSort);
        assert_cycle!(HoldingsSort);
        assert_cycle!(MfSort);
        assert_cycle!(GttSort);
        assert_cycle!(AlertsSort);
    }
}
