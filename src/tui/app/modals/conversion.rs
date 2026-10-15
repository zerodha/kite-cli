use super::super::{App, Modal, ToastLevel};
use crate::tui::data;
use crate::tui::ticket::{
    ConversionField, PositionConversionState, build_position_conversion_confirm, edit_quantity,
};
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    pub(in crate::tui::app) fn on_position_conversion(&mut self, result: Result<(), String>) {
        self.pending_action = false;
        match result {
            Ok(()) => {
                self.toast(ToastLevel::Success, "position product converted");
                data::spawn_refresh_positions(self.client.clone(), self.events_tx.clone());
            }
            Err(err) => {
                self.toast(
                    ToastLevel::Error,
                    format!("position conversion request failed: {err}"),
                );
                data::spawn_refresh_positions(self.client.clone(), self.events_tx.clone());
            }
        }
    }

    pub(in crate::tui::app) fn key_position_convert(
        &mut self,
        key: KeyEvent,
        mut state: Box<PositionConversionState>,
    ) {
        match key.code {
            KeyCode::Esc => return,
            KeyCode::Enter => match build_position_conversion_confirm(&state) {
                Ok(confirm) => {
                    state.issue = None;
                    self.modals.push(Modal::PositionConvert(state));
                    self.modals.push(Modal::Confirm(Box::new(confirm)));
                    return;
                }
                Err(issue) => {
                    if let Some(field) = issue.field {
                        state.focus = field;
                    }
                    state.viewport.request_reveal();
                    state.issue = Some(issue);
                }
            },
            KeyCode::Tab => state.move_focus(1),
            KeyCode::BackTab => state.move_focus(-1),
            KeyCode::PageUp => state.viewport.page(-1),
            KeyCode::PageDown => state.viewport.page(1),
            code => {
                let field = state.focus;
                match (field, Self::choice_intent(code)) {
                    (ConversionField::Bucket, Some(intent)) => state.select(field, intent),

                    (ConversionField::Quantity, _) => {
                        edit_quantity(&mut state.qty, code);
                        state.clear_issue(ConversionField::Quantity);
                        state.viewport.request_reveal();
                    }
                    _ => {}
                }
            }
        }
        self.modals.push(Modal::PositionConvert(state));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{H, W, click, position, region_of, render, test_app};
    use super::super::super::{Modal, ui};

    use crate::tui::form::ControlIntent;
    use crate::tui::ticket::{ConversionField, PositionConversionState};
    use crate::tui::workspace::PaneId;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn position_convert_field_click_focuses_the_quantity() {
        let mut app = test_app();
        let mut net = position("INFY", 1, 10, 0.0, 100.0, 100.0);
        net.day_buy_quantity = 10;
        let state = PositionConversionState::new(&net, 1).expect("a day bucket is convertible");
        app.modals.push(Modal::PositionConvert(Box::new(state)));
        render(&app, W, H);
        let qty = region_of(&app, |action| {
            action
                == ui::HitAction::ModalControl {
                    field: ui::ModalField::Conversion(ConversionField::Quantity),
                    intent: ControlIntent::Focus,
                }
        });
        click(&mut app, W, H, qty.rect.x + 12, qty.rect.y);
        let Some(Modal::PositionConvert(state)) = app.modals.last() else {
            panic!("the convert form is still open");
        };
        assert_eq!(state.focus, ConversionField::Quantity);
    }

    #[test]
    fn position_conversion_waits_for_instrument_lot_size() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioPositions);
        let mut open = position("NIFTY", 42, 50, 0.0, 100.0, 100.0);
        open.exchange = "NFO".to_string();
        open.day_buy_quantity = 50;
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![open];
            row
        });

        app.set_workspace(PaneId::PortfolioPositions.preset());
        app.focus_pane(PaneId::PortfolioPositions);
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));

        assert!(
            !app.modals
                .iter()
                .any(|modal| matches!(modal, Modal::PositionConvert(_)))
        );
        assert!(
            app.toasts
                .back()
                .is_some_and(|toast| toast.text.contains("instrument details"))
        );
    }
}
