use super::super::{App, PaneId, ToastLevel};
use crate::tui::data::{self, Order};
use crate::tui::pane_state;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OrdersSort {
    #[default]
    Time,
    Status,
    Symbol,
}

impl OrdersSort {
    pub const ALL: [Self; 3] = [Self::Time, Self::Status, Self::Symbol];

    pub(in crate::tui::app) fn next(self) -> Self {
        match self {
            OrdersSort::Time => OrdersSort::Status,
            OrdersSort::Status => OrdersSort::Symbol,
            OrdersSort::Symbol => OrdersSort::Time,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            OrdersSort::Time => "time ▼",
            OrdersSort::Status => "status ▲",
            OrdersSort::Symbol => "symbol ▲",
        }
    }
}

pub(in crate::tui::app) fn order_key(order: &Order) -> String {
    order.order_id.clone()
}

pub(in crate::tui::app) fn order_identity(order: &Order) -> String {
    format!("order:{}", order.order_id)
}

pub fn orders_order(orders: &[Order], mode: OrdersSort) -> Vec<usize> {
    let mut order: Vec<usize> = (0..orders.len()).collect();
    match mode {
        OrdersSort::Time => order.sort_by(|&a, &b| {
            orders[b]
                .order_timestamp
                .cmp(&orders[a].order_timestamp)
                .then(
                    crate::tui::data::order_is_open(&orders[b])
                        .cmp(&crate::tui::data::order_is_open(&orders[a])),
                )
        }),
        OrdersSort::Status => {
            order.sort_by(|&a, &b| orders[a].status.cmp(&orders[b].status));
        }
        OrdersSort::Symbol => {
            order.sort_by(|&a, &b| orders[a].tradingsymbol.cmp(&orders[b].tradingsymbol));
        }
    }
    order
}

impl App {
    pub fn orders_pane_of(&self, pane: PaneId) -> Option<&pane_state::OrdersPane> {
        match pane {
            PaneId::OrdersBook => Some(&self.books.orders),
            PaneId::TradingOrders => Some(&self.books.trading_orders),
            _ => None,
        }
    }

    pub(in crate::tui::app) fn orders_instance_mut(
        &mut self,
        pane: PaneId,
    ) -> Option<&mut pane_state::OrdersPane> {
        match pane {
            PaneId::OrdersBook => Some(&mut self.books.orders),
            PaneId::TradingOrders => Some(&mut self.books.trading_orders),
            _ => None,
        }
    }

    pub(in crate::tui::app) fn key_orders(&mut self, pane: PaneId, key: KeyEvent) {
        match key.code {
            KeyCode::Char(' ') => {
                self.toggle_selected_order_mark(pane);
                return;
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.toggle_all_open_order_marks(pane);
                return;
            }
            _ => {}
        }
        let Some(order) = self.selected_order_in(pane).cloned() else {
            return;
        };
        match key.code {
            KeyCode::Enter => self.open_order_detail(Box::new(order)),
            KeyCode::Char('x') => {
                if self.trading_disabled() {
                    return;
                }
                if !self
                    .orders_pane_of(pane)
                    .is_some_and(pane_state::OrdersPane::has_marks)
                {
                    if !data::order_is_open(&order) {
                        self.toast(
                            ToastLevel::Error,
                            format!("order {} is not open", order.order_id),
                        );
                        return;
                    }
                    self.open_orders_cancel_confirm(&[order], false);
                    return;
                }

                let marked = self.marked_open_orders(pane);
                if !marked.is_empty() {
                    self.open_orders_cancel_confirm(&marked, true);
                }
            }
            KeyCode::Char('m') => {
                if !data::order_is_open(&order) {
                    self.toast(
                        ToastLevel::Error,
                        format!("order {} is not open", order.order_id),
                    );
                    return;
                }
                self.open_modify_ticket(&order);
            }
            _ => {}
        }
    }

    pub fn orders_order_in(&self, pane: PaneId) -> Vec<usize> {
        let Some(instance) = self.orders_pane_of(pane) else {
            return Vec::new();
        };
        let Some(orders) = self.orders.as_deref() else {
            return Vec::new();
        };
        instance
            .list
            .cached_order(self.orders_gen, |sort| orders_order(orders, sort))
    }

