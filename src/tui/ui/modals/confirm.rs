use crate::tui::app::{App, Modal};
use crate::tui::form::FormViewport;
use crate::tui::format;
use crate::tui::ticket::{ConfirmState, PendingAction};
use crate::tui::ui::forms;
use crate::tui::ui::{HitAction, HitText, always};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

const BACK: forms::FormAction = forms::FormAction {
    key: "n/Esc",
    label: "Back",
    kind: forms::ActionKind::Secondary,
};

const DRAFT_HELP: &str = "n/Esc returns to the draft. Estimates can change.";

macro_rules! confirm_footer {
    ($help:expr, $label:literal, $hint:literal, $kind:ident) => {
        const {
            &forms::FormFooter {
                help: &[$help],
                actions: &[
                    forms::FormAction {
                        key: "y/Enter",
                        label: $label,
                        kind: forms::ActionKind::$kind,
                    },
                    BACK,
                ],
                hints: &[always(&[concat!("y/Enter:", $hint), "n/Esc:back"])],
            }
        }
    };
}

pub(in crate::tui::ui) fn confirm_footer(state: &ConfirmState) -> &'static forms::FormFooter {
    match &state.action {
        PendingAction::SetAlertStatus { .. } => confirm_footer!(
            "n/Esc keeps the alert disabled.",
            "Enable ATO",
            "enable ATO",
            Primary
        ),
        PendingAction::DiscardAlertDraft => confirm_footer!(
            "n/Esc keeps the draft.",
            "Discard changes",
            "discard changes",
            Destructive
        ),
        PendingAction::Place { .. } => {
            confirm_footer!(DRAFT_HELP, "Place order", "place order", Primary)
        }
        PendingAction::Modify { .. } => {
            confirm_footer!(DRAFT_HELP, "Modify order", "modify order", Primary)
        }
        PendingAction::PlaceGtt { .. } => {
            confirm_footer!(DRAFT_HELP, "Create GTT", "create GTT", Primary)
        }
        PendingAction::ModifyGtt { .. } => {
            confirm_footer!(DRAFT_HELP, "Modify GTT", "modify GTT", Primary)
        }
        PendingAction::ConvertPosition { .. } => {
            confirm_footer!(DRAFT_HELP, "Convert position", "convert position", Primary)
        }
        PendingAction::AuthoriseHoldings => confirm_footer!(
            "n/Esc returns without opening Kite Web.",
            "Authorise holdings",
            "authorise holdings",
            Primary
        ),
        PendingAction::CancelOrders { .. } => confirm_footer!(
            "n/Esc leaves the orders as they are.",
            "Cancel orders",
            "cancel orders",
            Destructive
        ),
        PendingAction::DeleteGtt { .. } => confirm_footer!(
            "n/Esc keeps the trigger and returns to the list.",
            "Delete GTT",
            "delete GTT",
            Destructive
        ),
        PendingAction::DeleteAlert { .. } => confirm_footer!(
            "n/Esc keeps the alert and returns to the list.",
            "Delete alert",
            "delete alert",
            Destructive
        ),
        PendingAction::DeleteWatchlist { .. } => confirm_footer!(
            "n/Esc keeps the list and returns.",
            "Delete watchlist",
            "delete watchlist",
            Destructive
        ),
        PendingAction::DeleteNote { .. } => confirm_footer!(
            "n/Esc keeps the note and returns to the list.",
            "Delete locally",
            "delete locally",
            Destructive
        ),
        PendingAction::Logout => confirm_footer!(
            "n/Esc keeps the session and returns.",
            "Clear session and exit",
            "clear session and exit",
            Primary
        ),
    }
}

struct ConfirmCopy {
    title: String,
    subtitle: &'static str,
    section: &'static str,
    impact: &'static [&'static str],
    wide_note: Option<&'static str>,
}

const REVIEW_NOTE: Option<&'static str> = Some("No editable fields in review.");

