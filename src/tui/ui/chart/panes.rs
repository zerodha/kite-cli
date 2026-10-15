use super::{
    CANDLE_STRIDE, CROSSHAIR, Candle, ChartInterval, Color, DOJI_GLYPH, Frame, IndicatorLine, Line,
    Modifier, Oscillator, Paragraph, PlacedLine, Rect, Span, Style, VBLOCKS, format, line_at,
    price_to_row,
};

#[expect(
    clippy::too_many_arguments,
    reason = "the price pane needs the resolved geometry and series"
)]
pub(super) fn draw_price_pane(
    frame: &mut Frame,
    area: Rect,
    visible: &[Candle],
    hovered: Option<usize>,
    min: f64,
    max: f64,
    pad: usize,
    axis_cols: usize,
    ltp: Option<f64>,
    selected_close: Option<f64>,
    overlays: &[IndicatorLine<'_>],
) {
    let rows = area.height as usize;
    if rows == 0 {
        return;
    }
    let axis = axis_labels(min, max, rows, axis_cols);
    let placed: Vec<PlacedLine> = overlays
        .iter()
        .map(|line| line.place(min, max, rows))
        .collect();
    let ltp_row = ltp
        .filter(|p| p.is_finite())
        .map(|p| price_to_row(p, min, max, rows));

    let tag = selected_close
        .filter(|close| close.is_finite() && *close >= min && *close <= max)
        .map(|close| {
            (
                price_to_row(close, min, max, rows),
                format!("C {}", format::price(close)),
            )
        });

    let ltp_dot = || Span::styled("·", Style::default().fg(format::accent()));

    let mut lines = Vec::with_capacity(rows);
    for (r, axis_label) in axis.into_iter().enumerate() {
        let mut spans: Vec<Span> = Vec::with_capacity(pad + visible.len() * CANDLE_STRIDE + 1);
        if pad > 0 {
            if ltp_row == Some(r) {
                spans.push(Span::styled(
                    "·".repeat(pad),
                    Style::default().fg(format::accent()),
                ));
            } else {
                spans.push(Span::raw(" ".repeat(pad)));
            }
        }
        for (col, candle) in visible.iter().enumerate() {
            let overlay = line_at(&placed, col, r);
            let overlay_span =
                |(ch, color): (&'static str, Color)| Span::styled(ch, Style::default().fg(color));
            if ltp_row == Some(r) {
                spans.push(ltp_dot());
            } else if let Some(overlay) = overlay {
                spans.push(overlay_span(overlay));
            } else {
                spans.push(Span::raw(" "));
            }
            let (ch, style) = candle_cell(candle, r, min, max, rows);
            if ch != " " {
                let style = if hovered == Some(col) {
                    style.add_modifier(Modifier::BOLD)
                } else {
                    style
                };
                spans.push(Span::styled(ch, style));
            } else if ltp_row == Some(r) {
                spans.push(ltp_dot());
            } else if let Some(overlay) = overlay {
                spans.push(overlay_span(overlay));
            } else if hovered == Some(col) {
                spans.push(Span::styled(CROSSHAIR, format::dim()));
            } else {
                spans.push(Span::raw(" "));
            }
        }
        let axis_span = if ltp_row == Some(r) {
            let text = ltp
                .and_then(|price| fit_or_omit(&format::price(price), axis_cols))
                .unwrap_or_else(|| " ".repeat(axis_cols));
            Span::styled(text, format::title_style())
        } else if let Some(text) = tag
            .as_ref()
            .filter(|(row, _)| *row == r && axis_label.trim().is_empty())
            .and_then(|(_, text)| fit_or_omit(text, axis_cols))
        {
            Span::styled(
                text,
                Style::default()
                    .fg(format::info())
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(axis_label, format::dim())
        };
        spans.push(axis_span);
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn draw_oscillator_pane(
    frame: &mut Frame,
    area: Rect,
    pane: &Oscillator<'_>,
    visible_n: usize,
    hovered: Option<usize>,
    pad: usize,
    axis_cols: usize,
) {
    let rows = area.height as usize;
    if rows == 0 {
        return;
    }
    let placed: Vec<PlacedLine> = pane
        .lines
        .iter()
        .map(|line| line.place(pane.min, pane.max, rows))
        .collect();

    let mut lines = Vec::with_capacity(rows);
    for r in 0..rows {
        let mut spans: Vec<Span> = Vec::with_capacity(pad + visible_n * CANDLE_STRIDE + 1);
        if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        for col in 0..visible_n {
            match line_at(&placed, col, r) {
                Some((ch, color)) => {
                    let style = if hovered == Some(col) {
                        Style::default().fg(color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(color)
                    };
                    for _ in 0..CANDLE_STRIDE {
                        spans.push(Span::styled(ch, style));
                    }
                }
                None if hovered == Some(col) => {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(CROSSHAIR, format::dim()));
                }
                None => spans.push(Span::raw("  ")),
            }
        }
        if axis_cols > 0 {
            let label = if r == 0 {
                Some(pane.top_label.as_str())
            } else if r + 1 == rows {
                Some(pane.bottom_label.as_str())
            } else {
                None
            };
            let text = label
                .and_then(|label| fit_or_omit(label, axis_cols))
                .unwrap_or_else(|| " ".repeat(axis_cols));
            spans.push(Span::styled(text, format::dim()));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn draw_volume_pane(
    frame: &mut Frame,
    area: Rect,
    visible: &[Candle],
    hovered: Option<usize>,
    pad: usize,
    axis_cols: usize,
) {
    let rows = area.height as usize;
    if rows == 0 {
        return;
    }
    let max_vol = visible.iter().map(|c| c.volume).max().unwrap_or(0);

    let mut lines = Vec::with_capacity(rows);
    for r in 0..rows {
        let from_bottom = rows - 1 - r;
        let mut spans: Vec<Span> = Vec::with_capacity(pad + visible.len() * CANDLE_STRIDE + 1);
        if pad > 0 {
            spans.push(Span::raw(" ".repeat(pad)));
        }
        for (col, candle) in visible.iter().enumerate() {
            spans.push(Span::raw(" "));
            let ch = vbar_cell(candle.volume, max_vol, from_bottom, rows);
            let color = candle_color(candle);
            let style = if hovered == Some(col) {
                Style::default().fg(color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(color)
            };
            spans.push(Span::styled(ch, style));
        }
        if axis_cols > 0 {
            spans.push(Span::raw(" ".repeat(axis_cols)));
        }
        lines.push(Line::from(spans));
    }
    frame.render_widget(Paragraph::new(lines), area);
}

pub(super) fn candle_color(candle: &Candle) -> Color {
    if candle.close >= candle.open {
        format::up()
    } else {
        format::down()
    }
}

pub(super) fn candle_cell(
    candle: &Candle,
    r: usize,
    min: f64,
    max: f64,
    rows: usize,
) -> (&'static str, Style) {
    let high_row = price_to_row(candle.high, min, max, rows);
    let low_row = price_to_row(candle.low, min, max, rows);
    let open_row = price_to_row(candle.open, min, max, rows);
    let close_row = price_to_row(candle.close, min, max, rows);
    let body_top = open_row.min(close_row);
    let body_bot = open_row.max(close_row);

    let ch = if r >= body_top && r <= body_bot {
        #[expect(
            clippy::float_cmp,
            reason = "doji glyph requires exactly equal open and close"
        )]
        if candle.open == candle.close {
            DOJI_GLYPH
        } else {
            "█"
        }
    } else if r >= high_row && r <= low_row {
        "│"
    } else {
        " "
    };
    (ch, Style::default().fg(candle_color(candle)))
}

pub(super) fn vbar_cell(
    volume: u64,
    max_vol: u64,
    from_bottom: usize,
    rows: usize,
) -> &'static str {
    if max_vol == 0 || volume == 0 {
        return " ";
    }
    let total = rows * 8;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "volume ratio rounds to clamped eighth-cell glyphs"
    )]
    let filled =
        (((volume as f64 / max_vol as f64) * total as f64).round() as usize).clamp(1, total);
    let eighths = filled.saturating_sub(from_bottom * 8).min(8);
    if eighths == 0 {
        " "
    } else {
        VBLOCKS[eighths - 1]
    }
}

pub(super) fn axis_labels(min: f64, max: f64, rows: usize, axis_cols: usize) -> Vec<String> {
    let blank = " ".repeat(axis_cols);
    let mut labels = vec![blank; rows];
    if axis_cols == 0 || rows == 0 || !min.is_finite() || !max.is_finite() {
        return labels;
    }
    let count = 4.min(rows);
    #[expect(
        clippy::cast_precision_loss,
        reason = "axis row count comes from u16 terminal height"
    )]
    let denom = (rows.max(2) - 1) as f64;
    for i in 0..count {
        let r = if count <= 1 {
            0
        } else {
            i * (rows - 1) / (count - 1)
        };
        #[expect(
            clippy::cast_precision_loss,
            reason = "axis row index comes from u16 terminal height"
        )]
        let from_bottom = (rows - 1 - r) as f64;
        let value = min + (from_bottom / denom) * (max - min);
        if let Some(label) = fit_or_omit(&format::price(value), axis_cols) {
            labels[r] = label;
        }
    }
    labels
}