    pub(in crate::tui::app) fn selected_order_in(&self, pane: PaneId) -> Option<&Order> {
        let instance = self.orders_pane_of(pane)?;
        let order = self.orders_order_in(pane);
        let orders = self.orders.as_deref().unwrap_or_default();
        instance
            .list
            .target(&order, |i| orders.get(i).map(order_key))
            .and_then(|natural| orders.get(natural))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::OrdersSort;
    use super::super::super::test_support::{sample_order, test_app};
    use super::orders_order;
    use crate::tui::data::{DataEvent, Order};
    use crate::tui::workspace::PaneId;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn order(id: &str, ts: &str, status: &str, symbol: &str) -> Order {
        {
            let mut row = crate::tui::data::test_models::order();
            row.order_id = id.into();
            row.order_timestamp = ts.replace(' ', "T").parse().expect("timestamp");
            row.status = status.into();
            row.status_message = None;
            row.exchange = "NSE".into();
            row.tradingsymbol = symbol.into();
            row.instrument_token = 0;
            row.transaction_type = "BUY".into();
            row.order_type = "LIMIT".into();
            row.product = "CNC".into();
            row.validity = "DAY".into();
            row.validity_ttl = None;
            row.variety = "regular".into();
            row.tags = Vec::new();
            row.price = 0.0;
            row.trigger_price = 0.0;
            row.average_price = 0.0;
            row.quantity = 0.0;
            row.filled_quantity = 0.0;
            row.pending_quantity = 0.0;
            row
        }
    }

    #[test]
    fn orders_sort_time_newest_first_then_open() {
        let os = [
            order("a", "2024-01-01 09:00:00", "COMPLETE", "ZZZ"),
            order("b", "2024-01-01 10:00:00", "COMPLETE", "YYY"),
            order("c", "2024-01-01 10:00:00", "OPEN", "XXX"),
        ];
        assert_eq!(orders_order(&os, OrdersSort::Time), vec![2, 1, 0]);
    }

    #[test]
    fn orders_sort_status_and_symbol_ascend() {
        let os = [
            order("a", "2024-01-01 10:00:00", "OPEN", "ZZZ"),
            order("b", "2024-01-01 09:00:00", "CANCELLED", "AAA"),
            order("c", "2024-01-01 11:00:00", "COMPLETE", "MMM"),
        ];

        assert_eq!(orders_order(&os, OrdersSort::Status), vec![1, 2, 0]);

        assert_eq!(orders_order(&os, OrdersSort::Symbol), vec![1, 2, 0]);
    }

    #[test]
    fn orders_chart_reconcile_keeps_exact_order_with_duplicate_token() {
        let mut app = test_app();
        let mut first = sample_order("COMPLETE");
        first.order_id = "OID1".into();
        first.order_timestamp = "2024-01-01T09:15:00".parse().expect("timestamp");
        let mut second = sample_order("COMPLETE");
        second.order_id = "OID2".into();
        second.order_timestamp = "2024-01-01T09:14:00".parse().expect("timestamp");
        app.set_pane(PaneId::OrdersBook);
        app.on_data(DataEvent::Orders(Ok(vec![first.clone(), second.clone()])));
        app.open_chart_panel_for_test(PaneId::OrdersBook);

        first.order_timestamp = "2024-01-01T09:13:00".parse().expect("timestamp");
        second.order_timestamp = "2024-01-01T09:16:00".parse().expect("timestamp");
        app.on_data(DataEvent::Orders(Ok(vec![first, second])));

        assert_eq!(app.pane_row(PaneId::OrdersBook), 1);
        assert_eq!(
            app.chart_pane
                .target()
                .as_ref()
                .map(|target| target.row_identity.as_str()),
            Some("order:OID1")
        );
    }

    #[test]
    fn ctrl_a_marks_only_open_orders_and_toggles_them() {
        let mut app = test_app();
        let mut open = sample_order("OPEN");
        open.order_id = "OID1".into();
        let mut pending = sample_order("TRIGGER PENDING");
        pending.order_id = "OID2".into();
        let mut closed = sample_order("COMPLETE");
        closed.order_id = "OID3".into();
        app.orders = Some(vec![open, pending, closed]);
        app.set_pane(PaneId::OrdersBook);
        let ctrl_a = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL);

        app.set_workspace(PaneId::OrdersBook.preset());
        app.focus_pane(PaneId::OrdersBook);
        app.on_key(ctrl_a);
        assert!(app.is_order_marked_in(PaneId::OrdersBook, "OID1"));
        assert!(app.is_order_marked_in(PaneId::OrdersBook, "OID2"));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID3"));

        app.set_workspace(PaneId::OrdersBook.preset());
        app.focus_pane(PaneId::OrdersBook);
        app.on_key(ctrl_a);
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID1"));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID2"));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID3"));
    }

    #[test]
    fn orders_display_order_cache_invalidates_on_data_and_sort_change() {
        let mut app = test_app();
        app.orders = Some(vec![sample_order("OPEN")]);
        assert_eq!(app.orders_order_in(PaneId::OrdersBook), vec![0]);

        app.orders = Some(vec![sample_order("COMPLETE"), sample_order("OPEN")]);
        app.orders_gen += 1;
        assert_eq!(
            app.orders_order_in(PaneId::OrdersBook),
            vec![1, 0],
            "OPEN floats ahead of COMPLETE under the default Time sort tiebreak"
        );

        app.set_pane(PaneId::OrdersBook);
        app.cycle_sort(PaneId::OrdersBook);
        assert_eq!(app.books.orders.list.sort(), OrdersSort::Status);
        assert_eq!(
            app.orders_order_in(PaneId::OrdersBook),
            orders_order(app.orders.as_ref().unwrap(), OrdersSort::Status),
            "cache must reflect the new sort mode, not the stale Time order"
        );
    }
}
