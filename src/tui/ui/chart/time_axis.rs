use super::{
    CANDLE_STRIDE, Candle, Frame, Paragraph, Rect, Span, TICK_GAP, TICK_GLYPH, TICK_LABEL_W, format,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TickUnit {
    Minute(u32),
    Hour(u32),
    Day,
    Week,
    Month,
    Year,
}

const TICK_LADDER: [TickUnit; 10] = [
    TickUnit::Minute(5),
    TickUnit::Minute(15),
    TickUnit::Minute(30),
    TickUnit::Hour(1),
    TickUnit::Hour(2),
    TickUnit::Hour(4),
    TickUnit::Day,
    TickUnit::Week,
    TickUnit::Month,
    TickUnit::Year,
];

impl TickUnit {
    fn bucket(self, date: format::WallDate) -> i64 {
        let day = date.epoch_day();
        let minutes = i64::from(date.hour) * 60 + i64::from(date.minute);
        match self {
            Self::Minute(step) => (day * 1_440 + minutes) / i64::from(step),
            Self::Hour(step) => (day * 24 + i64::from(date.hour)) / i64::from(step),
            Self::Day => day,
            Self::Week => (day + 3).div_euclid(7),
            Self::Month => i64::from(date.year) * 12 + i64::from(date.month),
            Self::Year => i64::from(date.year),
        }
    }

    fn label(self, date: format::WallDate) -> String {
        match self {
            Self::Minute(_) | Self::Hour(_) => format!("{:02}:{:02}", date.hour, date.minute),
            Self::Day => format!("{}{:02}", date.weekday_name(), date.day),
            Self::Week => format!("{:02}{}", date.day, date.month_name()),
            Self::Month => date.month_name().to_string(),
            Self::Year => format!("{:04}", date.year),
        }
    }

    fn reads_the_clock(self) -> bool {
        matches!(self, Self::Minute(_) | Self::Hour(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct AxisTick {
    col: usize,
    label: String,
}

impl AxisTick {
    fn end(&self) -> usize {
        self.col + 1 + self.label.chars().count()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum TickPlan {
    Ticks {
        unit: TickUnit,
        ticks: Vec<AxisTick>,
    },
    LeftEdge(String),
}

pub(super) fn boundaries(
    unit: TickUnit,
    dates: &[Option<format::WallDate>],
    pad: usize,
) -> Vec<AxisTick> {
    let mut ticks = Vec::new();
    let mut bucket = None;
    let mut day = None;
    for (slot, date) in dates.iter().enumerate() {
        let Some(date) = *date else {
            continue;
        };
        let next = unit.bucket(date);
        let opens_session = day.is_some_and(|previous| previous != date.epoch_day());
        day = Some(date.epoch_day());
        if bucket == Some(next) {
            continue;
        }
        bucket = Some(next);
        let label = if unit.reads_the_clock() && opens_session {
            TickUnit::Day.label(date)
        } else {
            unit.label(date)
        };
        ticks.push(AxisTick {
            col: pad + slot * CANDLE_STRIDE + (CANDLE_STRIDE - 1),
            label,
        });
    }
    ticks
}

pub(super) fn spaced(ticks: &[AxisTick]) -> bool {
    ticks
        .windows(2)
        .all(|pair| pair[1].col >= pair[0].end() + TICK_GAP)
}

pub(super) fn choose_ticks(visible: &[Candle], pad: usize, chart_w: usize) -> TickPlan {
    let dates: Vec<_> = visible
        .iter()
        .map(|candle| format::WallDate::parse(&crate::tui::data::candle_timestamp(candle)))
        .collect();
    let mut clocks = dates.iter().flatten().map(|date| (date.hour, date.minute));
    let first_clock = clocks.next();
    let clocks_differ = clocks.any(|clock| Some(clock) != first_clock);

    for unit in TICK_LADDER {
        if unit.reads_the_clock() && !clocks_differ {
            continue;
        }
        let mut ticks = boundaries(unit, &dates, pad);
        while ticks.last().is_some_and(|last| last.end() > chart_w) {
            ticks.pop();
        }
        if ticks.len() >= 2 && ticks[1].col < ticks[0].end() + TICK_GAP {
            ticks.remove(0);
        }
        if ticks.len() >= 2 && spaced(&ticks) {
            return TickPlan::Ticks { unit, ticks };
        }
    }

    TickPlan::LeftEdge(
        dates
            .first()
            .copied()
            .flatten()
            .map(|date| TickUnit::Week.label(date))
            .unwrap_or_default(),
    )
}

pub(super) fn time_axis_text(
    visible: &[Candle],
    pad: usize,
    chart_w: usize,
    axis_cols: usize,
) -> String {
    let mut cells = vec![' '; chart_w.saturating_add(axis_cols)];
    if chart_w < TICK_LABEL_W {
        return cells.into_iter().collect();
    }

    match choose_ticks(visible, pad, chart_w) {
        TickPlan::Ticks { ticks, .. } => {
            for tick in ticks {
                cells[tick.col] = TICK_GLYPH;
                for (offset, ch) in tick.label.chars().enumerate() {
                    cells[tick.col + 1 + offset] = ch;
                }
            }
        }
        TickPlan::LeftEdge(label) => {
            for (offset, ch) in label.chars().enumerate().take(chart_w) {
                cells[offset] = ch;
            }
        }
    }
    cells.into_iter().collect()
}

pub(super) fn draw_time_axis(
    frame: &mut Frame,
    area: Rect,
    visible: &[Candle],
    pad: usize,
    chart_w: usize,
    axis_cols: usize,
) {
    frame.render_widget(
        Paragraph::new(Span::styled(
            time_axis_text(visible, pad, chart_w, axis_cols),
            format::dim(),
        )),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::super::panes::candle_cell;
    use super::super::test_support::candle;
    use super::super::{AXIS_W, DOJI_GLYPH, candle_window, price_to_row, time_axis_rows};
    use super::{
        AxisTick, CANDLE_STRIDE, TICK_GAP, TICK_GLYPH, TickPlan, TickUnit, choose_ticks,
        time_axis_text,
    };
    use crate::tui::data::Candle;
    use crate::tui::format;

    const JULY_2026: u64 = 1_782_844_200;
    const SESSION_OPEN: u64 = 1_788_752_700;
    const SEP_2024: u64 = 1_725_215_400;

    fn series(start: u64, step: u64, n: usize) -> Vec<Candle> {
        (0..n)
            .map(|index| {
                let ts = format::datetime_ist_string(start + index as u64 * step);
                candle(&ts, 100.0, 100.0, 100.0, 100.0, 0)
            })
            .collect()
    }

    fn trading_days(start: u64, n: usize) -> Vec<Candle> {
        let mut candles = Vec::new();
        let mut secs = start;
        while candles.len() < n {
            if (1..=5).contains(&format::weekday_ist(secs)) {
                let ts = format::datetime_ist_string(secs);
                candles.push(candle(&ts, 100.0, 100.0, 100.0, 100.0, 0));
            }
            secs += 86_400;
        }
        candles
    }

    fn pane(cols: usize, candles: &[Candle]) -> (Vec<Candle>, usize, usize) {
        let chart_w = cols - 2 - AXIS_W;
        let visible_n = candles.len().min(chart_w / CANDLE_STRIDE);
        let pad = chart_w - visible_n * CANDLE_STRIDE;
        (candles[candles.len() - visible_n..].to_vec(), pad, chart_w)
    }

    fn ticks_at(plan: &TickPlan, unit: TickUnit) -> &[AxisTick] {
        match plan {
            TickPlan::Ticks {
                unit: chosen,
                ticks,
            } => {
                assert_eq!(*chosen, unit, "wrong granularity: {plan:?}");
                ticks
            }
            TickPlan::LeftEdge(label) => panic!("expected {unit:?} ticks, got left edge {label:?}"),
        }
    }

    fn assert_spaced(ticks: &[AxisTick], chart_w: usize) {
        assert!(ticks.len() >= 2, "an axis needs two labels: {ticks:?}");
        for pair in ticks.windows(2) {
            assert!(
                pair[1].col >= pair[0].end() + TICK_GAP,
                "labels crowd: {pair:?}"
            );
        }
        let last = ticks.last().expect("checked above");
        assert!(last.end() <= chart_w, "label overflows {chart_w}: {last:?}");
    }

    #[test]
    fn candle_window_live_shows_latest() {
        let (start, readout) = candle_window(200, 20, None);
        assert_eq!(start, 180);
        assert_eq!(readout, 199); // latest candle
    }

    #[test]
    fn candle_window_cursor_centers_selected() {
        let (start, readout) = candle_window(200, 20, Some(50));
        assert_eq!(readout, 149);
        assert_eq!(start, 139);
    }

    #[test]
    fn candle_window_cursor_clamps_at_start() {
        let (start, readout) = candle_window(20, 10, Some(19));
        assert_eq!(readout, 0);
        assert_eq!(start, 0);
    }

    #[test]
    fn candle_window_cursor_clamps_at_end() {
        let (start, readout) = candle_window(20, 10, Some(0));
        assert_eq!(readout, 19);
        assert_eq!(start, 10);
    }

    #[test]
    fn candle_window_fewer_candles_than_slots() {
        let (start, readout) = candle_window(3, 10, None);
        assert_eq!(start, 0);
        assert_eq!(readout, 2);

        let (start, readout) = candle_window(3, 10, Some(2));
        assert_eq!(start, 0);
        assert_eq!(readout, 0);
    }

    #[test]
    fn candle_window_zero_slots_handles_any_cursor() {
        let (start, readout) = candle_window(200, 0, None);
        assert_eq!(readout, 199); // absolute readout index is still valid
        assert!(start <= 200);

        let (start, readout) = candle_window(200, 0, Some(50));
        assert_eq!(readout, 149);
        assert!(start <= 200);
    }

    #[test]
    fn min_maps_to_bottom_max_to_top() {
        assert_eq!(price_to_row(10.0, 10.0, 20.0, 8), 7);
        assert_eq!(price_to_row(20.0, 10.0, 20.0, 8), 0);
    }

    #[test]
    fn scaling_is_monotonic() {
        let mut previous = usize::MAX;
        for step in 0..=10 {
            let price = 100.0 + f64::from(step);
            let row = price_to_row(price, 100.0, 110.0, 12);
            assert!(row <= previous, "row went up for a higher price");
            previous = row;
        }
    }

    #[test]
    fn midpoint_lands_in_the_middle() {
        let row = price_to_row(15.0, 10.0, 20.0, 10);
        assert!((4..=5).contains(&row), "midpoint row was {row}");
    }

    #[test]
    fn flat_range_maps_to_middle() {
        assert_eq!(price_to_row(10.0, 10.0, 10.0, 8), 4);
    }

    #[test]
    fn zero_rows_never_panics() {
        assert_eq!(price_to_row(10.0, 5.0, 20.0, 0), 0);
    }

    #[test]
    fn a_true_doji_is_horizontal_with_vertical_wicks() {
        let doji = candle("2026-08-20 09:15:00", 100.0, 110.0, 90.0, 100.0, 0);
        let rows = 5;
        let body_row = price_to_row(100.0, 90.0, 110.0, rows);
        for row in 0..rows {
            let (glyph, _) = candle_cell(&doji, row, 90.0, 110.0, rows);
            if row == body_row {
                assert_eq!(glyph, DOJI_GLYPH);
            } else {
                assert_eq!(glyph, "│");
            }
        }
    }

    #[test]
    fn a_flat_candle_is_one_horizontal_cell_not_a_block() {
        let flat = candle("2026-08-20 09:30:00", 100.0, 100.0, 100.0, 100.0, 0);
        for row in 0..5 {
            let (glyph, _) = candle_cell(&flat, row, 90.0, 110.0, 5);
            let expected = if row == 2 { DOJI_GLYPH } else { " " };
            assert_eq!(glyph, expected);
        }
    }

    #[test]
    fn a_real_body_stays_solid_when_both_prices_quantize_to_one_row() {
        let candle = candle("2026-08-20 09:45:00", 100.0, 100.01, 100.0, 100.01, 0);
        let row = price_to_row(100.0, 90.0, 110.0, 5);
        assert_eq!(candle_cell(&candle, row, 90.0, 110.0, 5).0, "█");
    }

    #[test]
    fn daily_bars_label_weeks_rather_than_crowding_every_date() {
        let candles = trading_days(JULY_2026, 45);
        for cols in [80, 120] {
            let (visible, pad, chart_w) = pane(cols, &candles);
            let plan = choose_ticks(&visible, pad, chart_w);
            let ticks = ticks_at(&plan, TickUnit::Week);
            assert_spaced(ticks, chart_w);
            assert!(ticks.len() >= 3, "at {cols} columns: {ticks:?}");
        }
    }

    #[test]
    fn five_minute_bars_label_the_session_clock() {
        let candles = series(SESSION_OPEN, 300, 76);
        let (visible, pad, chart_w) = pane(80, &candles);
        let plan = choose_ticks(&visible, pad, chart_w);
        let ticks = ticks_at(&plan, TickUnit::Minute(30));
        assert_spaced(ticks, chart_w);
        for tick in ticks {
            let minute: u32 = tick.label[3..].parse().expect("HH:MM label");
            assert_eq!(minute % 30, 0, "off-clock label: {tick:?}");
        }
    }

    #[test]
    fn one_minute_bars_label_minute_multiples() {
        let candles = series(SESSION_OPEN, 60, 375);
        let (visible, pad, chart_w) = pane(80, &candles);
        let plan = choose_ticks(&visible, pad, chart_w);
        let ticks = ticks_at(&plan, TickUnit::Minute(5));
        assert_spaced(ticks, chart_w);
        for tick in ticks {
            let minute: u32 = tick.label[3..].parse().expect("HH:MM label");
            assert_eq!(minute % 5, 0, "off-clock label: {tick:?}");
        }
    }

    #[test]
    fn weekly_bars_label_months() {
        let candles = series(JULY_2026, 7 * 86_400, 40);
        let (visible, pad, chart_w) = pane(80, &candles);
        let plan = choose_ticks(&visible, pad, chart_w);
        let ticks = ticks_at(&plan, TickUnit::Month);
        assert_spaced(ticks, chart_w);
        assert!(
            ticks.iter().all(|tick| tick.label.chars().count() == 3),
            "month labels: {ticks:?}"
        );
    }

    #[test]
    fn a_two_year_window_labels_years() {
        let candles = series(SEP_2024, 24 * 86_400, 33);
        let (visible, pad, chart_w) = pane(80, &candles);
        let plan = choose_ticks(&visible, pad, chart_w);
        let ticks = ticks_at(&plan, TickUnit::Year);
        assert_spaced(ticks, chart_w);
        assert_eq!(
            ticks
                .iter()
                .map(|tick| tick.label.as_str())
                .collect::<Vec<_>>(),
            ["2024", "2025", "2026"]
        );
    }

    #[test]
    fn an_intraday_window_labels_each_session_open_with_its_date() {
        let mut candles = series(SESSION_OPEN + 4 * 3_600, 900, 4);
        candles.extend(series(SESSION_OPEN + 86_400, 900, 4));
        let (visible, pad, chart_w) = pane(54, &candles);
        let plan = choose_ticks(&visible, pad, chart_w);
        let ticks = ticks_at(&plan, TickUnit::Hour(4));
        assert_spaced(ticks, chart_w);
        assert_eq!(
            ticks
                .iter()
                .map(|tick| tick.label.as_str())
                .collect::<Vec<_>>(),
            ["13:15", "Tue08"]
        );
    }

    #[test]
    fn one_session_of_hourly_bars_labels_four_hour_buckets() {
        let candles = series(SESSION_OPEN + 2 * 86_400 - 900, 3_600, 15);
        let (visible, pad, chart_w) = pane(44, &candles);
        assert_eq!((pad, chart_w), (0, 30));
        let plan = choose_ticks(&visible, pad, chart_w);
        let ticks = ticks_at(&plan, TickUnit::Hour(4));
        assert_spaced(ticks, chart_w);
        assert_eq!(
            ticks
                .iter()
                .map(|tick| (tick.col, tick.label.as_str()))
                .collect::<Vec<_>>(),
            [(7, "12:00"), (15, "16:00"), (23, "20:00")]
        );
    }

    #[test]
    fn a_pane_too_narrow_for_two_labels_labels_the_left_edge() {
        let candles = trading_days(JULY_2026, 45);
        let (visible, pad, chart_w) = pane(20, &candles);
        match choose_ticks(&visible, pad, chart_w) {
            TickPlan::LeftEdge(label) => assert_eq!(label.chars().count(), 5),
            plan @ TickPlan::Ticks { .. } => panic!("expected a left-edge label, got {plan:?}"),
        }
    }

    #[test]
    fn the_axis_row_ticks_each_label_and_leaves_the_price_columns_blank() {
        let candles = series(SESSION_OPEN + 4 * 3_600, 900, 8);
        let text = time_axis_text(&candles, 0, 24, 12);
        assert_eq!(text.chars().count(), 36);
        assert_eq!(
            text.trim_end(),
            format!("       {TICK_GLYPH}14:00  {TICK_GLYPH}15:00")
        );
        assert!(
            text.chars().skip(24).all(|cell| cell == ' '),
            "price-axis columns must stay blank: {text:?}"
        );
    }

    #[test]
    fn the_time_axis_uses_only_spare_price_height_and_usable_width() {
        assert_eq!(time_axis_rows(16, 60, 6, 3), 1);
        assert_eq!(time_axis_rows(15, 60, 6, 3), 0);
        assert_eq!(time_axis_rows(20, 16, 6, 3), 0);
    }
}
