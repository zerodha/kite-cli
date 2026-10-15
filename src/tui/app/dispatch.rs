use super::marketwatch;
use super::{App, ToastLevel};
use crate::tui::data;
use crate::tui::marketwatch::Mutation;
use crate::tui::ticket::{ConfirmEffect, PendingAction};

impl App {
    pub(in crate::tui::app) fn trading_disabled(&mut self) -> bool {
        if self.pending_action {
            self.toast(ToastLevel::Info, "an order action is in flight");
            return true;
        }
        self.auth_expired()
    }

    pub(in crate::tui::app) fn auth_expired(&mut self) -> bool {
        if let Some(reason) = &self.ws_fatal {
            let message = if reason == super::SESSION_EXPIRED || reason == "TokenException" {
                "auth expired, run `kite auth login`".to_string()
            } else {
                reason.clone()
            };
            self.toast(ToastLevel::Error, message);
            return true;
        }
        false
    }

    pub(in crate::tui::app) fn dispatch(
        &mut self,
        action: PendingAction,
    ) -> Result<(), PendingAction> {
        if action.effect() == ConfirmEffect::KiteRequest {
            if self.trading_disabled() {
                return Err(action);
            }
            self.pending_action = true;
        }
        let client = self.client.clone();
        let tx = self.events_tx.clone();
        match action {
            PendingAction::SetAlertStatus {
                uuid,
                label,
                status,
                alert_type,
            } => {
                self.alert_status_pending.insert(uuid.clone());
                data::spawn_set_alert_status(client, tx, uuid, label, status, alert_type, true);
            }
            PendingAction::Logout => self.should_logout = true,
            PendingAction::DiscardAlertDraft => {
                self.modals.pop();
            }
            PendingAction::DeleteNote { id, label } => self.delete_note(id, &label),
            PendingAction::DeleteWatchlist { watch, name } => self.send_mutation(
                Mutation::Delete {
                    watch: marketwatch::WatchId::from(watch.as_str()),
                    name,
                },
                marketwatch::Origin::Command,
            ),
            PendingAction::Place { variety, form } => {
                data::spawn_place_order(client, tx, variety, form);
            }
            PendingAction::ConvertPosition { form } => {
                data::spawn_convert_position(client, tx, form);
            }
            PendingAction::AuthoriseHoldings => {
                data::spawn_authorise_holdings(client, tx);
            }
            PendingAction::Modify {
                variety,
                order_id,
                form,
            } => data::spawn_modify_order(client, tx, variety, order_id, form),
            PendingAction::CancelOrders { targets } => {
                data::spawn_cancel_orders(client, tx, targets);
            }
            PendingAction::DeleteGtt { trigger_id } => {
                data::spawn_delete_gtt(client, tx, trigger_id);
            }
            PendingAction::PlaceGtt { form } => data::spawn_place_gtt(client, tx, form),
            PendingAction::ModifyGtt { trigger_id, form } => {
                data::spawn_modify_gtt(client, tx, trigger_id, form);
            }
            PendingAction::DeleteAlert { uuid, label } => {
                data::spawn_delete_alert(client, tx, uuid, label);
            }
        }
        Ok(())
    }
}
