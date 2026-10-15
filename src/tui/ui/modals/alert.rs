use super::alert_review::alert_review_lines;
use super::{CANCEL, FIELD_HELP};
use crate::tui::alerts;
use crate::tui::app::{AlertFormFocus, AlertFormStage, AlertFormState, App, Modal};
use crate::tui::form::ModalField;
use crate::tui::format;
use crate::tui::ui::forms;
use crate::tui::ui::{act, always, nav};
use kiteconnect::ticker::Tick;
use ratatui::Frame;
use ratatui::style::{Modifier, Style};

const BASKET_BODY_ROWS: u16 = 22;

#[expect(
    clippy::too_many_lines,
    reason = "declarative footer variants keep action and hint order together"
)]
pub(in crate::tui::ui) fn alert_footer(state: &AlertFormState) -> &'static forms::FormFooter {
    match state.stage {
        AlertFormStage::Submitting(_) => {
            return const {
                &forms::FormFooter {
                    help: &["Sending to Kite..."],
                    actions: &[
                        forms::FormAction {
                            key: "y/Enter",
                            label: "Save to Kite",
                            kind: forms::ActionKind::Disabled,
                        },
                        forms::FormAction {
                            key: "Esc",
                            label: "Close (request continues)",
                            kind: forms::ActionKind::Secondary,
                        },
                    ],
                    hints: &[always(&["sending to Kite…"])],
                }
            };
        }
        AlertFormStage::Review => {
            return const {
                &forms::FormFooter {
                    help: &["Review all armed and removed orders.", "PgUp/PgDn Scroll"],
                    actions: &[
                        forms::FormAction {
                            key: "y/Enter",
                            label: "Save to Kite",
                            kind: forms::ActionKind::Primary,
                        },
                        forms::FormAction {
                            key: "Esc",
                            label: "Back to form",
                            kind: forms::ActionKind::Secondary,
                        },
                    ],
                    hints: &[always(&["y/Enter:send", "Esc:back"])],
                }
            };
        }
        AlertFormStage::Edit => {}
    }
    if state.focus == AlertFormFocus::Basket {
        return const {
            &forms::FormFooter {
                help: &[
                    "a Add",
                    "c Copy",
                    "d Remove",
                    "[/] Move",
                    "PgUp/PgDn Scroll",
                ],
                actions: &[
                    forms::FormAction {
                        key: "e/Enter",
                        label: "Edit",
                        kind: forms::ActionKind::Primary,
                    },
                    forms::FormAction {
                        key: "Tab",
                        label: "Fields",
                        kind: forms::ActionKind::Secondary,
                    },
                    CANCEL,
                ],
                hints: &[
                    act(&["a:add order", "e/Enter:edit", "d:remove", "c:copy"]),
                    nav(&["j/k:select", "[ ]:move", "Tab:fields"]),
                    always(&["Esc:cancel"]),
                ],
            }
        };
    }
    if state.ato {
        return const {
            &forms::FormFooter {
                help: FIELD_HELP,
                actions: &[
                    forms::FormAction {
                        key: "Enter",
                        label: "Review save",
                        kind: forms::ActionKind::Primary,
                    },
                    CANCEL,
                ],
                hints: &[
                    nav(&["Tab/Shift-Tab:field", "←→/space:choose", "type/↑↓:value"]),
                    always(&["Enter:review save", "Esc:cancel"]),
                ],
            }
        };
    }
    const {
        &forms::FormFooter {
            help: FIELD_HELP,
            actions: &[
                forms::FormAction {
                    key: "Enter",
                    label: "Save to Kite",
                    kind: forms::ActionKind::Primary,
                },
                CANCEL,
            ],
            hints: &[
                nav(&["Tab/Shift-Tab:field", "←→/space:choose", "type/↑↓:value"]),
                always(&["Enter:save to Kite", "Esc:cancel"]),
            ],
        }
    }
}

const BASKET_LABEL_WIDTH: usize = 16;