#[expect(
    clippy::too_many_lines,
    reason = "each pending action defines its review copy in one exhaustive match"
)]
fn confirm_copy(state: &ConfirmState) -> ConfirmCopy {
    let review = |stage: &str| format!("{stage} / {}", state.title);
    let confirm = || format!("{} / Confirm", state.title);
    let sending = "Review values before sending to Kite";
    match &state.action {
        PendingAction::SetAlertStatus { .. } => ConfirmCopy {
            title: confirm(),
            subtitle: "Enable the saved condition and order basket",
            section: "ATO alert",
            impact: &["Arms these orders to execute when the condition fires."],
            wide_note: REVIEW_NOTE,
        },
        PendingAction::DiscardAlertDraft => ConfirmCopy {
            title: confirm(),
            subtitle: "Discard unsaved basket changes",
            section: "Draft",
            impact: &["Discards the changes in this alert editor."],
            wide_note: None,
        },
        PendingAction::Place { .. } => ConfirmCopy {
            title: review("Review order"),
            subtitle: sending,
            section: "Order",
            impact: &["Places this order on your Kite account."],
            wide_note: REVIEW_NOTE,
        },
        PendingAction::Modify { .. } => ConfirmCopy {
            title: review("Review order"),
            subtitle: sending,
            section: "Order",
            impact: &["Modifies this order on your Kite account."],
            wide_note: REVIEW_NOTE,
        },
        PendingAction::PlaceGtt { .. } => ConfirmCopy {
            title: review("Review"),
            subtitle: sending,
            section: "GTT",
            impact: &["Creates this stored trigger on your Kite account."],
            wide_note: REVIEW_NOTE,
        },
        PendingAction::ModifyGtt { .. } => ConfirmCopy {
            title: review("Review"),
            subtitle: sending,
            section: "GTT",
            impact: &["Modifies this stored trigger on your Kite account."],
            wide_note: REVIEW_NOTE,
        },
        PendingAction::ConvertPosition { .. } => ConfirmCopy {
            title: review("Review"),
            subtitle: sending,
            section: "Conversion",
            impact: &["Converts this position on your Kite account."],
            wide_note: REVIEW_NOTE,
        },
        PendingAction::AuthoriseHoldings => ConfirmCopy {
            title: confirm(),
            subtitle: "Sent to Kite | authorises the holdings below",
            section: "Holdings",
            impact: &["Opens Kite Web to authorise these holdings with CDSL."],
            wide_note: None,
        },
        PendingAction::CancelOrders { .. } => ConfirmCopy {
            title: confirm(),
            subtitle: "Sent to Kite | cancels the orders below",
            section: "Orders",
            impact: &["Cancels these orders on your Kite account."],
            wide_note: None,
        },
        PendingAction::DeleteGtt { .. } => ConfirmCopy {
            title: confirm(),
            subtitle: "Sent to Kite | deletes the trigger below",
            section: "GTT",
            impact: &["Deletes this stored trigger from your Kite account."],
            wide_note: None,
        },
        PendingAction::DeleteAlert { .. } => ConfirmCopy {
            title: confirm(),
            subtitle: "Sent to Kite | deletes the alert below",
            section: "Alert",
            impact: &["Deletes this alert from your Kite account."],
            wide_note: None,
        },
        PendingAction::DeleteWatchlist { .. } => ConfirmCopy {
            title: confirm(),
            subtitle: "Sent to Kite | deletes the list below",
            section: "Watchlist",
            impact: &["Deletes this list and every instrument on it from your Kite account."],
            wide_note: None,
        },
        PendingAction::DeleteNote { .. } => ConfirmCopy {
            title: confirm(),
            subtitle: "Local change | nothing is sent to Kite",
            section: "Note",
            impact: &["Deletes this note from the local notes file."],
            wide_note: None,
        },
        PendingAction::Logout => ConfirmCopy {
            title: confirm(),
            subtitle: "Clear stored session and exit",
            section: "Session",
            impact: &["Clears the saved session and exits the terminal."],
            wide_note: None,
        },
    }
}

fn review_prose(out: &mut Vec<Line<'static>>, text: &str, indent: u16, content: u16, style: Style) {
    let width = usize::from(content.saturating_sub(indent)).max(1);
    let pad = " ".repeat(usize::from(indent));
    for chunk in forms::wrap_cells(text, width) {
        out.push(Line::from(Span::styled(format!("{pad}{chunk}"), style)));
    }
}

