use super::{
    BAND_GLYPH, ChartIndicator, Color, LINE_GLYPH, Range, format, indicators, price_to_row,
};

pub(super) struct IndicatorLine<'a> {
    pub(super) values: &'a [Option<f64>],
    pub(super) glyph: &'static str,
    pub(super) color: Color,
}

pub(super) struct PlacedLine {
    pub(super) rows: Vec<Option<usize>>,
    pub(super) glyph: &'static str,
    pub(super) color: Color,
}

impl IndicatorLine<'_> {
    pub(super) fn place(&self, min: f64, max: f64, rows: usize) -> PlacedLine {
        PlacedLine {
            rows: self
                .values
                .iter()
                .map(|value| value.map(|value| price_to_row(value, min, max, rows)))
                .collect(),
            glyph: self.glyph,
            color: self.color,
        }
    }
}

pub(super) struct Oscillator<'a> {
    pub(super) lines: Vec<IndicatorLine<'a>>,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) top_label: String,
    pub(super) bottom_label: String,
}

pub(super) fn line_at(
    placed: &[PlacedLine],
    col: usize,
    r: usize,
) -> Option<(&'static str, Color)> {
    placed
        .iter()
        .find(|line| line.rows.get(col).copied().flatten() == Some(r))
        .map(|line| (line.glyph, line.color))
}

pub(super) fn fold_range(range: (f64, f64), values: &[Option<f64>]) -> (f64, f64) {
    values
        .iter()
        .flatten()
        .filter(|value| value.is_finite())
        .fold(range, |(lo, hi), value| (lo.min(*value), hi.max(*value)))
}

pub(super) fn price_overlays<'a>(
    indicator: ChartIndicator,
    series: &'a indicators::Series,
    len: usize,
    window: Range<usize>,
) -> Vec<IndicatorLine<'a>> {
    if window.end > len || window.is_empty() {
        return Vec::new();
    }
    let line = |values: &'a [Option<f64>], glyph: &'static str, color: Color| IndicatorLine {
        values: &values[window.clone()],
        glyph,
        color,
    };
    match (indicator, series) {
        (ChartIndicator::Sma | ChartIndicator::Ema, indicators::Series::Line(values)) => {
            vec![line(values, LINE_GLYPH, format::info())]
        }
        (ChartIndicator::Bollinger, indicators::Series::Bands(bands)) => vec![
            line(&bands.middle, LINE_GLYPH, format::info()),
            line(&bands.upper, BAND_GLYPH, format::text_muted()),
            line(&bands.lower, BAND_GLYPH, format::text_muted()),
        ],
        _ => Vec::new(),
    }
}

pub(super) fn oscillator(
    indicator: ChartIndicator,
    series: &indicators::Series,
    len: usize,
    window: Range<usize>,
) -> Option<Oscillator<'_>> {
    if window.end > len || window.is_empty() {
        return None;
    }
    match (indicator, series) {
        (ChartIndicator::Rsi, indicators::Series::Line(rsi)) => {
            let values = &rsi[window];
            Some(Oscillator {
                lines: vec![IndicatorLine {
                    values,
                    glyph: LINE_GLYPH,
                    color: format::info(),
                }],
                min: 0.0,
                max: 100.0,
                top_label: "100".to_string(),
                bottom_label: "0".to_string(),
            })
        }
        (ChartIndicator::Macd, indicators::Series::Macd(macd)) => {
            let line = &macd.macd[window.clone()];
            let signal = &macd.signal[window];
            let scale = [&line, &signal]
                .iter()
                .flat_map(|series| series.iter().flatten())
                .filter(|value| value.is_finite())
                .fold(0.0f64, |acc, value| acc.max(value.abs()));
            let (top_label, bottom_label) = macd_axis_labels(scale);
            Some(Oscillator {
                lines: vec![
                    IndicatorLine {
                        values: line,
                        glyph: LINE_GLYPH,
                        color: format::info(),
                    },
                    IndicatorLine {
                        values: signal,
                        glyph: BAND_GLYPH,
                        color: format::warning(),
                    },
                ],
                min: -scale,
                max: scale,
                top_label,
                bottom_label,
            })
        }
        _ => None,
    }
}

const MACD_MAX_DECIMALS: usize = 6;

