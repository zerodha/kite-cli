mod indicator;
mod panes;
#[cfg(test)]
mod test_support;
mod time_axis;

use indicator::{
    IndicatorLine, Oscillator, PlacedLine, fold_range, line_at, oscillator, price_overlays,
};
use panes::{draw_oscillator_pane, draw_price_pane, draw_volume_pane, readout_line};
use time_axis::draw_time_axis;

use crate::tui::data::{Candle, ChartInterval};
use crate::tui::format;
use crate::tui::indicators::{self, ChartIndicator};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::ops::Range;

const AXIS_W: usize = 12;
const VOL_ROWS: u16 = 3;
const OSC_ROWS: u16 = 6;
const PRICE_MIN_ROWS: u16 = 5;
const VBLOCKS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
const CANDLE_STRIDE: usize = 2;
const DOJI_GLYPH: &str = "━";
const TICK_LABEL_W: usize = 5;
const TICK_GAP: usize = 2;
const TICK_GLYPH: char = '╷';
const LINE_GLYPH: &str = "─";
const BAND_GLYPH: &str = "╌";
const CROSSHAIR: &str = "┊";
const TITLE_HINTS: &[&str] = &[
    "c hide",
    "i interval",
    "I indicator",
    "h/l scan",
    "v volume",
];
const TITLE_HINT_SEP: &str = " · ";

fn candle_window(candles_len: usize, slots: usize, cursor: Option<usize>) -> (usize, usize) {
    let visible_n = candles_len.min(slots);
    let readout_idx = match cursor {
        Some(offset) => candles_len.saturating_sub(1).saturating_sub(offset),
        None => candles_len.saturating_sub(1),
    };
    let start = if cursor.is_none() {
        candles_len.saturating_sub(visible_n)
    } else {
        let centre_floor = visible_n / 2;
        let raw_start = readout_idx.saturating_sub(centre_floor);
        raw_start.min(candles_len.saturating_sub(visible_n))
    };
    (start, readout_idx)
}

fn price_to_row(price: f64, min: f64, max: f64, rows: usize) -> usize {
    if rows == 0 {
        return 0;
    }
    if max <= min {
        return rows / 2;
    }
    let frac = ((price - min) / (max - min)).clamp(0.0, 1.0);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "clamped price ratio rounds to a terminal row"
    )]
    let from_bottom = (frac * (rows as f64 - 1.0)).round() as usize;
    (rows - 1).saturating_sub(from_bottom)
}

fn aux_pane_rows(height: u16, show_volume: bool, show_oscillator: bool) -> (u16, u16) {
    let budget = height.saturating_sub(2);
    let vol_rows = if show_volume { VOL_ROWS.min(budget) } else { 0 };
    let price_rows = height.saturating_sub(1 + vol_rows);
    let osc_rows = if show_oscillator && price_rows >= PRICE_MIN_ROWS + OSC_ROWS {
        OSC_ROWS
    } else {
        0
    };
    (osc_rows, vol_rows)
}

fn time_axis_rows(height: u16, width: u16, osc_rows: u16, vol_rows: u16) -> u16 {
    let price_rows = height.saturating_sub(1 + osc_rows + vol_rows);
    u16::from(
        price_rows > PRICE_MIN_ROWS && width as usize >= AXIS_W.saturating_add(TICK_LABEL_W + 1),
    )
}

fn title_head(indicator: ChartIndicator, interval: ChartInterval, osc_rows: u16) -> String {
    let interval = interval.label();
    match indicator.label() {
        None => format!(" chart · {interval} "),
        Some(label) if indicator.is_oscillator() && osc_rows == 0 => {
            format!(" chart · {interval} · {label} no room ")
        }
        Some(label) => format!(" chart · {interval} · {label} "),
    }
}

fn title_text(
    indicator: ChartIndicator,
    interval: ChartInterval,
    osc_rows: u16,
    width: u16,
) -> (String, String) {
    let head = title_head(indicator, interval, osc_rows);
    let budget = width.saturating_sub(u16::try_from(head.chars().count()).unwrap_or(u16::MAX) + 4);
    let hints = super::fit_hints(budget, TITLE_HINT_SEP, TITLE_HINTS);
    let tail = if hints.is_empty() {
        String::new()
    } else {
        format!(" ({hints}) ")
    };
    (head, tail)
}