fn review_value(
    out: &mut Vec<Line<'static>>,
    label: &str,
    value: &str,
    content: u16,
    stacked: bool,
) {
    let label_style = format::dim();
    if stacked {
        out.push(Line::from(Span::styled(
            format::pad_or_clip(&format!("  {label}"), usize::from(content)),
            label_style,
        )));
        review_prose(out, value, 2, content, Style::default());
        return;
    }
    let available = usize::from(content.saturating_sub(forms::CONTROL_COLUMN)).max(1);
    let mut chunks = forms::wrap_cells(value, available).into_iter();
    let first = chunks.next().unwrap_or_default();
    out.push(Line::from(vec![
        Span::styled("  ".to_string(), label_style),
        Span::styled(
            format::pad_or_clip_right(label, usize::from(forms::LABEL_WIDTH)),
            label_style,
        ),
        Span::raw("  "),
        Span::raw(first),
    ]));
    for chunk in chunks {
        review_prose(
            out,
            &chunk,
            forms::CONTROL_COLUMN,
            content,
            Style::default(),
        );
    }
}

fn review_lines(state: &ConfirmState, copy: &ConfirmCopy, area: Rect) -> Vec<Line<'static>> {
    let content = forms::content_width(area);
    let stacked = forms::stacked(area);
    let mut lines: Vec<Line<'static>> = Vec::new();
    if !forms::title_fits(area, &copy.title) {
        review_prose(&mut lines, &copy.title, 0, content, format::dim());
    }
    lines.push(super::layout::section_line(copy.section, content));
    for (label, value) in &state.lines {
        review_value(&mut lines, label, value, content, stacked);
    }
    if state.preview_request.is_some() {
        lines.push(super::layout::section_line("Estimate", content));
        match &state.margin {
            None => review_value(&mut lines, "Margin", "Calculating...", content, stacked),
            Some(Ok(preview)) => {
                review_value(
                    &mut lines,
                    "Margin",
                    &format::price(preview.total),
                    content,
                    stacked,
                );
                review_value(
                    &mut lines,
                    "Charges",
                    &format!("~{}", format::price(preview.charges)),
                    content,
                    stacked,
                );
            }
            Some(Err(err)) => review_prose(
                &mut lines,
                &format!("! Estimate unavailable: {err}"),
                if stacked { 2 } else { forms::CONTROL_COLUMN },
                content,
                Style::default().fg(format::error()),
            ),
        }
    }
    lines.push(super::layout::section_line("Impact", content));
    let impact = Style::default().fg(
        if confirm_footer(state)
            .actions
            .iter()
            .any(|action| action.kind == forms::ActionKind::Destructive)
        {
            format::error()
        } else {
            format::warning()
        },
    );
    for sentence in copy.impact {
        review_prose(&mut lines, sentence, 0, content, impact);
    }
    if area.width >= forms::WIDE_TERMINAL {
        lines.push(Line::default());
        if let Some(note) = copy.wide_note {
            review_prose(&mut lines, note, 0, content, format::dim());
        }
    }
    lines
}

