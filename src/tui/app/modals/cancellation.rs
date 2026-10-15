use super::super::{App, Modal, ToastLevel};
use crate::tui::data::{self, Order};
use crate::tui::ticket::{ConfirmState, PendingAction};
use crate::tui::workspace::PaneId;

impl App {
    pub(in crate::tui::app) fn on_orders_cancelled(&mut self, result: &data::CancelOrdersResult) {
        self.pending_action = false;
        let released_positions = !result.cancelled_order_ids.is_empty();
        let level = if result.failures.is_empty() {
            ToastLevel::Success
        } else {
            ToastLevel::Error
        };
        self.toast(level, result.summary());

        data::spawn_refresh_orders(self.client.clone(), self.events_tx.clone());
        if released_positions {
            data::spawn_refresh_positions(self.client.clone(), self.events_tx.clone());
        }
    }

    pub(in crate::tui::app) fn toggle_selected_order_mark(&mut self, pane: PaneId) {
        let Some(order) = self.selected_order_in(pane) else {
            return;
        };
        if !data::order_is_open(order) {
            return;
        }
        let order_id = order.order_id.clone();
        if let Some(instance) = self.orders_instance_mut(pane) {
            instance.toggle_mark(order_id);
        }
    }

    pub(in crate::tui::app) fn toggle_all_open_order_marks(&mut self, pane: PaneId) {
        let open_order_ids: Vec<String> = self
            .orders
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|order| data::order_is_open(order))
            .map(|order| order.order_id.clone())
            .collect();
        if let Some(instance) = self.orders_instance_mut(pane) {
            instance.toggle_marks(open_order_ids);
        }
    }

    pub(in crate::tui::app) fn marked_open_orders(&self, pane: PaneId) -> Vec<Order> {
        let (Some(orders), Some(instance)) = (self.orders.as_ref(), self.orders_pane_of(pane))
        else {
            return Vec::new();
        };
        self.orders_order_in(pane)
            .into_iter()
            .filter_map(|index| orders.get(index))
            .filter(|order| data::order_is_open(order) && instance.is_marked(&order.order_id))
            .cloned()
            .collect()
    }

    pub fn is_order_marked_in(&self, pane: PaneId, order_id: &str) -> bool {
        self.orders_pane_of(pane)
            .is_some_and(|instance| instance.is_marked(order_id))
    }

    pub(in crate::tui::app) fn reconcile_marked_orders(&mut self, orders: &[Order]) {
        let still_open = |order_id: &str| {
            orders
                .iter()
                .any(|order| order.order_id.as_str() == order_id && data::order_is_open(order))
        };
        self.books.orders.retain_marks(still_open);
        self.books.trading_orders.retain_marks(still_open);
    }

    pub(in crate::tui::app) fn open_orders_cancel_confirm(
        &mut self,
        orders: &[Order],
        batch: bool,
    ) {
        let Some(first) = orders.first() else {
            return;
        };
        let targets = orders
            .iter()
            .map(|order| data::CancelOrderTarget {
                parent_order_id: order.parent_order_id.clone(),
                variety: if order.variety.is_empty() {
                    "regular".to_string()
                } else {
                    order.variety.clone()
                },
                order_id: order.order_id.clone(),
            })
            .collect();

        let (title, lines) = if batch {
            const PREVIEW_LIMIT: usize = 6;
            let mut lines = Vec::with_capacity(PREVIEW_LIMIT + 2);
            lines.push(("Orders".to_string(), orders.len().to_string()));
            lines.extend(orders.iter().take(PREVIEW_LIMIT).map(|order| {
                (
                    "Order".to_string(),
                    format!(
                        "{} · {}:{}",
                        order.order_id, order.exchange, order.tradingsymbol
                    ),
                )
            }));
            let hidden = orders.len().saturating_sub(PREVIEW_LIMIT);
            if hidden > 0 {
                lines.push(("More".to_string(), format!("… and {hidden} more")));
            }
            let noun = if orders.len() == 1 { "order" } else { "orders" };
            (format!("Cancel {} {noun}", orders.len()), lines)
        } else {
            (
                "Cancel order".to_string(),
                vec![
                    ("Order".to_string(), first.order_id.clone()),
                    (
                        "Symbol".to_string(),
                        format!("{}:{}", first.exchange, first.tradingsymbol),
                    ),
                    (
                        "Side".to_string(),
                        format!("{} {}", first.transaction_type, first.quantity),
                    ),
                    ("Status".to_string(), first.status.clone()),
                ],
            )
        };

        self.modals
            .push(Modal::Confirm(Box::new(ConfirmState::plain(
                title,
                lines,
                PendingAction::CancelOrders { targets },
            ))));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, char_key, render, sample_order, screen, test_app,
    };
    use super::super::super::{Modal, ui};
    use crate::tui::data::{self, DataEvent};
    use crate::tui::ticket::PendingAction;
    use crate::tui::workspace::PaneId;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn key_orders_cancel_is_noop_when_trading_disabled() {
        let mut app = test_app();
        app.orders = Some(vec![sample_order("OPEN")]);
        app.ws_fatal = Some("access token expired".to_string());
        app.key_pane(PaneId::OrdersBook, char_key('x'));
        assert!(
            app.modals.is_empty(),
            "cancel confirm must not open while trading is disabled"
        );
    }

    #[test]
    fn key_orders_cancel_opens_confirm_when_enabled() {
        let mut app = test_app();
        app.orders = Some(vec![sample_order("OPEN")]);
        app.key_pane(PaneId::OrdersBook, char_key('x'));
        assert_eq!(app.modals.len(), 1, "cancel confirm should open normally");
    }

    #[test]
    fn simplification_regression_one_marked_order_has_a_singular_title() {
        let mut app = test_app();
        app.set_pane(PaneId::OrdersBook);
        app.orders = Some(vec![sample_order("OPEN")]);
        app.on_key(char_key(' '));
        app.on_key(char_key('x'));
        let Some(Modal::Confirm(confirm)) = app.modals.last() else {
            panic!("marked cancellation opens a review");
        };
        assert_eq!(confirm.title, "Cancel 1 order");
        assert_eq!(confirm.lines[0], ("Orders".to_string(), "1".to_string()));
    }

    #[tokio::test]
    async fn a_cancellation_review_too_small_to_show_cannot_be_confirmed() {
        let affirmative = |action: ui::HitAction| {
            matches!(action, ui::HitAction::Key(key)
                if matches!(key.code, KeyCode::Char('y') | KeyCode::Enter))
        };
        let mut app = test_app();
        app.orders = Some(vec![sample_order("OPEN")]);
        app.key_pane(PaneId::OrdersBook, char_key('x'));
        let tiny = screen(&app, 80, 7);
        assert!(!tiny.contains("y/Enter"), "{tiny}");
        assert!(
            !app.hit
                .borrow()
                .hints
                .iter()
                .any(|hint| affirmative(hint.action)),
            "the hint line offers a send it cannot show"
        );
        app.on_key(char_key('y'));
        assert!(!app.pending_action, "cancelled from a resize notice");

        render(&app, W, H);
        render(&app, 80, 7);
        app.on_key(char_key('y'));
        assert!(!app.pending_action, "an earlier frame vouched for this one");
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));

        render(&app, W, H);
        app.on_key(char_key('y'));
        assert!(
            app.pending_action,
            "a review with room for its contract sends"
        );
    }

    #[test]
    fn order_marks_follow_ids_across_sort_and_refresh() {
        let mut app = test_app();
        let mut first = sample_order("OPEN");
        first.order_id = "OID1".into();
        first.order_timestamp = "2024-01-01T10:00:00".parse().expect("timestamp");
        first.tradingsymbol = "ZZZ".into();
        let mut second = sample_order("OPEN");
        second.order_id = "OID2".into();
        second.order_timestamp = "2024-01-01T09:00:00".parse().expect("timestamp");
        second.tradingsymbol = "AAA".into();
        app.orders = Some(vec![first.clone(), second.clone()]);
        app.set_pane(PaneId::OrdersBook);

        app.key_pane(PaneId::OrdersBook, char_key(' '));
        assert!(app.is_order_marked_in(PaneId::OrdersBook, "OID1"));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID2"));

        app.cycle_sort(PaneId::OrdersBook);
        app.cycle_sort(PaneId::OrdersBook);
        assert_eq!(app.orders_order_in(PaneId::OrdersBook), vec![1, 0]);
        assert!(app.is_order_marked_in(PaneId::OrdersBook, "OID1"));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID2"));

        app.on_data(DataEvent::Orders(Ok(vec![second.clone(), first.clone()])));
        assert!(app.is_order_marked_in(PaneId::OrdersBook, "OID1"));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID2"));

        first.status = "CANCELLED".into();
        app.on_data(DataEvent::Orders(Ok(vec![first, second])));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID1"));
        assert!(!app.is_order_marked_in(PaneId::OrdersBook, "OID2"));
    }

    #[test]
    fn marked_orders_build_one_confirm_with_all_targets() {
        let mut app = test_app();
        let mut older = sample_order("OPEN");
        older.order_id = "OID1".into();
        older.order_timestamp = "2024-01-01T09:15:00".parse().expect("timestamp");
        older.tradingsymbol = "ZZZ".into();
        let mut closed = sample_order("COMPLETE");
        closed.order_id = "OID2".into();
        let mut newer = sample_order("OPEN");
        newer.order_id = "OID3".into();
        newer.order_timestamp = "2024-01-01T09:16:00".parse().expect("timestamp");
        newer.tradingsymbol = "AAA".into();
        app.orders = Some(vec![older, closed, newer]);
        app.set_pane(PaneId::OrdersBook);

        app.key_pane(
            PaneId::OrdersBook,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        );
        app.key_pane(PaneId::OrdersBook, char_key('x'));

        let Some(Modal::Confirm(confirm)) = app.modals.last() else {
            panic!("marked cancellation must open one confirm");
        };
        assert_eq!(confirm.title, "Cancel 2 orders");
        assert_eq!(confirm.lines[0], ("Orders".to_string(), "2".to_string()));
        assert_eq!(
            confirm.lines[1..].to_vec(),
            vec![
                ("Order".to_string(), "OID3 · NSE:AAA".to_string()),
                ("Order".to_string(), "OID1 · NSE:ZZZ".to_string()),
            ]
        );
        let PendingAction::CancelOrders { targets } = &confirm.action else {
            panic!("expected aggregate cancel action");
        };
        assert_eq!(
            targets,
            &[
                data::CancelOrderTarget {
                    parent_order_id: None,
                    variety: "regular".to_string(),
                    order_id: "OID3".to_string(),
                },
                data::CancelOrderTarget {
                    parent_order_id: None,
                    variety: "regular".to_string(),
                    order_id: "OID1".to_string(),
                },
            ]
        );
    }

    #[test]
    fn batch_cancel_confirmation_caps_preview_but_keeps_every_target() {
        let mut app = test_app();
        app.set_pane(PaneId::OrdersBook);
        app.orders = Some(
            (0..10)
                .map(|index| {
                    let mut order = sample_order("OPEN");
                    order.order_id = format!("OID{index}");
                    order
                })
                .collect(),
        );
        app.key_pane(
            PaneId::OrdersBook,
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL),
        );
        app.key_pane(PaneId::OrdersBook, char_key('x'));

        let Some(Modal::Confirm(confirm)) = app.modals.last() else {
            panic!("batch cancellation must open one confirm");
        };
        assert_eq!(confirm.lines.len(), 8);
        assert_eq!(
            confirm.lines.last(),
            Some(&("More".to_string(), "… and 4 more".to_string()))
        );
        let PendingAction::CancelOrders { targets } = &confirm.action else {
            panic!("expected aggregate cancel action");
        };
        assert_eq!(targets.len(), 10);
    }
}
