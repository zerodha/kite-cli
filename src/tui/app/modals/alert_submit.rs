use super::super::{App, Modal};
use super::alert::{AlertFormFocus, AlertFormStage, AlertFormState};
use crate::tui::data;
use crate::tui::form::FormIssue;
use crate::tui::format;

impl App {
    pub(super) fn stage_alert_save(
        &mut self,
        state: &mut AlertFormState,
        level: f64,
    ) -> Result<(), FormIssue<AlertFormFocus>> {
        if let Some(issue) = state.save_blocker() {
            return Err(issue);
        }
        if !state.ato && !state.was_ato {
            return self.submit_alert(state, level).map(|_| ());
        }

        if self.trading_disabled() {
            return Err(FormIssue::general(
                "orders are unavailable right now; try again shortly",
            ));
        }

        state.level = format!("{level}");
        state.level_edited = true;
        state.stage = AlertFormStage::Review;
        state.viewport.set_drawn(false);
        Ok(())
    }

    fn duplicate_is_allowed(&self, state: &AlertFormState, level: f64) -> bool {
        if state.ato {
            return true;
        }
        let params = state.params(level);
        !self
            .alert_list()
            .iter()
            .any(|alert| data::alert_matches_params(alert, &params))
    }

    fn submit_alert(
        &mut self,
        state: &AlertFormState,
        level: f64,
    ) -> Result<crate::tui::data::AlertRequestId, FormIssue<AlertFormFocus>> {
        let params = state.params(level);
        let label = format!(
            "{} alert: {} {} {} {}",
            state.operator.glyph(),
            state.label(),
            state.attribute.label(),
            state.operator.wire(),
            format::price(level),
        );
        let client = self.client.clone();
        let tx = self.events_tx.clone();
        let request = self.next_alert_request();
        if let Some(uuid) = &state.uuid {
            data::spawn_modify_alert(client, tx, request, uuid.clone(), label, params);
        } else {
            if !self.duplicate_is_allowed(state, level) {
                return Err(FormIssue::general(
                    "this alert already exists on your account",
                ));
            }
            data::spawn_create_alert(client, tx, request, label, params);
        }
        Ok(request)
    }

    pub(in crate::tui::app) fn next_alert_request(&mut self) -> crate::tui::data::AlertRequestId {
        self.alert_request_seq = self.alert_request_seq.wrapping_add(1);
        data::AlertRequestId(self.alert_request_seq)
    }

    pub(super) fn send_alert_form(&mut self, state: &mut AlertFormState) {
        if self.trading_disabled() {
            return;
        }
        let Ok(level) = state.level.trim().parse::<f64>() else {
            state.stage = AlertFormStage::Edit;
            state.fail_at(AlertFormFocus::Level, "Level must be a number.");
            return;
        };
        match self.submit_alert(state, level) {
            Ok(request) => {
                state.stage = AlertFormStage::Submitting(request);
                state.issue = None;
            }
            Err(issue) => {
                state.stage = AlertFormStage::Edit;
                state.fail(issue);
            }
        }
    }

    pub(in crate::tui::app) fn resolve_submitting_alert_form(
        &mut self,
        request: Option<crate::tui::data::AlertRequestId>,
        result: &Result<String, String>,
    ) {
        let Some(request) = request else {
            return;
        };
        let Some(Modal::Alert(form)) = self.modals.last_mut() else {
            return;
        };
        if form.stage != AlertFormStage::Submitting(request) {
            return;
        }
        match result {
            Ok(_) => {
                self.modals.pop();
            }
            Err(err) => {
                form.stage = AlertFormStage::Edit;

                form.fail(FormIssue::general(err.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, alert_form, ato_alert, char_key, char_key_code, issue_message, ltp_tick, press_at,
        region_of, render, server_alert, test_app,
    };
    use super::super::super::{App, Modal, ui};
    use super::super::alert::{AlertFormFocus, AlertFormStage};
    use crate::tui::alerts::{self, Attribute, Operator};
    use crate::tui::data;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn pending_alert_request(app: &App) -> crate::tui::data::AlertRequestId {
        let Some(Modal::Alert(form)) = app.modals.last() else {
            panic!("an alert form must be open");
        };
        let AlertFormStage::Submitting(request) = form.stage else {
            panic!("the form must be waiting for a reply");
        };
        request
    }

    #[test]
    fn duplicate_guard_does_not_block_the_opposite_bound() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 110.0)]);

        let params = alerts::params(
            "INFY",
            "NSE",
            "INFY",
            Attribute::LastTradedPrice,
            Operator::LessThanOrEqual,
            110.0,
            true,
        );
        assert!(
            !app.alert_list()
                .iter()
                .any(|a| data::alert_matches_params(a, &params))
        );

        let refusal = app.submit_alert(
            &alert_form(
                100,
                Attribute::LastTradedPrice,
                Operator::GreaterThanOrEqual,
            ),
            110.0,
        );
        assert_eq!(
            refusal,
            Err(crate::tui::form::FormIssue::general(
                "this alert already exists on your account"
            )),
            "an identical condition must be refused locally"
        );
    }