fn draw_review_body(
    frame: &mut Frame,
    app: &App,
    layout: &forms::ModalLayout,
    lines: &[Line<'static>],
    viewport: &FormViewport,
) {
    if layout.body.height == 0 || layout.body.width == 0 {
        return;
    }
    let scroll = viewport.resolve(review_rows(lines), layout.body.height, None);
    app.hit
        .borrow_mut()
        .push(layout.body, HitAction::Text(HitText::ModalBody));
    for offset in 0..layout.body.height {
        let Some(line) = lines.get(usize::from(scroll) + usize::from(offset)) else {
            break;
        };
        frame.render_widget(
            line,
            Rect {
                y: layout.body.y + offset,
                height: 1,
                ..layout.body
            },
        );
    }
}

fn review_rows(lines: &[Line<'static>]) -> u16 {
    u16::try_from(lines.len()).unwrap_or(u16::MAX)
}

const CONFIRM_TOO_SMALL: &str = "n/Esc returns without acting.";

pub(super) fn draw_confirm(frame: &mut Frame, state: &ConfirmState, app: &App) {
    let area = frame.area();
    let top =
        matches!(app.modals.last(), Some(Modal::Confirm(top)) if std::ptr::eq(top.as_ref(), state));
    state.viewport.set_drawn(top && forms::contract_fits(area));
    let copy = confirm_copy(state);
    let lines = review_lines(state, &copy, area);
    let layout = forms::ModalLayout::measure(area, review_rows(&lines));
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &copy.title,
        Style::default()
            .fg(format::warning())
            .add_modifier(Modifier::BOLD),
        copy.subtitle,
        CONFIRM_TOO_SMALL,
    ) else {
        return;
    };
    draw_review_body(frame, app, &layout, &lines, &state.viewport);
    forms::draw_footer(frame, app, &layout, confirm_footer(state));
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{
        assert_mockup, mockup_app, modal_ascii, modal_rect, rect_tuple, rendered_text,
    };
    use super::{confirm_footer, draw_confirm};
    use crate::tui::data::MarginPreview;
    use crate::tui::form::FormViewport;
    use crate::tui::ticket::{ConfirmEffect, ConfirmState, PendingAction};
    use crate::tui::ui::{HitAction, HitText};

    fn mockup_review() -> ConfirmState {
        ConfirmState {
            title: "BUY NSE:INFY".to_string(),
            lines: vec![
                ("Quantity".to_string(), "10".to_string()),
                ("Order type".to_string(), "LIMIT".to_string()),
                ("Price".to_string(), "1,500.00".to_string()),
                ("Product".to_string(), "CNC".to_string()),
                ("Validity".to_string(), "DAY".to_string()),
            ],
            action: PendingAction::Place {
                variety: "regular".to_string(),
                form: Vec::new(),
            },
            preview_request: Some(serde_json::json!({})),
            preview_seq: Some(1),
            margin: Some(Ok(MarginPreview {
                total: 15_000.0,
                charges: 18.5,
            })),
            ticket_prefs: None,
            viewport: FormViewport::default(),
        }
    }

    fn mockup_note_delete() -> ConfirmState {
        ConfirmState::plain(
            "Delete note".to_string(),
            vec![
                ("Instrument".to_string(), "NSE:INFY".to_string()),
                ("Written".to_string(), "2026-09-10 09:30".to_string()),
                (
                    "Preview".to_string(),
                    "Watch the opening range.".to_string(),
                ),
            ],
            PendingAction::DeleteNote {
                id: 1,
                label: "Watch the opening range.".to_string(),
            },
        )
    }

    #[test]
    fn order_review_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_review();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app)),
            "
+- Review order / BUY NSE:INFY ----------------------------------------+
| Review values before sending to Kite                                 |
+----------------------------------------------------------------------+
| Order                                                                |
|       Quantity  10                                                   |
|     Order type  LIMIT                                                |
|          Price  1,500.00                                             |
|        Product  CNC                                                  |
|       Validity  DAY                                                  |
| Estimate                                                             |
|         Margin  15,000.00                                            |
|        Charges  ~18.50                                               |
| Impact                                                               |
| Places this order on your Kite account.                              |
+----------------------------------------------------------------------+
| n/Esc returns to the draft. Estimates can change.                    |
| [y/Enter Place order]  [n/Esc Back]                                  |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 3, 72, 18));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_confirm(frame, &state, &app)),
            "
+- Review order / BUY NSE:INFY --------------------------------------------------------+
| Review values before sending to Kite                                                 |
+--------------------------------------------------------------------------------------+
| Order                                                                                |
|       Quantity  10                                                                   |
|     Order type  LIMIT                                                                |
|          Price  1,500.00                                                             |
|        Product  CNC                                                                  |
|       Validity  DAY                                                                  |
| Estimate                                                                             |
|         Margin  15,000.00                                                            |
|        Charges  ~18.50                                                               |
| Impact                                                                               |
| Places this order on your Kite account.                                              |
|                                                                                      |
| No editable fields in review.                                                        |
+--------------------------------------------------------------------------------------+
| n/Esc returns to the draft. Estimates can change.                                    |
| [y/Enter Place order]  [n/Esc Back]                                                  |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 7, 88, 20));
    }

    #[test]
    fn generic_confirmation_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_note_delete();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app)),
            "
+- Delete note / Confirm ----------------------------------------------+
| Local change | nothing is sent to Kite                               |
+----------------------------------------------------------------------+
| Note                                                                 |
|     Instrument  NSE:INFY                                             |
|        Written  2026-09-10 09:30                                     |
|        Preview  Watch the opening range.                             |
| Impact                                                               |
| Deletes this note from the local notes file.                         |
+----------------------------------------------------------------------+
| n/Esc keeps the note and returns to the list.                        |
| [y/Enter Delete locally]  [n/Esc Back]                               |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 5, 72, 13));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_confirm(frame, &state, &app)),
            "
