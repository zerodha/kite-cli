use super::book_state::loaded;
use super::headers::{positions_header_sort, positions_sort_targets};
use super::inline_chart::{chart_split, draw_chart_panel};
use super::table::{
    TableRows, record_header_hits, right, right_styled, selectable_table, sorted_title,
    symbol_cell, symbol_cells,
};
use crate::tui::app::{App, PositionsSort};
use crate::tui::data::Position;
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::pane::{BookList, InlineChartInput, PaneHits};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};

pub(super) struct PositionsSummary {
    pub(super) total: f64,
    pub(super) realized: f64,
    pub(super) unrealized: f64,
    pub(super) open: usize,
    pub(super) total_count: usize,
}

fn positions_summary(positions: &[Position], ltp: impl Fn(u32) -> Option<f64>) -> PositionsSummary {
    let mut summary = PositionsSummary {
        total: 0.0,
        realized: 0.0,
        unrealized: 0.0,
        open: 0,
        total_count: positions.len(),
    };
    for position in positions {
        let live_pnl =
            crate::tui::data::position_live_pnl(position, ltp(position.instrument_token));
        summary.total += live_pnl;
        summary.realized += position.realised;
        summary.unrealized += live_pnl - position.realised;
        if position.quantity != 0 {
            summary.open += 1;
        }
    }
    summary
}

fn signed_inr(value: f64) -> String {
    if value >= 0.0 {
        format!("+{}", format::inr(value))
    } else {
        format::inr(value)
    }
}

pub(super) fn positions_summary_spans(
    summary: &PositionsSummary,
    width: u16,
) -> Vec<Span<'static>> {
    let segments = [
        ("  Positions P&L ", signed_inr(summary.total), summary.total),
        (
            "   ·   realized ",
            signed_inr(summary.realized),
            summary.realized,
        ),
        (
            "  ·  unrealized ",
            signed_inr(summary.unrealized),
            summary.unrealized,
        ),
    ];
    let counts = format!(
        "   ·   {} open / {} total",
        summary.open, summary.total_count
    );
    let width_of = |label: &str, value: &str| -> u16 {
        u16::try_from(label.chars().count() + value.chars().count()).unwrap_or(u16::MAX)
    };
    let fit = columns::fit_columns(
        width,
        0,
        &[
            col(width_of(segments[0].0, &segments[0].1), 0), // P&L total
            col(width_of(segments[1].0, &segments[1].1), 2), // realized
            col(width_of(segments[2].0, &segments[2].1), 1), // unrealized
            col(u16::try_from(counts.chars().count()).unwrap_or(u16::MAX), 3), // counts
        ],
    );

    let mut spans: Vec<Span<'static>> = Vec::new();
    for (idx, (label, value, amount)) in segments.into_iter().enumerate() {
        if fit.shows(idx) {
            spans.push(Span::styled(label.to_string(), format::dim()));
            spans.push(Span::styled(value, format::change_style(amount)));
        }
    }
    if fit.shows(3) {
        spans.push(Span::styled(counts, format::dim()));
    }
    spans
}

const POS_SYM_MIN: u16 = 10;
const POS_SYM_MAX: u16 = 24;
const POS_PNL_W: u16 = 12;

pub(super) const POS_COLS: [FitCol; 7] = [
    col(POS_SYM_MIN, 0), // SYMBOL
    col(6, 6),           // PROD
    col(7, 3),           // QTY
    col(11, 5),          // AVG
    col(11, 2),          // LTP
    col(POS_PNL_W, 1),   // P&L
    col(8, 4),           // DAY%
];

pub(super) const POS_LABELS: [(&str, bool); 7] = [
    ("SYMBOL", false),
    ("PROD", false),
    ("QTY", true),
    ("AVG", true),
    ("LTP", true),
    ("P&L", true),
    ("DAY%", true),
];

