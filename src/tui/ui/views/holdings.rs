use super::book_state::{BookState, book_state, loaded};
use super::headers::{holdings_header_sort, holdings_sort_targets};
use super::inline_chart::{chart_split, draw_chart_panel};
use super::table::{
    TableRows, record_header_hits, right, right_styled, selectable_table, sorted_title,
    symbol_cell, symbol_cells, titles_fit,
};
use crate::tui::app::{App, HoldingsSort};

use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::pane::{BookList, InlineChartInput, PaneHits};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Table;

fn holding_pnl_pct(average_price: f64, qty: i64, ltp: f64) -> Option<f64> {
    (average_price > 0.0 && qty != 0).then(|| (ltp - average_price) / average_price * 100.0)
}

const HOLDINGS_SYM_MIN: u16 = 10;
const HOLDINGS_SYM_MAX: u16 = 24;
const HOLDINGS_PNL_W: u16 = 12;
const HOLDINGS_VALUE_W: u16 = 13;

pub(super) const HOLDINGS_COLS: [FitCol; 8] = [
    col(HOLDINGS_SYM_MIN, 0), // SYMBOL
    col(7, 3),                // QTY
    col(11, 7),               // AVG
    col(11, 2),               // LTP
    col(HOLDINGS_VALUE_W, 5), // VALUE
    col(HOLDINGS_PNL_W, 1),   // P&L
    col(8, 4),                // P&L%
    col(8, 6),                // DAY%
];

pub(super) const HOLDINGS_LABELS: [(&str, bool); 8] = [
    ("SYMBOL", false),
    ("QTY", true),
    ("AVG", true),
    ("LTP", true),
    ("VALUE", true),
    ("P&L", true),
    ("P&L%", true),
    ("DAY%", true),
];

pub fn draw_holdings_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &BookList<'_, HoldingsSort>,
    chart: &InlineChartInput<'_>,
    hits: &PaneHits<'_>,
) {
    if let (None, Some(error)) = (&app.holdings, &app.holdings_error) {
        book_state(frame, area, "Holdings", BookState::Failed(error));
        return;
    }
    let Some(holdings) = loaded(
        frame,
        area,
        "Holdings",
        app.holdings.as_deref(),
        "No holdings",
    ) else {
        return;
    };

    let split = chart_split(area, &app.splits, chart.open);
    let (content_area, chart_area) = (split.table, split.chart);
    let targets = holdings_sort_targets();
    let cols = columns::sort_floors(
        &HOLDINGS_COLS,
        &HOLDINGS_LABELS,
        &targets.map(|(idx, _)| idx),
    );
    let fit = columns::fit_columns(table_budget(content_area.width), 1, &cols);
    let layout = columns::spread(
        &fit,
        &cols,
        &[columns::text(
            0,
            columns::widest(
                holdings
                    .iter()
                    .map(|holding| symbol_cells(&holding.exchange, &holding.tradingsymbol)),
            ),
            HOLDINGS_SYM_MAX,
        )],
    );
    let sym_w = layout.width(0);
    let (rows, total_value, total_pnl, total_day) =
        holdings_rows(app, holdings, list.display, &fit, sym_w);
    let title_style = format::change_style(total_pnl).add_modifier(Modifier::BOLD);
    let (header, marked) = columns::fit_header_sorted(
        &fit,
        &layout,
        &HOLDINGS_LABELS,
        holdings_header_sort(list.sort),
    );
    let panel_title = sorted_title("Holdings", list.sort.label(), marked);
    let totals = vec![
        Span::styled(
            format!(
                " {} • total {} • day ",
                format::inr(total_value),
                format::signed_price(total_pnl)
            ),
            title_style,
        ),
        Span::styled(
            format!("{} ", format::signed_price(total_day)),
            format::change_style(total_day).add_modifier(Modifier::BOLD),
        ),
    ];
    let mut block = ui::panel(list.title(&panel_title));
    if titles_fit(content_area.width, &panel_title, &totals) {
        block = block.title(Line::from(totals).right_aligned());
    }
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(header)
        .column_spacing(layout.spacing)
        .block(block);

    record_header_hits(hits, content_area, &fit, &layout, &cols, &targets);
    selectable_table(
        frame,
        content_area,
        table,
        &layout,
        TableRows {
            selected: list.selected,
            count: holdings.len(),
            viewport: Some(list.viewport),
        },
        hits,
    );
    if let Some(chart_area) = chart_area {
        draw_chart_panel(frame, chart_area, app, chart.target, chart.chart, hits);
    }
    split.push_divider(app, area);
}

