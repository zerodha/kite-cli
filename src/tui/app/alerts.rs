use super::{AlertHistoryState, App, ToastLevel, alert_key};
use crate::tui::alerts;
use crate::tui::data::{self, Alert, AlertTrigger};

impl App {
    pub(in crate::tui::app) fn on_alert_result(
        &mut self,
        verb: &str,
        request: Option<crate::tui::data::AlertRequestId>,
        result: Result<String, String>,
    ) {
        self.resolve_submitting_alert_form(request, &result);

        let issued = self
            .alert_fetch_requests
            .load(std::sync::atomic::Ordering::SeqCst);
        self.alerts_applied = self.alerts_applied.max(data::AlertListSequence(issued));
        match result {
            Ok(label) => self.toast(ToastLevel::Success, format!("alert {verb}: {label}")),
            Err(err) => self.toast(
                ToastLevel::Error,
                format!("alert {verb} request failed: {err}"),
            ),
        }
        data::spawn_refresh_alerts(
            self.client.clone(),
            self.events_tx.clone(),
            self.alert_fetch_requests.clone(),
        );
    }

    pub fn alert_list(&self) -> &[Alert] {
        self.alerts.as_deref().unwrap_or(&[])
    }

    pub fn alerts_fetched(&self) -> Option<&[Alert]> {
        self.alerts.as_deref()
    }

    pub fn set_alerts(&mut self, alerts: Vec<Alert>) {
        self.alert_index =
            alerts::AlertIndex::build(alerts.iter().map(|alert| alert.lhs_tradingsymbol.as_str()));
        self.alerts = Some(alerts);
        self.alerts_gen += 1;
    }

    fn alerts_for_token(&self, token: u32) -> impl Iterator<Item = &Alert> {
        let instrument = self
            .instruments
            .as_ref()
            .and_then(|store| store.by_token(token));
        let list = self.alert_list();
        let candidates = instrument
            .map(|instrument| self.alert_index.candidates(&instrument.tradingsymbol))
            .unwrap_or_default();
        candidates.iter().filter_map(move |idx| {
            let alert = list.get(*idx)?;
            let instrument = instrument?;
            alerts::lhs_matches(
                &alert.lhs_exchange,
                &alert.lhs_tradingsymbol,
                &instrument.exchange,
                &instrument.segment,
                &instrument.tradingsymbol,
            )
            .then_some(alert)
        })
    }

    pub(in crate::tui::app) fn triggers_since_last_fetch(&self, fetched: &[Alert]) -> Vec<String> {
        let Some(previous) = self.alerts.as_ref() else {
            return Vec::new();
        };
        fetched
            .iter()
            .filter(|alert| {
                previous
                    .iter()
                    .find(|known| known.uuid == alert.uuid)
                    .is_some_and(|known| alert.alert_count > known.alert_count)
            })
            .map(|alert| {
                format!(
                    "{} alert triggered: {} · {}",
                    alerts::operator_glyph(&alert.operator),
                    if alert.name.is_empty() {
                        data::alert_label(alert)
                    } else {
                        alert.name.clone()
                    },
                    data::alert_condition(alert),
                )
            })
            .collect()
    }

    pub fn alert_state(&self, token: u32) -> Option<bool> {
        let mut alerts = self.alerts_for_token(token).peekable();
        alerts.peek()?;
        Some(alerts.any(crate::tui::data::alert_is_enabled))
    }

    pub(crate) fn selected_alert(&self) -> Option<&Alert> {
        let order = self.alerts_display_order();
        let alerts = self.alert_list();
        let natural = self
            .books
            .alerts
            .target(&order, |i| alerts.get(i).map(alert_key))?;
        alerts.get(natural)
    }

    pub fn alert_history(&self) -> Option<&AlertHistoryState> {
        self.alert_history.as_ref()
    }

    pub(in crate::tui::app) fn open_alert_history(&mut self, alert: &Alert) {
        let uuid = alert.uuid.clone();
        let label = if alert.name.is_empty() {
            data::alert_label(alert)
        } else {
            alert.name.clone()
        };
        let request = self.next_alert_request();
        data::spawn_alert_history(
            self.client.clone(),
            self.events_tx.clone(),
            uuid.clone(),
            request,
        );
        self.alert_history = Some(AlertHistoryState {
            uuid,
            request,
            label,
            triggers: None,
            basket_len: data::alert_basket_len(alert),
        });
    }