fn basket_lines(state: &AlertFormState) -> Vec<forms::FormItem> {
    let focused = state.focus == AlertFormFocus::Basket;
    if state.draft.baseline_unavailable() {
        return vec![
            forms::FormItem::Section("Basket (not loaded)".to_string()),
            forms::FormItem::Warning(
                "This alert's saved orders did not load. They stay exactly as saved and cannot be changed here."
                    .to_string(),
            ),
        ];
    }
    let mut items = vec![forms::FormItem::Section(format!(
        "Basket {}/{}",
        state.draft.len(),
        alerts::MAX_BASKET_ITEMS
    ))];
    if state.draft.is_empty() {
        items.push(forms::FormItem::Note(
            "No orders yet. Press a to add.".to_string(),
        ));
        return items;
    }
    for (index, item) in state.draft.items().iter().enumerate() {
        let side = item
            .editable()
            .map(|params| params.transaction_type.as_str());
        items.push(forms::FormItem::List(forms::ListRow {
            field: Some(ModalField::Alert(AlertFormFocus::Basket)),
            index,
            selected: focused && index == state.basket_selected,
            label: format::pad_or_clip(
                &format!("{} {}", index + 1, item.label()),
                BASKET_LABEL_WIDTH,
            ),
            value: item.summary(),
            value_style: match side {
                Some("SELL") => Style::default().fg(format::down()),
                Some(_) => Style::default().fg(format::up()),
                None => Style::default(),
            },
            note: item
                .editable()
                .is_none()
                .then(|| "This form cannot edit this saved order.".to_string()),
        }));
    }
    items
}

fn alert_subtitle(state: &AlertFormState, current_value: Option<f64>) -> String {
    if state.stage != AlertFormStage::Edit {
        return format!(
            "{} {} {} | status {} | {}",
            state.attribute.label(),
            state.operator.wire(),
            state.level.trim(),
            state.status,
            if state.is_edit() {
                "edited alert"
            } else {
                "new alert"
            }
        );
    }
    format!(
        "Current {} {} | server alert{}",
        state.attribute.label(),
        current_value.map_or_else(|| ".".to_string(), format::price),
        if state.ato { " with orders" } else { "" }
    )
}

pub(super) fn draw_alert(frame: &mut Frame, state: &AlertFormState, app: &App) {
    let area = frame.area();
    if state.stage == AlertFormStage::Review {
        let last_modal = app.modals.last();
        let top =
            matches!(last_modal, Some(Modal::Alert(top)) if std::ptr::eq(top.as_ref(), state));
        state.viewport.set_drawn(top && forms::contract_fits(area));
    }
    let wide = area.width >= forms::WIDE_TERMINAL;
    let current_value = app
        .tick(state.instrument_token)
        .and_then(|tick: &Tick| state.attribute.current_value(tick));
    let reviewing = state.stage != AlertFormStage::Edit;
    let focus = state.focus;
    let title = match (reviewing, state.ato, focus) {
        (true, _, _) => format!("Review {} / {}", state.kind_label(), state.label()),
        (false, true, AlertFormFocus::Basket) => format!("ATO {} / Edit basket", state.label()),
        (false, true, _) => format!("ATO {} / Edit", state.label()),
        (false, false, _) => format!("Alert {} / Edit", state.label()),
    };

    let mut items = Vec::new();
    if !forms::title_fits(area, &title) {
        items.push(forms::FormItem::Note(title.clone()));
    }
    if reviewing {
        items.extend(alert_review_lines(state, wide));
    } else {
        items.extend(alert_edit_items(state, wide));
    }
    if state.edits_a_disabled_alert() {
        items.push(forms::FormItem::Warning(
            "Saving keeps it muted; re-arm it from the Alerts view with r.".to_string(),
        ));
    }
    forms::general_issue(&mut items, state.issue.as_ref());
    let plan = forms::BodyPlan::new(&items, forms::content_width(area), forms::stacked(area));
    let body_rows = if state.ato && !reviewing {
        plan.rows().max(BASKET_BODY_ROWS)
    } else {
        plan.rows()
    };
    let layout = forms::ModalLayout::measure(area, body_rows);
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &title,
        Style::default()
            .fg(if reviewing {
                format::warning()
            } else {
                format::accent()
            })
            .add_modifier(Modifier::BOLD),
        &alert_subtitle(state, current_value),
        "Esc closes without saving.",
    ) else {
        return;
    };
    forms::draw_body(frame, app, &layout, &plan, &state.viewport);
    forms::draw_footer(frame, app, &layout, alert_footer(state));
}

