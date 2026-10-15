use super::super::{App, Modal};
use crate::tui::data::{self, Order, OrderHistoryRequest};
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug)]
pub struct OrderDetailState {
    pub order: Order,

    pub request: OrderHistoryRequest,
    pub history: Option<Result<Vec<Order>, String>>,
}

impl App {
    pub(in crate::tui::app) fn open_order_detail(&mut self, order: Box<Order>) {
        self.order_history_seq += 1;
        let request = OrderHistoryRequest {
            order_id: order.order_id.clone(),
            seq: self.order_history_seq,
        };
        data::spawn_order_history(self.client.clone(), self.events_tx.clone(), request.clone());
        self.modals
            .push(Modal::OrderDetail(Box::new(OrderDetailState {
                order: *order,
                request,
                history: None,
            })));
    }

    pub(in crate::tui::app) fn key_order_detail(
        &mut self,
        key: KeyEvent,
        order: Box<OrderDetailState>,
    ) {
        if !matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
            self.modals.push(Modal::OrderDetail(order));
        }
    }
}