pub(super) fn fit_or_omit(text: &str, width: usize) -> Option<String> {
    (text.chars().count() <= width).then(|| format!("{text:>width$}"))
}

const MODE_W: usize = 8;

pub(super) fn readout_line(
    candle: &Candle,
    interval: ChartInterval,
    mode: &str,
    width: usize,
) -> Line<'static> {
    let when = candle
        .date
        .format(if interval == ChartInterval::Day {
            "%Y-%m-%d"
        } else {
            "%Y-%m-%d %H:%M"
        })
        .to_string();

    let mode_span = Span::styled(format!("{mode:<MODE_W$}"), format::title_style());
    let mut spans = vec![mode_span];
    let mut left = width.saturating_sub(MODE_W);

    let close = format::price(candle.close);
    let open = format::price(candle.open);
    let high = format::price(candle.high);
    let low = format::price(candle.low);
    let volume = format::compact_qty(candle.volume);
    let cost = |separator: &str, value: &str| separator.chars().count() + value.chars().count();
    if !spend(&mut left, cost(" C ", &close)) {
        return Line::from(spans);
    }
    let show_time = spend(&mut left, when.chars().count());
    let show_open = show_time && spend(&mut left, cost("  O ", &open));
    let show_high = show_open && spend(&mut left, cost(" H ", &high));
    let show_low = show_high && spend(&mut left, cost(" L ", &low));
    let show_volume = show_low && spend(&mut left, cost("  V ", &volume));

    if show_time {
        spans.push(Span::styled(
            when,
            Style::default().fg(format::text().unwrap_or(Color::Gray)),
        ));
    }
    if show_open {
        spans.push(Span::styled("  O ", format::dim()));
        spans.push(Span::raw(open));
    }
    if show_high {
        spans.push(Span::styled(" H ", format::dim()));
        spans.push(Span::raw(high));
    }
    if show_low {
        spans.push(Span::styled(" L ", format::dim()));
        spans.push(Span::raw(low));
    }
    spans.push(Span::styled(" C ", format::dim()));
    spans.push(Span::styled(
        close,
        Style::default().fg(candle_color(candle)),
    ));
    if show_volume {
        spans.push(Span::styled("  V ", format::dim()));
        spans.push(Span::raw(volume));
    }
    Line::from(spans)
}

