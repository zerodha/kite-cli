use crate::tui::data::Candle;

pub const SMA_PERIOD: usize = 20;
pub const EMA_PERIOD: usize = 20;
pub const RSI_PERIOD: usize = 14;
pub const MACD_FAST: usize = 12;
pub const MACD_SLOW: usize = 26;
pub const MACD_SIGNAL: usize = 9;
pub const BOLLINGER_PERIOD: usize = 20;
pub const BOLLINGER_MULT: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChartIndicator {
    #[default]
    None,
    Sma,
    Ema,
    Bollinger,
    Rsi,
    Macd,
}

impl ChartIndicator {
    pub const ALL: &'static [ChartIndicator] = &[
        ChartIndicator::None,
        ChartIndicator::Sma,
        ChartIndicator::Ema,
        ChartIndicator::Bollinger,
        ChartIndicator::Rsi,
        ChartIndicator::Macd,
    ];

    pub fn cycle(self) -> ChartIndicator {
        crate::util::cycle_next(Self::ALL, self)
    }

    pub fn label(self) -> Option<String> {
        match self {
            ChartIndicator::None => None,
            ChartIndicator::Sma => Some(format!("SMA({SMA_PERIOD})")),
            ChartIndicator::Ema => Some(format!("EMA({EMA_PERIOD})")),
            ChartIndicator::Bollinger => {
                Some(format!("BB({BOLLINGER_PERIOD},{BOLLINGER_MULT:.1})"))
            }
            ChartIndicator::Rsi => Some(format!("RSI({RSI_PERIOD})")),
            ChartIndicator::Macd => Some(format!("MACD({MACD_FAST},{MACD_SLOW},{MACD_SIGNAL})")),
        }
    }

    pub fn config_token(self) -> &'static str {
        match self {
            ChartIndicator::None => "none",
            ChartIndicator::Sma => "sma",
            ChartIndicator::Ema => "ema",
            ChartIndicator::Bollinger => "bollinger",
            ChartIndicator::Rsi => "rsi",
            ChartIndicator::Macd => "macd",
        }
    }

    pub fn is_oscillator(self) -> bool {
        matches!(self, ChartIndicator::Rsi | ChartIndicator::Macd)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Macd {
    pub macd: Vec<Option<f64>>,
    pub signal: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Bollinger {
    pub middle: Vec<Option<f64>>,
    pub upper: Vec<Option<f64>>,
    pub lower: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum Series {
    #[default]
    None,
    Line(Vec<Option<f64>>),
    Bands(Bollinger),
    Macd(Macd),
}

pub fn series(indicator: ChartIndicator, candles: &[Candle]) -> Series {
    match indicator {
        ChartIndicator::None => Series::None,
        ChartIndicator::Sma => Series::Line(sma(candles, SMA_PERIOD)),
        ChartIndicator::Ema => Series::Line(ema(candles, EMA_PERIOD)),
        ChartIndicator::Rsi => Series::Line(rsi(candles, RSI_PERIOD)),
        ChartIndicator::Bollinger => {
            Series::Bands(bollinger(candles, BOLLINGER_PERIOD, BOLLINGER_MULT))
        }
        ChartIndicator::Macd => Series::Macd(macd(candles, MACD_FAST, MACD_SLOW, MACD_SIGNAL)),
    }
}

pub fn sma(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    sma_values(&closes_of(candles), period)
}

pub fn ema(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    ema_values(&closes_of(candles), period)
}

pub fn rsi(candles: &[Candle], period: usize) -> Vec<Option<f64>> {
    let closes = closes_of(candles);
    let mut out = vec![None; closes.len()];
    if period == 0 || closes.len() <= period {
        return out;
    }

    let deltas = || closes.windows(2).map(|w| w[1] - w[0]);
    #[expect(
        clippy::cast_precision_loss,
        reason = "indicator periods are small integers, exact in f64"
    )]
    let period_f = period as f64;

    let mut avg_gain = deltas().take(period).map(gain_of).sum::<f64>() / period_f;
    let mut avg_loss = deltas().take(period).map(loss_of).sum::<f64>() / period_f;
    out[period] = wilder_rsi(avg_gain, avg_loss);

    for (j, delta) in deltas().enumerate().skip(period) {
        avg_gain = (avg_gain * (period_f - 1.0) + gain_of(delta)) / period_f;
        avg_loss = (avg_loss * (period_f - 1.0) + loss_of(delta)) / period_f;
        out[j + 1] = wilder_rsi(avg_gain, avg_loss);
    }
    out
}

pub fn macd(candles: &[Candle], fast: usize, slow: usize, signal: usize) -> Macd {
    let closes = closes_of(candles);
    let fast_ema = ema_values(&closes, fast);
    let slow_ema = ema_values(&closes, slow);

    let line: Vec<Option<f64>> = fast_ema
        .iter()
        .zip(&slow_ema)
        .map(|pair| match pair {
            (Some(fast), Some(slow)) => finite(fast - slow),
            _ => None,
        })
        .collect();

    Macd {
        signal: ema_of_optional(&line, signal),
        macd: line,
    }
}

pub fn bollinger(candles: &[Candle], period: usize, mult: f64) -> Bollinger {
    let closes = closes_of(candles);
    let len = closes.len();
    let middle = sma_values(&closes, period);
    let mut upper = vec![None; len];
    let mut lower = vec![None; len];
    if period == 0 {
        return Bollinger {
            middle,
            upper,
            lower,
        };
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "indicator periods are small integers, exact in f64"
    )]
    let period_f = period as f64;
    for (offset, window) in closes.windows(period).enumerate() {
        let i = offset + period - 1;
        let Some(mean) = middle[i] else { continue };
        let variance = window.iter().map(|c| (c - mean) * (c - mean)).sum::<f64>() / period_f;
        let deviation = mult * variance.sqrt();
        upper[i] = finite(mean + deviation);
        lower[i] = finite(mean - deviation);
    }

    Bollinger {
        middle,
        upper,
        lower,
    }
}

