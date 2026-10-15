use super::{CANCEL, FIELD_HELP};
use crate::tui::alerts;
use crate::tui::app::{App, AtoItemFocus, AtoItemFormState, Side};
use crate::tui::form::ModalField;
use crate::tui::format;
use crate::tui::ui::forms;
use crate::tui::ui::{always, nav};
use ratatui::Frame;

pub(in crate::tui::ui) fn ato_item_footer(state: &AtoItemFormState) -> &'static forms::FormFooter {
    if state.editing.is_some() {
        return const {
            &forms::FormFooter {
                help: FIELD_HELP,
                actions: &[
                    forms::FormAction {
                        key: "Enter",
                        label: "Keep changes",
                        kind: forms::ActionKind::Primary,
                    },
                    forms::FormAction {
                        key: "Esc",
                        label: "Discard changes",
                        kind: forms::ActionKind::Secondary,
                    },
                ],
                hints: &[
                    nav(&["Tab:field", "←→/space:choose", "type/↑↓:value"]),
                    always(&["Enter:keep changes", "Esc:discard"]),
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
                    label: "Add to basket",
                    kind: forms::ActionKind::Primary,
                },
                CANCEL,
            ],
            hints: &[
                nav(&["Tab:field", "←→/space:choose", "type/↑↓:value"]),
                always(&["Enter:add to basket", "Esc:cancel"]),
            ],
        }
    }
}

fn well(
    state: &AtoItemFormState,
    field: AtoItemFocus,
    label: &'static str,
    value: &str,
) -> forms::FormItem {
    forms::FormItem::Row(
        forms::FormRow::well(
            ModalField::AtoItem(field),
            label,
            value,
            state.focus == field && state.visible().contains(&field),
        )
        .issue(forms::issue_for(state.issue.as_ref(), field)),
    )
}

fn choices(
    state: &AtoItemFormState,
    field: AtoItemFocus,
    label: &'static str,
    values: &[&str],
    current: usize,
) -> forms::FormItem {
    forms::FormItem::Row(
        forms::FormRow::segments(
            ModalField::AtoItem(field),
            label,
            values
                .iter()
                .enumerate()
                .map(|(index, value)| forms::Segment::new(*value, index == current))
                .collect(),
            state.focus == field && state.visible().contains(&field),
        )
        .issue(forms::issue_for(state.issue.as_ref(), field)),
    )
}

fn ato_item_items(
    state: &AtoItemFormState,
    area: ratatui::layout::Rect,
    title: &str,
) -> Vec<forms::FormItem> {
    let visible = state.visible();
    let mut items = Vec::new();
    if !forms::title_fits(area, title) {
        items.push(forms::FormItem::Note(title.to_string()));
    }
    items.push(forms::FormItem::Section("Order".to_string()));
    items.push(choices(
        state,
        AtoItemFocus::Side,
        "Side",
        &["BUY", "SELL"],
        usize::from(state.side == Side::Sell),
    ));
    items.push(well(
        state,
        AtoItemFocus::Quantity,
        "Quantity",
        &state.quantity,
    ));
    items.push(choices(
        state,
        AtoItemFocus::Product,
        "Product",
        state.products(),
        state.product,
    ));
    items.push(choices(
        state,
        AtoItemFocus::OrderType,
        "Order type",
        &alerts::ATO_ORDER_TYPES,
        state.order_type,
    ));
    if visible.contains(&AtoItemFocus::Price) {
        items.push(well(state, AtoItemFocus::Price, "Price", &state.price));
    }
    if visible.contains(&AtoItemFocus::Trigger) {
        items.push(well(
            state,
            AtoItemFocus::Trigger,
            "Trigger",
            &state.trigger,
        ));
    }
    items.push(forms::FormItem::Section("Execution".to_string()));
    items.push(choices(
        state,
        AtoItemFocus::Validity,
        "Validity",
        &alerts::ATO_VALIDITIES,
        state.validity,
    ));
    if visible.contains(&AtoItemFocus::Ttl) {
        items.push(well(state, AtoItemFocus::Ttl, "TTL mins", &state.ttl));
    }
    if state.order_type() == "MARKET" {
        items.push(forms::FormItem::Note(
            "Kite places a MARKET basket order as a price-protected LIMIT order.".to_string(),
        ));
    }
    items.push(forms::FormItem::Note(
        "Adds to the draft basket. Nothing is sent yet.".to_string(),
    ));
    if area.width >= forms::WIDE_TERMINAL {
        items.push(forms::FormItem::Note(
            "Saving the parent alert is a separate action.".to_string(),
        ));
    }
    forms::general_issue(&mut items, state.issue.as_ref());

    items
}