#[derive(Debug, Clone, Copy)]
pub struct HoverAnchor<'a> {
    pub selected_ts: &'a str,
    pub window_ts: &'a str,
    pub visible_n: usize,
}

pub struct ChartRequest<'a> {
    pub token: u32,
    pub candles: Option<&'a [Candle]>,
    pub cursor: Option<usize>,
    pub hover: Option<HoverAnchor<'a>>,
    pub interval: ChartInterval,
    pub loading: bool,
    pub ltp: Option<f64>,
    pub show_volume: bool,
    pub indicator: ChartIndicator,
    pub series: &'a indicators::Series,
}

#[derive(Debug, Clone)]
pub struct ChartHit {
    pub token: u32,
    pub interval: ChartInterval,
    pub outer: Rect,
    pub plot: Rect,
    pub start: usize,
    pub pad: usize,
    pub visible_ts: Vec<String>,
}

impl ChartHit {
    pub fn candle_at(&self, column: u16, row: u16) -> Option<usize> {
        if !self.plot.contains(Position { x: column, y: row }) {
            return None;
        }
        let local_x = column.checked_sub(self.plot.x)? as usize;
        let slot = local_x.checked_sub(self.pad)? / CANDLE_STRIDE;
        (slot < self.visible_ts.len()).then(|| self.start + slot)
    }
}

#[derive(Clone, Copy)]
struct ChartGeometry {
    readout: Rect,
    price: Rect,
    time_axis: Rect,
    oscillator: Rect,
    volume: Rect,
    axis_cols: usize,
    chart_w: usize,
    pad: usize,
    start: usize,
    visible_n: usize,
    readout_idx: usize,
    selected: bool,
    selected_col: Option<usize>,
}

fn index_of_ts(candles: &[Candle], ts: &str) -> Option<usize> {
    candles
        .iter()
        .rposition(|candle| crate::tui::data::candle_timestamp(candle) == ts)
}

fn chart_geometry(
    inner: Rect,
    candles: &[Candle],
    cursor: Option<usize>,
    hover: Option<HoverAnchor<'_>>,
    show_volume: bool,
    show_oscillator: bool,
) -> ChartGeometry {
    let (osc_rows, vol_rows) = aux_pane_rows(inner.height, show_volume, show_oscillator);
    let axis_rows = time_axis_rows(inner.height, inner.width, osc_rows, vol_rows);
    let [readout, price, time_axis, oscillator, volume] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
        Constraint::Length(axis_rows),
        Constraint::Length(osc_rows),
        Constraint::Length(vol_rows),
    ])
    .areas(inner);

    let width = price.width as usize;
    let chart_w = width.saturating_sub(AXIS_W);
    let axis_cols = width - chart_w;
    let slots = chart_w / CANDLE_STRIDE;

    let anchored = hover.and_then(|anchor| {
        let visible_n = anchor.visible_n.min(slots).min(candles.len());
        let selected = index_of_ts(candles, anchor.selected_ts)?;
        let start = index_of_ts(candles, anchor.window_ts)?;
        Some((start.min(candles.len() - visible_n), selected, visible_n))
    });
    let visible_n = anchored.map_or_else(|| candles.len().min(slots), |(_, _, n)| n);
    let pad = chart_w - visible_n * CANDLE_STRIDE;
    let (start, readout_idx) = match anchored {
        Some((start, selected, _)) => (start, selected),
        None => candle_window(candles.len(), slots, cursor),
    };
    let selected = anchored.is_some() || cursor.is_some();
    let selected_col = selected
        .then(|| readout_idx.checked_sub(start))
        .flatten()
        .filter(|col| *col < visible_n);

    ChartGeometry {
        readout,
        price,
        time_axis,
        oscillator,
        volume,
        axis_cols,
        chart_w,
        pad,
        start,
        visible_n,
        readout_idx,
        selected,
        selected_col,
    }
}