fn closes_of(candles: &[Candle]) -> Vec<f64> {
    candles.iter().map(|c| c.close).collect()
}

fn finite(value: f64) -> Option<f64> {
    value.is_finite().then_some(value)
}

fn gain_of(delta: f64) -> f64 {
    delta.max(0.0)
}

fn loss_of(delta: f64) -> f64 {
    (-delta).max(0.0)
}

fn wilder_rsi(avg_gain: f64, avg_loss: f64) -> Option<f64> {
    if avg_gain <= 0.0 && avg_loss <= 0.0 {
        return None;
    }
    if avg_loss <= 0.0 {
        return Some(100.0);
    }
    if avg_gain <= 0.0 {
        return Some(0.0);
    }
    let rs = avg_gain / avg_loss;
    finite(100.0 - 100.0 / (1.0 + rs))
}

fn sma_values(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; closes.len()];
    if period == 0 || closes.len() < period {
        return out;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "indicator periods are small integers, exact in f64"
    )]
    let period_f = period as f64;
    for (window, slot) in closes.windows(period).zip(out.iter_mut().skip(period - 1)) {
        *slot = finite(window.iter().sum::<f64>() / period_f);
    }
    out
}

fn ema_values(closes: &[f64], period: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; closes.len()];
    if period == 0 || closes.len() < period {
        return out;
    }
    #[expect(
        clippy::cast_precision_loss,
        reason = "indicator periods are small integers, exact in f64"
    )]
    let alpha = 2.0 / (period as f64 + 1.0);
    #[expect(
        clippy::cast_precision_loss,
        reason = "indicator periods are small integers, exact in f64"
    )]
    let seed = closes[..period].iter().sum::<f64>() / period as f64;
    out[period - 1] = finite(seed);

    let mut prev = seed;
    for (i, close) in closes.iter().enumerate().skip(period) {
        prev = close * alpha + prev * (1.0 - alpha);
        out[i] = finite(prev);
    }
    out
}