    pub(in crate::tui::app) fn on_alert_history(
        &mut self,
        uuid: &str,
        request: data::AlertRequestId,
        result: Result<Vec<AlertTrigger>, String>,
    ) {
        let Some(state) = self.alert_history.as_mut() else {
            return;
        };
        if state.uuid != uuid || state.request != request {
            return;
        }
        state.triggers = Some(result);
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{
        char_key, char_key_code, ltp_tick, master_inst, server_alert, test_app,
    };
    use crate::tui::data::{self, DataEvent};
    use crate::tui::instruments::InstrumentStore;
    use crate::tui::workspace::PaneId;
    use crossterm::event::KeyCode;
    use kiteconnect::AlertStatus;
    use kiteconnect::ticker::TickerEvent;
    use std::sync::Arc;

    #[test]
    fn a_risen_server_trigger_count_announces_the_firing() {
        let mut app = test_app();
        app.on_data(DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
            result: Ok(vec![server_alert("INFY", "NSE", ">=", 1520.0)]),
        });

        let mut fired = server_alert("INFY", "NSE", ">=", 1520.0);
        fired.alert_count = 1;
        fired.status = AlertStatus::Disabled;
        app.on_data(DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
            result: Ok(vec![fired]),
        });

        let announced: Vec<&str> = app
            .toasts()
            .iter()
            .filter(|toast| toast.text.contains("alert triggered"))
            .map(|toast| toast.text.as_str())
            .collect();
        assert_eq!(announced.len(), 1, "one firing, one announcement");
        assert!(
            announced[0].contains("INFY") && announced[0].contains("1520"),
            "the notice must name the alert and its condition: {:?}",
            announced[0]
        );
    }

    #[test]
    fn unchanged_and_first_seen_alerts_are_not_announced() {
        let mut app = test_app();
        let mut history = server_alert("INFY", "NSE", ">=", 1520.0);
        history.alert_count = 4;

        app.on_data(DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
            result: Ok(vec![history.clone()]),
        });

        let mut created_elsewhere = server_alert("TCS", "NSE", "<=", 3000.0);
        created_elsewhere.alert_count = 2;
        app.on_data(DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
            result: Ok(vec![history, created_elsewhere]),
        });

        assert!(
            !app.toasts()
                .iter()
                .any(|toast| toast.text.contains("alert triggered")),
            "only a count that rose against a list this session already held is a firing"
        );
    }

    #[tokio::test]
    async fn a_tick_never_announces_a_firing_the_server_has_not_recorded() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "INFY", 408_065,
        )])));
        app.on_data(DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
            result: Ok(vec![server_alert("INFY", "NSE", ">=", 1500.0)]),
        });

        app.on_ws(TickerEvent::Ticks(vec![ltp_tick(408_065, 1520.0)]));

        assert!(
            app.toasts()
                .iter()
                .all(|toast| !toast.text.contains("alert")),
            "a tick is not a firing: the server decides, and it has not said so"
        );
        let alert = &app.alert_list()[0];
        assert!(
            data::alert_is_enabled(alert),
            "status is the server's to change"
        );
        assert_eq!(
            alert.alert_count, 0,
            "trigger count is the server's to bump"
        );
    }

    #[tokio::test]
    async fn an_alert_reply_never_clears_an_order_action_in_flight() {
        use crate::tui::data::{self, DataEvent};
        let mut app = test_app();

        app.pending_action = true;

        app.on_data(DataEvent::AlertModified {
            request: None,
            result: Ok("INFY above 110".into()),
        });
        assert!(
            app.pending_action,
            "an alert modify/status reply must not release the order interlock"
        );
        app.on_data(DataEvent::AlertCreated {
            request: data::AlertRequestId(1),
            result: Ok("INFY above 110".into()),
        });
        assert!(app.pending_action);
        assert!(
            app.trading_disabled(),
            "the order in flight must still block a second submit"
        );

        app.on_data(DataEvent::AlertDeleted(Ok("INFY above 110".into())));
        assert!(!app.pending_action);
    }

    #[tokio::test]
    async fn enter_opens_the_history_in_its_loading_state_and_esc_closes_it() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        app.set_pane(PaneId::OrdersAlerts);

        app.on_key(char_key_code(KeyCode::Enter));
        let state = app.alert_history().expect("Enter must open the history");
        assert_eq!(state.uuid, app.alert_list()[0].uuid);
        assert!(
            state.triggers.is_none(),
            "a history that has not arrived must render as loading, not as empty"
        );

        app.on_key(char_key_code(KeyCode::Esc));
        assert!(app.alert_history().is_none());
        assert_eq!(app.focused_pane(), PaneId::OrdersAlerts);
    }

    #[tokio::test]
    async fn a_stale_history_reply_is_dropped() {
        let mut app = test_app();
        app.set_alerts(vec![
            server_alert("INFY", "NSE", ">=", 110.0),
            server_alert("TCS", "NSE", ">=", 4000.0),
        ]);
        app.set_pane(PaneId::OrdersAlerts);
        app.on_key(char_key_code(KeyCode::Enter));
        let open = app.alert_history().expect("history open");
        let (first, first_request) = (open.uuid.clone(), open.request);

        app.on_key(char_key('j'));
        app.on_key(char_key_code(KeyCode::Enter));
        let open = app.alert_history().expect("history open");
        let (second, second_request) = (open.uuid.clone(), open.request);
        assert_ne!(first, second);

        app.on_data(DataEvent::AlertHistory {
            uuid: first.clone(),
            request: first_request,
            result: Ok(vec![{
                let mut row = crate::tui::data::test_models::history();
                row.condition = "stale".into();
                row
            }]),
        });
        assert!(
            app.alert_history()
                .expect("history open")
                .triggers
                .is_none(),
            "a reply for the alert the user moved off must not be rendered"
        );

        app.on_data(DataEvent::AlertHistory {
            uuid: first,
            request: first_request,
            result: Err("offline".into()),
        });
        assert!(
            app.alert_history()
                .expect("history open")
                .triggers
                .is_none()
        );

        app.on_data(DataEvent::AlertHistory {
            uuid: second,
            request: second_request,
            result: Ok(Vec::new()),
        });
        let triggers = app
            .alert_history()
            .expect("history open")
            .triggers
            .as_ref()
            .expect("the current alert's reply must be applied");
        assert!(
            triggers.as_ref().expect("ok").is_empty(),
            "an alert that never fired lands as an empty history, not as loading"
        );

        app.on_key(char_key_code(KeyCode::Esc));
        app.on_data(DataEvent::AlertHistory {
            uuid: "whatever".into(),
            request: second_request,
            result: Ok(Vec::new()),
        });
        assert!(app.alert_history().is_none());
    }

    #[tokio::test]
    async fn an_older_history_reply_for_the_same_alert_is_dropped() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        app.set_pane(PaneId::OrdersAlerts);
        app.on_key(char_key_code(KeyCode::Enter));
        let older = app.alert_history().expect("history open").request;
        app.on_key(char_key_code(KeyCode::Enter));
        let open = app.alert_history().expect("history open");
        let (uuid, newer) = (open.uuid.clone(), open.request);
        assert_ne!(older, newer, "each fetch carries its own identity");

        app.on_data(DataEvent::AlertHistory {
            uuid: uuid.clone(),
            request: newer,
            result: Ok(vec![{
                let mut row = crate::tui::data::test_models::history();
                row.condition = "fresh".into();
                row
            }]),
        });
        app.on_data(DataEvent::AlertHistory {
            uuid,
            request: older,
            result: Err("offline".into()),
        });

        let triggers = app
            .alert_history()
            .expect("history open")
            .triggers
            .as_ref()
            .expect("the newer reply was applied");
        let triggers = triggers
            .as_ref()
            .expect("the older failure must not replace it");
        assert_eq!(triggers[0].condition, "fresh");
    }

    #[tokio::test]
    async fn the_history_panel_closes_when_its_alert_leaves_the_server_list() {
        let mut app = test_app();
        let alert = server_alert("INFY", "NSE", ">=", 110.0);
        app.set_alerts(vec![alert.clone()]);
        app.set_pane(PaneId::OrdersAlerts);
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(app.alert_history().is_some());

        app.on_data(DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
            result: Ok(vec![alert]),
        });
        assert!(app.alert_history().is_some());

        app.on_data(DataEvent::Alerts {
            sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
            result: Ok(vec![server_alert("TCS", "NSE", ">=", 4000.0)]),
        });
        assert!(app.alert_history().is_none());
    }

    #[test]
    fn alert_state_needs_the_instrument_master() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        assert_eq!(app.alert_state(408_065), None);

        app.set_alerts(Vec::new());
        assert_eq!(app.alert_state(408_065), None);
    }

    #[test]
    fn the_alert_index_is_rebuilt_whenever_the_list_changes() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![
            master_inst("NSE", "NSE", "INFY", 408_065),
            master_inst("NSE", "NSE", "TCS", 2_953_217),
        ])));
        assert_eq!(app.alert_state(408_065), None, "nothing fetched yet");

        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);
        assert_eq!(app.alert_state(408_065), Some(true));
        assert_eq!(app.alert_state(2_953_217), None);

        let mut fired = server_alert("TCS", "NSE", ">=", 4000.0);
        fired.status = AlertStatus::Disabled;
        app.set_alerts(vec![fired]);
        assert_eq!(app.alert_state(408_065), None, "the INFY alert is gone");
        assert_eq!(
            app.alert_state(2_953_217),
            Some(false),
            "TCS has an alert, and it is no longer armed"
        );

        app.set_alerts(Vec::new());
        assert_eq!(app.alert_state(2_953_217), None);
    }

    #[test]
    fn the_alert_index_does_not_widen_what_matches() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![
            master_inst("NSE", "NSE", "INFY", 408_065),
            master_inst("NSE", "INDICES", "NIFTY 50", 256_265),
        ])));
        app.set_alerts(vec![
            server_alert("INFY", "BSE", ">=", 110.0),
            server_alert("nifty 50", "INDICES", ">=", 30000.0),
        ]);
        assert_eq!(app.alert_state(408_065), None, "BSE:INFY is not NSE:INFY");
        assert_eq!(app.alert_state(256_265), Some(true));
    }
}
