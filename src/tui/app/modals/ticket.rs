use super::super::{App, Modal};
use crate::tui::data;
use crate::tui::form::ControlIntent;
use crate::tui::ticket::{GttSeed, TicketState, build_confirm, gtt_default_trigger_percent};
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(in crate::tui::app) fn choice_intent(code: KeyCode) -> Option<ControlIntent> {
        match code {
            KeyCode::Left | KeyCode::Char('h') => Some(ControlIntent::Previous),
            KeyCode::Right | KeyCode::Char('l' | ' ') => Some(ControlIntent::Next),
            _ => None,
        }
    }

    pub(in crate::tui::app) fn gtt_seed(&self, state: &TicketState) -> GttSeed {
        GttSeed {
            ltp: self.live_ltp(state.instrument_token),
            percent: gtt_default_trigger_percent(self.contract_segment(state.instrument_token)),
        }
    }

    pub(in crate::tui::app) fn key_ticket(&mut self, key: KeyEvent, mut state: Box<TicketState>) {
        match key.code {
            KeyCode::Esc => return,
            KeyCode::Enter => {
                let ltp = self.live_ltp(state.instrument_token);
                self.preview_seq = self.preview_seq.wrapping_add(1);
                let seq = self.preview_seq;
                match build_confirm(&state, ltp, seq) {
                    Ok(confirm) => {
                        state.issue = None;
                        if let Some(request) = confirm.preview_request.clone() {
                            data::spawn_margin_preview(
                                self.client.clone(),
                                self.events_tx.clone(),
                                seq,
                                request,
                            );
                        }
                        self.modals.push(Modal::Ticket(state));
                        self.modals.push(Modal::Confirm(Box::new(confirm)));
                    }
                    Err(issue) => {
                        if let Some(field) = issue.field {
                            state.focus_field(field);
                        }
                        state.viewport.request_reveal();
                        state.issue = Some(issue);
                        self.modals.push(Modal::Ticket(state));
                    }
                }
                return;
            }

            KeyCode::Tab => state.move_focus(1),
            KeyCode::BackTab => state.move_focus(-1),
            KeyCode::PageUp => state.viewport.page(-1),
            KeyCode::PageDown => state.viewport.page(1),
            KeyCode::Up | KeyCode::Down => {
                let dir: i8 = if key.code == KeyCode::Up { 1 } else { -1 };
                let ltp = self.live_ltp(state.instrument_token).unwrap_or(0.0);
                let field = state.focused();
                state.step(field, dir, ltp);
            }
            code => {
                let field = state.focused();
                match Self::choice_intent(code) {
                    Some(intent) if field.is_choice() => {
                        let seed = self.gtt_seed(&state);
                        state.select(field, intent, seed);
                    }
                    _ => state.edit(field, code),
                }
            }
        }
        self.modals.push(Modal::Ticket(state));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        char_key_code, edit_ticket, master_inst, open_and_review, sample_order,
        set_non_default_fields, test_app,
    };
    use super::super::super::{App, Modal, PRODUCTS, Side, segment_class};
    use crate::tui::data::Order;
    use crate::tui::instruments::InstrumentStore;
    use crate::tui::ticket::{ModifyTarget, PendingAction, TicketField, build_confirm};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use kiteconnect::OrderType;
    use std::sync::Arc;

    #[tokio::test]
    async fn ticket_prefs_written_only_on_confirm_not_on_esc() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, set_non_default_fields);
        app.on_key(char_key_code(KeyCode::Esc));
        assert!(
            app.ticket_prefs.is_empty(),
            "Esc must never write ticket prefs"
        );

        open_and_review(&mut app, "NSE", "INFY", 1, set_non_default_fields);
        app.on_key(char_key_code(KeyCode::Char('y')));
        assert!(
            app.ticket_prefs.contains_key(&segment_class("NSE")),
            "y must write ticket prefs"
        );
    }

    #[tokio::test]
    async fn ticket_prefs_written_on_enter_confirm_too() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, set_non_default_fields);
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(app.ticket_prefs.contains_key(&segment_class("NSE")));
    }

    #[tokio::test]
    async fn ticket_prefs_restore_on_next_fresh_ticket_same_segment() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, set_non_default_fields);
        app.on_key(char_key_code(KeyCode::Char('y')));

        app.pending_action = false;

        app.open_ticket(Side::Buy, "NSE".to_string(), "TCS".to_string(), 2, 0, None);
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("expected a ticket modal");
        };
        assert_eq!(
            state.product,
            PRODUCTS.iter().position(|p| *p == "MIS").unwrap()
        );
        assert!(state.autoslice);
        assert_eq!(state.order_type, OrderType::Market);
        assert_eq!(state.validity, 1);
    }

    #[tokio::test]
    async fn ticket_prefs_equity_does_not_leak_to_fno() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, set_non_default_fields);
        app.on_key(char_key_code(KeyCode::Char('y')));

        app.pending_action = false;

        app.open_ticket(
            Side::Buy,
            "NFO".to_string(),
            "NIFTY24JULFUT".to_string(),
            3,
            0,
            None,
        );
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("expected a ticket modal");
        };
        assert_ne!(
            state.product,
            PRODUCTS.iter().position(|p| *p == "MIS").unwrap()
        );
        assert!(!state.autoslice);
        assert_eq!(state.order_type, OrderType::Limit);
        assert_eq!(state.validity, 0);
    }

    #[tokio::test]
    async fn ticket_prefs_explicit_product_wins_over_remembered() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, set_non_default_fields);
        app.on_key(char_key_code(KeyCode::Char('y')));

        app.pending_action = false;

        app.open_ticket(
            Side::Sell,
            "NSE".to_string(),
            "TCS".to_string(),
            2,
            0,
            Some("CNC"),
        );
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("expected a ticket modal");
        };
        assert_eq!(
            state.product,
            PRODUCTS.iter().position(|p| *p == "CNC").unwrap()
        );
    }

    #[tokio::test]
    async fn ticket_prefs_modify_ticket_ignores_remembered_prefs() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, set_non_default_fields);
        app.on_key(char_key_code(KeyCode::Char('y')));

        app.pending_action = false;

        let order = sample_order("OPEN");
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE",
            "NSE",
            "INFY",
            order.instrument_token,
        )])));
        app.open_modify_ticket(&order);
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("expected a ticket modal");
        };
        assert_eq!(
            state.product,
            PRODUCTS.iter().position(|p| *p == order.product).unwrap(),
            "modify must keep the order's own product, not the remembered MIS"
        );
        assert!(!state.autoslice, "modify never carries autoslice");
    }

    #[test]
    fn ticket_prefs_confirm_state_none_for_modify_and_gtt() {
        let mut app = test_app();
        app.open_ticket(Side::Buy, "NSE".to_string(), "INFY".to_string(), 1, 0, None);
        edit_ticket(&mut app, |state| {
            state.order_type = OrderType::Market;
            state.modify = Some(ModifyTarget {
                parent_order_id: None,
                order_id: "OID1".to_string(),
                variety: "regular".to_string(),
                quantity_editable: true,
            });
        });
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Confirm(state)) = app.modals.last() else {
            panic!("expected a confirm modal");
        };
        assert!(state.ticket_prefs.is_none());
        app.modals.clear();

        app.open_gtt_ticket(
            Side::Buy,
            "NSE".to_string(),
            "INFY".to_string(),
            1,
            0,
            None,
            Some(1500.0),
            Some(1490.0),
            None,
        );
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Confirm(state)) = app.modals.last() else {
            panic!("expected a confirm modal");
        };
        assert!(state.ticket_prefs.is_none());
    }

    #[test]
    fn variety_selection_uses_automatic_amo_and_preserves_product() {
        let mut app = test_app();
        app.open_ticket(
            Side::Buy,
            "NSE".to_string(),
            "INFY".to_string(),
            1,
            1,
            Some("MTF"),
        );
        let Some(Modal::Ticket(mut ticket)) = app.modals.pop() else {
            panic!("expected ticket");
        };
        ticket.focus = TicketField::Variety;
        app.key_ticket(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE), ticket);
        let Some(Modal::Ticket(ticket)) = app.modals.last() else {
            panic!("expected ticket");
        };
        assert_eq!(ticket.active_variety(), "iceberg");
        assert_eq!(PRODUCTS[ticket.product], "MTF");
        assert!(ticket.issue.is_none());
    }

    fn cds_app() -> App {
        let mut app = test_app();
        let mut future = master_inst("CDS", "CDS-FUT", "USDINR26SEPFUT", 7001);
        future.tick_size = 0.0025;
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![future])));
        app
    }

    fn cds_order() -> Order {
        {
            let mut row = sample_order("TRIGGER PENDING");
            row.exchange = "CDS".into();
            row.tradingsymbol = "USDINR26SEPFUT".into();
            row.instrument_token = 7001;
            row.order_type = "SL".into();
            row.product = "NRML".into();
            row.price = 83.1275;
            row.trigger_price = 83.1225;
            row
        }
    }

    #[test]
    fn an_untouched_cds_modify_sends_back_the_exact_price_and_trigger() {
        let mut app = cds_app();
        app.open_modify_ticket(&cds_order());
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("expected a ticket modal");
        };
        assert_eq!(state.price, "83.1275");
        assert_eq!(state.trigger, "83.1225");

        let confirm = build_confirm(state, None, 0).expect("the untouched modify is valid");
        let PendingAction::Modify { form, .. } = confirm.action else {
            panic!("expected a modify");
        };
        let form: Vec<(&str, &str)> = form
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        assert_eq!(
            form,
            vec![
                ("order_type", "SL"),
                ("quantity", "1"),
                ("price", "83.1275"),
                ("trigger_price", "83.1225"),
            ]
        );
    }

    #[test]
    fn a_cds_gtt_modify_seeds_the_exact_price_and_trigger() {
        let mut app = cds_app();
        app.open_gtt_ticket(
            Side::Sell,
            "CDS".to_string(),
            "USDINR26SEPFUT".to_string(),
            7001,
            1,
            Some("NRML"),
            Some(83.1275),
            Some(83.1225),
            Some(5),
        );
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("expected a ticket modal");
        };
        assert_eq!(state.price, "83.1275");
        assert_eq!(state.trigger, "83.1225");
    }
}