fn ema_of_optional(series: &[Option<f64>], period: usize) -> Vec<Option<f64>> {
    let mut out = vec![None; series.len()];
    if period == 0 {
        return out;
    }
    let Some(start) = series.iter().position(Option::is_some) else {
        return out;
    };
    let defined: Vec<f64> = series[start..].iter().map_while(|value| *value).collect();

    for (offset, value) in ema_values(&defined, period).into_iter().enumerate() {
        out[start + offset] = value;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const EPS: f64 = 1e-9;
    const PUBLISHED_EPS: f64 = 6e-3;

    const MA_SERIES: [f64; 30] = [
        22.27, 22.19, 22.08, 22.17, 22.18, 22.13, 22.23, 22.43, 22.24, 22.29, 22.15, 22.39, 22.38,
        22.61, 23.36, 24.05, 23.75, 23.83, 23.95, 23.63, 23.82, 23.87, 23.65, 23.19, 23.10, 23.33,
        22.68, 23.10, 22.40, 22.17,
    ];

    const RSI_SERIES: [f64; 33] = [
        44.3389, 44.0902, 44.1497, 43.6124, 44.3278, 44.8264, 45.0955, 45.4245, 45.8433, 46.0826,
        45.8931, 46.0328, 45.6140, 46.2820, 46.2820, 46.0028, 46.0328, 46.4116, 46.2222, 45.6439,
        46.2122, 46.2521, 45.7137, 46.4515, 45.7835, 45.3548, 44.0288, 44.1783, 44.2181, 44.5672,
        43.4205, 42.6628, 43.1314,
    ];

    fn candles(closes: &[f64]) -> Vec<Candle> {
        closes
            .iter()
            .enumerate()
            .map(|(i, &close)| {
                let mut row = crate::tui::data::test_models::candle();
                row.date = format!(
                    "{}+05:30",
                    (format!("2026-01-01 09:{i:02}:00")).replace(' ', "T")
                )
                .parse()
                .expect("date");
                row.open = close;
                row.high = close;
                row.low = close;
                row.close = close;
                row.volume = 100;
                row
            })
            .collect()
    }

    fn first_defined(series: &[Option<f64>]) -> Option<usize> {
        series.iter().position(Option::is_some)
    }

    #[track_caller]
    fn assert_series(actual: &[Option<f64>], expected: &[Option<f64>], eps: f64) {
        assert_eq!(
            actual.len(),
            expected.len(),
            "length mismatch: {actual:?} vs {expected:?}"
        );
        for (i, (got, want)) in actual.iter().zip(expected).enumerate() {
            match (got, want) {
                (None, None) => {}
                (Some(got), Some(want)) => assert!(
                    (got - want).abs() < eps,
                    "index {i}: got {got}, want {want} (eps {eps})"
                ),
                _ => panic!("index {i}: got {got:?}, want {want:?}"),
            }
        }
    }

    fn expected(nones: usize, values: &[f64]) -> Vec<Option<f64>> {
        let mut out = vec![None; nones];
        out.extend(values.iter().map(|v| Some(*v)));
        out
    }

    const EVERY_INDICATOR: [ChartIndicator; 6] = [
        ChartIndicator::None,
        ChartIndicator::Sma,
        ChartIndicator::Ema,
        ChartIndicator::Bollinger,
        ChartIndicator::Rsi,
        ChartIndicator::Macd,
    ];

    #[test]
    fn chart_indicator_cycle_visits_every_variant_and_wraps_to_off() {
        let mut seen = Vec::new();
        let mut indicator = ChartIndicator::None;
        for _ in 0..EVERY_INDICATOR.len() {
            seen.push(indicator);
            indicator = indicator.cycle();
        }
        assert_eq!(seen, EVERY_INDICATOR.to_vec());
        assert_eq!(indicator, ChartIndicator::None, "cycle must wrap to off");
        assert_eq!(ChartIndicator::default(), ChartIndicator::None);
    }

    #[test]
    fn chart_indicator_labels_carry_the_real_periods() {
        assert_eq!(ChartIndicator::None.label(), None);
        assert_eq!(
            ChartIndicator::Sma.label(),
            Some(format!("SMA({SMA_PERIOD})"))
        );
        assert_eq!(
            ChartIndicator::Ema.label(),
            Some(format!("EMA({EMA_PERIOD})"))
        );
        assert_eq!(
            ChartIndicator::Rsi.label(),
            Some(format!("RSI({RSI_PERIOD})"))
        );
        assert_eq!(ChartIndicator::Bollinger.label(), Some("BB(20,2.0)".into()));
        assert_eq!(ChartIndicator::Macd.label(), Some("MACD(12,26,9)".into()));
        assert_eq!((SMA_PERIOD, EMA_PERIOD, RSI_PERIOD), (20, 20, 14));
        assert_eq!((MACD_FAST, MACD_SLOW, MACD_SIGNAL), (12, 26, 9));
        assert_eq!((BOLLINGER_PERIOD, BOLLINGER_MULT), (20, 2.0));
    }

    #[test]
    fn chart_indicator_classifies_oscillators_and_overlays() {
        for indicator in [ChartIndicator::Rsi, ChartIndicator::Macd] {
            assert!(indicator.is_oscillator(), "{indicator:?}");
        }
        for indicator in [
            ChartIndicator::None,
            ChartIndicator::Sma,
            ChartIndicator::Ema,
            ChartIndicator::Bollinger,
        ] {
            assert!(!indicator.is_oscillator(), "{indicator:?}");
        }
    }

    #[test]
    fn chart_indicator_config_tokens_are_unique() {
        let mut tokens: Vec<&str> = EVERY_INDICATOR.iter().map(|i| i.config_token()).collect();
        let count = tokens.len();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), count, "duplicate config token");
        assert!(
            tokens
                .iter()
                .all(|t| t.chars().all(|c| c.is_ascii_lowercase())),
            "tokens must be plain lowercase: {tokens:?}"
        );
    }

    #[test]
    fn sma_matches_worked_ten_period_example() {
        let want = expected(
            9,
            &[
                22.221, 22.209, 22.229, 22.259, 22.303, 22.421, 22.613, 22.765, 22.905, 23.076,
                23.210, 23.377, 23.525, 23.652, 23.710, 23.684, 23.612, 23.505, 23.432, 23.277,
                23.131,
            ],
        );
        assert_series(&sma(&candles(&MA_SERIES), 10), &want, EPS);
    }

    #[test]
    fn sma_is_aligned_to_the_input() {
        let candles = candles(&MA_SERIES);
        for period in [1usize, 2, 5, 10, 20, 30] {
            let series = sma(&candles, period);
            assert_eq!(series.len(), candles.len(), "period {period}");
            assert_eq!(first_defined(&series), Some(period - 1), "period {period}");
        }
    }

    #[test]
    fn sma_handles_degenerate_inputs() {
        assert_eq!(sma(&[], 20).len(), 0);
        assert_eq!(sma(&[], 0).len(), 0);
        assert_series(&sma(&candles(&[42.0]), 1), &[Some(42.0)], EPS);
        assert_series(&sma(&candles(&[42.0]), 2), &[None], EPS);
        assert!(sma(&candles(&MA_SERIES), 0).iter().all(Option::is_none));
        assert!(sma(&candles(&MA_SERIES), 31).iter().all(Option::is_none));
        let full = sma(&candles(&MA_SERIES), MA_SERIES.len());
        assert_eq!(first_defined(&full), Some(MA_SERIES.len() - 1));
        #[expect(
            clippy::cast_precision_loss,
            reason = "fixture period at most 30 is exactly representable in f64"
        )]
        let mean = MA_SERIES.iter().sum::<f64>() / MA_SERIES.len() as f64;
        assert!((full[MA_SERIES.len() - 1].unwrap() - mean).abs() < EPS);
    }

    #[test]
    fn ema_matches_worked_ten_period_example() {
        let want = expected(
            9,
            &[
                22.221,
                22.208_090_909_091,
                22.241_165_289_256,
                22.266_407_963_937,
                22.328_879_243_221,
                22.516_355_744_454,
                22.795_200_154_553,
                22.968_800_126_452,
                23.125_381_921_643,
                23.275_312_481_344,
                23.339_801_121_100,
                23.427_110_008_173,
                23.507_635_461_232,
                23.533_519_922_826,
                23.471_061_755_040,
                23.403_595_981_396,
                23.390_214_893_870,
                23.261_084_913_166,
                23.231_796_747_136,
                23.080_560_974_929,
                22.915_004_434_033,
            ],
        );
        assert_series(&ema(&candles(&MA_SERIES), 10), &want, EPS);
    }

    #[test]
    fn ema_reproduces_published_column() {
        let published = [
            22.22, 22.21, 22.24, 22.27, 22.33, 22.52, 22.80, 22.97, 23.13, 23.28, 23.34, 23.43,
            23.51, 23.53, 23.47, 23.40, 23.39, 23.26, 23.23, 23.08, 22.92,
        ];
        let series = ema(&candles(&MA_SERIES), 10);
        for (offset, want) in published.iter().enumerate() {
            let got = series[offset + 9].expect("defined from index 9");
            assert!(
                (got - want).abs() < PUBLISHED_EPS,
                "index {}: got {got}, published {want}",
                offset + 9
            );
        }
    }

    #[test]
    fn ema_seed_is_the_simple_mean_of_the_first_period_closes() {
        for period in [2usize, 3, 10, 26] {
            let series = ema(&candles(&MA_SERIES), period);
            assert_eq!(first_defined(&series), Some(period - 1), "period {period}");
            #[expect(
                clippy::cast_precision_loss,
                reason = "fixture period at most 30 is exactly representable in f64"
            )]
            let seed = MA_SERIES[..period].iter().sum::<f64>() / period as f64;
            assert!(
                (series[period - 1].unwrap() - seed).abs() < EPS,
                "period {period}"
            );
        }
    }

    #[test]
    fn ema_is_aligned_to_the_input() {
        let candles = candles(&MA_SERIES);
        for period in [1usize, 2, 12, 26, 30] {
            let series = ema(&candles, period);
            assert_eq!(series.len(), candles.len(), "period {period}");
            assert_eq!(first_defined(&series), Some(period - 1), "period {period}");
        }
    }

    #[test]
    fn ema_handles_degenerate_inputs() {
        assert_eq!(ema(&[], 12).len(), 0);
        assert_eq!(ema(&[], 0).len(), 0);
        assert_series(&ema(&candles(&[42.0]), 1), &[Some(42.0)], EPS);
        assert_series(&ema(&candles(&[42.0]), 2), &[None], EPS);
        assert!(ema(&candles(&MA_SERIES), 0).iter().all(Option::is_none));
        assert!(ema(&candles(&MA_SERIES), 31).iter().all(Option::is_none));
        let full = ema(&candles(&MA_SERIES), MA_SERIES.len());
        assert_eq!(first_defined(&full), Some(MA_SERIES.len() - 1));
        #[expect(
            clippy::cast_precision_loss,
            reason = "fixture period at most 30 is exactly representable in f64"
        )]
        let mean = MA_SERIES.iter().sum::<f64>() / MA_SERIES.len() as f64;
        assert!((full[MA_SERIES.len() - 1].unwrap() - mean).abs() < EPS);
    }

    #[test]
    fn ema_of_flat_series_is_flat() {
        let series = ema(&candles(&[5.0; 20]), 10);
        for value in series.iter().skip(9) {
            assert!((value.unwrap() - 5.0).abs() < EPS);
        }
    }

    #[test]
    fn rsi_reproduces_published_wilder_example() {
        let published = [
            70.53, 66.32, 66.55, 69.41, 66.36, 57.97, 62.93, 63.26, 56.06, 62.38, 54.71, 50.42,
            39.99, 41.46, 41.87, 45.46, 37.30, 33.08, 37.77,
        ];
        let series = rsi(&candles(&RSI_SERIES), 14);
        assert_eq!(series.len(), RSI_SERIES.len());
        assert_eq!(first_defined(&series), Some(14));
        for (offset, want) in published.iter().enumerate() {
            let got = series[offset + 14].expect("defined from index 14");
            assert!(
                (got - want).abs() < PUBLISHED_EPS,
                "index {}: got {got}, published {want}",
                offset + 14
            );
        }
    }

    #[test]
    fn rsi_matches_worked_example_at_full_precision() {
        let want = expected(
            14,
            &[
                70.532_789, 66.318_562, 66.549_830, 69.406_305, 66.355_169, 57.974_856, 62.929_607,
                63.257_148, 56.059_299, 62.377_071, 54.707_573, 50.422_774, 39.989_823, 41.460_482,
                41.868_916, 45.463_212, 37.304_042, 33.079_523, 37.772_952,
            ],
        );
        assert_series(&rsi(&candles(&RSI_SERIES), 14), &want, 1e-5);
    }

    #[test]
    fn rsi_first_defined_index_is_period() {
        let candles = candles(&RSI_SERIES);
        for period in [1usize, 2, 14, 20, 32] {
            let series = rsi(&candles, period);
            assert_eq!(series.len(), candles.len(), "period {period}");
            assert_eq!(first_defined(&series), Some(period), "period {period}");
        }
    }

    #[test]
    fn rsi_of_strictly_rising_series_is_100() {
        let closes: Vec<f64> = (0..30).map(|i| 100.0 + f64::from(i)).collect();
        let series = rsi(&candles(&closes), 14);
        assert_eq!(first_defined(&series), Some(14));
        for value in series.iter().skip(14) {
            assert!((value.unwrap() - 100.0).abs() < EPS);
        }
    }

    #[test]
    fn rsi_of_strictly_falling_series_is_0() {
        let closes: Vec<f64> = (0..30).map(|i| 100.0 - f64::from(i)).collect();
        let series = rsi(&candles(&closes), 14);
        assert_eq!(first_defined(&series), Some(14));
        for value in series.iter().skip(14) {
            assert!(value.unwrap().abs() < EPS);
        }
    }

    #[test]
    fn rsi_of_flat_series_is_undefined_rather_than_a_confident_50() {
        let series = rsi(&candles(&[123.45; 30]), 14);
        assert!(
            series.iter().all(Option::is_none),
            "a series with no movement must not report momentum: {series:?}"
        );

        let mut closes = vec![123.45; 30];
        closes[20] = 124.0;
        let series = rsi(&candles(&closes), 14);
        assert_eq!(first_defined(&series), Some(20));
        assert!(
            series[19].is_none(),
            "the flat run before the move is a gap"
        );
        assert!((series[20].unwrap() - 100.0).abs() < EPS);
    }

    #[test]
    fn rsi_handles_degenerate_inputs() {
        assert_eq!(rsi(&[], 14).len(), 0);
        assert_eq!(rsi(&[], 0).len(), 0);
        assert_series(&rsi(&candles(&[42.0]), 1), &[None], EPS);
        assert!(rsi(&candles(&MA_SERIES), 0).iter().all(Option::is_none));
        assert!(rsi(&candles(&MA_SERIES), 31).iter().all(Option::is_none));
        assert!(
            rsi(&candles(&MA_SERIES), MA_SERIES.len())
                .iter()
                .all(Option::is_none)
        );
        let series = rsi(&candles(&MA_SERIES), MA_SERIES.len() - 1);
        assert_eq!(first_defined(&series), Some(MA_SERIES.len() - 1));
    }

    #[test]
    fn macd_matches_worked_small_period_example() {
        let result = macd(&candles(&MA_SERIES), 3, 6, 3);

        let want_line = expected(
            5,
            &[
                -0.016_250_000_000,
                0.004_732_142_857,
                0.054_406_887_755,
                0.023_661_169_825,
                0.020_014_674_875,
                -0.014_146_884_018,
                0.027_102_114_094,
                0.035_819_311_407,
                0.083_101_265_960,
                0.248_830_354_592,
                0.420_329_264_162,
                0.357_246_122_699,
                0.300_824_126_077,
                0.263_412_823_558,
                0.143_849_812_150,
                0.121_313_049_197,
                0.106_648_055_400,
                0.039_032_263_987,
                -0.089_263_699_230,
                -0.141_617_443_346,
                -0.090_798_431_481,
                -0.198_963_294_175,
                -0.119_170_274_540,
                -0.223_648_442_594,
                -0.278_298_010_814,
            ],
        );
        let want_signal = expected(
            7,
            &[
                0.014_296_343_537,
                0.018_978_756_681,
                0.019_496_715_778,
                0.002_674_915_880,
                0.014_888_514_987,
                0.025_353_913_197,
                0.054_227_589_579,
                0.151_528_972_085,
                0.285_929_118_124,
                0.321_587_620_411,
                0.311_205_873_244,
                0.287_309_348_401,
                0.215_579_580_276,
                0.168_446_314_736,
                0.137_547_185_068,
                0.088_289_724_528,
                -0.000_486_987_351,
                -0.071_052_215_349,
                -0.080_925_323_415,
                -0.139_944_308_795,
                -0.129_557_291_668,
                -0.176_602_867_131,
                -0.227_450_438_972,
            ],
        );

        assert_series(&result.macd, &want_line, EPS);
        assert_series(&result.signal, &want_signal, EPS);
    }

    #[test]
    fn macd_default_periods_have_the_expected_first_defined_indices() {
        let closes: Vec<f64> = (0..60)
            .map(|i| 100.0 + (f64::from(i) * 0.37).sin())
            .collect();
        let result = macd(&candles(&closes), 12, 26, 9);
        assert_eq!(result.macd.len(), closes.len());
        assert_eq!(result.signal.len(), closes.len());
        assert_eq!(first_defined(&result.macd), Some(25));
        assert_eq!(first_defined(&result.signal), Some(33));

        let short = macd(&candles(&closes[..34]), 12, 26, 9);
        assert_eq!(first_defined(&short.signal), Some(33));
        let shorter = macd(&candles(&closes[..33]), 12, 26, 9);
        assert_eq!(first_defined(&shorter.macd), Some(25));
        assert!(shorter.signal.iter().all(Option::is_none));
    }

    #[test]
    fn macd_signal_seed_is_the_mean_of_the_first_defined_macd_values() {
        let result = macd(&candles(&MA_SERIES), 3, 6, 3);
        let seed = result.macd[5..=7]
            .iter()
            .map(|v| v.expect("defined"))
            .sum::<f64>()
            / 3.0;
        assert!((result.signal[7].unwrap() - seed).abs() < EPS);
    }

    #[test]
    fn macd_handles_degenerate_inputs() {
        let empty = macd(&[], 12, 26, 9);
        assert!(empty.macd.is_empty() && empty.signal.is_empty());

        let one = macd(&candles(&[42.0]), 12, 26, 9);
        assert_series(&one.macd, &[None], EPS);
        assert_series(&one.signal, &[None], EPS);

        for (fast, slow, signal) in [(0, 26, 9), (12, 0, 9), (12, 26, 0)] {
            let result = macd(&candles(&MA_SERIES), fast, slow, signal);
            assert!(
                result.signal.iter().all(Option::is_none),
                "{fast}/{slow}/{signal}"
            );
        }

        let long = macd(&candles(&MA_SERIES), 40, 50, 9);
        assert!(long.macd.iter().all(Option::is_none));
        assert!(long.signal.iter().all(Option::is_none));

        let edge = macd(&candles(&MA_SERIES), 12, MA_SERIES.len(), 9);
        assert_eq!(first_defined(&edge.macd), Some(MA_SERIES.len() - 1));
        assert!(edge.signal.iter().all(Option::is_none));
    }

    #[test]
    fn macd_with_equal_periods_is_a_zero_line() {
        let result = macd(&candles(&MA_SERIES), 10, 10, 3);
        for value in result.macd.iter().skip(9) {
            assert!(value.unwrap().abs() < EPS);
        }
        for value in result.signal.iter().skip(11) {
            assert!(value.unwrap().abs() < EPS);
        }
    }

    #[test]
    fn bollinger_uses_population_not_sample_stddev() {
        let closes = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let bands = bollinger(&candles(&closes), 8, 2.0);
        assert_eq!(first_defined(&bands.middle), Some(7));
        assert!((bands.middle[7].unwrap() - 5.0).abs() < EPS);
        assert!((bands.upper[7].unwrap() - 9.0).abs() < EPS);
        assert!((bands.lower[7].unwrap() - 1.0).abs() < EPS);
    }

    #[test]
    fn bollinger_matches_worked_twenty_period_example() {
        let bands = bollinger(&candles(&MA_SERIES), 20, 2.0);
        let want_middle = expected(
            19,
            &[
                22.715_500, 22.793_000, 22.877_000, 22.955_500, 23.006_500, 23.052_500, 23.112_500,
                23.135_000, 23.168_500, 23.176_500, 23.170_500,
            ],
        );
        let want_upper = expected(
            19,
            &[
                24.126_052_728_543,
                24.266_066_189_959,
                24.393_892_876_903,
                24.461_746_659_747,
                24.471_413_308_015,
                24.467_644_868_909,
                24.466_543_943_157,
                24.443_839_180_343,
                24.437_125_634_299,
                24.423_436_646_346,
                24.435_466_007_448,
            ],
        );
        let want_lower = expected(
            19,
            &[
                21.304_947_271_457,
                21.319_933_810_041,
                21.360_107_123_097,
                21.449_253_340_253,
                21.541_586_691_985,
                21.637_355_131_091,
                21.758_456_056_843,
                21.826_160_819_657,
                21.899_874_365_701,
                21.929_563_353_654,
                21.905_533_992_552,
            ],
        );
        assert_series(&bands.middle, &want_middle, EPS);
        assert_series(&bands.upper, &want_upper, EPS);
        assert_series(&bands.lower, &want_lower, EPS);
    }

    #[test]
    fn bollinger_middle_band_is_the_sma() {
        let candles = candles(&MA_SERIES);
        assert_series(
            &bollinger(&candles, 20, 2.0).middle,
            &sma(&candles, 20),
            EPS,
        );
    }

    #[test]
    fn bollinger_of_flat_series_has_zero_width() {
        let bands = bollinger(&candles(&[7.5; 25]), 20, 2.0);
        for i in 19..25 {
            assert!((bands.middle[i].unwrap() - 7.5).abs() < EPS);
            assert!((bands.upper[i].unwrap() - 7.5).abs() < EPS);
            assert!((bands.lower[i].unwrap() - 7.5).abs() < EPS);
        }
    }

    #[test]
    fn bollinger_is_aligned_to_the_input() {
        let candles = candles(&MA_SERIES);
        for period in [1usize, 2, 20, 30] {
            let bands = bollinger(&candles, period, 2.0);
            assert_eq!(bands.middle.len(), candles.len(), "period {period}");
            assert_eq!(bands.upper.len(), candles.len(), "period {period}");
            assert_eq!(bands.lower.len(), candles.len(), "period {period}");
            for series in [&bands.middle, &bands.upper, &bands.lower] {
                assert_eq!(first_defined(series), Some(period - 1), "period {period}");
            }
        }
    }

    #[test]
    fn bollinger_handles_degenerate_inputs() {
        let empty = bollinger(&[], 20, 2.0);
        assert!(empty.middle.is_empty() && empty.upper.is_empty() && empty.lower.is_empty());

        let one = bollinger(&candles(&[42.0]), 1, 2.0);
        assert_series(&one.middle, &[Some(42.0)], EPS);
        assert_series(&one.upper, &[Some(42.0)], EPS);
        assert_series(&one.lower, &[Some(42.0)], EPS);
        assert_series(&bollinger(&candles(&[42.0]), 2, 2.0).middle, &[None], EPS);

        let candles = candles(&MA_SERIES);
        for (period, mult) in [(0usize, 2.0), (31, 2.0)] {
            let bands = bollinger(&candles, period, mult);
            assert!(bands.middle.iter().all(Option::is_none), "period {period}");
            assert!(bands.upper.iter().all(Option::is_none), "period {period}");
            assert!(bands.lower.iter().all(Option::is_none), "period {period}");
        }

        let full = bollinger(&candles, MA_SERIES.len(), 2.0);
        assert_eq!(first_defined(&full.middle), Some(MA_SERIES.len() - 1));
        assert_eq!(first_defined(&full.upper), Some(MA_SERIES.len() - 1));

        let flat = bollinger(&candles, 20, 0.0);
        for i in 19..MA_SERIES.len() {
            assert!((flat.upper[i].unwrap() - flat.middle[i].unwrap()).abs() < EPS);
            assert!((flat.lower[i].unwrap() - flat.middle[i].unwrap()).abs() < EPS);
        }
    }

    #[test]
    fn no_indicator_returns_non_finite_values_for_extreme_closes() {
        let mut closes = MA_SERIES.to_vec();
        closes[15] = 1.0e308;
        closes.push(f64::MAX);
        let candles = candles(&closes);

        let mut series: Vec<Vec<Option<f64>>> = vec![
            sma(&candles, 20),
            sma(&candles, 1),
            ema(&candles, 12),
            ema(&candles, 1),
            rsi(&candles, 14),
            rsi(&candles, 1),
        ];
        let macd = macd(&candles, 12, 26, 9);
        series.extend([macd.macd, macd.signal]);
        let bands = bollinger(&candles, 20, 2.0);
        series.extend([bands.middle, bands.upper, bands.lower]);

        for (which, values) in series.iter().enumerate() {
            assert_eq!(values.len(), candles.len(), "series {which}");
            for (i, value) in values.iter().enumerate() {
                if let Some(value) = value {
                    assert!(value.is_finite(), "series {which} index {i}: {value}");
                }
            }
        }
    }

    #[test]
    fn non_finite_closes_never_surface_in_outputs() {
        let mut closes = MA_SERIES.to_vec();
        closes[3] = f64::NAN;
        closes[7] = f64::INFINITY;
        let candles = candles(&closes);

        let mut series = vec![
            sma(&candles, 5),
            ema(&candles, 5),
            rsi(&candles, 14),
            bollinger(&candles, 20, 2.0).upper,
        ];
        series.push(macd(&candles, 3, 6, 3).signal);
        for values in &series {
            assert_eq!(values.len(), candles.len());
            for value in values.iter().flatten() {
                assert!(value.is_finite(), "leaked {value}");
            }
        }
    }

    fn arb_closes() -> impl Strategy<Value = Vec<f64>> {
        prop::collection::vec(0.01f64..1.0e6, 0..40)
    }

    proptest! {
        #[test]
        fn sma_lies_within_its_window(closes in arb_closes(), period in 1usize..25) {
            let series = sma(&candles(&closes), period);
            prop_assert_eq!(series.len(), closes.len());
            for (i, value) in series.iter().enumerate() {
                let Some(value) = value else { continue };
                let window = &closes[i + 1 - period..=i];
                let min = window.iter().copied().fold(f64::INFINITY, f64::min);
                let max = window.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let slack = 1e-9 * max.abs().max(1.0);
                prop_assert!(*value >= min - slack, "index {}: {} < min {}", i, value, min);
                prop_assert!(*value <= max + slack, "index {}: {} > max {}", i, value, max);
            }
        }

        #[test]
        fn bollinger_bands_are_ordered(
            closes in arb_closes(),
            period in 1usize..25,
            mult in 0.0f64..5.0,
        ) {
            let bands = bollinger(&candles(&closes), period, mult);
            prop_assert_eq!(bands.middle.len(), closes.len());
            prop_assert_eq!(bands.upper.len(), closes.len());
            prop_assert_eq!(bands.lower.len(), closes.len());
            for i in 0..closes.len() {
                if let (Some(lower), Some(middle), Some(upper)) =
                    (bands.lower[i], bands.middle[i], bands.upper[i])
                {
                    prop_assert!(lower <= middle, "index {}: {} > {}", i, lower, middle);
                    prop_assert!(middle <= upper, "index {}: {} > {}", i, middle, upper);
                }
            }
        }

        #[test]
        fn rsi_stays_within_bounds(closes in arb_closes(), period in 1usize..20) {
            let series = rsi(&candles(&closes), period);
            prop_assert_eq!(series.len(), closes.len());
            for (i, value) in series.iter().enumerate() {
                let Some(value) = value else { continue };
                prop_assert!(value.is_finite(), "index {}: {}", i, value);
                prop_assert!((0.0..=100.0).contains(value), "index {}: {}", i, value);
            }
        }

        #[test]
        fn every_indicator_is_aligned(closes in arb_closes(), period in 0usize..30) {
            let candles = candles(&closes);
            let len = closes.len();
            let expect_first = if period > 0 && period <= len {
                Some(period - 1)
            } else {
                None
            };

            let series = sma(&candles, period);
            prop_assert_eq!(series.len(), len);
            prop_assert_eq!(first_defined(&series), expect_first);

            let series = ema(&candles, period);
            prop_assert_eq!(series.len(), len);
            prop_assert_eq!(first_defined(&series), expect_first);

            let series = rsi(&candles, period);
            prop_assert_eq!(series.len(), len);
            if let Some(idx) = first_defined(&series) {
                prop_assert!(period > 0 && idx >= period && idx < len);
            }
            #[expect(clippy::float_cmp, reason = "any exact close change defines a non-flat RSI seed")]
            let seed_moves = period > 0
                && period < len
                && closes[..=period].windows(2).any(|w| w[0] != w[1]);
            if seed_moves {
                prop_assert_eq!(first_defined(&series), Some(period));
            }

            let bands = bollinger(&candles, period, 2.0);
            prop_assert_eq!(bands.middle.len(), len);
            prop_assert_eq!(first_defined(&bands.middle), expect_first);
            prop_assert_eq!(first_defined(&bands.upper), expect_first);
            prop_assert_eq!(first_defined(&bands.lower), expect_first);
        }
    }
}
