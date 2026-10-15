use super::book_state::loaded;
use super::headers::{gtt_header_sort, gtt_sort_targets};
use super::table::{
    TableRows, record_header_hits, right, selectable_table, sorted_title, symbol_cell, symbol_cells,
};
use crate::tui::app::{App, GttSort};
use crate::tui::data::Gtt;
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::pane::{BookList, PaneHits};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::{Cell, Row, Table};

fn gtt_type_label(gtt: &Gtt) -> &str {
    match gtt.trigger_type.as_str() {
        "single" => "Single",
        "two-leg" => "OCO",
        "trailing" => {
            if gtt.orders.len() >= 2 || gtt.condition.trigger_values.len() >= 2 {
                "Trailing OCO"
            } else {
                "Trailing single"
            }
        }
        "trailing-two-leg" => "Trailing OCO",
        other => other,
    }
}

const GTT_SYM_MIN: u16 = 12;
const GTT_SYM_MAX: u16 = 40;

pub(super) const GTT_COLS: [FitCol; 8] = [
    col(10, 6),          // ID
    col(GTT_SYM_MIN, 0), // SYMBOL
    col(15, 5),          // TYPE
    col(16, 1),          // TRIGGER
    col(10, 2),          // LTP
    col(10, 3),          // STATUS
    col(22, 4),          // ORDER
    col(11, 7),          // CREATED
];

pub(super) const GTT_LABELS: [(&str, bool); 8] = [
    ("ID", false),
    ("SYMBOL", false),
    ("TYPE", false),
    ("TRIGGER", true),
    ("LTP", true),
    ("STATUS", false),
    ("ORDER", false),
    ("CREATED", false),
];

pub fn draw_gtts_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &BookList<'_, GttSort>,
    hits: &PaneHits<'_>,
) {
    let Some(gtts) = loaded(frame, area, "GTT", app.gtts.as_deref(), "No GTT triggers") else {
        return;
    };

    let targets = gtt_sort_targets();
    let cols = columns::sort_floors(&GTT_COLS, &GTT_LABELS, &targets.map(|(idx, _)| idx));
    let fit = columns::fit_columns(table_budget(area.width), 1, &cols);
    let layout = columns::spread(
        &fit,
        &cols,
        &[columns::text(
            1,
            columns::widest(
                gtts.iter()
                    .map(|gtt| symbol_cells(&gtt.condition.exchange, &gtt.condition.tradingsymbol)),
            ),
            GTT_SYM_MAX,
        )],
    );
    let sym_w = layout.width(1);

    let display = list.display;
    let rows: Vec<Row> = display
        .iter()
        .map(|&i| {
            let gtt = &gtts[i];
            let triggers = || {
                gtt.condition
                    .trigger_values
                    .iter()
                    .map(|value| format::price(*value))
                    .collect::<Vec<_>>()
                    .join(" / ")
            };
            let order = || {
                gtt.orders.first().map_or_else(
                    || ".".to_string(),
                    |order| {
                        format!(
                            "{} {} @ {}",
                            order.transaction_type,
                            order.quantity,
                            format::price(order.price)
                        )
                    },
                )
            };
            let side_style = match gtt.orders.first().map(|o| o.transaction_type.as_str()) {
                Some("BUY") => Style::default().fg(format::up()),
                Some("SELL") => Style::default().fg(format::down()),
                _ => Style::default(),
            };
            let status_style = match gtt.status.as_str() {
                "active" => Style::default().fg(format::up()),
                "triggered" => Style::default().fg(format::warning()),
                _ => format::dim(),
            };
            let created = || {
                gtt.created_at
                    .map(|date| date.format("%Y-%m-%d").to_string())
                    .unwrap_or_default()
            };
            fit_row(
                &fit,
                &[
                    &|| Cell::from(Span::styled(gtt.id.to_string(), format::dim())),
                    &|| symbol_cell(&gtt.condition.exchange, &gtt.condition.tradingsymbol, sym_w),
                    &|| Cell::from(Span::styled(gtt_type_label(gtt).to_string(), format::dim())),
                    &|| right(triggers()),
                    &|| right(format::price(gtt.condition.last_price)),
                    &|| Cell::from(Span::styled(gtt.status.clone(), status_style)),
                    &|| Cell::from(Span::styled(order(), side_style)),
                    &|| Cell::from(Span::styled(created(), format::dim())),
                ],
            )
        })
        .collect();

    let (header, marked) =
        columns::fit_header_sorted(&fit, &layout, &GTT_LABELS, gtt_header_sort(list.sort));
    let title = sorted_title("GTT Triggers", list.sort.label(), marked);
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(header)
        .column_spacing(layout.spacing)
        .block(ui::panel(list.title(&title)));

    record_header_hits(hits, area, &fit, &layout, &cols, &targets);
    selectable_table(
        frame,
        area,
        table,
        &layout,
        TableRows {
            selected: list.selected,
            count: gtts.len(),
            viewport: Some(list.viewport),
        },
        hits,
    );
}

