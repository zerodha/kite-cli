use super::{CANCEL, TOO_SMALL};
use crate::tui::app::App;
use crate::tui::form::ModalField;
use crate::tui::format;
use crate::tui::ticket::{ConversionField, PositionConversionState};
use crate::tui::ui::forms;
use crate::tui::ui::{always, nav};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};

pub(in crate::tui::ui) fn conversion_footer() -> &'static forms::FormFooter {
    const {
        &forms::FormFooter {
            help: &[
                "Tab/Shift-Tab Field",
                "Left/Right Bucket",
                "Digits Quantity",
            ],
            actions: &[
                forms::FormAction {
                    key: "Enter",
                    label: "Review conversion",
                    kind: forms::ActionKind::Primary,
                },
                CANCEL,
            ],
            hints: &[
                nav(&["Tab:field", "←→/space:bucket", "type qty"]),
                always(&["Enter:review conversion", "Esc:cancel"]),
            ],
        }
    }
}

pub(super) fn draw_position_convert(frame: &mut Frame, state: &PositionConversionState, app: &App) {
    let area = frame.area();
    let focus = state.focus;
    let issue_for = |field: ConversionField| forms::issue_for(state.issue.as_ref(), field);
    let buckets: Vec<forms::Segment> = state
        .choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            forms::Segment::new(
                format!(
                    "{} ({})",
                    choice.position_type.label(),
                    choice.signed_quantity.unsigned_abs()
                ),
                index == state.selected,
            )
        })
        .collect();
    let title = format!(
        "Convert position / {}:{}",
        state.exchange, state.tradingsymbol
    );
    let mut items = Vec::new();
    if !forms::title_fits(area, &title) {
        items.push(forms::FormItem::Note(title.clone()));
    }
    items.extend([
        forms::FormItem::Section("Conversion".to_string()),
        forms::FormItem::Row(
            forms::FormRow::segments(
                ModalField::Conversion(ConversionField::Bucket),
                "Type",
                buckets,
                focus == ConversionField::Bucket,
            )
            .issue(issue_for(ConversionField::Bucket)),
        ),
        forms::FormItem::Row(
            forms::FormRow::well(
                ModalField::Conversion(ConversionField::Quantity),
                "Quantity",
                &state.qty,
                focus == ConversionField::Quantity,
            )
            .issue(issue_for(ConversionField::Quantity)),
        ),
        forms::FormItem::Row(forms::FormRow::fixed(
            "Product",
            format!("{} -> {}", state.old_product, state.new_product()),
            "fixed",
        )),
        forms::FormItem::Section("Impact".to_string()),
        forms::FormItem::Note("Converts the selected bucket quantity.".to_string()),
        forms::FormItem::Note("The review shows the exact signed-side conversion.".to_string()),
    ]);
    if area.width >= forms::WIDE_TERMINAL {
        items.push(forms::FormItem::Note(
            "Only the chosen bucket supplies the conversion side.".to_string(),
        ));
    }
    forms::general_issue(&mut items, state.issue.as_ref());

    let plan = forms::BodyPlan::new(&items, forms::content_width(area), forms::stacked(area));
    let layout = forms::ModalLayout::measure(area, plan.rows());
    let border = Style::default()
        .fg(format::warning())
        .add_modifier(Modifier::BOLD);
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &title,
        border,
        &format!(
            "Product {} to {} | choose the position bucket",
            state.old_product,
            state.new_product()
        ),
        TOO_SMALL,
    ) else {
        return;
    };
    forms::draw_body(frame, app, &layout, &plan, &state.viewport);
    forms::draw_footer(frame, app, &layout, conversion_footer());
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{
        assert_mockup, mockup_app, modal_ascii, modal_rect, rect_tuple,
    };
    use super::draw_position_convert;
    use crate::tui::data::Position;
    use crate::tui::ticket::PositionConversionState;

    fn mockup_position() -> Position {
        {
            let mut row = crate::tui::data::test_models::position();
            row.tradingsymbol = "INFY".to_string();
            row.exchange = "NSE".to_string();
            row.instrument_token = 408_065;
            row.product = "MIS".to_string();
            row.overnight_quantity = 5;
            row.day_buy_quantity = 10;
            row.day_sell_quantity = 0;
            row.quantity = 15;
            row.average_price = 1500.0;
            row.last_price = 1500.0;
            row.close_price = 1500.0;
            row.pnl = 0.0;
            row.realised = 0.0;
            row.multiplier = 1.0;
            row
        }
    }

    #[test]
    fn position_conversion_matches_the_spec_mockup() {
        let app = mockup_app();
        let state =
            PositionConversionState::new(&mockup_position(), 1).expect("a bucket is convertible");
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_position_convert(frame, &state, &app)),
            "
+- Convert position / NSE:INFY ----------------------------------------+
| Product MIS to CNC | choose the position bucket                      |
+----------------------------------------------------------------------+
| Conversion                                                           |
| >         Type  [*DAY (10) | OVERNIGHT (5) ]                         |
|       Quantity  [10            ]                                     |
|        Product  MIS -> CNC (fixed)                                   |
| Impact                                                               |
| Converts the selected bucket quantity.                               |
| The review shows the exact signed-side conversion.                   |
+----------------------------------------------------------------------+
| Tab/Shift-Tab Field  Left/Right Bucket  Digits Quantity              |
| [Enter Review conversion]  [Esc Cancel]                              |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 5, 72, 14));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_position_convert(frame, &state, &app)),
            "
+- Convert position / NSE:INFY --------------------------------------------------------+
| Product MIS to CNC | choose the position bucket                                      |
+--------------------------------------------------------------------------------------+
| Conversion                                                                           |
| >         Type  [*DAY (10) | OVERNIGHT (5) ]                                         |
|       Quantity  [10            ]                                                     |
|        Product  MIS -> CNC (fixed)                                                   |
| Impact                                                                               |
| Converts the selected bucket quantity.                                               |
| The review shows the exact signed-side conversion.                                   |
| Only the chosen bucket supplies the conversion side.                                 |
+--------------------------------------------------------------------------------------+
| Tab/Shift-Tab Field  Left/Right Bucket  Digits Quantity                              |
| [Enter Review conversion]  [Esc Cancel]                                              |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 10, 88, 15));
    }
}