fn position_row(
    app: &App,
    position: &Position,
    fit: &columns::ColumnFit,
    sym_w: u16,
) -> Row<'static> {
    let live_ltp = app.live_ltp(position.instrument_token);
    let pnl = crate::tui::data::position_live_pnl(position, live_ltp);
    let ltp = live_ltp.unwrap_or(position.last_price);
    let day_pct = if position.close_price > 0.0 {
        Some((ltp - position.close_price) / position.close_price * 100.0)
    } else {
        None
    };
    let qty_style = if position.quantity == 0 {
        format::dim()
    } else {
        Style::default()
    };
    fit_row(
        fit,
        &[
            &|| symbol_cell(&position.exchange, &position.tradingsymbol, sym_w),
            &|| Cell::from(Span::styled(position.product.clone(), format::dim())),
            &|| right_styled(position.quantity.to_string(), qty_style),
            &|| right(format::price(position.average_price)),
            &|| right(format::price(ltp)),
            &|| {
                right_styled(
                    format::fit_money(pnl, &format::signed_price(pnl), POS_PNL_W as usize),
                    format::change_style(pnl),
                )
            },
            &|| {
                right_styled(
                    day_pct.map_or_else(|| ".".into(), format::percent),
                    day_pct.map(format::change_style).unwrap_or_default(),
                )
            },
        ],
    )
}

pub fn draw_positions_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &BookList<'_, PositionsSort>,
    chart: &InlineChartInput<'_>,
    hits: &PaneHits<'_>,
) {
    let Some(positions) = loaded(
        frame,
        area,
        "Positions",
        app.positions
            .as_ref()
            .map(|positions| positions.net.as_slice()),
        "No positions today",
    ) else {
        return;
    };

    let split = chart_split(area, &app.splits, chart.open);
    let (content_area, chart_area) = (split.table, split.chart);
    let summary = positions_summary(positions, |token| app.live_ltp(token));
    let summary_line = Line::from(positions_summary_spans(&summary, content_area.width));
    let [summary_area, table_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(content_area);
    frame.render_widget(Paragraph::new(summary_line), summary_area);

    let targets = positions_sort_targets();
    let cols = columns::sort_floors(&POS_COLS, &POS_LABELS, &targets.map(|(idx, _)| idx));
    let fit = columns::fit_columns(table_budget(table_area.width), 1, &cols);
    let layout = columns::spread(
        &fit,
        &cols,
        &[columns::text(
            0,
            columns::widest(
                positions
                    .iter()
                    .map(|position| symbol_cells(&position.exchange, &position.tradingsymbol)),
            ),
            POS_SYM_MAX,
        )],
    );
    let sym_w = layout.width(0);
    let display = list.display;
    let rows: Vec<Row> = display
        .iter()
        .map(|&i| position_row(app, &positions[i], &fit, sym_w))
        .collect();

    let count = positions.len();
    let (header, marked) =
        columns::fit_header_sorted(&fit, &layout, &POS_LABELS, positions_header_sort(list.sort));
    let title = sorted_title("Positions", list.sort.label(), marked);
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(header)
        .column_spacing(layout.spacing)
        .block(ui::panel(list.title(&title)));

    record_header_hits(hits, table_area, &fit, &layout, &cols, &targets);
    selectable_table(
        frame,
        table_area,
        table,
        &layout,
        TableRows {
            selected: list.selected,
            count,
            viewport: Some(list.viewport),
        },
        hits,
    );
    if let Some(chart_area) = chart_area {
        draw_chart_panel(frame, chart_area, app, chart.target, chart.chart, hits);
    }
    split.push_divider(app, area);
}

#[cfg(test)]
mod positions_tests {
    use super::positions_summary;
    use crate::tui::data::Position;

    fn position(token: u32, quantity: i64, pnl: f64, realised: f64, last_price: f64) -> Position {
        {
            let mut row = crate::tui::data::test_models::position();
            row.tradingsymbol = "TEST".into();
            row.exchange = "NSE".into();
            row.instrument_token = token;
            row.product = "MIS".into();
            row.overnight_quantity = 0;
            row.day_buy_quantity = 0;
            row.day_sell_quantity = 0;
            row.quantity = quantity;
            row.average_price = 0.0;
            row.last_price = last_price;
            row.close_price = 0.0;
            row.pnl = pnl;
            row.realised = realised;
            row.multiplier = 1.0;
            row
        }
    }

    #[test]
    fn positions_summary_splits_realized_and_unrealized() {
        let open = position(1, 10, 100.0, 0.0, 50.0);
        let squared = position(2, 0, 200.0, 200.0, 0.0);
        let summary = positions_summary(&[open, squared], |token| match token {
            1 => Some(55.0),
            _ => None,
        });
        assert_eq!(summary.total.to_bits(), 350.0_f64.to_bits());
        assert_eq!(summary.realized.to_bits(), 200.0_f64.to_bits());
        assert_eq!(summary.unrealized.to_bits(), 150.0_f64.to_bits());
        assert_eq!(summary.open, 1);
        assert_eq!(summary.total_count, 2);
    }
}
