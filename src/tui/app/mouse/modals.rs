use super::super::{AlertFormStage, App, Modal, ui};
use crate::tui::form::{ControlIntent, FormViewport};
use crossterm::event::KeyCode;

impl App {
    pub(in crate::tui::app) fn place_note_caret(
        &mut self,
        rect: ratatui::layout::Rect,
        scroll: usize,
        column: u16,
        row: u16,
    ) {
        let Some(Modal::Notes(state)) = self.modals.last_mut() else {
            return;
        };
        let Some(editor) = state.editor.as_mut() else {
            return;
        };
        editor.buffer.click(
            rect.width.max(1) as usize,
            scroll + usize::from(row.saturating_sub(rect.y)),
            usize::from(column.saturating_sub(rect.x)),
        );
    }

    pub(in crate::tui::app) fn scroll_text(&mut self, text: ui::HitText, delta: isize) {
        match text {
            ui::HitText::Help => self.press(if delta < 0 {
                KeyCode::Up
            } else {
                KeyCode::Down
            }),

            ui::HitText::ModalBody => {
                if let Some(viewport) = self.modal_viewport() {
                    viewport.scroll_by(delta);
                }
            }
        }
    }

    pub(in crate::tui::app) fn modal_control(
        &mut self,
        field: ui::ModalField,
        intent: ControlIntent,
    ) {
        match (field, self.modals.pop()) {
            (ui::ModalField::Ticket(field), Some(Modal::Ticket(mut state))) => {
                let seed = self.gtt_seed(&state);
                state.select(field, intent, seed);
                self.modals.push(Modal::Ticket(state));
            }
            (ui::ModalField::Conversion(field), Some(Modal::PositionConvert(mut state))) => {
                state.select(field, intent);
                self.modals.push(Modal::PositionConvert(state));
            }
            (ui::ModalField::Alert(field), Some(Modal::Alert(mut state))) => {
                if state.stage == AlertFormStage::Edit {
                    self.select_alert(&mut state, field, intent);
                }
                self.modals.push(Modal::Alert(state));
            }
            (ui::ModalField::AtoItem(field), Some(Modal::AtoItem(mut state))) => {
                state.select(field, intent);
                self.modals.push(Modal::AtoItem(state));
            }
            (_, Some(modal)) => self.modals.push(modal),
            (_, None) => {}
        }
    }