#[expect(
    clippy::needless_pass_by_value,
    reason = "chart callers pass a single-use render request"
)]
pub fn draw_chart(frame: &mut Frame, area: Rect, req: ChartRequest<'_>) -> Option<ChartHit> {
    let ChartRequest {
        token,
        candles,
        cursor,
        hover,
        interval,
        loading,
        show_volume,
        indicator,
        ..
    } = req;
    let inner = Block::new().borders(Borders::TOP).inner(area);
    let (osc_rows, _) = aux_pane_rows(inner.height, show_volume, indicator.is_oscillator());

    let (head, hints) = title_text(indicator, interval, osc_rows, area.width);
    let title = Line::from(vec![
        Span::styled(head, format::title_style()),
        Span::styled(hints, format::dim()),
    ]);
    frame.render_widget(
        Block::new()
            .borders(Borders::TOP)
            .border_style(format::border_style())
            .title(title),
        area,
    );

    if inner.width == 0 || inner.height == 0 {
        return None;
    }

    let candles = match candles {
        Some(candles) if !candles.is_empty() => candles,
        _ => {
            let msg = if loading {
                "loading candles…"
            } else {
                "no candle data"
            };
            frame.render_widget(Paragraph::new(Span::styled(msg, format::dim())), inner);
            return None;
        }
    };

    let geo = chart_geometry(
        inner,
        candles,
        cursor,
        hover,
        show_volume,
        indicator.is_oscillator(),
    );
    draw_chart_panes(frame, candles, &req, geo);
    let ChartGeometry {
        price: price_area,
        chart_w,
        start,
        pad,
        visible_n,
        ..
    } = geo;
    let visible = &candles[start..start + visible_n];

    let plot = Rect {
        x: price_area.x,
        y: price_area.y,
        width: u16::try_from(chart_w).unwrap_or(u16::MAX),
        height: price_area.height,
    };
    (visible_n > 0 && plot.width > 0 && plot.height > 0).then(|| ChartHit {
        token,
        interval,
        outer: area,
        plot,
        start,
        pad,
        visible_ts: visible
            .iter()
            .map(crate::tui::data::candle_timestamp)
            .collect(),
    })
}

fn draw_chart_panes(
    frame: &mut Frame,
    candles: &[Candle],
    req: &ChartRequest<'_>,
    geo: ChartGeometry,
) {
    let ChartRequest {
        interval,
        ltp,
        indicator,
        series,
        ..
    } = *req;
    let ChartGeometry {
        readout: readout_area,
        price: price_area,
        time_axis: axis_area,
        oscillator: oscillator_area,
        volume: volume_area,
        axis_cols,
        chart_w,
        pad,
        start,
        visible_n,
        readout_idx,
        selected: has_selection,
        selected_col: hovered,
    } = geo;
    let visible = &candles[start..start + visible_n];
    let selected = &candles[readout_idx];

    let mode = if has_selection {
        format!("{} ago", candles.len() - 1 - readout_idx)
    } else {
        "LIVE".to_string()
    };
    frame.render_widget(
        Paragraph::new(readout_line(
            selected,
            interval,
            &mode,
            readout_area.width as usize,
        )),
        readout_area,
    );

    let (mut min, mut max) = visible
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), candle| {
            (lo.min(candle.low), hi.max(candle.high))
        });
    if let Some(price) = ltp.filter(|p| p.is_finite()) {
        min = min.min(price);
        max = max.max(price);
    }

    let window = start..start + visible_n;
    let overlays = price_overlays(indicator, series, candles.len(), window.clone());
    for line in &overlays {
        (min, max) = fold_range((min, max), line.values);
    }

    let selected_close = hovered.map(|_| selected.close);
    draw_price_pane(
        frame,
        price_area,
        visible,
        hovered,
        min,
        max,
        pad,
        axis_cols,
        ltp,
        selected_close,
        &overlays,
    );
    if axis_area.height > 0 {
        draw_time_axis(frame, axis_area, visible, pad, chart_w, axis_cols);
    }
    if oscillator_area.height > 0
        && let Some(pane) = oscillator(indicator, series, candles.len(), window)
    {
        draw_oscillator_pane(
            frame,
            oscillator_area,
            &pane,
            visible_n,
            hovered,
            pad,
            axis_cols,
        );
    }
    if volume_area.height > 0 {
        draw_volume_pane(frame, volume_area, visible, hovered, pad, axis_cols);
    }
}