pub(super) fn spend(left: &mut usize, cost: usize) -> bool {
    let affordable = *left >= cost;
    if affordable {
        *left -= cost;
    }
    affordable
}

#[cfg(test)]
mod ltp_and_readout_tests {
    use super::super::test_support::candle;
    use super::super::{ChartRequest, HoverAnchor, TICK_GLYPH};
    use super::{DOJI_GLYPH, fit_or_omit, readout_line};
    use crate::tui::data::{Candle, ChartInterval};
    use crate::tui::format;
    use crate::tui::indicators;
    use crate::tui::indicators::ChartIndicator;
    use ratatui::style::Modifier;

    fn render(
        w: u16,
        h: u16,
        candles: &[Candle],
        cursor: Option<usize>,
        ltp: Option<f64>,
    ) -> ratatui::buffer::Buffer {
        render_with(w, h, candles, cursor, ltp, ChartIndicator::None)
    }

    fn render_with(
        w: u16,
        h: u16,
        candles: &[Candle],
        cursor: Option<usize>,
        ltp: Option<f64>,
        indicator: ChartIndicator,
    ) -> ratatui::buffer::Buffer {
        render_full(w, h, candles, cursor, None, ltp, indicator).0
    }

    fn render_full(
        w: u16,
        h: u16,
        candles: &[Candle],
        cursor: Option<usize>,
        hover: Option<HoverAnchor<'_>>,
        ltp: Option<f64>,
        indicator: ChartIndicator,
    ) -> (ratatui::buffer::Buffer, Option<super::super::ChartHit>) {
        super::super::test_support::render(
            w,
            h,
            ratatui::layout::Rect::new(0, 0, w, h),
            ChartRequest {
                token: 111,
                candles: Some(candles),
                cursor,
                hover,
                interval: ChartInterval::M5,
                loading: false,
                ltp,
                show_volume: true,
                indicator,
                series: &indicators::series(indicator, candles),
            },
        )
    }