    #[tokio::test]
    async fn saving_an_ato_with_no_orders_is_refused() {
        let mut app = test_app();
        app.ticks.insert(408_065, ltp_tick(408_065, 1000.0));
        app.open_alert_form("NSE", "INFY", 408_065, "NSE");
        app.on_key(char_key_code(KeyCode::Right));

        app.on_key(char_key_code(KeyCode::Tab));
        app.on_key(char_key_code(KeyCode::Tab));
        for ch in "1500".chars() {
            app.on_key(char_key(ch));
        }
        app.on_key(char_key_code(KeyCode::Enter));

        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("an empty ATO must keep the form open");
        };
        assert_eq!(state.stage, AlertFormStage::Edit, "no review, no send");
        assert!(
            issue_message(state).is_some_and(|err| err.contains("at least one order")),
            "the refusal must say what is missing: {:?}",
            state.issue
        );
        assert_eq!(
            state.issue.as_ref().and_then(|issue| issue.field),
            Some(AlertFormFocus::Basket),
            "an empty basket is a fact about the Basket section"
        );
    }

    #[tokio::test]
    async fn an_ato_save_is_reviewed_sent_once_and_survives_a_rejection() {
        use crate::tui::data::DataEvent;
        let mut app = test_app();
        app.open_alert_edit(&ato_alert());
        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }

        app.on_key(char_key('c'));

        app.on_key(char_key_code(KeyCode::Tab));
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open for the review");
        };
        assert_eq!(state.stage, AlertFormStage::Review);
        assert_eq!(state.draft.len(), 2, "the review keeps the draft");

        app.on_key(char_key_code(KeyCode::Esc));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("Esc must return to the form");
        };
        assert_eq!(state.stage, AlertFormStage::Edit);
        assert_eq!(state.draft.len(), 2);

        app.on_key(char_key_code(KeyCode::Enter));
        render(&app, W, H);
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form stays open while the request is in flight");
        };
        assert!(matches!(state.stage, AlertFormStage::Submitting(_)));

        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert!(matches!(state.stage, AlertFormStage::Submitting(_)));

        app.on_data(DataEvent::AlertModified {
            request: Some(pending_alert_request(&app)),
            result: Err("InputException: Invalid operator".to_string()),
        });
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("a rejection must not close the form");
        };
        assert_eq!(state.stage, AlertFormStage::Edit);
        assert_eq!(state.draft.len(), 2, "the draft survived the rejection");
        assert_eq!(
            issue_message(state),
            Some("InputException: Invalid operator")
        );

        app.on_key(char_key_code(KeyCode::Enter));
        render(&app, W, H);
        app.on_key(char_key_code(KeyCode::Enter));
        app.on_data(DataEvent::AlertModified {
            request: Some(pending_alert_request(&app)),
            result: Ok("buy gold".to_string()),
        });
        assert!(app.modals.is_empty(), "a successful save closes the form");
    }

    fn edited_ato() -> App {
        let mut app = test_app();
        app.open_alert_edit(&ato_alert());
        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }
        app.on_key(char_key('c'));
        app.on_key(char_key_code(KeyCode::Tab));
        app
    }

    fn stage(app: &App) -> AlertFormStage {
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the alert form is open");
        };
        state.stage
    }

    #[tokio::test]
    async fn a_queued_second_enter_cannot_send_an_undrawn_ato_review() {
        let mut app = edited_ato();
        app.on_key(char_key_code(KeyCode::Enter));
        app.on_key(char_key_code(KeyCode::Enter));
        assert_eq!(stage(&app), AlertFormStage::Review, "sent unseen");

        render(&app, W, H);
        assert_eq!(
            stage(&app),
            AlertFormStage::Review,
            "the Enter was replayed"
        );
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(matches!(stage(&app), AlertFormStage::Submitting(_)));
    }

    #[tokio::test]
    async fn an_ato_review_too_small_to_show_cannot_be_sent() {
        let mut app = edited_ato();
        app.on_key(char_key_code(KeyCode::Enter));
        render(&app, 80, 7);
        app.on_key(char_key_code(KeyCode::Enter));
        assert_eq!(stage(&app), AlertFormStage::Review, "sent from a notice");

        render(&app, W, H);
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(matches!(stage(&app), AlertFormStage::Submitting(_)));
    }

    #[tokio::test]
    async fn an_ato_review_with_no_body_width_cannot_be_sent() {
        let mut app = edited_ato();
        app.on_key(char_key_code(KeyCode::Enter));
        render(&app, 6, 24);
        assert!(
            !super::super::confirm::tests::offers_confirm(&app),
            "a zero-width review offers a send"
        );
        app.on_key(char_key_code(KeyCode::Enter));
        assert_eq!(stage(&app), AlertFormStage::Review, "sent unpainted");

        render(&app, W, H);
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(matches!(stage(&app), AlertFormStage::Submitting(_)));
    }

    #[tokio::test]
    async fn a_second_queued_click_cannot_send_an_undrawn_ato_review() {
        let enter = |action: ui::HitAction| {
            action == ui::HitAction::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        };
        let mut app = edited_ato();
        render(&app, W, H);
        let button = region_of(&app, enter);
        press_at(&mut app, button.rect.x + 1, button.rect.y);
        assert_eq!(stage(&app), AlertFormStage::Review, "the click reviews");
        assert!(
            !app.hit_map_is_current(),
            "the edit form's targets must not answer for its review"
        );
        press_at(&mut app, button.rect.x + 1, button.rect.y);
        assert_eq!(stage(&app), AlertFormStage::Review, "sent unseen");

        render(&app, W, H);
        let send = region_of(&app, |action| {
            action == ui::HitAction::Key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE))
        });
        press_at(&mut app, send.rect.x + 1, send.rect.y);
        assert!(matches!(stage(&app), AlertFormStage::Submitting(_)));
    }

    #[tokio::test]
    async fn an_alert_reply_resolves_only_the_submission_that_sent_it() {
        use crate::tui::data::DataEvent;
        use kiteconnect::{BasketOperation, BasketWrite};

        fn send(app: &mut App) {
            for _ in 0..4 {
                app.on_key(char_key_code(KeyCode::Tab));
            }

            app.on_key(char_key('c'));
            app.on_key(char_key_code(KeyCode::Tab));
            app.on_key(char_key_code(KeyCode::Enter));
            render(app, W, H);
            app.on_key(char_key_code(KeyCode::Enter));
        }

        let mut app = test_app();
        app.open_alert_edit(&ato_alert());
        send(&mut app);
        let first = pending_alert_request(&app);
        app.on_key(char_key_code(KeyCode::Esc));
        assert!(app.modals.is_empty(), "Esc abandons the submitting form");

        app.open_alert_edit(&ato_alert());
        send(&mut app);
        let second = pending_alert_request(&app);
        assert_ne!(first, second, "each submission has its own identity");

        app.on_data(DataEvent::AlertModified {
            request: Some(first),
            result: Ok("buy gold".to_string()),
        });

        app.on_data(DataEvent::AlertModified {
            request: None,
            result: Ok("buy gold".to_string()),
        });
        app.on_data(DataEvent::AlertDeleted(Ok("buy gold".to_string())));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("another submission's reply must not close this form");
        };
        assert_eq!(state.stage, AlertFormStage::Submitting(second));

        app.on_key(char_key_code(KeyCode::Enter));
        assert_eq!(
            pending_alert_request(&app),
            second,
            "a repeated Enter must not allocate a second write"
        );

        app.on_data(DataEvent::AlertModified {
            request: Some(second),
            result: Err("InputException: rejected".to_string()),
        });
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("a rejection must not close the form");
        };
        assert_eq!(state.stage, AlertFormStage::Edit);
        assert_eq!(state.draft.len(), 2, "the right draft came back");
        assert_eq!(issue_message(state), Some("InputException: rejected"));

        let Some(BasketWrite::Modify(modify)) = state.basket_write() else {
            panic!("a changed basket writes operations");
        };
        assert_eq!(
            modify
                .items
                .iter()
                .filter(|op| matches!(op, BasketOperation::Insert { .. }))
                .count(),
            1,
            "the duplicated order is inserted once"
        );
    }

    #[tokio::test]
    async fn expired_auth_and_an_order_in_flight_block_an_ato_write() {
        for block in ["auth", "inflight"] {
            let mut app = test_app();
            app.open_alert_edit(&ato_alert());

            for _ in 0..4 {
                app.on_key(char_key_code(KeyCode::Tab));
            }
            app.on_key(char_key('c'));
            app.on_key(char_key_code(KeyCode::Tab));

            match block {
                "auth" => app.ws_fatal = Some("TokenException".to_string()),
                _ => app.pending_action = true,
            }
            app.on_key(char_key_code(KeyCode::Enter));

            let Some(Modal::Alert(state)) = app.modals.last() else {
                panic!("{block}: a blocked ATO must keep the form open");
            };
            assert_eq!(
                state.stage,
                AlertFormStage::Edit,
                "{block}: no review and no send"
            );
            assert!(
                issue_message(state).is_some_and(|err| err.contains("orders are unavailable")),
                "{block}: the refusal must say why: {:?}",
                state.issue
            );
            assert_eq!(state.draft.len(), 2, "{block}: the draft is untouched");
        }

        let mut app = test_app();
        app.ws_fatal = Some("TokenException".to_string());
        app.ticks.insert(408_065, ltp_tick(408_065, 1000.0));
        app.open_alert_form("NSE", "INFY", 408_065, "NSE");
        app.on_key(char_key_code(KeyCode::Tab));
        app.on_key(char_key_code(KeyCode::Tab));
        for ch in "1500".chars() {
            app.on_key(char_key(ch));
        }
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(
            app.modals.is_empty(),
            "a simple alert save is not an order and is not gated"
        );
    }

    #[tokio::test]
    async fn an_ato_review_refused_by_expired_auth_stays_open() {
        let mut app = edited_ato();
        app.on_key(char_key_code(KeyCode::Enter));
        render(&app, W, H);
        assert_eq!(stage(&app), AlertFormStage::Review);
        app.on_ws(kiteconnect::ticker::TickerEvent::Closed(
            kiteconnect::CloseReason::AuthRejected("TokenException".to_string()),
        ));
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the refusal closed the form");
        };
        assert_eq!(state.stage, AlertFormStage::Review, "sent on expired auth");
        assert_eq!(state.draft.len(), 2, "the basket is untouched");
        assert!(
            app.toasts()
                .iter()
                .any(|toast| toast.text.contains("run `kite auth login`")),
            "the refusal says why"
        );
    }

    #[tokio::test]
    async fn the_duplicate_guard_does_not_block_a_second_ato_on_one_condition() {
        let mut app = test_app();
        app.set_alerts(vec![server_alert("INFY", "NSE", ">=", 1500.0)]);

        let mut simple = alert_form(
            408_065,
            Attribute::LastTradedPrice,
            Operator::GreaterThanOrEqual,
        );
        simple.level = "1500".into();
        assert!(
            !app.duplicate_is_allowed(&simple, 1500.0),
            "an identical simple condition is still refused"
        );

        let mut ato = alert_form(
            408_065,
            Attribute::LastTradedPrice,
            Operator::GreaterThanOrEqual,
        );
        ato.ato = true;
        assert!(
            app.duplicate_is_allowed(&ato, 1500.0),
            "two ATOs may share a condition and do different things"
        );
    }

    #[tokio::test]
    async fn dropping_an_atos_basket_is_reviewed_before_it_is_sent() {
        let mut app = test_app();
        app.open_alert_edit(&ato_alert());
        app.on_key(char_key_code(KeyCode::Left));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert!(!state.ato);
        assert!(state.drops_the_basket());
        let params = state.params(100.0);
        assert_eq!(params.alert_type, kiteconnect::AlertType::Simple);
        assert_eq!(params.basket, None);

        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("dropping a basket must be reviewed, not sent");
        };
        assert_eq!(state.stage, AlertFormStage::Review);
    }
}