fn alert_edit_items(state: &AlertFormState, wide: bool) -> Vec<forms::FormItem> {
    let mut items = Vec::new();
    let focus = state.focus;
    let issue_for = |field: AlertFormFocus| forms::issue_for(state.issue.as_ref(), field);
    items.push(forms::FormItem::Section("Condition".to_string()));
    let ineligible = state.ato_ineligible_reason();
    items.push(forms::FormItem::Row(
        forms::FormRow::segments(
            ModalField::Alert(AlertFormFocus::Kind),
            "Type",
            vec![
                forms::Segment::new("Simple", !state.ato),
                if ineligible.is_some() && !state.ato {
                    forms::Segment::disabled("ATO")
                } else {
                    forms::Segment::new("ATO", state.ato)
                },
            ],
            focus == AlertFormFocus::Kind,
        )
        .issue(issue_for(AlertFormFocus::Kind)),
    ));
    if let Some(reason) = ineligible {
        items.push(forms::FormItem::Note(format!("ATO unavailable: {reason}")));
    }
    items.push(forms::FormItem::Row(
        forms::FormRow::carousel(
            ModalField::Alert(AlertFormFocus::Attribute),
            "Attribute",
            state.attribute.label(),
            focus == AlertFormFocus::Attribute,
        )
        .issue(issue_for(AlertFormFocus::Attribute)),
    ));
    items.push(forms::FormItem::Row(
        forms::FormRow::well(
            ModalField::Alert(AlertFormFocus::Level),
            "Level",
            &state.level,
            focus == AlertFormFocus::Level,
        )
        .issue(issue_for(AlertFormFocus::Level)),
    ));
    items.push(forms::FormItem::Row(
        forms::FormRow::segments(
            ModalField::Alert(AlertFormFocus::Operator),
            "Operator",
            crate::tui::alerts::Operator::ALL
                .iter()
                .map(|operator| forms::Segment::new(operator.wire(), *operator == state.operator))
                .collect(),
            focus == AlertFormFocus::Operator,
        )
        .issue(issue_for(AlertFormFocus::Operator)),
    ));
    if state.ato {
        items.extend(basket_lines(state));
        if let Some(issue) = issue_for(AlertFormFocus::Basket) {
            items.push(forms::FormItem::Error(issue));
        }
        items.push(forms::FormItem::Note(
            "Tab returns to fields; Enter there reviews the save.".to_string(),
        ));
        items.push(forms::FormItem::Warning(
            "Saving can arm orders on your Kite account.".to_string(),
        ));
        if wide {
            items.push(forms::FormItem::Note(String::new()));
            items.push(forms::FormItem::Note(
                "Order count includes preserved items.".to_string(),
            ));
            items.push(forms::FormItem::Note(
                "Review lists every armed and removed order.".to_string(),
            ));
        }
    } else {
        items.push(forms::FormItem::Section("Effect".to_string()));
        items.push(forms::FormItem::Note(
            "Notifies when the condition matches.".to_string(),
        ));
        items.push(forms::FormItem::Note(
            "Kite keeps watching after you close the terminal.".to_string(),
        ));
        if wide {
            items.push(forms::FormItem::Note(
                "The Level error stays until Level or its controlling attribute changes."
                    .to_string(),
            ));
        }
    }
    items
}

