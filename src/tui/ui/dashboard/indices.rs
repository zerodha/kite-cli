use crate::tui::ticks;
use std::collections::HashSet;

use crate::tui::app::App;
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol, col};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const IDX_SYM_MIN: u16 = 6;
const IDX_SYM_MAX: u16 = 12;

const IDX_COLS: [FitCol; 4] = [
    col(IDX_SYM_MIN, 0), // SYMBOL
    col(11, 1),          // LEVEL
    col(11, 3),          // CHG (2 spaces + 9)
    col(10, 2),          // CHG% (" (" + 7 + ")")
];

pub(in crate::tui::ui) fn index_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let mut seen: HashSet<u32> = HashSet::new();
    let mut indices: Vec<(&str, u32)> = Vec::new();
    for item in app.watch_items() {
        if item.is_index() && seen.insert(item.instrument_token) {
            indices.push((item.tradingsymbol.as_str(), item.instrument_token));
        }
    }
    indices.sort_by(|a, b| a.0.cmp(b.0));

    let fit = columns::fit_columns(width.saturating_sub(1), 0, &IDX_COLS);
    let symbol_w = fit.flex(IDX_SYM_MIN, IDX_SYM_MAX) as usize;

    let mut lines: Vec<Line> = Vec::new();
    for (symbol, token) in &indices {
        let Some(tick) = app.tick(*token) else {
            continue;
        };
        let Some(level) = tick.last_price else {
            continue;
        };
        let change = ticks::change(tick);
        let pct = ticks::change_percent(tick);
        let style = change.map(format::change_style).unwrap_or_default();
        let mut spans = vec![Span::raw(format!(
            " {}",
            format::pad_or_clip(symbol, symbol_w)
        ))];
        if fit.shows(1) {
            spans.push(Span::styled(
                format!("{:>11}", format::price(level)),
                format::bold(),
            ));
        }
        if let (Some(change), Some(pct)) = (change, pct) {
            if fit.shows(2) {
                let change_text = format::fit_money(change, &format::signed_price(change), 9);
                spans.push(Span::styled(format!("  {change_text:>9}"), style));
            }
            if fit.shows(3) {
                spans.push(Span::styled(
                    format!(" ({:>7})", format::percent(pct)),
                    style,
                ));
            }
        }
        lines.push(Line::from(spans));
    }

    if lines.is_empty() {
        let hint = if indices.is_empty() {
            " add an index to your watchlist"
        } else {
            " waiting for index quotes…"
        };
        lines.push(Line::from(Span::styled(hint, format::dim())));
    }
    lines
}

pub(in crate::tui::ui) fn render_index_panel(
    frame: &mut Frame,
    area: Rect,
    lines: Vec<Line<'static>>,
    focused: bool,
) {
    let block = ui::focus_panel("Indices", focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use crate::tui::ui::dashboard::fixtures::test_app;

    #[test]
    fn index_lines_drop_columns_instead_of_clipping() {
        use crate::tui::marketwatch::WatchItem;
        use kiteconnect::ticker::Tick;

        let mut app = test_app();
        app.set_watch_for_test(crate::tui::marketwatch::State::test_loaded(vec![
            WatchItem {
                exchange: "NSE".into(),
                tradingsymbol: "NIFTY BANK".into(),
                instrument_token: 260_105,
                segment: "INDICES".into(),
                ..WatchItem::default()
            },
        ]));
        app.ticks.insert(260_105, {
            let mut value = Tick::default();
            value.instrument_token = 260_105;
            value.last_price = Some(56_789.45);
            value.close = Some(56_666.40);
            value.is_index = true;
            value
        });
        for width in 10..=80u16 {
            let text: String = super::index_lines(&app, width)
                .iter()
                .flat_map(|line| line.spans.iter())
                .map(|span| span.content.as_ref().to_string())
                .collect();
            assert!(
                text.chars().count() <= width as usize,
                "width {width} overflows: {text:?}"
            );
            for (full, tail) in [("56,789.45", "789.45"), ("+123.05", "23.05")] {
                assert!(
                    text.contains(full) || !text.contains(tail),
                    "width {width}: partial {full}: {text:?}"
                );
            }
        }
    }
}
