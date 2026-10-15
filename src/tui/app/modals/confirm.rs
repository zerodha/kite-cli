use super::super::{App, Modal};
use crate::tui::ticket::{ConfirmEffect, ConfirmState};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

impl App {
    pub(in crate::tui::app) fn key_confirm(&mut self, key: KeyEvent, mut state: Box<ConfirmState>) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Enter
                if key.kind != KeyEventKind::Repeat && state.viewport.drawn() =>
            {
                let local = state.action.effect() == ConfirmEffect::Local;

                if let Err(action) = self.dispatch(state.action) {
                    state.action = action;
                    self.modals.push(Modal::Confirm(state));
                    return;
                }

                if let Some((segment, prefs)) = state.ticket_prefs {
                    self.ticket_prefs.insert(segment, prefs);
                }

                if !local {
                    self.modals.clear();
                }
            }
            KeyCode::Char('n') | KeyCode::Esc => {}

            KeyCode::PageUp | KeyCode::PageDown => {
                state
                    .viewport
                    .page(if key.code == KeyCode::PageUp { -1 } else { 1 });
                self.modals.push(Modal::Confirm(state));
            }
            _ => self.modals.push(Modal::Confirm(state)),
        }
    }
}

#[cfg(test)]
pub(in crate::tui::app) mod tests {
    use super::super::super::test_support::{
        H, W, char_key, char_key_code, click, edit_ticket, open_and_review, open_ticket_state,
        region_of, render, screen, test_app, ticket_app,
    };
    use super::super::super::{Modal, ui};
    use super::super::notes::tests::notes_app;
    use crate::tui::data::DataEvent;
    use crate::tui::ticket::{PendingAction, TicketField};
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
    use kiteconnect::ticker::TickerEvent;

    pub(in crate::tui::app) fn long_review() -> Box<crate::tui::ticket::ConfirmState> {
        Box::new(crate::tui::ticket::ConfirmState::plain(
            "Delete note".to_string(),
            (0..40)
                .map(|row| (format!("Row {row}"), format!("value {row}")))
                .collect(),
            PendingAction::DeleteNote {
                id: 1,
                label: "a thesis".to_string(),
            },
        ))
    }