    fn modal_viewport(&self) -> Option<&FormViewport> {
        match self.modals.last()? {
            Modal::Ticket(state) => Some(&state.viewport),
            Modal::PositionConvert(state) => Some(&state.viewport),
            Modal::Confirm(state) => Some(&state.viewport),
            Modal::Alert(state) => Some(&state.viewport),
            Modal::AtoItem(state) => Some(&state.viewport),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::modals::confirm::tests::long_review;
    use super::super::super::test_support::{
        H, W, char_key, click, edit_ticket, holding, master_inst, open_ticket_state, region_of,
        render, sample_order, screen, scroll, test_app, ticket_app, ticket_option, ticket_row,
    };
    use super::super::super::{
        AlertFormFocus, App, KeyCode, KeyEvent, KeyModifiers, Modal, Side, TicketField, ui,
    };
    use crate::tui::form::ControlIntent;
    use crate::tui::workspace::PaneId;
    use kiteconnect::OrderType;

    #[test]
    fn ticket_field_click_focuses_it_and_leaves_a_disabled_one_alone() {
        let mut app = test_app();
        app.open_ticket(Side::Buy, "NSE".into(), "INFY".into(), 1, 0, None);
        render(&app, W, H);
        let validity = ticket_row(&app, TicketField::Validity);
        click(&mut app, W, H, validity.rect.x + 2, validity.rect.y);
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("the ticket is still open");
        };
        assert_eq!(state.focused(), TicketField::Validity);

        render(&app, W, H);
        assert!(
            app.hit.borrow().regions.iter().all(|region| !matches!(
                region.action,
                ui::HitAction::ModalControl {
                    field: ui::ModalField::Ticket(TicketField::Trigger),
                    ..
                }
            )),
            "a hidden row must leave no target behind"
        );
    }

    #[test]
    fn clicking_a_segment_selects_that_exact_option_at_its_centre_and_its_edges() {
        for offset in [0u16, 1, 5] {
            let mut app = ticket_app();
            render(&app, W, H);
            let sl_m = ticket_option(&app, TicketField::OrderType, 3);
            let x = (sl_m.rect.x + offset).min(sl_m.rect.right() - 1);
            click(&mut app, W, H, x, sl_m.rect.y);
            let state = open_ticket_state(&app);
            assert_eq!(state.order_type, OrderType::SlM, "offset {offset}");
            assert_eq!(state.focused(), TicketField::OrderType);
        }
    }

    #[test]
    fn a_clicked_option_and_a_walked_one_build_the_same_payload() {
        let mut clicked = ticket_app();
        render(&clicked, W, H);
        let sl_m = ticket_option(&clicked, TicketField::OrderType, 3);
        click(&mut clicked, W, H, sl_m.rect.x + 1, sl_m.rect.y);

        let mut walked = ticket_app();
        render(&walked, W, H);
        let row = ticket_row(&walked, TicketField::OrderType);
        click(&mut walked, W, H, row.rect.x, row.rect.y);

        for _ in 0..2 {
            walked.press(KeyCode::Right);
        }

        for app in [&mut clicked, &mut walked] {
            let Some(Modal::Ticket(state)) = app.modals.last_mut() else {
                panic!("expected a ticket");
            };
            state.focus = TicketField::Trigger;
            state.trigger = "99.00".to_string();
        }
        let left = crate::tui::ticket::build_confirm(open_ticket_state(&clicked), None, 0).unwrap();
        let right = crate::tui::ticket::build_confirm(open_ticket_state(&walked), None, 0).unwrap();
        assert_eq!(format!("{:?}", left.action), format!("{:?}", right.action));
    }

    #[test]
    fn clicking_the_option_already_selected_keeps_the_dependent_field() {
        let mut app = ticket_app();

        let price = open_ticket_state(&app).price.clone();
        render(&app, W, H);
        let limit = ticket_option(&app, TicketField::OrderType, 1);
        click(&mut app, W, H, limit.rect.x + 1, limit.rect.y);
        let state = open_ticket_state(&app);
        assert_eq!(state.order_type, OrderType::Limit);
        assert_eq!(state.price, price, "a no-op choice clears nothing");
    }

    #[test]
    fn clicking_a_section_header_or_a_fixed_row_does_nothing() {
        let mut app = test_app();
        app.instruments = Some(std::sync::Arc::new(
            crate::tui::instruments::InstrumentStore::new(vec![master_inst(
                "NSE", "NSE", "INFY", 408_065,
            )]),
        ));
        app.open_modify_ticket(&sample_order("OPEN"));
        render(&app, W, H);
        let before = open_ticket_state(&app).focused();

        let modal = region_of(&app, |action| action == ui::HitAction::Capture);
        assert!(
            app.hit.borrow().regions.iter().all(|region| !matches!(
                region.action,
                ui::HitAction::ModalControl {
                    field: ui::ModalField::Ticket(TicketField::Product),
                    ..
                }
            )),
            "a fixed row is not a target"
        );
        click(&mut app, W, H, modal.rect.x + 3, modal.rect.y + 3);
        assert_eq!(open_ticket_state(&app).focused(), before);
    }

    #[test]
    fn the_wheel_scrolls_a_form_body_without_touching_a_value() {
        let mut app = test_app();
        app.open_gtt_ticket(
            Side::Sell,
            "NSE".to_string(),
            "INFY".to_string(),
            1,
            0,
            None,
            Some(1500.0),
            Some(1490.0),
            None,
        );
        edit_ticket(&mut app, |state| state.gtt_oco = true);

        render(&app, 80, 16);
        let qty = open_ticket_state(&app).qty.clone();
        let body = region_of(&app, |action| {
            action == ui::HitAction::Text(ui::HitText::ModalBody)
        });
        scroll(&mut app, 80, 16, body.rect.x + 2, body.rect.y + 1, true);
        let state = open_ticket_state(&app);
        assert_eq!(state.qty, qty, "a notch is not a step");
        assert_eq!(state.viewport.height(), body.rect.height);
    }

    #[test]
    fn a_wheel_notch_over_a_value_scrolls_the_body_and_leaves_it_alone() {
        let mut app = test_app();
        app.open_gtt_ticket(
            Side::Sell,
            "NSE".to_string(),
            "INFY".to_string(),
            1,
            0,
            None,
            Some(1500.0),
            Some(1490.0),
            None,
        );
        edit_ticket(&mut app, |state| state.gtt_oco = true);

        render(&app, 80, 16);
        let rows_on_screen = |app: &App| -> Vec<TicketField> {
            app.hit
                .borrow()
                .regions
                .iter()
                .filter_map(|region| match region.action {
                    ui::HitAction::ModalControl {
                        field: ui::ModalField::Ticket(field),
                        intent: ControlIntent::Focus,
                    } => Some(field),
                    _ => None,
                })
                .collect()
        };
        let before = rows_on_screen(&app);
        let qty_row = ticket_row(&app, TicketField::Quantity);
        let qty = open_ticket_state(&app).qty.clone();

        scroll(&mut app, 80, 16, qty_row.rect.x + 2, qty_row.rect.y, true);
        assert_eq!(open_ticket_state(&app).qty, qty, "a notch is not a step");

        render(&app, 80, 16);
        let after = rows_on_screen(&app);
        assert_ne!(before, after, "the body scrolled under the pointer");
        assert!(
            after.last().is_some_and(|field| !before.contains(field)),
            "and it scrolled down, bringing a lower field into view"
        );
    }

    #[test]
    fn clicking_a_footer_button_presses_the_key_it_advertises() {
        let mut app = ticket_app();
        render(&app, W, H);
        let cancel = region_of(&app, |action| {
            action == ui::HitAction::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))
        });
        click(&mut app, W, H, cancel.rect.x + 1, cancel.rect.y);
        assert!(app.modals.is_empty(), "[Esc Cancel] closes the ticket");
    }

    #[test]
    fn a_click_inside_a_modal_that_misses_its_targets_is_swallowed() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.holdings = Some(
            (0u32..5)
                .map(|i| holding(&format!("H{i}"), 100 + i, 100.0, 110.0, 100.0, 1))
                .collect(),
        );
        app.open_ticket(Side::Buy, "NSE".into(), "INFY".into(), 1, 0, None);
        render(&app, W, H);
        let modal = region_of(&app, |action| action == ui::HitAction::Capture);

        click(&mut app, W, H, modal.rect.x + 1, modal.rect.y);
        assert_eq!(
            app.pane_row(PaneId::PortfolioHoldings),
            0,
            "the table behind the modal is not clickable"
        );
        assert!(matches!(app.modals.last(), Some(Modal::Ticket(_))));
    }

    #[test]
    fn help_scroll_over_its_body_scrolls_it() {
        let mut app = test_app();
        app.set_pane(PaneId::TradingWatch);
        app.on_key(char_key('?'));

        render(&app, 100, 14);
        let body = region_of(&app, |action| {
            action == ui::HitAction::Text(ui::HitText::Help)
        });
        let offset = |app: &App| match app.modals.last() {
            Some(Modal::Help(offset)) => *offset,
            _ => panic!("the help is open"),
        };
        assert_eq!(offset(&app), 0);
        scroll(&mut app, 100, 14, body.rect.x + 2, body.rect.y + 1, true);
        assert_eq!(offset(&app), 1);
        scroll(&mut app, 100, 14, body.rect.x + 2, body.rect.y + 1, false);
        assert_eq!(offset(&app), 0);
    }

    #[test]
    fn alert_form_field_click_focuses_it() {
        let mut app = test_app();
        app.open_alert_form("NSE", "INFY", 1, "NSE");
        render(&app, W, H);
        let operator = region_of(&app, |action| {
            action
                == ui::HitAction::ModalControl {
                    field: ui::ModalField::Alert(AlertFormFocus::Operator),
                    intent: ControlIntent::Focus,
                }
        });
        click(&mut app, W, H, operator.rect.x + 14, operator.rect.y);
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the alert form is still open");
        };
        assert_eq!(state.focus, AlertFormFocus::Operator);
    }

    #[test]
    fn the_wheel_scrolls_a_review_body() {
        let mut app = test_app();
        app.modals.push(Modal::Confirm(long_review()));
        render(&app, 80, 24);
        let body = region_of(&app, |action| {
            action == ui::HitAction::Text(ui::HitText::ModalBody)
        });
        for _ in 0..3 {
            scroll(&mut app, 80, 24, body.rect.x + 1, body.rect.y + 1, true);
        }
        let text = screen(&app, 80, 24);
        assert!(!text.contains("Row 0"), "the wheel moved the body:\n{text}");
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "the wheel confirmed nothing"
        );
    }
}