#[cfg(test)]
mod pointer_tests {
    use super::{
        AXIS_W, CANDLE_STRIDE, CROSSHAIR, ChartHit, ChartRequest, HoverAnchor, chart_geometry,
        draw_chart,
    };
    use crate::tui::data::{Candle, ChartInterval};
    use crate::tui::indicators::{self, ChartIndicator};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    const TOKEN: u32 = 738_561;

    fn series(n: usize) -> Vec<Candle> {
        (0..n)
            .map(|index| {
                let mut row = crate::tui::data::test_models::candle();
                row.date = format!(
                    "{}+05:30",
                    (format!("2026-09-09 09:{:02}:00", 15 + index)).replace(' ', "T")
                )
                .parse()
                .expect("date");
                let value = f64::from(u32::try_from(index).unwrap());
                row.open = 100.0 + value;
                row.high = 101.0 + value;
                row.low = 99.0 + value;
                row.close = 100.5 + value;
                row.volume = 1_000 + index as u64;
                row
            })
            .collect()
    }

    fn draw_into(
        w: u16,
        h: u16,
        area: Rect,
        candles: &[Candle],
        cursor: Option<usize>,
        hover: Option<HoverAnchor<'_>>,
    ) -> (Buffer, Option<ChartHit>) {
        super::test_support::render(
            w,
            h,
            area,
            ChartRequest {
                token: TOKEN,
                candles: Some(candles),
                cursor,
                hover,
                interval: ChartInterval::M5,
                loading: false,
                ltp: None,
                show_volume: true,
                indicator: ChartIndicator::None,
                series: &indicators::series(ChartIndicator::None, candles),
            },
        )
    }

