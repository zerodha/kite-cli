use super::super::{App, Modal, ToastLevel};
use crate::tui::data::{self, Alert};
use crate::tui::ticket::{ConfirmState, PendingAction};
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlertsSort {
    #[default]
    Name,
    Symbol,
    Status,

    Fired,
    Created,
}

impl AlertsSort {
    pub const ALL: [Self; 5] = [
        Self::Name,
        Self::Symbol,
        Self::Status,
        Self::Fired,
        Self::Created,
    ];

    pub(in crate::tui::app) fn next(self) -> Self {
        match self {
            AlertsSort::Name => AlertsSort::Symbol,
            AlertsSort::Symbol => AlertsSort::Status,
            AlertsSort::Status => AlertsSort::Fired,
            AlertsSort::Fired => AlertsSort::Created,
            AlertsSort::Created => AlertsSort::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            AlertsSort::Name => "name ▲",
            AlertsSort::Symbol => "symbol ▲",
            AlertsSort::Status => "status ▲",
            AlertsSort::Fired => "triggered ▼",
            AlertsSort::Created => "created ▼",
        }
    }
}

pub(in crate::tui::app) fn alert_key(alert: &Alert) -> String {
    alert.uuid.clone()
}

pub fn alerts_order(alerts: &[Alert], mode: AlertsSort) -> Vec<usize> {
    let mut order: Vec<usize> = (0..alerts.len()).collect();
    match mode {
        AlertsSort::Name => order.sort_by_cached_key(|&i| alerts[i].name.to_lowercase()),

        AlertsSort::Symbol => {
            order.sort_by_cached_key(|&i| alerts[i].lhs_tradingsymbol.to_lowercase());
        }
        AlertsSort::Status => order.sort_by(|&a, &b| alerts[a].status.cmp(&alerts[b].status)),
        AlertsSort::Fired => {
            order.sort_by(|&a, &b| alerts[b].alert_count.cmp(&alerts[a].alert_count));
        }

        AlertsSort::Created => {
            order.sort_by(|&a, &b| alerts[b].created_at.cmp(&alerts[a].created_at));
        }
    }
    order
}

impl App {
    pub fn alerts_display_order(&self) -> Vec<usize> {
        let Some(alerts) = self.alerts.as_deref() else {
            return Vec::new();
        };
        self.books
            .alerts
            .cached_order(self.alerts_gen, |sort| alerts_order(alerts, sort))
    }

    pub fn alerts_display_row(&self, display: &[usize]) -> usize {
        let alerts = self.alert_list();
        self.books
            .alerts
            .display_row(display, |i| alerts.get(i).map(alert_key))
    }