pub(super) fn macd_axis_labels(scale: f64) -> (String, String) {
    let none = (String::new(), String::new());
    if !scale.is_finite() || scale <= 0.0 {
        return none;
    }
    if format::price(scale) != format::price(0.0) {
        return (format::signed_price(scale), format::signed_price(-scale));
    }
    for decimals in 3..=MACD_MAX_DECIMALS {
        let magnitude = format!("{scale:.decimals$}");
        if magnitude.parse::<f64>().unwrap_or(0.0) > 0.0 {
            return (format!("+{magnitude}"), format!("-{magnitude}"));
        }
    }
    none
}

#[cfg(test)]
mod indicator_tests {
    use super::super::panes::fit_or_omit;
    use super::super::{
        BAND_GLYPH, CANDLE_STRIDE, CROSSHAIR, ChartRequest, LINE_GLYPH, OSC_ROWS, PRICE_MIN_ROWS,
        TITLE_HINTS, VOL_ROWS, aux_pane_rows, title_head,
    };
    use super::{
        IndicatorLine, fold_range, line_at, macd_axis_labels, oscillator, price_overlays,
        price_to_row,
    };
    use crate::tui::data::{Candle, ChartInterval};
    use crate::tui::indicators::{self, ChartIndicator};
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    fn overlays_of<'a>(
        indicator: ChartIndicator,
        series: &'a indicators::Series,
        candles: &[Candle],
        window: std::ops::Range<usize>,
    ) -> Vec<IndicatorLine<'a>> {
        price_overlays(indicator, series, candles.len(), window)
    }

    fn oscillator_of<'a>(
        indicator: ChartIndicator,
        series: &'a indicators::Series,
        candles: &[Candle],
        window: std::ops::Range<usize>,
    ) -> Option<super::Oscillator<'a>> {
        oscillator(indicator, series, candles.len(), window)
    }

    fn solid() -> Candle {
        crate::tui::data::test_models::candle_at(
            "2026-01-01 09:15:00",
            100.0,
            200.0,
            100.0,
            200.0,
            100,
        )
    }

    fn thin(close: f64) -> Candle {
        crate::tui::data::test_models::candle_at(
            "2026-01-01 09:15:00",
            close,
            close,
            close,
            close,
            100,
        )
    }

    fn line(values: &[Option<f64>]) -> IndicatorLine<'_> {
        IndicatorLine {
            values,
            glyph: LINE_GLYPH,
            color: Color::Reset,
        }
    }

    fn render(h: u16, candles: &[Candle], indicator: ChartIndicator) -> Buffer {
        render_at(h, candles, indicator, None)
    }

    fn render_at(
        h: u16,
        candles: &[Candle],
        indicator: ChartIndicator,
        cursor: Option<usize>,
    ) -> Buffer {
        super::super::test_support::render(
            60,
            h,
            ratatui::layout::Rect::new(0, 0, 60, h),
            ChartRequest {
                token: 111,
                candles: Some(candles),
                cursor,
                hover: None,
                interval: ChartInterval::M5,
                loading: false,
                ltp: None,
                show_volume: true,
                indicator,
                series: &indicators::series(indicator, candles),
            },
        )
        .0
    }

    fn cells_with(buffer: &Buffer, glyph: &str) -> Vec<(u16, u16)> {
        let mut found = Vec::new();
        for y in 1..buffer.area.height {
            for x in 0..buffer.area.width {
                if buffer[(x, y)].symbol() == glyph {
                    found.push((x, y));
                }
            }
        }
        found
    }

    fn buffer_text(buffer: &Buffer, y: u16) -> String {
        (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect()
    }

    fn wavy(n: usize) -> Vec<Candle> {
        (0..n)
            .map(|i| {
                let value = f64::from(u32::try_from(i).unwrap());
                thin(100.0 + (value * 0.7).sin() * 8.0 + value * 0.3)
            })
            .collect()
    }

    #[test]
    fn fold_range_widens_to_include_values_outside_the_candle_range() {
        let range = (100.0, 110.0);
        assert_eq!(fold_range(range, &[Some(95.0), Some(130.0)]), (95.0, 130.0));
        assert_eq!(fold_range(range, &[Some(101.0), Some(109.0)]), range);
        assert_eq!(fold_range(range, &[None, None]), range);
        assert_eq!(
            fold_range(range, &[Some(f64::INFINITY), Some(f64::NAN), None]),
            range
        );
        assert_eq!(fold_range(range, &[]), range);
    }

    #[test]
    fn bollinger_bands_widen_the_price_range_rather_than_being_clipped() {
        let candles = wavy(40);
        let window = 20..40;
        let (candle_min, candle_max) = candles[window.clone()]
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), c| {
                (lo.min(c.low), hi.max(c.high))
            });
        let series = indicators::series(ChartIndicator::Bollinger, &candles);
        let overlays = overlays_of(ChartIndicator::Bollinger, &series, &candles, window);
        let (mut min, mut max) = (candle_min, candle_max);
        for line in &overlays {
            (min, max) = fold_range((min, max), line.values);
        }
        let upper = overlays[1]
            .values
            .iter()
            .flatten()
            .copied()
            .fold(f64::MIN, f64::max);
        let lower = overlays[2]
            .values
            .iter()
            .flatten()
            .copied()
            .fold(f64::MAX, f64::min);
        assert!(
            upper > candle_max || lower < candle_min,
            "fixture must actually push a band outside the candles"
        );
        assert!(max >= upper, "upper band {upper} outside widened max {max}");
        assert!(min <= lower, "lower band {lower} outside widened min {min}");
    }

    #[test]
    fn warm_up_values_place_no_row() {
        let placed = line(&[None, None, Some(105.0), None]).place(100.0, 110.0, 10);
        assert_eq!(placed.rows[0], None);
        assert_eq!(placed.rows[1], None);
        assert_eq!(placed.rows[3], None);
        assert_eq!(placed.rows[2], Some(price_to_row(105.0, 100.0, 110.0, 10)));
    }

    #[test]
    fn placement_uses_the_same_scale_as_the_candles() {
        let (min, max, rows) = (100.0, 200.0, 12);
        let values: Vec<Option<f64>> = vec![Some(100.0), Some(150.0), Some(200.0)];
        let placed = line(&values).place(min, max, rows);
        for (i, value) in values.iter().enumerate() {
            let value = value.unwrap();
            assert_eq!(placed.rows[i], Some(price_to_row(value, min, max, rows)));
        }
        assert_eq!(placed.rows[0], Some(rows - 1));
        assert_eq!(placed.rows[2], Some(0));
    }

    #[test]
    fn line_at_resolves_a_collision_in_declaration_order() {
        let first = line(&[Some(150.0)]).place(100.0, 200.0, 11);
        let mut second = line(&[Some(150.0)]).place(100.0, 200.0, 11);
        second.glyph = BAND_GLYPH;
        let row = first.rows[0].unwrap();
        let placed = vec![first, second];
        assert_eq!(line_at(&placed, 0, row).map(|(ch, _)| ch), Some(LINE_GLYPH));
        assert_eq!(line_at(&placed, 0, row + 1), None);
        assert_eq!(line_at(&placed, 7, row), None);
    }

    #[test]
    fn overlays_are_computed_over_the_full_history_then_sliced() {
        let candles = wavy(40);
        let window = 20..40;
        let series = indicators::series(ChartIndicator::Sma, &candles);
        let overlays = overlays_of(ChartIndicator::Sma, &series, &candles, window.clone());
        let expected = indicators::sma(&candles, indicators::SMA_PERIOD)[window].to_vec();
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].values, expected);
        assert!(overlays[0].values.iter().all(Option::is_some));
    }

    #[test]
    fn overlays_and_oscillators_are_mutually_exclusive() {
        let candles = wavy(40);
        let window = 0..40;
        for indicator in [
            ChartIndicator::None,
            ChartIndicator::Rsi,
            ChartIndicator::Macd,
        ] {
            assert!(
                overlays_of(
                    indicator,
                    &indicators::series(indicator, &candles),
                    &candles,
                    window.clone()
                )
                .is_empty(),
                "{indicator:?} must not draw on the price axis"
            );
        }
        for indicator in [
            ChartIndicator::None,
            ChartIndicator::Sma,
            ChartIndicator::Ema,
            ChartIndicator::Bollinger,
        ] {
            assert!(
                oscillator_of(
                    indicator,
                    &indicators::series(indicator, &candles),
                    &candles,
                    window.clone()
                )
                .is_none(),
                "{indicator:?} must not claim an oscillator pane"
            );
        }
        assert_eq!(
            overlays_of(
                ChartIndicator::Bollinger,
                &indicators::series(ChartIndicator::Bollinger, &candles),
                &candles,
                window.clone()
            )
            .len(),
            3,
            "Bollinger draws three lines"
        );
        assert_eq!(
            oscillator_of(
                ChartIndicator::Macd,
                &indicators::series(ChartIndicator::Macd, &candles),
                &candles,
                window
            )
            .unwrap()
            .lines
            .len(),
            2,
            "MACD draws its line and its signal"
        );
    }

    #[test]
    fn out_of_range_windows_are_ignored() {
        let candles = wavy(5);
        assert!(
            overlays_of(
                ChartIndicator::Sma,
                &indicators::series(ChartIndicator::Sma, &candles),
                &candles,
                0..9
            )
            .is_empty()
        );
        assert!(
            overlays_of(
                ChartIndicator::Sma,
                &indicators::series(ChartIndicator::Sma, &candles),
                &candles,
                2..2
            )
            .is_empty()
        );
        assert!(
            oscillator_of(
                ChartIndicator::Rsi,
                &indicators::series(ChartIndicator::Rsi, &candles),
                &candles,
                0..9
            )
            .is_none()
        );
        assert!(
            oscillator_of(
                ChartIndicator::Rsi,
                &indicators::series(ChartIndicator::Rsi, &candles),
                &candles,
                2..2
            )
            .is_none()
        );
        assert!(
            overlays_of(
                ChartIndicator::Sma,
                &indicators::series(ChartIndicator::Sma, &[]),
                &[],
                0..0
            )
            .is_empty()
        );
    }

    #[test]
    fn oscillator_scales_follow_their_conventions() {
        let candles = wavy(60);
        let series = indicators::series(ChartIndicator::Rsi, &candles);
        let rsi = oscillator_of(ChartIndicator::Rsi, &series, &candles, 30..60).unwrap();
        assert_eq!((rsi.min, rsi.max), (0.0, 100.0));
        assert_eq!(
            (rsi.top_label.as_str(), rsi.bottom_label.as_str()),
            ("100", "0")
        );

        let series = indicators::series(ChartIndicator::Macd, &candles);
        let macd = oscillator_of(ChartIndicator::Macd, &series, &candles, 30..60).unwrap();
        assert!(macd.max > 0.0, "fixture must produce a non-flat MACD");
        assert_eq!(
            macd.min.to_bits(),
            (-macd.max).to_bits(),
            "MACD scale must be symmetric"
        );
        assert!(macd.top_label.starts_with('+'));
        assert!(macd.bottom_label.starts_with('-'));

        let flat: Vec<Candle> = (0..60).map(|_| thin(100.0)).collect();
        let series = indicators::series(ChartIndicator::Macd, &flat);
        let macd = oscillator_of(ChartIndicator::Macd, &series, &flat, 0..60).unwrap();
        assert_eq!((macd.min, macd.max), (0.0, 0.0));
    }

    #[test]
    fn aux_pane_rows_never_starves_the_price_pane() {
        assert_eq!(aux_pane_rows(20, true, true), (6, 3));
        assert_eq!(aux_pane_rows(20, false, true), (6, 0));
        assert_eq!(aux_pane_rows(20, true, false), (0, 3));
        assert_eq!(aux_pane_rows(20, false, false), (0, 0));

        assert_eq!(aux_pane_rows(15, true, true), (6, 3));
        assert_eq!(aux_pane_rows(14, true, true), (0, 3));
        assert_eq!(aux_pane_rows(14, false, true), (6, 0));
        assert_eq!(aux_pane_rows(12, false, true), (6, 0));
        assert_eq!(aux_pane_rows(11, false, true), (0, 0));

        for height in 0..=60u16 {
            for volume in [false, true] {
                let (osc, vol) = aux_pane_rows(height, volume, true);
                assert!(osc == 0 || osc == OSC_ROWS, "partial pane at {height}");
                if osc > 0 {
                    assert!(
                        height.saturating_sub(1 + vol + osc) >= PRICE_MIN_ROWS,
                        "price pane starved at height {height}"
                    );
                }
            }
        }

        for height in 0..=6u16 {
            let (osc, vol) = aux_pane_rows(height, true, true);
            assert_eq!(vol, VOL_ROWS.min(height.saturating_sub(2)));
            assert_eq!(osc, 0, "no oscillator at height {height}");
            assert!(height.saturating_sub(1 + vol + osc) >= 1 || height <= 2);
        }
    }

    #[test]
    fn the_oscillator_pane_resolves_the_levels_a_trader_reads() {
        let rows_for_levels = |rows: usize| -> std::collections::BTreeSet<usize> {
            [30.0, 50.0, 70.0]
                .iter()
                .map(|value| price_to_row(*value, 0.0, 100.0, rows))
                .collect()
        };
        assert_eq!(
            rows_for_levels(usize::from(VOL_ROWS)).len(),
            1,
            "a {VOL_ROWS}-row pane cannot separate 30 from 70"
        );
        assert_eq!(
            rows_for_levels(usize::from(OSC_ROWS)).len(),
            3,
            "30/50/70 must land on three different rows at {OSC_ROWS} rows"
        );
        assert!(price_to_row(70.0, 0.0, 100.0, usize::from(OSC_ROWS)) > 0);
        assert!(price_to_row(30.0, 0.0, 100.0, usize::from(OSC_ROWS)) < usize::from(OSC_ROWS) - 1);
    }

    #[test]
    fn fit_or_omit_drops_a_label_that_does_not_fit() {
        assert_eq!(fit_or_omit("100", 5).as_deref(), Some("  100"));
        assert_eq!(fit_or_omit("100", 3).as_deref(), Some("100"));
        assert_eq!(fit_or_omit("100", 2), None);
        assert_eq!(fit_or_omit("+1,23,456.00", 11), None);
        assert_eq!(fit_or_omit("", 0).as_deref(), Some(""));
        assert_eq!(fit_or_omit("₹100", 4).as_deref(), Some("₹100"));
        assert_eq!(fit_or_omit("₹100", 3), None);
    }

    #[test]
    fn oscillator_axis_labels_render_in_full() {
        let buffer = render(20, &wavy(24), ChartIndicator::Rsi);
        let top = buffer_text(&buffer, 11);
        let bottom = buffer_text(&buffer, 16);
        assert!(top.ends_with("         100"), "top RSI label: {top:?}");
        assert!(
            bottom.ends_with("           0"),
            "bottom RSI label: {bottom:?}"
        );
    }

    #[test]
    fn overlays_never_reduce_solid_candle_glyph_count() {
        let candles: Vec<Candle> = (0..24).map(|_| solid()).collect();
        let count = |buffer: &Buffer| {
            buffer
                .content()
                .iter()
                .filter(|cell| cell.symbol() == "█")
                .count()
        };
        let baseline = count(&render(20, &candles, ChartIndicator::None));
        for indicator in [
            ChartIndicator::Sma,
            ChartIndicator::Ema,
            ChartIndicator::Bollinger,
        ] {
            assert_eq!(
                count(&render(20, &candles, indicator)),
                baseline,
                "{indicator:?} must not replace a candle glyph"
            );
        }
    }

    #[test]
    fn an_overlay_stays_visible_through_solid_candles() {
        let candles: Vec<Candle> = (0..24).map(|_| solid()).collect();
        let buffer = render(20, &candles, ChartIndicator::Sma);
        assert!(
            !cells_with(&buffer, LINE_GLYPH).is_empty(),
            "an overlay hidden behind every candle is no overlay at all"
        );
    }

    #[test]
    fn warm_up_columns_draw_no_overlay_glyph() {
        let candles = wavy(24);
        let buffer = render(20, &candles, ChartIndicator::Sma);
        let drawn = cells_with(&buffer, LINE_GLYPH);
        assert!(!drawn.is_empty(), "the defined tail must draw something");
        let first_defined_col = indicators::SMA_PERIOD - 1;
        for (x, y) in &drawn {
            assert!(
                *x as usize >= first_defined_col * CANDLE_STRIDE,
                "overlay drawn at ({x}, {y}) inside the SMA warm-up gap"
            );
        }
    }

    #[test]
    fn an_oscillator_never_draws_on_the_price_pane() {
        let candles = wavy(40);
        for indicator in [ChartIndicator::Rsi, ChartIndicator::Macd] {
            let buffer = render(20, &candles, indicator);
            let drawn = cells_with(&buffer, LINE_GLYPH);
            assert!(!drawn.is_empty(), "{indicator:?} drew nothing at all");
            for (x, y) in &drawn {
                assert!(
                    (11..=16).contains(y),
                    "{indicator:?} drew at ({x}, {y}), outside its own pane"
                );
            }
        }
    }

    #[test]
    fn the_oscillator_pane_only_exists_while_selected() {
        let candles = wavy(24);
        let without = render(20, &candles, ChartIndicator::None);
        let with = render(20, &candles, ChartIndicator::Rsi);
        assert_eq!(cells_with(&without, LINE_GLYPH).len(), 0);
        assert_ne!(cells_with(&with, LINE_GLYPH).len(), 0);
        let squeezed = render(11, &candles, ChartIndicator::Rsi);
        assert!(
            cells_with(&squeezed, LINE_GLYPH).is_empty(),
            "a short chart must drop the oscillator pane whole"
        );
    }

    #[test]
    fn macd_line_and_signal_use_different_glyphs() {
        let buffer = render(20, &wavy(40), ChartIndicator::Macd);
        assert!(!cells_with(&buffer, LINE_GLYPH).is_empty(), "MACD line");
        assert!(!cells_with(&buffer, BAND_GLYPH).is_empty(), "signal line");
    }

    #[test]
    fn bollinger_bands_use_a_different_glyph_from_the_centre_line() {
        let buffer = render(20, &wavy(40), ChartIndicator::Bollinger);
        assert!(!cells_with(&buffer, LINE_GLYPH).is_empty(), "centre line");
        assert!(!cells_with(&buffer, BAND_GLYPH).is_empty(), "bands");
    }

    #[test]
    fn the_title_labels_the_active_indicator_with_its_period() {
        let candles = wavy(24);
        for indicator in [
            ChartIndicator::Sma,
            ChartIndicator::Ema,
            ChartIndicator::Bollinger,
            ChartIndicator::Rsi,
            ChartIndicator::Macd,
        ] {
            let buffer = render(20, &candles, indicator);
            let title = buffer_text(&buffer, 0);
            let label = indicator
                .label()
                .expect("every drawn indicator is labelled");
            assert!(title.contains(&label), "title {title:?} missing {label:?}");
            assert!(title.contains("5m"), "title {title:?} lost the interval");
        }
        let title = buffer_text(&render(20, &candles, ChartIndicator::None), 0);
        assert!(title.contains("5m"));
        assert!(
            !title.contains("SMA") && !title.contains("RSI"),
            "an idle chart must not name an indicator: {title:?}"
        );
    }

    #[test]
    fn the_title_does_not_silently_name_an_undrawn_oscillator() {
        assert_eq!(
            title_head(ChartIndicator::Rsi, ChartInterval::M5, OSC_ROWS),
            " chart · 5m · RSI(14) "
        );
        assert_eq!(
            title_head(ChartIndicator::Rsi, ChartInterval::M5, 0),
            " chart · 5m · RSI(14) no room "
        );
        assert_eq!(
            title_head(ChartIndicator::Sma, ChartInterval::M5, 0),
            " chart · 5m · SMA(20) "
        );
        assert_eq!(
            title_head(ChartIndicator::None, ChartInterval::M5, 0),
            " chart · 5m "
        );

        let candles = wavy(24);
        let squeezed = render(14, &candles, ChartIndicator::Rsi);
        assert!(
            cells_with(&squeezed, LINE_GLYPH).is_empty(),
            "fixture must actually drop the oscillator pane"
        );
        assert!(
            buffer_text(&squeezed, 0).contains("no room"),
            "title named an RSI it never drew: {:?}",
            buffer_text(&squeezed, 0)
        );
        let tall = render(20, &candles, ChartIndicator::Rsi);
        assert_ne!(cells_with(&tall, LINE_GLYPH).len(), 0);
        let title = buffer_text(&tall, 0);
        assert!(
            title.contains("RSI(14)") && !title.contains("no room"),
            "{title:?}"
        );
    }

    #[test]
    fn macd_axis_labels_can_tell_their_bounds_apart_on_a_cheap_instrument() {
        assert_eq!(
            macd_axis_labels(12.345),
            ("+12.35".to_string(), "-12.35".to_string())
        );
        let (top, bottom) = macd_axis_labels(0.0032);
        assert_eq!((top.as_str(), bottom.as_str()), ("+0.003", "-0.003"));
        assert_ne!(top, bottom);
        assert_eq!(macd_axis_labels(4e-7), (String::new(), String::new()));
        assert_eq!(macd_axis_labels(0.0), (String::new(), String::new()));
        assert_eq!(macd_axis_labels(f64::NAN), (String::new(), String::new()));

        for scale in [1e-6, 0.0032, 0.05, 12.345, 9_999.99] {
            let (top, bottom) = macd_axis_labels(scale);
            for label in [top, bottom] {
                assert!(
                    fit_or_omit(&label, super::super::AXIS_W).is_some(),
                    "label {label:?} does not fit the axis"
                );
            }
        }

        let candles: Vec<Candle> = (0..60)
            .map(|i| thin(14.0 + (f64::from(i) * 0.5).sin() * 0.02))
            .collect();
        let series = indicators::series(ChartIndicator::Macd, &candles);
        let pane = oscillator_of(ChartIndicator::Macd, &series, &candles, 30..60).unwrap();
        assert!(pane.max > 0.0 && pane.max < 0.005, "scale {}", pane.max);
        assert_ne!(
            pane.top_label, pane.bottom_label,
            "a ₹14 instrument's MACD axis labelled both bounds the same"
        );
    }

    #[test]
    fn the_hover_crosshair_continues_into_the_oscillator_pane() {
        let candles = wavy(24);
        let buffer = render_at(20, &candles, ChartIndicator::Rsi, Some(0));
        let marks = cells_with(&buffer, CROSSHAIR);
        assert!(!marks.is_empty(), "no crosshair drawn at all");
        assert!(
            marks.iter().all(|(x, _)| *x == 47),
            "the crosshair must stay in the hovered bar's glyph column: {marks:?}"
        );
        assert!(
            marks.iter().any(|(_, y)| (11..=16).contains(y)),
            "the crosshair stopped at the oscillator pane boundary: {marks:?}"
        );
        assert!(
            cells_with(&render(20, &candles, ChartIndicator::Rsi), CROSSHAIR).is_empty(),
            "a live chart with no cursor must draw no crosshair"
        );
    }

    #[test]
    fn the_chart_title_drops_hints_rather_than_overflowing() {
        for width in 0..=120u16 {
            for indicator in [
                ChartIndicator::None,
                ChartIndicator::Sma,
                ChartIndicator::Rsi,
            ] {
                let (head, tail) = super::super::title_text(indicator, ChartInterval::M5, 0, width);
                let rendered = head.chars().count() + tail.chars().count();
                assert!(
                    rendered <= width as usize || head.chars().count() > width as usize,
                    "{indicator:?} at width {width} renders {rendered} columns"
                );
                assert!(
                    tail.is_empty() || (tail.starts_with(" (") && tail.ends_with(") ")),
                    "hint tail {tail:?} is not whole"
                );
                for hint in TITLE_HINTS {
                    let partial = tail.contains(&hint[..3]) && !tail.contains(hint);
                    assert!(
                        !partial,
                        "hint {hint:?} was clipped at width {width}: {tail:?}"
                    );
                }
            }
        }
        let (_, tail) = super::super::title_text(ChartIndicator::None, ChartInterval::M5, 0, 200);
        for hint in TITLE_HINTS {
            assert!(tail.contains(hint), "{hint:?} missing from {tail:?}");
        }
    }

    #[test]
    fn tiny_charts_and_short_histories_never_panic() {
        for indicator in [
            ChartIndicator::None,
            ChartIndicator::Sma,
            ChartIndicator::Ema,
            ChartIndicator::Bollinger,
            ChartIndicator::Rsi,
            ChartIndicator::Macd,
        ] {
            for height in [1u16, 2, 3, 5, 8, 12] {
                render(height, &wavy(3), indicator);
                render(height, &[thin(100.0)], indicator);
            }
        }
    }
}