    #[test]
    fn ltp_line_never_reduces_solid_candle_glyph_count() {
        let candles: Vec<Candle> = (0..10)
            .map(|_| candle("2026-08-20 09:15:00", 100.0, 200.0, 100.0, 200.0, 100))
            .collect();
        let count_glyphs = |buffer: &ratatui::buffer::Buffer| {
            buffer
                .content()
                .iter()
                .filter(|cell| cell.symbol() == "█")
                .count()
        };
        let without_ltp = render(60, 20, &candles, None, None);
        let with_ltp = render(60, 20, &candles, None, Some(150.0));
        assert_eq!(
            count_glyphs(&with_ltp),
            count_glyphs(&without_ltp),
            "an LTP line must never replace a candle glyph cell"
        );
    }

    #[test]
    fn ltp_line_stays_visible_through_spacer_columns_when_candles_are_solid() {
        let candles: Vec<Candle> = (0..10)
            .map(|_| candle("2026-08-20 09:15:00", 100.0, 200.0, 100.0, 200.0, 100))
            .collect();
        let buffer = render(60, 20, &candles, None, Some(150.0));
        let has_dot = (1..20).any(|y| (0..60).any(|x| buffer[(x, y)].symbol() == "·"));
        assert!(
            has_dot,
            "LTP line must still render through spacer/padding columns even when every candle is solid"
        );
    }

    #[test]
    fn ltp_line_draws_only_through_blank_cells() {
        let candles = vec![
            candle("2026-08-20 09:15:00", 100.0, 105.0, 95.0, 100.0, 100),
            candle("2026-08-20 09:15:00", 300.0, 305.0, 295.0, 300.0, 100),
        ];
        let buffer = render(40, 20, &candles, None, Some(200.0));
        let has_dot = buffer.content().iter().any(|cell| cell.symbol() == "·");
        assert!(has_dot, "expected an LTP dot somewhere in the blank gap");
    }

