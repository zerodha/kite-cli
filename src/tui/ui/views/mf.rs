use super::book_state::{BookState, book_state, state_panel};
use super::headers::{mf_header_sort, mf_sort_targets};
use super::table::{
    TableRows, record_header_hits, right, right_styled, selectable_table, sorted_title, titles_fit,
};
use crate::tui::app::{App, MfSort};
use crate::tui::data::{fund_display_name, mf_totals};

use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col, fit_constraints, fit_row, table_budget};
use crate::tui::ui::pane::{BookList, PaneHits};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table};

fn pct_or_dash(pct: Option<f64>) -> (String, Style) {
    match pct {
        Some(v) => (format::percent(v), format::change_style(v)),
        None => (".".into(), Style::default()),
    }
}

const MF_NAME_MIN: u16 = 14;
const MF_NAME_MAX: u16 = 48;

pub(super) const MF_COLS: [FitCol; 9] = [
    col(MF_NAME_MIN, 0), // FUND
    col(12, 6),          // QTY
    col(10, 8),          // AVG
    col(10, 5),          // NAV
    col(13, 4),          // INVESTED
    col(13, 2),          // CUR.VAL
    col(12, 1),          // P&L
    col(9, 3),           // RET%
    col(9, 7),           // DAY%
];

pub(super) const MF_LABELS: [(&str, bool); 9] = [
    ("FUND", false),
    ("QTY", true),
    ("AVG", true),
    ("NAV", true),
    ("INVESTED", true),
    ("CUR.VAL", true),
    ("P&L", true),
    ("RET%", true),
    ("DAY%", true),
];

fn mf_row(h: &crate::tui::data::MfHolding, fit: &columns::ColumnFit, name_w: u16) -> Row<'static> {
    fit_row(
        fit,
        &[
            &|| {
                Cell::from(format::pad_or_clip(
                    fund_display_name(&h.tradingsymbol),
                    name_w as usize,
                ))
            },
            &|| right(format!("{:.3}", h.quantity)),
            &|| right(format!("{:.3}", h.average_price)),
            &|| right(format!("{:.3}", h.last_price)),
            &|| right(format::price(crate::tui::data::mf_invested(h))),
            &|| right(format::price(crate::tui::data::mf_current(h))),
            &|| right_styled(format::signed_price(h.pnl), format::change_style(h.pnl)),
            &|| {
                let (text, style) = pct_or_dash(crate::tui::data::mf_return_pct(h));
                right_styled(text, style)
            },
            &|| {
                let (text, style) = pct_or_dash(crate::tui::data::mf_day_change_pct(h));
                right_styled(text, style)
            },
        ],
    )
}

pub fn draw_mf_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    list: &BookList<'_, MfSort>,
    hits: &PaneHits<'_>,
) {
    let Some(mf) = &app.mf_holdings else {
        book_state(frame, area, "MF", BookState::Unknown);
        return;
    };
    if mf.is_empty() {
        state_panel(
            frame,
            area,
            " MF ",
            ui::MessageTone::Normal,
            "No mutual fund holdings",
            None,
            Some("Invest in direct MFs at coin.zerodha.com"),
        );
        return;
    }

    let targets = mf_sort_targets();
    let cols = columns::sort_floors(&MF_COLS, &MF_LABELS, &targets.map(|(idx, _)| idx));
    let fit = columns::fit_columns(table_budget(area.width), 1, &cols);
    let layout = columns::spread(
        &fit,
        &cols,
        &[columns::text(
            0,
            columns::widest(
                mf.iter()
                    .map(|h| format::display_width(fund_display_name(&h.tradingsymbol))),
            ),
            MF_NAME_MAX,
        )],
    );
    let name_w = layout.width(0);
    let display = list.display;
    let rows: Vec<Row> = display
        .iter()
        .map(|&i| mf_row(&mf[i], &fit, name_w))
        .collect();

    let totals = mf_totals(mf);
    let (pnl_pct_text, _) = pct_or_dash(totals.pnl_pct());
    let (day_pct_text, _) = pct_or_dash(totals.day_pct());
    let title_style = format::change_style(totals.pnl).add_modifier(Modifier::BOLD);
    let (header, marked) =
        columns::fit_header_sorted(&fit, &layout, &MF_LABELS, mf_header_sort(list.sort));
    let panel_title = list.title(&sorted_title("MF", list.sort.label(), marked));
    let totals_strip = vec![
        Span::styled(
            format!(
                " inv {} • cur {} • ",
                format::inr(totals.invested),
                format::inr(totals.current),
            ),
            format::dim(),
        ),
        Span::styled(
            format!("{} ({}) ", format::signed_price(totals.pnl), pnl_pct_text),
            title_style,
        ),
        Span::styled("• day ", format::dim()),
        Span::styled(
            format!(
                "{} ({}) ",
                format::signed_price(totals.day_pnl),
                day_pct_text
            ),
            format::change_style(totals.day_pnl).add_modifier(Modifier::BOLD),
        ),
    ];
    let totals_fit = titles_fit(area.width, &panel_title, &totals_strip);
    let mut block = ui::panel(panel_title);
    if totals_fit {
        block = block.title(Line::from(totals_strip).right_aligned());
    }
    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(header)
        .column_spacing(layout.spacing)
        .block(block);

    record_header_hits(hits, area, &fit, &layout, &cols, &targets);
    selectable_table(
        frame,
        area,
        table,
        &layout,
        TableRows {
            selected: list.selected,
            count: mf.len(),
            viewport: Some(list.viewport),
        },
        hits,
    );
}