#[cfg(test)]
mod render_tests {
    use super::super::ato::draw_ato_item;
    use super::super::render_tests::{
        assert_mockup, mockup_alert, mockup_app, mockup_ato, mockup_ato_item, modal_ascii,
        modal_rect, rect_tuple, rendered_text,
    };
    use super::draw_alert;
    use crate::tui::app::{AlertFormFocus, AlertFormStage, AlertFormState};
    use crate::tui::form::ModalField;
    use crate::tui::ui::{ControlIntent, HitAction};

    #[test]
    fn simple_alert_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_alert();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_alert(frame, &state, &app)),
            "
+- Alert NSE:INFY / Edit ----------------------------------------------+
| Current Last price 1,500.00 | server alert                           |
+----------------------------------------------------------------------+
| Condition                                                            |
|           Type  [*Simple | ATO ]                                     |
|      Attribute  [< Last price >]                                     |
| >        Level  [_             ]                                     |
|                 ! Enter a level to alert on.                         |
|       Operator  [ == | > |*>= | < | <= ]                             |
| Effect                                                               |
| Notifies when the condition matches.                                 |
| Kite keeps watching after you close the terminal.                    |
+----------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                 |
| [Enter Save to Kite]  [Esc Cancel]                                   |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 4, 72, 16));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_alert(frame, &state, &app)),
            "
+- Alert NSE:INFY / Edit --------------------------------------------------------------+
| Current Last price 1,500.00 | server alert                                           |
+--------------------------------------------------------------------------------------+
| Condition                                                                            |
|           Type  [*Simple | ATO ]                                                     |
|      Attribute  [< Last price >]                                                     |
| >        Level  [_             ]                                                     |
|                 ! Enter a level to alert on.                                         |
|       Operator  [ == | > |*>= | < | <= ]                                             |
| Effect                                                                               |
| Notifies when the condition matches.                                                 |
| Kite keeps watching after you close the terminal.                                    |
| The Level error stays until Level or its controlling attribute changes.              |
+--------------------------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                                 |
| [Enter Save to Kite]  [Esc Cancel]                                                   |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 9, 88, 17));
    }

    #[test]
    fn ato_basket_editor_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_ato();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_alert(frame, &state, &app)),
            "
+- ATO NSE:INFY / Edit basket -----------------------------------------+
| Current Last price 1,500.00 | server alert with orders               |
+----------------------------------------------------------------------+
| Condition                                                            |
|           Type  [ Simple |*ATO ]                                     |
|      Attribute  [< Last price >]                                     |
|          Level  [1510.00       ]                                     |
|       Operator  [ == | > |*>= | < | <= ]                             |
| Basket 2/20                                                          |
| > 1 NSE:INFY      BUY 10 CNC LIMIT 1505.00 DAY                       |
|   2 NSE:TCS       Kept as saved                                      |
|     ! This form cannot edit this saved order.                        |
| Tab returns to fields; Enter there reviews the save.                 |
| Saving can arm orders on your Kite account.                          |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
+----------------------------------------------------------------------+
| a Add  c Copy  d Remove  [/] Move  PgUp/PgDn Scroll                  |
| [e/Enter Edit]  [Tab Fields]  [Esc Cancel]                           |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 1, 72, 22));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_alert(frame, &state, &app)),
            "