    #[test]
    fn flat_candles_render_as_a_dashed_horizontal_run() {
        let candles: Vec<Candle> = (0..10)
            .map(|_| {
                let mut candle = candle("2026-08-20 09:15:00", 100.0, 100.0, 100.0, 100.0, 100);
                candle.volume = 0;
                candle
            })
            .collect();
        let buffer = render(60, 20, &candles, None, None);
        let dojis = buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == DOJI_GLYPH)
            .count();
        let blocks = buffer
            .content()
            .iter()
            .filter(|cell| cell.symbol() == "█")
            .count();
        assert_eq!(dojis, candles.len());
        assert_eq!(blocks, 0);
    }

    fn trading_days(n: usize) -> Vec<Candle> {
        let mut candles = Vec::new();
        let mut secs = 1_782_844_200;
        while candles.len() < n {
            if (1..=5).contains(&format::weekday_ist(secs)) {
                let ts = format::datetime_ist_string(secs);
                candles.push(candle(&ts, 100.0, 100.0, 100.0, 100.0, 100));
            }
            secs += 86_400;
        }
        candles
    }

    fn axis_row(buffer: &ratatui::buffer::Buffer) -> Vec<char> {
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().chars().next().unwrap_or(' '))
                    .collect::<Vec<char>>()
            })
            .find(|row| row.contains(&TICK_GLYPH))
            .expect("chart drew no x-axis ticks")
    }

    #[test]
    fn rendered_chart_labels_each_visible_session_open() {
        let candles: Vec<Candle> = (0..24)
            .map(|index| {
                let ts = if index < 12 {
                    "2026-08-19 09:15:00"
                } else {
                    "2026-08-20 09:15:00"
                };
                candle(ts, 100.0, 100.0, 100.0, 100.0, 100)
            })
            .collect();
        let buffer = render(60, 20, &candles, None, None);
        let text: String = buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        assert!(
            text.contains("Wed19"),
            "chart buffer missing first session: {text:?}"
        );
        assert!(
            text.contains("Thu20"),
            "chart buffer missing next session: {text:?}"
        );
    }

    #[test]
    fn no_two_axis_labels_touch() {
        let candles = trading_days(45);
        for width in [60, 80, 120] {
            let row = axis_row(&render(width, 20, &candles, None, None));
            let ticks: Vec<usize> = row
                .iter()
                .enumerate()
                .filter(|(_, cell)| **cell == TICK_GLYPH)
                .map(|(col, _)| col)
                .collect();
            let text: String = row.iter().collect();
            assert!(
                ticks.len() >= 2,
                "at {width} columns the axis said nothing: {text:?}"
            );
            for &col in &ticks {
                assert_ne!(row[col + 1], ' ', "unlabelled tick at {col}: {text:?}");
            }
            for &col in ticks.iter().skip(1) {
                assert_eq!(
                    (row[col - 2], row[col - 1]),
                    (' ', ' '),
                    "labels touch at {col} in {text:?}"
                );
            }
        }
    }

    #[test]
    fn no_reversed_modifier_anywhere_on_hover() {
        let candles: Vec<Candle> = (0..10)
            .map(|i| {
                candle(
                    "2026-08-20 09:15:00",
                    100.0,
                    110.0,
                    90.0,
                    if i % 2 == 0 { 105.0 } else { 95.0 },
                    100,
                )
            })
            .collect();
        let buffer = render(60, 20, &candles, Some(2), None);
        assert!(
            !buffer
                .content()
                .iter()
                .any(|cell| cell.modifier.contains(Modifier::REVERSED)),
            "no cell should use the REVERSED modifier for the hover cursor"
        );
    }

    #[test]
    fn large_indian_grouped_price_fits_the_axis() {
        let text = "2,20,452.00";
        let fitted = fit_or_omit(text, 12).expect("11 chars must fit 12 columns");
        assert_eq!(fitted.trim(), text);
        assert_eq!(fitted.chars().count(), 12);
    }

    #[test]
    fn oversized_price_is_omitted_rather_than_clipped() {
        assert_eq!(fit_or_omit("1,00,00,000.00", 12), None);
        assert_eq!(fit_or_omit("日本語テストvalue", 5), None);
        assert_eq!(
            fit_or_omit("日本語", 5).as_deref(),
            Some("  日本語"),
            "a fitting multibyte string is right-aligned by char count"
        );
    }

    #[test]
    fn readout_shows_full_datetime_for_intraday_and_date_only_for_day() {
        let c = candle("2026-07-07 09:15:00", 100.0, 110.0, 90.0, 105.0, 100);
        let intraday = readout_line(&c, ChartInterval::M5, "LIVE", 120);
        let text: String = intraday.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("2026-07-07 09:15"),
            "intraday readout missing full date+time: {text:?}"
        );

        let day = readout_line(&c, ChartInterval::Day, "LIVE", 120);
        let text: String = day.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("2026-07-07"),
            "day readout missing date: {text:?}"
        );
        assert!(
            !text.contains("09:15"),
            "day readout must not show a time: {text:?}"
        );
    }
}