    fn draw(w: u16, h: u16, candles: &[Candle]) -> (Buffer, Option<ChartHit>) {
        draw_into(w, h, Rect::new(0, 0, w, h), candles, None, None)
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    #[test]
    fn narrow_widths_publish_a_hit_only_once_a_candle_slot_exists() {
        let candles = series(30);
        for width in [0u16, 1, 12, 13] {
            let (_, hit) = draw_into(20, 20, Rect::new(0, 0, width, 20), &candles, None, None);
            assert!(hit.is_none(), "width {width} must publish no chart hit");
        }
        let (_, hit) = draw(14, 20, &series(30));
        let hit = hit.expect("width 14 has one candle slot");
        assert_eq!(hit.visible_ts.len(), 1);
        assert_eq!(hit.pad, 0);
    }

    #[test]
    fn an_odd_width_keeps_its_left_padding_and_rejects_it() {
        let candles = series(30);
        let (_, hit) = draw(15, 20, &candles);
        let hit = hit.expect("width 15 has one candle slot");
        assert_eq!(hit.pad, 1);
        assert_eq!(hit.plot.width as usize, 3);
        let row = hit.plot.y;
        assert_eq!(hit.candle_at(hit.plot.x, row), None, "padding is not a bar");
        assert_eq!(hit.candle_at(hit.plot.x + 1, row), Some(hit.start));
        assert_eq!(hit.candle_at(hit.plot.x + 2, row), Some(hit.start));
    }

    #[test]
    fn both_cells_of_a_stride_map_to_the_same_candle() {
        let candles = series(40);
        let (_, hit) = draw(60, 20, &candles);
        let hit = hit.expect("a 60-column chart has candle slots");
        assert_eq!(hit.pad, 0, "48 chart columns hold exactly 24 strides");
        let row = hit.plot.y;
        for slot in 0..u16::try_from(hit.visible_ts.len()).unwrap() {
            let spacer = hit.plot.x + slot * u16::try_from(CANDLE_STRIDE).unwrap();
            let glyph = spacer + 1;
            let expected = Some(hit.start + slot as usize);
            assert_eq!(
                hit.candle_at(spacer, row),
                expected,
                "spacer of slot {slot}"
            );
            assert_eq!(hit.candle_at(glyph, row), expected, "glyph of slot {slot}");
        }
        let past = hit.plot.x + hit.plot.width;
        assert_eq!(hit.candle_at(past, row), None);
    }

    #[test]
    fn fewer_candles_than_slots_pads_from_the_left() {
        let candles = series(5);
        let (_, hit) = draw(60, 20, &candles);
        let hit = hit.expect("five candles still draw");
        assert_eq!(hit.start, 0);
        assert_eq!(hit.visible_ts.len(), 5);
        assert_eq!(hit.pad, 48 - 5 * CANDLE_STRIDE);
        let row = hit.plot.y;
        for column in 0..u16::try_from(hit.pad).unwrap() {
            assert_eq!(hit.candle_at(hit.plot.x + column, row), None);
        }
        assert_eq!(
            hit.candle_at(hit.plot.x + u16::try_from(hit.pad).unwrap(), row),
            Some(0)
        );
    }

    #[test]
    fn the_hit_excludes_every_pane_but_the_price_plot() {
        let candles = series(40);
        let area = Rect::new(3, 2, 60, 24);
        let (_, hit) = draw_into(80, 30, area, &candles, None, None);
        let hit = hit.expect("a 60x24 chart draws");
        assert_eq!(hit.outer, area);
        assert_eq!(hit.plot.x, area.x, "the plot starts at the chart's left");
        assert_eq!(
            hit.plot.width as usize,
            area.width as usize - AXIS_W,
            "the price axis is not part of the plot"
        );
        let inside = hit.plot.x + u16::try_from(hit.pad).unwrap() + 1;
        assert_eq!(hit.candle_at(inside, area.y), None, "title row");
        assert_eq!(hit.candle_at(inside, hit.plot.y - 1), None, "readout row");
        assert_eq!(
            hit.candle_at(inside, hit.plot.y + hit.plot.height),
            None,
            "the row below the price pane"
        );
        assert_eq!(
            hit.candle_at(area.x + area.width - 1, hit.plot.y),
            None,
            "price axis gutter"
        );
    }

    #[test]
    fn empty_and_loading_charts_publish_no_hit() {
        let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
        for (candles, loading) in [(None, true), (None, false), (Some(&[][..]), false)] {
            let mut hit = None;
            terminal
                .draw(|frame| {
                    hit = draw_chart(
                        frame,
                        frame.area(),
                        ChartRequest {
                            token: TOKEN,
                            candles,
                            cursor: None,
                            hover: None,
                            interval: ChartInterval::M5,
                            loading,
                            ltp: None,
                            show_volume: true,
                            indicator: ChartIndicator::None,
                            series: &indicators::series(ChartIndicator::None, &[]),
                        },
                    );
                })
                .unwrap();
            assert!(hit.is_none(), "no data means nothing to point at");
        }
    }

    #[test]
    fn a_chart_with_no_inner_rows_publishes_no_hit() {
        let (_, hit) = draw(60, 1, &series(30));
        assert!(hit.is_none());
    }

    #[test]
    fn hover_holds_the_window_still_across_moves() {
        let candles = series(40);
        let live = chart_geometry(Rect::new(0, 0, 60, 19), &candles, None, None, true, false);
        assert_eq!((live.start, live.visible_n), (16, 24));

        let anchor = HoverAnchor {
            selected_ts: &crate::tui::data::candle_timestamp(&candles[20]),
            window_ts: &crate::tui::data::candle_timestamp(&candles[16]),
            visible_n: candles.len(),
        };
        let hovered = chart_geometry(
            Rect::new(0, 0, 60, 19),
            &candles,
            None,
            Some(anchor),
            true,
            false,
        );
        assert_eq!(
            hovered.start, 16,
            "the window must not move under the pointer"
        );
        assert_eq!(hovered.readout_idx, 20);
        assert_eq!(hovered.selected_col, Some(4));

        let scanned = chart_geometry(
            Rect::new(0, 0, 60, 19),
            &candles,
            Some(19),
            None,
            true,
            false,
        );
        assert_eq!(scanned.readout_idx, 20);
        assert_eq!(scanned.start, 8);
    }

    #[test]
    fn hover_survives_an_appended_bar_and_names_the_same_candle() {
        let mut candles = series(40);
        let selected = crate::tui::data::candle_timestamp(&candles[20]).clone();
        let window = crate::tui::data::candle_timestamp(&candles[16]).clone();
        let before = chart_geometry(
            Rect::new(0, 0, 60, 19),
            &candles,
            None,
            Some(HoverAnchor {
                selected_ts: &selected,
                window_ts: &window,
                visible_n: candles.len(),
            }),
            true,
            false,
        );
        candles.push(crate::tui::data::test_models::candle_at(
            "2026-09-09 09:55:00",
            140.0,
            141.0,
            139.0,
            140.5,
            1,
        ));
        let after = chart_geometry(
            Rect::new(0, 0, 60, 19),
            &candles,
            None,
            Some(HoverAnchor {
                selected_ts: &selected,
                window_ts: &window,
                visible_n: candles.len(),
            }),
            true,
            false,
        );
        assert_eq!(before.start, after.start);
        assert_eq!(before.readout_idx, after.readout_idx);
        assert_eq!(after.selected_col, before.selected_col);
    }

    #[test]
    fn hover_survives_a_drained_cap_by_resolving_timestamps() {
        let candles = series(40);
        let selected = crate::tui::data::candle_timestamp(&candles[20]).clone();
        let window = crate::tui::data::candle_timestamp(&candles[16]).clone();
        let drained = candles[10..].to_vec();
        let geometry = chart_geometry(
            Rect::new(0, 0, 60, 19),
            &drained,
            None,
            Some(HoverAnchor {
                selected_ts: &selected,
                window_ts: &window,
                visible_n: drained.len(),
            }),
            true,
            false,
        );
        assert_eq!(geometry.readout_idx, 10, "same bar, ten indices earlier");
        assert_eq!(geometry.start, 6);
        assert_eq!(geometry.selected_col, Some(4));
    }

    #[test]
    fn an_unresolvable_anchor_falls_back_to_the_keyboard_window() {
        let candles = series(40);
        let geometry = chart_geometry(
            Rect::new(0, 0, 60, 19),
            &candles,
            None,
            Some(HoverAnchor {
                selected_ts: "2020-01-01 09:15:00",
                window_ts: &crate::tui::data::candle_timestamp(&candles[16]),
                visible_n: candles.len(),
            }),
            true,
            false,
        );
        assert_eq!(geometry.start, 16);
        assert_eq!(geometry.readout_idx, 39, "LIVE, not a guessed replacement");
        assert_eq!(geometry.selected_col, None);
    }

    #[test]
    fn hover_draws_the_crosshair_and_readout_of_the_pointed_candle() {
        let candles = series(40);
        let anchor = HoverAnchor {
            selected_ts: &crate::tui::data::candle_timestamp(&candles[20]),
            window_ts: &crate::tui::data::candle_timestamp(&candles[16]),
            visible_n: candles.len(),
        };
        let (buffer, hit) = draw_into(
            60,
            20,
            Rect::new(0, 0, 60, 20),
            &candles,
            None,
            Some(anchor),
        );
        let hit = hit.expect("a hovered chart still publishes its plot");
        let readout = row_text(&buffer, hit.plot.y - 1);
        assert!(
            readout.starts_with("19 ago"),
            "hover names the bar's age, never LIVE: {readout:?}"
        );
        assert!(
            readout.contains(&crate::tui::data::candle_timestamp(&candles[20])[0..16]),
            "hover readout must describe the pointed candle: {readout:?}"
        );
        let column = hit.plot.x + u16::try_from(4 * CANDLE_STRIDE + 1).unwrap();
        let marked = (hit.plot.y..hit.plot.y + hit.plot.height)
            .any(|y| buffer[(column, y)].symbol() == CROSSHAIR);
        assert!(marked, "the inspected bar's column must be marked");
    }

    #[test]
    fn a_window_that_scrolled_the_selection_away_still_reads_it_out() {
        let candles = series(40);
        let geometry = chart_geometry(
            Rect::new(0, 0, u16::try_from(AXIS_W).unwrap() + 8, 19),
            &candles,
            None,
            Some(HoverAnchor {
                selected_ts: &crate::tui::data::candle_timestamp(&candles[20]),
                window_ts: &crate::tui::data::candle_timestamp(&candles[16]),
                visible_n: candles.len(),
            }),
            true,
            false,
        );
        assert_eq!(geometry.visible_n, 4);
        assert_eq!(geometry.readout_idx, 20, "the readout still names the bar");
        assert_eq!(
            geometry.selected_col, None,
            "but nothing on screen marks it"
        );
    }
}
