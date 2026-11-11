use super::book_state::{BookState, book_state, loaded};
use super::headers::{holdings_header_sort, holdings_sort_targets};
use super::inline_chart::{chart_split, draw_chart_panel};
use super::table::{
    TableRows, record_header_hits, right, right_styled, selectable_table, sorted_title,
    symbol_cell, symbol_cells, totals_strip,
};
use crate::tui::app::{App, HoldingsSort};
use crate::tui::data::BookTotals;

use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::pane::{BookList, InlineChartInput, PaneHits};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
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

const HOLDINGS_AVG: usize = 2;
const HOLDINGS_LTP: usize = 3;

fn holding_ltp(app: &App, holding: &crate::tui::data::Holding) -> f64 {
    app.live_ltp(holding.instrument_token)
        .unwrap_or(holding.last_price)
}

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
    let mut cols = columns::sort_floors(
        &HOLDINGS_COLS,
        &HOLDINGS_LABELS,
        &targets.map(|(idx, _)| idx),
    );
    columns::floor_to_cells(
        &mut cols,
        HOLDINGS_AVG,
        holdings
            .iter()
            .map(|holding| format::price(holding.average_price).len()),
    );
    columns::floor_to_cells(
        &mut cols,
        HOLDINGS_LTP,
        holdings
            .iter()
            .map(|holding| format::price(holding_ltp(app, holding)).len()),
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
    let (rows, totals) = holdings_rows(app, holdings, list.display, &fit, sym_w);
    let (header, marked) = columns::fit_header_sorted(
        &fit,
        &layout,
        &HOLDINGS_LABELS,
        holdings_header_sort(list.sort),
    );
    let panel_title = list.title(&sorted_title("Holdings", list.sort.label(), marked));
    let strip = totals_strip(content_area.width, &panel_title, &totals);
    let mut block = ui::panel(panel_title);
    if let Some(strip) = strip {
        block = block.title(strip);
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
) -> (Vec<ratatui::widgets::Row<'static>>, BookTotals) {
    let mut totals = BookTotals::default();
    let mut rows = Vec::with_capacity(display.len());
    for &i in display {
        let holding = &holdings[i];
        let qty = crate::tui::data::holding_total_quantity(holding);
        let ltp = holding_ltp(app, holding);
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
            #[expect(
                clippy::cast_precision_loss,
                reason = "holding totals are display-only f64 arithmetic"
            )]
            let qty = qty as f64;
            totals.invested += holding.average_price * qty;
            totals.current += value;
            totals.pnl += pnl;
            totals.day_pnl += crate::tui::data::holding_live_day_pnl(holding, Some(ltp));
            totals.prev_value += holding.close_price * qty;
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
    (rows, totals)
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

    fn app_with(holdings: Vec<crate::tui::data::Holding>) -> crate::tui::app::App {
        let client = kiteconnect::KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let mut app = crate::tui::app::App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            crate::tui::notes::Pad::default(),
            false,
            &crate::tui::settings::Settings::default(),
        );
        app.holdings = Some(holdings);
        app
    }

    fn holding(
        symbol: &str,
        token: u32,
        quantity: i64,
        [average, last, close]: [f64; 3],
    ) -> crate::tui::data::Holding {
        let mut row = crate::tui::data::test_models::holding();
        row.tradingsymbol = symbol.into();
        row.exchange = "NSE".into();
        row.instrument_token = token;
        row.quantity = quantity;
        row.t1_quantity = 0;
        row.collateral_quantity = 0;
        row.mtf = None;
        row.average_price = average;
        row.last_price = last;
        row.close_price = close;
        row.day_change = Some(last - close);
        row.discrepancy = false;
        row
    }

    fn title_at(app: &crate::tui::app::App, width: u16) -> String {
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, 6)).unwrap();
        terminal
            .draw(|frame| super::super::draw_holdings(frame, frame.area(), app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        (0..width).map(|x| buffer[(x, 0)].symbol()).collect()
    }

    const FULL: &str = "inv ₹2,000 • cur ₹2,100 • +100.00 (+5.00%) • day +50.00 (+2.44%)";
    const NO_INVESTED: &str = "cur ₹2,100 • +100.00 (+5.00%) • day +50.00 (+2.44%)";
    const PNL_ONLY: &str = "+100.00 (+5.00%) • day +50.00 (+2.44%)";

    fn summary_app() -> crate::tui::app::App {
        // Invested 10 x 100 + 5 x 200 = 2,000; current 10 x 120 + 5 x 180 = 2,100.
        // Day 10 x 10 - 5 x 10 = 50 on a previous close of 10 x 110 + 5 x 190 = 2,050.
        app_with(vec![
            holding("GAINER", 1, 10, [100.0, 120.0, 110.0]),
            holding("LOSER", 2, 5, [200.0, 180.0, 190.0]),
        ])
    }

    #[test]
    fn the_holdings_title_summarises_invested_current_pnl_and_day_change() {
        let title = title_at(&summary_app(), 160);
        assert!(title.contains(FULL), "{title:?}");
    }

    #[test]
    fn a_narrow_holdings_title_drops_invested_then_current_value() {
        let app = summary_app();
        let mut seen = [false; 3];
        for width in 20..=160u16 {
            let title = title_at(&app, width);
            let variant = if title.contains(FULL) {
                0
            } else if title.contains(NO_INVESTED) {
                1
            } else if title.contains(PNL_ONLY) {
                2
            } else {
                assert!(!title.contains("day"), "width {width}: {title:?}");
                continue;
            };
            assert!(
                seen[..variant].iter().all(|richer| !richer),
                "width {width} shows less than a narrower width: {title:?}"
            );
            seen[variant] = true;
        }
        assert_eq!(seen, [true; 3]);
    }

    #[test]
    fn a_price_above_ten_lakh_is_drawn_whole() {
        let app = app_with(vec![holding(
            "MRF",
            1,
            1,
            [987_654.32, 1_234_567.89, 1_200_000.0],
        )]);
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 6)).unwrap();
        terminal
            .draw(|frame| super::super::draw_holdings(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let row: String = (0..120).map(|x| buffer[(x, 2)].symbol()).collect();
        assert!(row.contains("9,87,654.32"), "{row:?}");
        assert!(row.contains("12,34,567.89"), "{row:?}");
    }
}