+- Delete note / Confirm --------------------------------------------------------------+
| Local change | nothing is sent to Kite                                               |
+--------------------------------------------------------------------------------------+
| Note                                                                                 |
|     Instrument  NSE:INFY                                                             |
|        Written  2026-09-10 09:30                                                     |
|        Preview  Watch the opening range.                                             |
| Impact                                                                               |
| Deletes this note from the local notes file.                                         |
|                                                                                      |
+--------------------------------------------------------------------------------------+
| n/Esc keeps the note and returns to the list.                                        |
| [y/Enter Delete locally]  [n/Esc Back]                                               |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 10, 88, 14));
    }

    #[test]
    fn a_margin_estimate_renders_pending_success_and_rejection() {
        let app = mockup_app();
        let mut state = mockup_review();

        state.margin = None;
        let text = modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app));
        assert!(text.contains("Margin  Calculating..."), "{text}");

        state.margin = Some(Ok(MarginPreview {
            total: 15_000.0,
            charges: 18.5,
        }));
        let text = modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app));
        assert!(text.contains("Margin  15,000.00"), "{text}");
        assert!(text.contains("Charges  ~18.50"), "{text}");

        state.margin = Some(Err("margin service unavailable".to_string()));
        let text = modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app));
        assert!(
            text.contains("! Estimate unavailable: margin service unavailable"),
            "{text}"
        );
        assert!(text.contains("[y/Enter Place order]"), "{text}");
    }

    #[test]
    fn a_confirm_without_a_preview_shows_no_estimate_group() {
        let app = mockup_app();
        let text = modal_ascii(80, 24, |frame| {
            draw_confirm(frame, &mockup_note_delete(), &app);
        });
        assert!(!text.contains("Estimate"), "{text}");
        assert!(!text.contains("Margin"), "{text}");
    }

    #[test]
    fn every_confirmable_action_names_its_own_primary() {
        let mut seen: Vec<&str> = Vec::new();
        for action in every_pending_action() {
            let effect = action.effect();
            let state = ConfirmState::plain("Subject".to_string(), Vec::new(), action);
            let footer = confirm_footer(&state);
            let primary = footer.actions[0];
            assert_ne!(primary.label, "Yes", "a confirm never says just Yes");
            assert_eq!(primary.key, "y/Enter");
            assert_eq!(footer.actions[1].key, "n/Esc");
            let destructive = matches!(primary.kind, super::forms::ActionKind::Destructive);
            assert_eq!(
                destructive,
                primary.label.starts_with("Delete") || primary.label.starts_with("Cancel"),
                "{} is styled wrong for what it does",
                primary.label
            );
            let hints: String = footer
                .hints
                .iter()
                .flat_map(|group| group.hints.iter())
                .copied()
                .collect::<Vec<&str>>()
                .join(" ");
            for button in footer.actions {
                assert!(
                    hints.contains(&format!("{}:", button.key)),
                    "{} is on a button but not in the hints: {hints}",
                    button.key
                );
            }
            let help = footer.help.join(" ");
            if effect != ConfirmEffect::KiteRequest {
                assert!(!help.contains("Kite"), "{help:?} claims a request");
            }
            assert!(
                !seen.contains(&primary.label),
                "two actions share the primary {}",
                primary.label
            );
            seen.push(primary.label);
        }
        assert_eq!(seen.len(), 12, "every variant is covered");
    }

    fn every_pending_action() -> Vec<PendingAction> {
        vec![
            PendingAction::Logout,
            PendingAction::Place {
                variety: "regular".to_string(),
                form: Vec::new(),
            },
            PendingAction::Modify {
                variety: "regular".to_string(),
                order_id: "1".to_string(),
                form: Vec::new(),
            },
            PendingAction::ConvertPosition { form: Vec::new() },
            PendingAction::AuthoriseHoldings,
            PendingAction::CancelOrders {
                targets: Vec::new(),
            },
            PendingAction::DeleteGtt { trigger_id: 7 },
            PendingAction::PlaceGtt { form: Vec::new() },
            PendingAction::ModifyGtt {
                trigger_id: 7,
                form: Vec::new(),
            },
            PendingAction::DeleteAlert {
                uuid: "u".to_string(),
                label: "INFY".to_string(),
            },
            PendingAction::DeleteNote {
                id: 1,
                label: "a thesis".to_string(),
            },
            PendingAction::DeleteWatchlist {
                watch: "1".to_string(),
                name: "Longs".to_string(),
            },
        ]
    }

    #[test]
    fn a_note_delete_confirm_promises_no_kite_request() {
        let app = mockup_app();
        let text = modal_ascii(80, 24, |frame| {
            draw_confirm(frame, &mockup_note_delete(), &app);
        });
        assert!(text.contains("nothing is sent to Kite"), "{text}");
        assert!(text.contains("local notes file"), "{text}");
        for promise in ["send", "Sent", "sending"] {
            assert!(
                !text.contains(promise),
                "a local note delete said {promise:?}:\n{text}"
            );
        }

        assert_eq!(
            PendingAction::DeleteNote {
                id: 1,
                label: "a thesis".to_string(),
            }
            .effect(),
            ConfirmEffect::Local
        );
        assert_eq!(PendingAction::Logout.effect(), ConfirmEffect::Logout);
        assert_eq!(
            PendingAction::DeleteGtt { trigger_id: 7 }.effect(),
            ConfirmEffect::KiteRequest
        );

        let gtt = ConfirmState::plain(
            "Delete GTT".to_string(),
            vec![("Trigger".to_string(), "7".to_string())],
            PendingAction::DeleteGtt { trigger_id: 7 },
        );
        let text = modal_ascii(80, 24, |frame| draw_confirm(frame, &gtt, &app));
        assert!(
            text.contains("Sent to Kite | deletes the trigger below"),
            "{text}"
        );
        assert!(text.contains("[y/Enter Delete GTT]"), "{text}");
    }

    #[test]
    fn a_logout_confirm_says_what_it_clears() {
        let app = mockup_app();
        let state = ConfirmState::plain(
            "LOGOUT".to_string(),
            vec![(
                "Session".to_string(),
                "clear saved Kite credentials and exit".to_string(),
            )],
            PendingAction::Logout,
        );
        let text = modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app));
        assert!(text.contains("Clear stored session and exit"), "{text}");
        assert!(text.contains("[y/Enter Clear session and exit]"), "{text}");
    }

    #[test]
    fn a_long_review_scrolls_its_body() {
        let app = mockup_app();
        let mut state = mockup_review();
        state.lines = (0..40)
            .map(|row| (format!("Row {row}"), format!("value {row}")))
            .collect();
        let text = modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app));
        assert!(text.contains("Row 0"), "the top is shown first:\n{text}");
        assert!(!text.contains("Row 39"), "{text}");
        state.viewport.page(1);
        let text = modal_ascii(80, 24, |frame| draw_confirm(frame, &state, &app));
        assert!(!text.contains("Row 0"), "the page moved:\n{text}");
        assert!(text.contains("Review order / BUY NSE:INFY"), "{text}");
        assert!(text.contains("[y/Enter Place order]"), "{text}");
        assert!(text.contains("[n/Esc Back]"), "{text}");
        let body = app
            .hit
            .borrow()
            .regions
            .iter()
            .rev()
            .find(|region| region.action == HitAction::Text(HitText::ModalBody))
            .map(|region| region.rect);
        assert!(body.is_some(), "the body records the wheel target");
    }

    #[test]
    fn a_long_review_value_wraps_under_its_label() {
        let app = mockup_app();
        let mut state = mockup_note_delete();
        state.lines = vec![
            ("Preview".to_string(), "東京証券取引所 ".repeat(12)),
            ("Written".to_string(), "2026-09-10 09:30".to_string()),
        ];
        for (w, h) in [(80, 24), (120, 35), (48, 18), (32, 10)] {
            let text = modal_ascii(w, h, |frame| draw_confirm(frame, &state, &app));
            for line in text.lines() {
                assert!(
                    line.ends_with('|') || line.ends_with('+'),
                    "{line:?} escaped the frame at {w}x{h}"
                );
            }
            if h >= 18 {
                assert!(
                    text.matches("東").count() > 1,
                    "the value wrapped onto more than one row at {w}x{h}:\n{text}"
                );
            }
        }
    }

    #[test]
    fn a_tiny_frame_draws_no_confirm_action() {
        let app = mockup_app();
        let state = mockup_review();
        for (w, h) in [(48, 18), (32, 10), (32, 9), (20, 6), (1, 1), (0, 0)] {
            if w == 0 || h == 0 {
                continue;
            }
            let text = rendered_text(w, h, |frame| draw_confirm(frame, &state, &app));
            if h < 10 {
                assert!(
                    !text.contains("Place order"),
                    "a {w}x{h} frame offered a send:\n{text}"
                );
            }
        }
    }
}
