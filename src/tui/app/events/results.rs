use super::super::{App, SESSION_EXPIRED, ToastLevel};
use crate::tui::data;

impl App {
    pub(in crate::tui::app) fn on_order_result(
        &mut self,
        verb: &str,
        result: Result<String, String>,
    ) {
        self.pending_action = false;
        match result {
            Ok(order_id) => {
                self.toast(ToastLevel::Success, format!("order {verb}: {order_id}"));
            }
            Err(err) => {
                self.toast(
                    ToastLevel::Error,
                    format!("order {verb} request failed: {err}"),
                );
            }
        }
        data::spawn_refresh_orders(self.client.clone(), self.events_tx.clone());
        data::spawn_refresh_positions(self.client.clone(), self.events_tx.clone());
    }

    pub(in crate::tui::app) fn on_holdings_authorised(&mut self, result: Result<String, String>) {
        self.pending_action = false;
        match result {
            Ok(url) => self.toast(
                ToastLevel::Success,
                format!("CDSL browser launch started: {url}"),
            ),

            Err(err) => self.toast(ToastLevel::Error, err),
        }
    }

    pub(in crate::tui::app) fn handle_data_error(&mut self, err: String) {
        if crate::app::errors::is_auth_error_text(&err) {
            self.expire_session();
        }
        self.toast_error(err);
    }

    pub(in crate::tui::app) fn expire_session(&mut self) {
        if self.ws_fatal.as_deref() != Some(SESSION_EXPIRED) {
            self.ws_fatal = Some(SESSION_EXPIRED.to_string());
            self.toast(
                ToastLevel::Error,
                "access token expired; recovering session",
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::test_app;

    #[test]
    fn handle_data_error_sets_ws_fatal_on_token_exception() {
        let mut app = test_app();
        app.handle_data_error("kite API error TokenException (HTTP 403): Invalid session".into());
        assert!(app.ws_fatal.is_some());
    }

    #[test]
    fn handle_data_error_does_not_set_ws_fatal_on_plain_403() {
        let mut app = test_app();
        app.handle_data_error("kite API error Forbidden (HTTP 403): no permission".into());
        assert!(app.ws_fatal.is_none());
    }

    #[test]
    fn handle_data_error_does_not_set_ws_fatal_on_unrelated_error() {
        let mut app = test_app();
        app.handle_data_error("request failed: connection reset".into());
        assert!(app.ws_fatal.is_none());
    }
}