#[cfg(test)]
mod tag_and_readout_tests {
    use super::super::test_support::candle;
    use super::super::{AXIS_W, ChartHit, ChartRequest, HoverAnchor};
    use super::readout_line;
    use crate::tui::data::{Candle, ChartInterval};
    use crate::tui::format;
    use crate::tui::indicators::{self, ChartIndicator};
    use ratatui::buffer::Buffer;
    use ratatui::style::Modifier;

    fn ramp() -> Vec<Candle> {
        (0..12)
            .map(|index| {
                let base = 100.0 + f64::from(index) * 10.0;
                candle(
                    &format!("2026-09-09 09:{:02}:00", 15 + index),
                    base,
                    base + 5.0,
                    base - 5.0,
                    base + 2.0,
                    12345,
                )
            })
            .collect()
    }

    fn render(
        w: u16,
        h: u16,
        candles: &[Candle],
        cursor: Option<usize>,
        hover: Option<HoverAnchor<'_>>,
        ltp: Option<f64>,
    ) -> (Buffer, ChartHit) {
        let (buffer, hit) = super::super::test_support::render(
            w,
            h,
            ratatui::layout::Rect::new(0, 0, w, h),
            ChartRequest {
                token: 1,
                candles: Some(candles),
                cursor,
                hover,
                interval: ChartInterval::M5,
                loading: false,
                ltp,
                show_volume: false,
                indicator: ChartIndicator::None,
                series: &indicators::series(ChartIndicator::None, candles),
            },
        );
        (buffer, hit.expect("these sizes always draw a plot"))
    }