    #[test]
    fn confirm_click_on_yes_applies_and_on_no_goes_back() {
        let mut app = notes_app();
        app.notes.upsert(None, "a thought", None, 1_000);
        app.open_notes();
        app.on_key(char_key('d'));
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));

        let key = |c: char| {
            move |action: ui::HitAction| {
                action == ui::HitAction::Key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE))
            }
        };
        render(&app, W, H);
        let no = region_of(&app, key('n'));
        click(&mut app, W, H, no.rect.x, no.rect.y);
        assert!(
            matches!(app.modals.last(), Some(Modal::Notes(_))),
            "`n` returns to the pad the confirm was raised from"
        );
        assert_eq!(app.notes.len(), 1, "nothing was deleted");

        app.on_key(char_key('d'));
        render(&app, W, H);
        let yes = region_of(&app, key('y'));
        click(&mut app, W, H, yes.rect.x, yes.rect.y);
        assert_eq!(app.notes.len(), 0, "`y` applied the delete");
    }

    #[test]
    fn a_failed_review_focuses_the_failing_row_and_opens_no_confirm() {
        let mut app = ticket_app();
        if let Some(Modal::Ticket(state)) = app.modals.last_mut() {
            state.focus = TicketField::Product;
            state.qty = "0".to_string();
        }
        app.press(KeyCode::Enter);
        let state = open_ticket_state(&app);
        assert_eq!(state.focused(), TicketField::Quantity);
        assert_eq!(
            state.issue.as_ref().map(|issue| issue.field),
            Some(Some(TicketField::Quantity))
        );
        assert_eq!(app.modals.len(), 1, "no review is opened on a failure");
    }

    #[test]
    fn paging_a_long_review_scrolls_it_and_confirms_nothing() {
        let mut app = test_app();
        app.modals.push(Modal::Confirm(long_review()));
        let top = screen(&app, 80, 24);
        assert!(top.contains("Row 0"), "{top}");

        app.on_key(char_key_code(KeyCode::PageDown));
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "PgDn is not a confirmation"
        );
        assert!(!app.pending_action, "PgDn armed a request");
        let paged = screen(&app, 80, 24);
        assert!(!paged.contains("Row 0"), "the body scrolled:\n{paged}");
        assert!(paged.contains("[y/Enter Delete locally]"), "{paged}");

        app.on_key(char_key_code(KeyCode::PageUp));
        assert_eq!(screen(&app, 80, 24), top, "PgUp returns to the top");
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));
    }

    fn reviewed_ticket() -> super::super::super::App {
        let mut app = ticket_app();
        edit_ticket(&mut app, |state| {
            state.qty = "10".to_string();
            state.price = "100".to_string();
        });
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "Enter opens the review"
        );
        app
    }

    #[tokio::test]
    async fn a_queued_second_enter_cannot_confirm_an_undrawn_review() {
        let mut app = reviewed_ticket();
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(
            matches!(app.modals.last(), Some(Modal::Confirm(_))),
            "the undrawn review is still open"
        );
        assert!(!app.pending_action, "the order was sent unseen");

        render(&app, W, H);
        assert!(!app.pending_action, "the dropped Enter was not replayed");
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(app.pending_action, "Enter on a drawn review sends it");
        assert!(app.modals.is_empty());
    }

    pub(in crate::tui::app) fn offers_confirm(app: &super::super::super::App) -> bool {
        app.hit.borrow().regions.iter().any(|region| {
            [KeyCode::Enter, KeyCode::Char('y')]
                .into_iter()
                .any(|code| {
                    region.action == ui::HitAction::Key(KeyEvent::new(code, KeyModifiers::NONE))
                })
        })
    }

    #[tokio::test]
    async fn a_review_with_no_body_width_cannot_confirm() {
        let mut app = reviewed_ticket();
        render(&app, 6, 24);
        assert!(!offers_confirm(&app), "a zero-width review offers a send");
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(!app.pending_action, "an unpainted review sent the order");
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));

        render(&app, W, H);
        assert!(offers_confirm(&app));
        app.on_key(char_key_code(KeyCode::Enter));
        assert!(app.pending_action, "the painted review sends");

        let mut app = notes_app();
        app.notes.upsert(None, "a thought", None, 1_000);
        app.open_notes();
        app.on_key(char_key('d'));
        render(&app, 6, 24);
        app.on_key(char_key('y'));
        assert_eq!(app.notes.len(), 1, "an unpainted review deleted a note");
    }

    #[tokio::test]
    async fn a_repeated_confirm_key_never_confirms() {
        let mut app = reviewed_ticket();
        render(&app, W, H);
        for code in [KeyCode::Enter, KeyCode::Char('y')] {
            app.on_key(KeyEvent::new_with_kind(
                code,
                KeyModifiers::NONE,
                KeyEventKind::Repeat,
            ));
            assert!(
                matches!(app.modals.last(), Some(Modal::Confirm(_))),
                "a repeated {code:?} closed the review"
            );
            assert!(!app.pending_action, "a repeated {code:?} sent the order");
        }
        app.on_key(char_key('y'));
        assert!(app.pending_action, "a pressed y still confirms");
    }

    #[test]
    fn a_drawn_review_does_not_vouch_for_the_next_one() {
        let mut app = notes_app();
        app.notes.upsert(None, "first", None, 100);
        app.notes.upsert(None, "second", None, 200);
        app.open_notes();
        app.on_key(char_key('d'));
        render(&app, W, H);
        app.on_key(char_key('y'));
        assert_eq!(app.notes.len(), 1, "the drawn review deleted its note");

        app.on_key(char_key('d'));
        app.on_key(char_key('y'));
        assert_eq!(app.notes.len(), 1, "an undrawn review deleted a note");
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));
    }

    fn in_flight_refusals(app: &super::super::super::App) -> usize {
        app.toasts()
            .iter()
            .filter(|toast| toast.text == "an order action is in flight")
            .count()
    }

    fn review_holds(app: &super::super::super::App, symbol: &str) -> bool {
        let pair = ("tradingsymbol".to_string(), symbol.to_string());
        matches!(
            app.modals.last(),
            Some(Modal::Confirm(review)) if review.viewport.drawn()
                && matches!(&review.action, PendingAction::Place { form, .. } if form.contains(&pair))
        )
    }

    #[tokio::test]
    async fn a_review_refused_mid_order_stays_open_and_sends_only_on_a_fresh_confirm() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "TCS", 2, |state| {
            state.qty = "5".to_string();
            state.price = "200".to_string();
        });
        let parked = std::mem::take(&mut app.modals);

        open_and_review(&mut app, "NSE", "INFY", 1, |state| {
            state.qty = "10".to_string();
            state.price = "100".to_string();
        });
        app.on_key(char_key('y'));
        assert!(app.pending_action, "the first order is in flight");
        assert!(app.modals.is_empty());
        app.ticket_prefs.clear();

        app.modals = parked;
        render(&app, W, H);
        for (attempt, code) in [KeyCode::Char('y'), KeyCode::Enter].into_iter().enumerate() {
            app.on_key(char_key_code(code));
            assert_eq!(in_flight_refusals(&app), attempt + 1, "{code:?} sent");
            assert!(
                review_holds(&app, "TCS"),
                "a refused {code:?} closed the review"
            );
            assert!(
                matches!(app.modals.first(), Some(Modal::Ticket(_))),
                "the ticket under the review is kept"
            );
            assert!(app.ticket_prefs.is_empty(), "a refused send wrote prefs");
        }

        app.on_data(DataEvent::OrderPlaced(Err("rejected".to_string())));
        assert!(
            !app.pending_action,
            "a refused confirm was queued and replayed"
        );
        assert!(
            review_holds(&app, "TCS"),
            "the reply closed the waiting review"
        );

        app.on_key(char_key('y'));
        assert!(app.pending_action, "a fresh confirm sends the second order");
        assert!(app.modals.is_empty(), "a sent review closes");
        assert_eq!(in_flight_refusals(&app), 2, "the fresh confirm was refused");
        assert!(!app.ticket_prefs.is_empty(), "the actual send writes prefs");
    }

    #[tokio::test]
    async fn a_review_refused_by_expired_auth_stays_open() {
        let mut app = test_app();
        open_and_review(&mut app, "NSE", "INFY", 1, |state| {
            state.qty = "10".to_string();
            state.price = "100".to_string();
        });
        app.on_ws(TickerEvent::Closed(kiteconnect::CloseReason::AuthRejected(
            "TokenException".to_string(),
        )));
        app.on_key(char_key('y'));
        assert!(!app.pending_action, "an order was sent on expired auth");
        assert!(review_holds(&app, "INFY"), "the refusal closed the review");
        assert!(
            app.toasts()
                .iter()
                .any(|toast| toast.text.contains("run `kite auth login`")),
            "the refusal says why"
        );
    }
}