    pub(in crate::tui::app) fn key_alerts(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.alert_history = None,

            KeyCode::Char('R') => {
                data::spawn_refresh_alerts(
                    self.client.clone(),
                    self.events_tx.clone(),
                    self.alert_fetch_requests.clone(),
                );
                self.toast(ToastLevel::Info, "refreshing…");
            }
            KeyCode::Char('a') => self.open_new_alert_search(),
            KeyCode::Enter | KeyCode::Char('d' | 'r' | 'e') => {
                let Some(alert) = self.selected_alert().cloned() else {
                    return;
                };
                match key.code {
                    KeyCode::Enter => self.open_alert_history(&alert),
                    KeyCode::Char('d') => {
                        let (uuid, label) = (alert.uuid.clone(), data::alert_label(&alert));

                        if self.trading_disabled() {
                            return;
                        }
                        self.modals
                            .push(Modal::Confirm(Box::new(ConfirmState::plain(
                                "Delete alert".to_string(),
                                vec![
                                    ("Alert".to_string(), label.clone()),
                                    ("Uuid".to_string(), uuid.clone()),
                                ],
                                PendingAction::DeleteAlert { uuid, label },
                            ))));
                    }
                    KeyCode::Char('r') => {
                        if self.alert_status_pending.contains(&alert.uuid) {
                            self.toast(ToastLevel::Info, "this alert is already being updated");
                            return;
                        }
                        let status = if data::alert_is_enabled(&alert) {
                            kiteconnect::AlertStatus::Disabled
                        } else {
                            kiteconnect::AlertStatus::Enabled
                        };
                        if data::alert_is_ato(&alert) && status == kiteconnect::AlertStatus::Enabled
                        {
                            let Some(basket) = &alert.basket else {
                                self.toast(
                                    ToastLevel::Error,
                                    "ATO basket is unavailable; review it on Kite before enabling",
                                );
                                return;
                            };
                            let draft =
                                crate::tui::alerts::AtoBasketDraft::from_fetched(&basket.items);
                            let mut lines =
                                vec![("Condition".to_string(), data::alert_condition(&alert))];
                            lines.extend(
                                draft
                                    .items()
                                    .iter()
                                    .map(|item| (item.label(), item.summary())),
                            );
                            self.modals
                                .push(Modal::Confirm(Box::new(ConfirmState::plain(
                                    format!("Enable ATO alert: {}", alert.name),
                                    lines,
                                    PendingAction::SetAlertStatus {
                                        uuid: alert.uuid.clone(),
                                        label: data::alert_label(&alert),
                                        status,
                                        alert_type: alert.alert_type,
                                    },
                                ))));
                        } else {
                            self.alert_status_pending.insert(alert.uuid.clone());
                            data::spawn_set_alert_status(
                                self.client.clone(),
                                self.events_tx.clone(),
                                alert.uuid.clone(),
                                data::alert_label(&alert),
                                status,
                                data::alert_wire_type(&alert),
                                false,
                            );
                        }
                    }
                    KeyCode::Char('e') => self.open_alert_edit(&alert),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, ato_alert, char_key, open_and_review, render, server_alert, test_app,
    };
    use super::super::super::{AlertsSort, App, Modal};
    use super::alerts_order;
    use crate::tui::data::alert_status_request;
    use crate::tui::data::{Alert, DataEvent};
    use crate::tui::ticket::{ConfirmState, PendingAction};
    use crate::tui::workspace::PaneId;
    use kiteconnect::{AlertStatus, AlertType};

    #[test]
    fn the_status_toggle_states_the_alerts_type() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0), {
            let mut row = ato_alert();
            row.status =
                serde_json::from_value(serde_json::json!(AlertStatus::Disabled)).expect("status");
            row
        }]);
        assert_eq!(
            crate::tui::data::alert_wire_type(&app.alert_list()[0]),
            AlertType::Simple
        );
        assert_eq!(
            crate::tui::data::alert_wire_type(&app.alert_list()[1]),
            AlertType::Ato
        );
        let request = alert_status_request(
            "uuid",
            AlertStatus::Enabled,
            crate::tui::data::alert_wire_type(&app.alert_list()[1]),
        );
        assert_eq!(request.path(), "/alerts/uuid");
        let kiteconnect::Body::Form(form) = request.body() else {
            panic!("a status write is a form");
        };
        assert_eq!(
            form,
            &[
                ("status".to_string(), "enabled".to_string()),
                ("type".to_string(), "ato".to_string()),
            ],
            "re-arming a triggered ATO must not be read as making it simple"
        );

        let request = alert_status_request(
            "uuid",
            AlertStatus::Disabled,
            crate::tui::data::alert_wire_type(&app.alert_list()[0]),
        );
        let kiteconnect::Body::Form(form) = request.body() else {
            panic!("a status write is a form");
        };
        assert_eq!(
            form,
            &[
                ("status".to_string(), "disabled".to_string()),
                ("type".to_string(), "simple".to_string()),
            ]
        );
    }

    #[test]
    fn deleting_from_the_alerts_pane_confirms_first_and_backs_out_to_the_pane() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        app.set_pane(PaneId::OrdersAlerts);

        app.on_key(char_key('d'));
        let Some(Modal::Confirm(confirm)) = app.modals.last() else {
            panic!("d must ask before deleting an account-side alert");
        };
        let crate::tui::ticket::PendingAction::DeleteAlert { uuid, .. } = &confirm.action else {
            panic!("expected a DeleteAlert confirmation");
        };
        assert_eq!(uuid, &app.alert_list()[0].uuid);

        app.on_key(char_key('n'));
        assert!(app.modals.is_empty(), "backing out must close the confirm");
        assert_eq!(
            app.focused_pane(),
            PaneId::OrdersAlerts,
            "backing out must leave the user on the alerts pane"
        );
    }

    #[tokio::test]
    async fn an_alert_delete_cannot_release_an_order_in_flight() {
        let refusals = |app: &App| {
            app.toasts()
                .iter()
                .filter(|toast| toast.text == "an order action is in flight")
                .count()
        };
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, |state| {
            state.qty = "10".to_string();
            state.price = "100".to_string();
        });
        app.on_key(char_key('y'));
        assert!(app.pending_action, "the order is in flight");

        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        app.set_pane(PaneId::OrdersAlerts);
        app.on_key(char_key('d'));
        assert!(app.modals.is_empty(), "a delete review opened mid-order");
        assert_eq!(refusals(&app), 1);

        let uuid = app.alert_list()[0].uuid.clone();
        app.modals.push(Modal::Confirm(Box::new(ConfirmState::plain(
            "Delete alert".to_string(),
            Vec::new(),
            PendingAction::DeleteAlert {
                uuid,
                label: "INFY".to_string(),
            },
        ))));
        render(&app, W, H);
        app.on_key(char_key('y'));
        assert_eq!(refusals(&app), 2, "dispatch sent a second request");
        assert!(app.pending_action, "the order still holds the interlock");

        app.on_data(DataEvent::OrderPlaced(Err("rejected".to_string())));
        assert!(!app.pending_action, "the order's reply releases it");

        app.on_key(char_key('n'));
        assert!(app.modals.is_empty());
        app.on_key(char_key('d'));
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));
    }

    #[test]
    fn alerts_view_keys_act_on_the_row_the_sort_actually_drew() {
        let mut app = test_app();
        app.set_alerts(vec![
            {
                let mut row = server_alert("ZEEL", "NSE", ">=", 300.0);
                row.name = "zebra".into();
                row
            },
            {
                let mut row = server_alert("INFY", "NSE", ">=", 110.0);
                row.name = "apple".into();
                row
            },
        ]);
        app.set_pane(PaneId::OrdersAlerts);

        assert_eq!(app.alerts_display_order(), vec![1, 0]);
        assert_eq!(app.selected_alert().expect("row 0").name, "apple");

        app.on_key(char_key('d'));
        let Some(Modal::Confirm(confirm)) = app.modals.last() else {
            panic!("expected a delete confirmation");
        };
        let crate::tui::ticket::PendingAction::DeleteAlert { uuid, .. } = &confirm.action else {
            panic!("expected a DeleteAlert confirmation");
        };
        assert_eq!(
            uuid,
            &app.alert_list()[1].uuid,
            "d deleted an alert other than the highlighted row"
        );
    }

    #[test]
    fn alerts_sort_cycles_every_key_and_returns_to_the_start() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        app.set_pane(PaneId::OrdersAlerts);

        let mut seen = vec![app.books.alerts.sort()];
        for _ in 0..4 {
            app.on_key(char_key('S'));
            seen.push(app.books.alerts.sort());
        }
        assert_eq!(
            seen,
            vec![
                AlertsSort::Name,
                AlertsSort::Symbol,
                AlertsSort::Status,
                AlertsSort::Fired,
                AlertsSort::Created,
            ]
        );

        app.on_key(char_key('S'));
        assert_eq!(app.books.alerts.sort(), AlertsSort::Name);

        let mut labels: Vec<&str> = seen.iter().map(|mode| mode.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), 5);
    }

    #[test]
    fn alerts_order_sorts_by_every_key() {
        let alerts = vec![
            {
                let mut row = server_alert("ZEEL", "NSE", ">=", 300.0);
                row.name = "zebra".into();
                row.status = serde_json::from_value(serde_json::json!(AlertStatus::Enabled))
                    .expect("status");
                row.alert_count = 1;
                row.created_at = "2025-01-01T10:00:00".parse().expect("timestamp");
                row
            },
            {
                let mut row = server_alert("INFY", "NSE", ">=", 110.0);
                row.name = "Apple".into();
                row.status = serde_json::from_value(serde_json::json!(AlertStatus::Disabled))
                    .expect("status");
                row.alert_count = 7;
                row.created_at = "2025-03-01T10:00:00".parse().expect("timestamp");
                row
            },
        ];

        assert_eq!(alerts_order(&alerts, AlertsSort::Name), vec![1, 0]);
        assert_eq!(alerts_order(&alerts, AlertsSort::Symbol), vec![1, 0]);

        assert_eq!(alerts_order(&alerts, AlertsSort::Status), vec![1, 0]);
        assert_eq!(alerts_order(&alerts, AlertsSort::Fired), vec![1, 0]);
        assert_eq!(alerts_order(&alerts, AlertsSort::Created), vec![1, 0]);

        let flipped: Vec<Alert> = alerts.iter().rev().cloned().collect();
        for mode in [
            AlertsSort::Name,
            AlertsSort::Symbol,
            AlertsSort::Status,
            AlertsSort::Fired,
            AlertsSort::Created,
        ] {
            assert_eq!(alerts_order(&flipped, mode), vec![0, 1], "{mode:?}");
        }
    }
}