    fn axis_rows(buffer: &Buffer, hit: &ChartHit) -> Vec<String> {
        let start = hit.plot.x + hit.plot.width;
        (hit.plot.y..hit.plot.y + hit.plot.height)
            .map(|y| {
                (start..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    fn axis_row(rows: &[String], text: &str) -> Option<usize> {
        rows.iter().position(|row| row.trim() == text)
    }

    #[test]
    fn a_keyboard_selection_tags_its_close_on_the_price_axis() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let candles = ramp();
        let mut tagged = 0;
        for offset in 0..candles.len() {
            let (buffer, hit) = render(60, 18, &candles, Some(offset), None, None);
            let rows = axis_rows(&buffer, &hit);
            let selected = &candles[candles.len() - 1 - offset];
            let expected = format!("C {}", format::price(selected.close));
            let Some(row) = axis_row(&rows, &expected) else {
                continue;
            };
            tagged += 1;
            assert_eq!(
                row,
                super::price_to_row(
                    selected.close,
                    candles.iter().fold(f64::INFINITY, |lo, c| lo.min(c.low)),
                    candles
                        .iter()
                        .fold(f64::NEG_INFINITY, |hi, c| hi.max(c.high)),
                    rows.len(),
                ),
                "the tag must sit on the close's own price row"
            );
            let style = buffer[(
                buffer.area.width - 1,
                hit.plot.y + u16::try_from(row).unwrap(),
            )]
                .style();
            assert!(style.add_modifier.contains(Modifier::BOLD));
            assert_eq!(style.fg, Some(format::info()));
        }
        assert!(tagged >= 4, "only {tagged} of twelve closes were tagged");
    }

    #[test]
    fn a_pointer_inspection_tags_the_close_under_the_pointer() {
        let candles = ramp();
        let mut tagged = 0;
        for index in 0..candles.len() {
            let (buffer, hit) = render(
                60,
                18,
                &candles,
                None,
                Some(HoverAnchor {
                    selected_ts: &crate::tui::data::candle_timestamp(&candles[index]),
                    window_ts: &crate::tui::data::candle_timestamp(&candles[0]),
                    visible_n: candles.len(),
                }),
                None,
            );
            let expected = format!("C {}", format::price(candles[index].close));
            if axis_row(&axis_rows(&buffer, &hit), &expected).is_some() {
                tagged += 1;
            }
        }
        assert!(tagged >= 4, "only {tagged} hovered closes were tagged");
    }

    #[test]
    fn a_live_chart_tags_nothing() {
        let candles = ramp();
        let (buffer, hit) = render(60, 18, &candles, None, None, None);
        for row in axis_rows(&buffer, &hit) {
            assert!(
                !row.contains("C "),
                "an unscanned chart must not tag a close: {row:?}"
            );
        }
    }

    #[test]
    fn the_tag_never_displaces_the_live_price() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let candles = ramp();
        for offset in 0..candles.len() {
            let selected = &candles[candles.len() - 1 - offset];
            let (buffer, hit) = render(60, 18, &candles, Some(offset), None, Some(selected.close));
            let rows = axis_rows(&buffer, &hit);
            let expected = format!("C {}", format::price(selected.close));
            assert_eq!(
                axis_row(&rows, &expected),
                None,
                "the LTP keeps its row at offset {offset}"
            );
            let ltp_row = axis_row(&rows, &format::price(selected.close))
                .expect("the LTP renders its own tag");
            assert_eq!(
                buffer[(
                    buffer.area.width - 1,
                    hit.plot.y + u16::try_from(ltp_row).unwrap()
                )]
                    .style()
                    .fg,
                Some(format::accent())
            );
        }
    }

    #[test]
    fn the_tag_never_displaces_an_ordinary_tick() {
        let candles = ramp();
        let (plain, plain_hit) = render(60, 18, &candles, None, None, None);
        let plain_rows = axis_rows(&plain, &plain_hit);
        for offset in 0..candles.len() {
            let (buffer, hit) = render(60, 18, &candles, Some(offset), None, None);
            let rows = axis_rows(&buffer, &hit);
            for (index, (row, before)) in rows.iter().zip(&plain_rows).enumerate() {
                if row.trim().starts_with("C ") {
                    assert!(
                        before.trim().is_empty(),
                        "offset {offset} tagged row {index} over the tick {before:?}"
                    );
                } else {
                    assert_eq!(
                        row, before,
                        "offset {offset} disturbed the tick on row {index}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_non_finite_close_is_never_tagged() {
        let mut candles = ramp();
        candles[4].close = f64::NAN;
        let (buffer, hit) = render(
            60,
            18,
            &candles,
            None,
            Some(HoverAnchor {
                selected_ts: &crate::tui::data::candle_timestamp(&candles[4]),
                window_ts: &crate::tui::data::candle_timestamp(&candles[0]),
                visible_n: candles.len(),
            }),
            None,
        );
        for row in axis_rows(&buffer, &hit) {
            assert!(
                !row.contains("C "),
                "a non-finite close yields no price tag: {row:?}"
            );
        }
    }

    #[test]
    fn an_out_of_range_close_is_never_tagged() {
        let mut candles = ramp();
        candles[4].close = 10_000.0;
        candles[4].high = candles[4].open;
        let (buffer, hit) = render(
            60,
            18,
            &candles,
            None,
            Some(HoverAnchor {
                selected_ts: &crate::tui::data::candle_timestamp(&candles[4]),
                window_ts: &crate::tui::data::candle_timestamp(&candles[0]),
                visible_n: candles.len(),
            }),
            None,
        );
        for row in axis_rows(&buffer, &hit) {
            assert!(
                !row.contains("C "),
                "an out-of-range close yields no price tag: {row:?}"
            );
        }
    }

    #[test]
    fn an_unpayable_axis_price_is_blank_rather_than_clipped() {
        let big = 50_000_000.0;
        let candles: Vec<Candle> = (0..12)
            .map(|index| {
                candle(
                    &format!("2026-09-09 09:{:02}:00", 15 + index),
                    big,
                    big + 1.0,
                    big - 1.0,
                    big,
                    12345,
                )
            })
            .collect();
        let (buffer, hit) = render(60, 18, &candles, Some(2), None, Some(big));
        for row in axis_rows(&buffer, &hit) {
            assert!(
                row.trim().is_empty(),
                "a 14-char price must be omitted, not clipped: {row:?}"
            );
            assert_eq!(row.chars().count(), AXIS_W);
        }
    }

    fn readout_text(candle: &Candle, width: usize) -> String {
        readout_line(candle, ChartInterval::M5, "3 ago", width)
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn a_wide_readout_keeps_every_field_in_the_familiar_order() {
        let c = candle("2026-09-09 09:15:00", 100.0, 110.0, 90.0, 105.0, 12345);
        let text = readout_text(&c, 200);
        let order: Vec<usize> = ["2026-09-09 09:15", " O ", " H ", " L ", " C ", " V "]
            .iter()
            .map(|needle| {
                text.find(needle)
                    .unwrap_or_else(|| panic!("{needle:?} missing from {text:?}"))
            })
            .collect();
        assert!(
            order.windows(2).all(|pair| pair[0] < pair[1]),
            "field order changed: {text:?}"
        );
    }

    #[test]
    fn a_narrow_readout_drops_whole_fields_from_the_least_important_end() {
        let c = candle("2026-09-09 09:15:00", 100.0, 110.0, 90.0, 105.0, 12345);
        let full = readout_text(&c, 200);
        let full_w = full.trim_end().chars().count();

        let text = readout_text(&c, full_w - 1);
        assert!(!text.contains(" V "), "{text:?}");
        assert!(text.contains(" L 90.00"), "{text:?}");
        assert!(text.contains(" C 105.00"), "{text:?}");
        assert!(text.contains("2026-09-09 09:15"), "{text:?}");

        let appears_at = |needle: &str| {
            (1..=full_w)
                .find(|width| readout_text(&c, *width).contains(needle))
                .unwrap_or(usize::MAX)
        };
        let order = ["2026-09-09 09:15", " O ", " H ", " L ", " V "];
        let widths: Vec<usize> = order.iter().map(|needle| appears_at(needle)).collect();
        assert!(
            widths.windows(2).all(|pair| pair[0] < pair[1]),
            "fields must appear most-important first: {order:?} at {widths:?}"
        );
        assert!(
            appears_at(" C 105.00") < widths[0],
            "the close must survive narrower than the timestamp does"
        );
    }

    #[test]
    fn a_readout_too_narrow_for_both_keeps_the_close_and_drops_the_time() {
        let c = candle("2026-09-09 09:15:00", 100.0, 110.0, 90.0, 105.0, 12345);
        let text = readout_text(&c, 8 + 9 + 15);
        assert_eq!(text.trim_end(), "3 ago    C 105.00");
    }

    #[test]
    fn a_readout_that_cannot_hold_a_whole_close_shows_none_of_it() {
        let c = candle("2026-09-09 09:15:00", 100.0, 110.0, 90.0, 105.0, 12345);
        let text = readout_text(&c, 8 + 8);
        assert_eq!(text.trim_end(), "3 ago");
        assert!(
            !text.contains("105"),
            "a clipped price is a different number: {text:?}"
        );
    }

    #[test]
    fn a_narrow_chart_pane_still_renders_a_whole_close() {
        let candles = ramp();
        let (buffer, _) = render(30, 18, &candles, Some(2), None, None);
        let selected = &candles[candles.len() - 1 - 2];
        let readout: String = (0..buffer.area.width)
            .map(|x| buffer[(x, 1)].symbol())
            .collect();
        assert!(
            readout.contains(&format!("C {}", format::price(selected.close))),
            "narrow readout lost its close: {readout:?}"
        );
    }
}