#[cfg(test)]
mod gtt_tests {
    use super::{GTT_COLS, GTT_SYM_MAX, gtt_type_label, table_budget};
    use crate::tui::data::Gtt;
    use crate::tui::ui::columns;
    use kiteconnect::GttOrder;

    fn shown(area_width: u16) -> Vec<&'static str> {
        let fit = columns::fit_columns(table_budget(area_width), 1, &GTT_COLS);
        super::GTT_LABELS
            .iter()
            .enumerate()
            .filter(|(idx, _)| fit.shows(*idx))
            .map(|(_, (label, _))| *label)
            .collect()
    }

    #[test]
    fn gtt_keeps_symbol_trigger_and_ltp_at_eighty_columns() {
        let shown = shown(80);
        for label in ["SYMBOL", "TRIGGER", "LTP", "STATUS"] {
            assert!(shown.contains(&label), "{label} missing at 80: {shown:?}");
        }
    }

    #[test]
    fn gtt_symbol_can_expand_to_a_full_derivative_contract() {
        assert!(
            GTT_SYM_MAX as usize >= "NFO:NIFTY26AUG24000CE".chars().count(),
            "wide GTT tables must not ellipsise ordinary derivative symbols"
        );
    }

    #[test]
    fn gtt_columns_drop_in_priority_order() {
        let mut previous: Vec<&str> = shown(200);
        assert_eq!(previous.len(), GTT_COLS.len(), "all columns at 200");
        let mut dropped: Vec<&str> = Vec::new();
        for width in (10..200).rev() {
            let shown = shown(width);
            for label in &shown {
                assert!(
                    previous.contains(label),
                    "{label} reappeared at width {width}"
                );
            }
            for label in previous.iter().filter(|l| !shown.contains(l)) {
                dropped.push(label);
            }
            previous = shown;
        }
        assert_eq!(
            dropped,
            vec!["CREATED", "ID", "TYPE", "ORDER", "STATUS", "LTP", "TRIGGER"]
        );
        assert_eq!(previous, vec!["SYMBOL"], "the symbol always survives");
    }

    #[test]
    fn gtt_shown_columns_always_fit_at_full_width() {
        for area_width in 10..=200u16 {
            let fit = columns::fit_columns(table_budget(area_width), 1, &GTT_COLS);
            let layout = columns::spread(
                &fit,
                &GTT_COLS,
                &[columns::text(1, GTT_SYM_MAX, GTT_SYM_MAX)],
            );
            let mut used = layout.inset;
            let mut count = 0u16;
            for idx in 0..GTT_COLS.len() {
                if fit.shows(idx) {
                    used += layout.width(idx);
                    count += 1;
                }
            }
            used += count.saturating_sub(1) * layout.spacing;
            assert!(
                used <= table_budget(area_width) || count == 1,
                "width {area_width}: columns render {used} in {}",
                table_budget(area_width)
            );
        }
    }

    fn gtt(trigger_type: &str, orders: Vec<GttOrder>, trigger_values: Vec<f64>) -> Gtt {
        {
            let mut row = crate::tui::data::test_models::gtt();
            row.id = 1;
            row.trigger_type = trigger_type.to_string();
            row.status = "active".to_string();
            row.created_at = (String::new()).replace(' ', "T").parse().ok();
            row.condition = {
                let mut row = crate::tui::data::test_models::gtt_condition();
                row.exchange = "NSE".to_string();
                row.tradingsymbol = "INFY".to_string();
                row.trigger_values = trigger_values;
                row.last_price = 1500.0;
                row.trailing_points = Vec::new();
                row
            };
            row.orders = orders;
            row
        }
    }

    fn order() -> GttOrder {
        {
            let mut row = crate::tui::data::test_models::gtt_order();
            row.transaction_type = "SELL".to_string();
            row.quantity = f64::from(1);
            row.price = 1490.0;
            row.product = "CNC".to_string();
            row
        }
    }

    #[test]
    fn known_types() {
        assert_eq!(
            gtt_type_label(&gtt("single", vec![order()], vec![700.0])),
            "Single"
        );
        assert_eq!(
            gtt_type_label(&gtt("two-leg", vec![order()], vec![700.0])),
            "OCO"
        );
        assert_eq!(
            gtt_type_label(&gtt("trailing-two-leg", vec![order()], vec![700.0])),
            "Trailing OCO"
        );
    }

    #[test]
    fn trailing_disambiguates_single_vs_oco() {
        assert_eq!(
            gtt_type_label(&gtt("trailing", vec![order()], vec![700.0])),
            "Trailing single"
        );
        assert_eq!(
            gtt_type_label(&gtt("trailing", vec![order(), order()], vec![700.0])),
            "Trailing OCO"
        );
        assert_eq!(
            gtt_type_label(&gtt("trailing", vec![order()], vec![700.0, 800.0])),
            "Trailing OCO"
        );
    }

    #[test]
    fn unknown_passes_through() {
        assert_eq!(
            gtt_type_label(&gtt("bracket", vec![order()], vec![700.0])),
            "bracket"
        );
    }
}