pub(super) fn draw_ato_item(frame: &mut Frame, state: &AtoItemFormState, app: &App) {
    let area = frame.area();
    let title = match state.editing {
        Some(index) => format!("Edit basket order {} / {}", index + 1, state.label()),
        None => format!("Add basket order / {}", state.label()),
    };
    let items = ato_item_items(state, area, &title);
    let plan = forms::BodyPlan::new(&items, forms::content_width(area), forms::stacked(area));
    let layout = forms::ModalLayout::measure(area, plan.rows());
    let ltp = app.live_ltp(state.instrument_token);
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &title,
        format::title_style(),
        &format!(
            "Draft only | LTP {} | lot {} | tick {}",
            ltp.map_or_else(|| ".".to_string(), format::price),
            state.lot_size.max(1),
            state.tick_size
        ),
        "Esc discards this order.",
    ) else {
        return;
    };
    forms::draw_body(frame, app, &layout, &plan, &state.viewport);
    forms::draw_footer(frame, app, &layout, ato_item_footer(state));
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{
        assert_mockup, mockup_app, mockup_ato_item, modal_ascii, modal_rect, rect_tuple,
        rendered_text,
    };
    use super::draw_ato_item;
    use crate::tui::app::{AtoItemFocus, AtoItemFormState};
    use crate::tui::form::ModalField;
    use crate::tui::ui::HitAction;

    #[test]
    fn ato_item_form_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_ato_item();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_ato_item(frame, &state, &app)),
            "
+- Add basket order / NSE:INFY ----------------------------------------+
| Draft only | LTP 1,500.00 | lot 1 | tick 0.05                        |
+----------------------------------------------------------------------+
| Order                                                                |
|           Side  [*BUY | SELL ]                                       |
| >     Quantity  [10_           ]                                     |
|        Product  [*CNC | MIS ]                                        |
|     Order type  [ MARKET |*LIMIT | SL ]                              |
|          Price  [1505.00       ]                                     |
| Execution                                                            |
|       Validity  [ DAY | IOC |*TTL ]                                  |
|       TTL mins  [5             ]                                     |
| Adds to the draft basket. Nothing is sent yet.                       |
+----------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                 |
| [Enter Add to basket]  [Esc Cancel]                                  |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 3, 72, 17));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_ato_item(frame, &state, &app)),
            "
+- Add basket order / NSE:INFY --------------------------------------------------------+
| Draft only | LTP 1,500.00 | lot 1 | tick 0.05                                        |
+--------------------------------------------------------------------------------------+
| Order                                                                                |
|           Side  [*BUY | SELL ]                                                       |
| >     Quantity  [10_           ]                                                     |
|        Product  [*CNC | MIS ]                                                        |
|     Order type  [ MARKET |*LIMIT | SL ]                                              |
|          Price  [1505.00       ]                                                     |
| Execution                                                                            |
|       Validity  [ DAY | IOC |*TTL ]                                                  |
|       TTL mins  [5             ]                                                     |
| Adds to the draft basket. Nothing is sent yet.                                       |
| Saving the parent alert is a separate action.                                        |
+--------------------------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                                 |
| [Enter Add to basket]  [Esc Cancel]                                                  |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 8, 88, 18));
    }

    #[test]
    fn the_basket_item_editor_draws_the_fields_its_order_type_uses() {
        let app = mockup_app();
        let base = mockup_ato_item();

        let market = AtoItemFormState {
            order_type: 0,
            validity: 0,
            ..mockup_ato_item()
        };
        let text = rendered_text(80, 24, |frame| draw_ato_item(frame, &market, &app));
        assert!(!text.contains("Price"), "{text}");
        assert!(!text.contains("Trigger"), "{text}");
        assert!(!text.contains("TTL mins"), "{text}");
        assert!(text.contains("price-protected"), "{text}");
        let price_targets = app
            .hit
            .borrow()
            .regions
            .iter()
            .filter(|region| {
                matches!(
                    region.action,
                    HitAction::ModalControl {
                        field: ModalField::AtoItem(AtoItemFocus::Price),
                        ..
                    }
                )
            })
            .count();
        assert_eq!(price_targets, 0, "a hidden price leaves no stale target");

        let stop = AtoItemFormState {
            order_type: 2,
            ..market
        };
        let text = rendered_text(80, 24, |frame| draw_ato_item(frame, &stop, &app));
        assert!(text.contains("Price"), "{text}");
        assert!(text.contains("Trigger"), "{text}");

        let editing = AtoItemFormState {
            editing: Some(1),
            ..base
        };
        let text = rendered_text(80, 24, |frame| draw_ato_item(frame, &editing, &app));
        assert!(text.contains("Edit basket order 2 / NSE:INFY"), "{text}");
        assert!(text.contains("[Enter Keep changes]"), "{text}");
        assert!(text.contains("[Esc Discard changes]"), "{text}");
    }

    #[test]
    fn a_refused_basket_order_shows_its_reason_under_its_own_field() {
        let app = mockup_app();
        let issue = crate::tui::alerts::validate_item(
            "NSE", 408_065, "BUY", "CNC", "LIMIT", "DAY", "", "25", "1520.03", "", 15, 0.05, None,
        )
        .expect_err("25 is not a multiple of a fifteen-share lot");
        assert_eq!(issue.field, Some(AtoItemFocus::Quantity));
        let state = AtoItemFormState {
            quantity: "25".into(),
            price: "1520.03".into(),
            lot_size: 15,
            validity: 0,
            focus: AtoItemFocus::Quantity,
            issue: Some(issue),
            ..mockup_ato_item()
        };
        let text = rendered_text(80, 24, |frame| draw_ato_item(frame, &state, &app));
        assert!(
            text.contains("! quantity must be a multiple of the lot size 15"),
            "the refusal must be on screen:\n{text}"
        );
        assert!(
            text.contains("[25"),
            "the typed quantity must survive:\n{text}"
        );
        assert!(
            text.contains("[1520.03"),
            "the typed price must survive:\n{text}"
        );
    }
}