fn holdings_rows(
    app: &App,
    holdings: &[crate::tui::data::Holding],
    display: &[usize],
    fit: &columns::ColumnFit,
    sym_w: u16,
) -> (Vec<ratatui::widgets::Row<'static>>, f64, f64, f64) {
    let mut total_value = 0.0;
    let mut total_pnl = 0.0;
    let mut total_day = 0.0;
    let mut rows = Vec::with_capacity(display.len());
    for &i in display {
        let holding = &holdings[i];
        let qty = crate::tui::data::holding_total_quantity(holding);
        let ltp = app
            .live_ltp(holding.instrument_token)
            .unwrap_or(holding.last_price);
        #[expect(
            clippy::cast_precision_loss,
            reason = "holding value is display-only f64 arithmetic"
        )]
        let value = ltp * qty as f64;
        let pnl = crate::tui::data::holding_live_pnl(holding, Some(ltp));
        let pnl_pct = holding_pnl_pct(holding.average_price, qty, ltp);
        let day_pct = if holding.close_price > 0.0 {
            Some((ltp - holding.close_price) / holding.close_price * 100.0)
        } else {
            None
        };
        let has_pnl = crate::tui::data::holding_has_pnl(holding, Some(ltp));
        if has_pnl {
            total_value += value;
            total_pnl += pnl;
            total_day += crate::tui::data::holding_live_day_pnl(holding, Some(ltp));
        }
        rows.push(fit_row(
            fit,
            &[
                &|| symbol_cell(&holding.exchange, &holding.tradingsymbol, sym_w),
                &|| right(qty.to_string()),
                &|| right(format::price(holding.average_price)),
                &|| right(format::price(ltp)),
                &|| {
                    right(format::fit_money(
                        value,
                        &format::inr(value),
                        HOLDINGS_VALUE_W as usize,
                    ))
                },
                &|| {
                    right_styled(
                        if has_pnl {
                            format::fit_money(
                                pnl,
                                &format::signed_price(pnl),
                                HOLDINGS_PNL_W as usize,
                            )
                        } else {
                            "N/A".into()
                        },
                        if has_pnl {
                            format::change_style(pnl)
                        } else {
                            Style::default()
                        },
                    )
                },
                &|| {
                    right_styled(
                        pnl_pct
                            .filter(|_| has_pnl)
                            .map_or_else(|| ".".into(), format::percent),
                        pnl_pct
                            .filter(|_| has_pnl)
                            .map(format::change_style)
                            .unwrap_or_default(),
                    )
                },
                &|| {
                    right_styled(
                        day_pct.map_or_else(|| ".".into(), format::percent),
                        day_pct.map(format::change_style).unwrap_or_default(),
                    )
                },
            ],
        ));
    }
    (rows, total_value, total_pnl, total_day)
}

#[cfg(test)]
mod holdings_tests {
    use super::holding_pnl_pct;

    #[test]
    fn holding_pnl_pct_none_when_qty_is_zero() {
        assert_eq!(holding_pnl_pct(100.0, 0, 150.0), None);
    }

    #[test]
    fn holding_pnl_pct_none_without_cost_basis() {
        assert_eq!(holding_pnl_pct(0.0, 10, 150.0), None);
    }

    #[test]
    fn holding_pnl_pct_computes_change_independent_of_qty() {
        assert_eq!(holding_pnl_pct(100.0, 10, 150.0), Some(50.0));
        assert_eq!(holding_pnl_pct(100.0, 1_000_000, 150.0), Some(50.0));
    }
}