+- ATO NSE:INFY / Edit basket ---------------------------------------------------------+
| Current Last price 1,500.00 | server alert with orders                               |
+--------------------------------------------------------------------------------------+
| Condition                                                                            |
|           Type  [ Simple |*ATO ]                                                     |
|      Attribute  [< Last price >]                                                     |
|          Level  [1510.00       ]                                                     |
|       Operator  [ == | > |*>= | < | <= ]                                             |
| Basket 2/20                                                                          |
| > 1 NSE:INFY      BUY 10 CNC LIMIT 1505.00 DAY                                       |
|   2 NSE:TCS       Kept as saved                                                      |
|     ! This form cannot edit this saved order.                                        |
| Tab returns to fields; Enter there reviews the save.                                 |
| Saving can arm orders on your Kite account.                                          |
|                                                                                      |
| Order count includes preserved items.                                                |
| Review lists every armed and removed order.                                          |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
+--------------------------------------------------------------------------------------+
| a Add  c Copy  d Remove  [/] Move  PgUp/PgDn Scroll                                  |
| [e/Enter Edit]  [Tab Fields]  [Esc Cancel]                                           |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 3, 88, 29));
    }

    #[test]
    fn an_ineligible_ato_is_shown_disabled_with_its_reason() {
        let app = mockup_app();
        let state = AlertFormState {
            exchange: "MCX".into(),
            tradingsymbol: "GOLDM25SEPFUT".into(),
            issue: None,
            ..mockup_alert()
        };
        let text = rendered_text(80, 24, |frame| draw_alert(frame, &state, &app));
        assert!(text.contains("[*Simple |-ATO ]"), "{text}");
        assert!(text.contains("ATO unavailable:"), "{text}");
        let options = app
            .hit
            .borrow()
            .regions
            .iter()
            .filter(|region| {
                matches!(
                    region.action,
                    HitAction::ModalControl {
                        field: ModalField::Alert(AlertFormFocus::Kind),
                        intent: ControlIntent::Option(_),
                    }
                )
            })
            .count();
        assert_eq!(options, 1, "the disabled ATO option must have no target");
    }

    #[test]
    fn an_empty_ato_basket_says_how_to_fill_it() {
        let app = mockup_app();
        let state = AlertFormState {
            draft: crate::tui::alerts::AtoBasketDraft::new(),
            ..mockup_ato()
        };
        let text = rendered_text(80, 24, |frame| draw_alert(frame, &state, &app));
        assert!(text.contains("Basket 0/20"), "{text}");
        assert!(text.contains("No orders yet. Press a to add."), "{text}");
    }

    #[test]
    fn an_ato_whose_basket_did_not_load_says_the_orders_are_unavailable() {
        let app = mockup_app();
        let state = AlertFormState {
            draft: crate::tui::alerts::AtoBasketDraft::unavailable(),
            ..mockup_ato()
        };
        let text = rendered_text(80, 24, |frame| draw_alert(frame, &state, &app));
        assert!(text.contains("Basket (not loaded)"), "{text}");
        assert!(!text.contains("Basket 0/20"), "{text}");
        assert!(!text.contains("Press a to add"), "{text}");
        assert!(text.contains("did not load"), "{text}");

        let review = AlertFormState {
            stage: AlertFormStage::Review,
            draft: crate::tui::alerts::AtoBasketDraft::unavailable(),
            ..mockup_ato()
        };
        let text = rendered_text(80, 40, |frame| draw_alert(frame, &review, &app));
        assert!(
            !text.contains("Orders armed"),
            "the review must not count a basket it does not hold:\n{text}"
        );
        assert!(
            text.contains("unchanged by this"),
            "the review must state the saved orders are left alone:\n{text}"
        );
    }

    #[test]
    fn a_basket_row_records_a_selection_target_for_its_own_index() {
        let app = mockup_app();
        let state = mockup_ato();
        rendered_text(80, 24, |frame| draw_alert(frame, &state, &app));
        let selections: Vec<usize> = app
            .hit
            .borrow()
            .regions
            .iter()
            .filter_map(|region| match region.action {
                HitAction::ModalControl {
                    field: ModalField::Alert(AlertFormFocus::Basket),
                    intent: ControlIntent::Option(index),
                } => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(selections, vec![0, 1], "one target per item, in order");
    }

    #[test]
    fn a_long_basket_scrolls_its_body_and_keeps_the_frame() {
        let app = mockup_app();
        let mut draft = crate::tui::alerts::AtoBasketDraft::new();
        for index in 1..=20 {
            draft
                .push(
                    "NSE".to_string(),
                    format!("SYMBOL{index}"),
                    408_065,
                    crate::tui::alerts::validate_item(
                        "NSE", 408_065, "BUY", "CNC", "LIMIT", "DAY", "", "1", "1505.00", "", 1,
                        0.05, None,
                    )
                    .expect("the fixture order is valid"),
                )
                .expect("twenty orders fit");
        }
        let state = AlertFormState {
            basket_selected: 19,
            draft,
            ..mockup_ato()
        };
        let text = rendered_text(80, 24, |frame| draw_alert(frame, &state, &app));
        assert!(text.contains("Basket 20/20"), "{text}");
        assert!(text.contains("NSE:SYMBOL1 "), "{text}");
        state.viewport.page(1);
        state.viewport.page(1);
        let text = rendered_text(80, 24, |frame| draw_alert(frame, &state, &app));
        assert!(text.contains("20 NSE:SYMBOL20"), "{text}");
        assert!(text.contains("Current Last price"), "the header is fixed");
        assert!(text.contains("[e/Enter Edit]"), "the footer is fixed");
    }

    #[test]
    fn editing_a_disabled_alert_says_saving_keeps_it_muted() {
        let app = mockup_app();
        let disabled = AlertFormState {
            uuid: Some("uuid-1".into()),
            status: kiteconnect::AlertStatus::Disabled,
            ..mockup_alert()
        };
        let text = rendered_text(80, 24, |frame| draw_alert(frame, &disabled, &app));
        assert!(
            text.contains("Saving keeps it muted; re-arm it from the Alerts view with r."),
            "a muted alert's edit form must say so, and where r works:\n{text}"
        );

        let enabled = AlertFormState {
            status: kiteconnect::AlertStatus::Enabled,
            ..disabled
        };
        let text = rendered_text(80, 24, |frame| draw_alert(frame, &enabled, &app));
        assert!(
            !text.contains("Saving keeps it muted"),
            "an armed alert must not be flagged as muted:\n{text}"
        );
    }

    #[test]
    fn the_alert_forms_survive_a_terminal_too_small_to_hold_them() {
        let app = mockup_app();
        let basket = mockup_ato();
        let item = mockup_ato_item();

        let text = modal_ascii(48, 18, |frame| draw_alert(frame, &basket, &app));
        assert_eq!(rect_tuple(modal_rect(&app)), (1, 1, 46, 16));
        assert!(text.contains("Esc Cancel"), "{text}");

        let text = modal_ascii(32, 10, |frame| draw_ato_item(frame, &item, &app));
        assert_eq!(rect_tuple(modal_rect(&app)), (1, 1, 30, 8));
        assert!(text.contains("Esc Cancel"), "{text}");

        for (w, h) in [(1, 1), (0, 0), (12, 4)] {
            let text = rendered_text(w.max(1), h.max(1), |frame| draw_alert(frame, &basket, &app));
            assert!(!text.contains("Enter Review"), "{w}x{h}:\n{text}");
            let text = rendered_text(w.max(1), h.max(1), |frame| {
                draw_ato_item(frame, &item, &app);
            });
            assert!(!text.contains("Add to basket"), "{w}x{h}:\n{text}");
        }
    }

    #[test]
    fn the_alert_footers_and_the_root_hints_advertise_the_same_actions() {
        let edit = mockup_alert();
        assert!(super::alert_footer(&edit).actions[0].label.contains("Save"));
        let ato = mockup_ato();
        let basket = super::alert_footer(&ato);
        assert_eq!(basket.actions[0].key, "e/Enter");
        assert_eq!(basket.actions[0].label, "Edit");
        let fields = super::alert_footer(&AlertFormState {
            focus: AlertFormFocus::Level,
            ..mockup_ato()
        });
        assert_eq!(fields.actions[0].label, "Review save");

        for state in [&edit, &ato] {
            let footer = super::alert_footer(state);
            let hints: String = footer
                .hints
                .iter()
                .flat_map(|group| group.hints.iter())
                .copied()
                .collect::<Vec<&str>>()
                .join(" ");
            for action in footer.actions {
                let key = action.key.split('/').next().unwrap_or(action.key);
                assert!(
                    hints.to_lowercase().contains(&key.to_lowercase()),
                    "{key} is a button with no hint: {hints}"
                );
            }
        }
    }
}
