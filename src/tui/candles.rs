use crate::tui::data::{Candle, ChartInterval};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const MAX_CANDLES: usize = 1000;

const DEBOUNCE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeriesKey {
    pub token: u32,
    pub interval: ChartInterval,
}

impl SeriesKey {
    pub fn new(token: u32, interval: ChartInterval) -> Self {
        Self { token, interval }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Roll {
    Open(u64),
    Extend,
    Skip,
}

struct Series {
    candles: Vec<Candle>,
    generation: u64,
    vol_base: Option<u64>,
}

#[derive(Default)]
pub struct CandleStore {
    series: HashMap<SeriesKey, Series>,
    retired: u64,
}

impl CandleStore {
    pub fn candles(&self, key: SeriesKey) -> Option<&[Candle]> {
        self.series
            .get(&key)
            .map(|series| series.candles.as_slice())
    }

    pub fn contains(&self, key: SeriesKey) -> bool {
        self.series.contains_key(&key)
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.series.len()
    }

    pub fn generation(&self, key: SeriesKey) -> u64 {
        self.series.get(&key).map_or(0, |series| series.generation)
    }

    pub fn insert(&mut self, key: SeriesKey, mut candles: Vec<Candle>) {
        let excess = candles.len().saturating_sub(MAX_CANDLES);
        candles.drain(..excess);
        let generation = self.generation(key).max(self.retired).wrapping_add(1);
        self.series.insert(
            key,
            Series {
                candles,
                generation,
                vol_base: None,
            },
        );
    }

    #[cfg(test)]
    pub fn candles_mut(&mut self, key: SeriesKey) -> Option<&mut Vec<Candle>> {
        self.series.get_mut(&key).map(|series| &mut series.candles)
    }

    #[cfg(test)]
    pub fn remove(&mut self, key: SeriesKey) {
        self.series.remove(&key);
    }

    #[cfg(test)]
    pub fn touch(&mut self, key: SeriesKey) {
        if let Some(series) = self.series.get_mut(&key) {
            series.generation = series.generation.wrapping_add(1);
        }
    }

    pub fn retain(&mut self, keep: impl Fn(SeriesKey) -> bool) {
        let retired = &mut self.retired;
        self.series.retain(|key, series| {
            let kept = keep(*key);
            if !kept {
                *retired = (*retired).max(series.generation);
            }
            kept
        });
    }

    pub fn fold_tick(
        &mut self,
        token: u32,
        price: f64,
        vol_cum: Option<u64>,
        classify: impl Fn(ChartInterval, i64) -> Roll,
    ) -> bool {
        let mut touched = false;
        for (key, series) in &mut self.series {
            if key.token != token {
                continue;
            }
            let Some(last) = series.candles.last() else {
                continue;
            };
            match classify(key.interval, last.date.timestamp()) {
                Roll::Open(ts) => {
                    let date = last.date
                        + Duration::from_secs(
                            ts - u64::try_from(last.date.timestamp())
                                .expect("live candle timestamp is nonnegative"),
                        );
                    series.candles.push(Candle::new(date, price));
                    if series.candles.len() > MAX_CANDLES {
                        let excess = series.candles.len() - MAX_CANDLES;
                        series.candles.drain(0..excess);
                    }
                    series.vol_base = vol_cum;
                    series.generation = series.generation.wrapping_add(1);
                    touched = true;
                }
                Roll::Extend => {
                    let base = series.vol_base;
                    if let Some(last) = series.candles.last_mut() {
                        last.close = price;
                        last.high = last.high.max(price);
                        last.low = last.low.min(price);
                        if let (Some(base), Some(v)) = (base, vol_cum) {
                            last.volume = v.saturating_sub(base);
                        }
                    }
                    series.generation = series.generation.wrapping_add(1);
                    touched = true;
                }
                Roll::Skip => {}
            }
        }
        touched
    }
}

struct Pending {
    key: SeriesKey,
    since: Instant,
    immediate: bool,
}

#[derive(Default)]
pub struct CandleRequests {
    queue: Vec<Pending>,
    inflight: Option<SeriesKey>,
    failed: Vec<SeriesKey>,
}

impl CandleRequests {
    pub fn want(&mut self, key: SeriesKey, immediate: bool) {
        if immediate {
            self.clear_failure(key);
        }
        if self.inflight == Some(key) || self.failed.contains(&key) {
            return;
        }
        if let Some(pending) = self.queue.iter_mut().find(|pending| pending.key == key) {
            pending.immediate |= immediate;
            return;
        }
        self.queue.push(Pending {
            key,
            since: Instant::now(),
            immediate,
        });
    }

    pub fn retain(&mut self, keep: impl Fn(SeriesKey) -> bool) {
        self.queue.retain(|pending| keep(pending.key));
    }

    pub fn clear_failure(&mut self, key: SeriesKey) {
        self.failed.retain(|failed| *failed != key);
    }

    pub fn take_next(&mut self) -> Option<SeriesKey> {
        if self.inflight.is_some() {
            return None;
        }
        let index = self
            .queue
            .iter()
            .position(|pending| pending.immediate || pending.since.elapsed() >= DEBOUNCE)?;
        let pending = self.queue.remove(index);
        self.inflight = Some(pending.key);
        Some(pending.key)
    }

    pub fn finish(&mut self, key: SeriesKey, ok: bool) {
        if self.inflight == Some(key) {
            self.inflight = None;
        }
        if ok {
            self.clear_failure(key);
        } else if !self.failed.contains(&key) {
            self.failed.push(key);
        }
    }

    pub fn is_loading(&self, key: SeriesKey) -> bool {
        self.inflight == Some(key) || self.queue.iter().any(|pending| pending.key == key)
    }

    #[cfg(test)]
    pub fn inflight(&self) -> Option<SeriesKey> {
        self.inflight
    }

    #[cfg(test)]
    pub fn has_failed(&self, key: SeriesKey) -> bool {
        self.failed.contains(&key)
    }

    #[cfg(test)]
    pub fn queued(&self) -> Vec<SeriesKey> {
        self.queue.iter().map(|pending| pending.key).collect()
    }

    #[cfg(test)]
    pub fn settle_for_test(&mut self) {
        let settled = Instant::now()
            .checked_sub(DEBOUNCE)
            .expect("test clock supports debounce backdating");
        for pending in &mut self.queue {
            pending.since = settled;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CandleRequests, CandleStore, Roll, SeriesKey};
    use crate::tui::data::{Candle, ChartInterval};

    fn bar(ts: &str, close: f64) -> Candle {
        crate::tui::data::test_models::candle_at(ts, close, close, close, close, 0)
    }

    const INFY: u32 = 408_065;

    fn stocked() -> CandleStore {
        let mut store = CandleStore::default();
        store.insert(
            SeriesKey::new(INFY, ChartInterval::M5),
            vec![bar("2026-07-22 10:00:00", 100.0)],
        );
        store.insert(
            SeriesKey::new(INFY, ChartInterval::Day),
            vec![bar("2026-07-22", 100.0)],
        );
        store
    }

    #[test]
    fn a_tick_folds_into_every_retained_interval_of_its_token_once() {
        let mut store = stocked();
        let touched = store.fold_tick(INFY, 105.0, Some(500), |interval, _| match interval {
            ChartInterval::M5 => Roll::Open(1_784_694_900),
            _ => Roll::Extend,
        });
        assert!(touched);

        let five = store
            .candles(SeriesKey::new(INFY, ChartInterval::M5))
            .unwrap();
        assert_eq!(five.len(), 2, "the five-minute series opened a bucket");
        assert_eq!(five[1].close.to_bits(), 105.0_f64.to_bits());
        assert_eq!(five[1].volume, 0, "a fresh bucket accrues from zero");

        let day = store
            .candles(SeriesKey::new(INFY, ChartInterval::Day))
            .unwrap();
        assert_eq!(day.len(), 1, "the daily series extended its bar");
        assert_eq!(day[0].close.to_bits(), 105.0_f64.to_bits());
        assert_eq!(day[0].high.to_bits(), 105.0_f64.to_bits());
    }

    #[test]
    fn a_fold_bumps_only_the_series_it_touched() {
        let mut store = stocked();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let day = SeriesKey::new(INFY, ChartInterval::Day);
        let before = (store.generation(five), store.generation(day));

        store.fold_tick(INFY, 105.0, None, |interval, _| {
            if interval == ChartInterval::M5 {
                Roll::Extend
            } else {
                Roll::Skip
            }
        });

        assert_eq!(store.generation(five), before.0 + 1);
        assert_eq!(store.generation(day), before.1, "untouched, so unchanged");
    }

    #[test]
    fn a_refetch_after_pruning_invalidates_the_chart_memo() {
        use crate::tui::indicators::{self, ChartIndicator, SMA_PERIOD};
        use crate::tui::pane_state::{ChartBinding, ChartId, ChartPane};
        use crate::tui::workspace::PaneId;

        let flat = |close: f64| -> Vec<Candle> {
            (0..SMA_PERIOD)
                .map(|minute| bar(&format!("2026-07-22 10:{minute:02}:00"), close))
                .collect()
        };
        let chart = ChartPane::new(
            ChartId::new(1),
            ChartBinding::SelectionOf(PaneId::TradingWatch),
            ChartInterval::M5,
            false,
            ChartIndicator::Sma,
        );
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let mut store = CandleStore::default();
        store.insert(five, flat(100.0));
        let old = chart.indicator_series(INFY, &store).clone();

        store.retain(|_| false);
        store.insert(five, flat(200.0));

        let expected = indicators::series(ChartIndicator::Sma, &flat(200.0));
        assert_ne!(old, expected, "the two histories draw different lines");
        assert_eq!(
            *chart.indicator_series(INFY, &store),
            expected,
            "the memo must recompute over the refetched bars"
        );
    }

    #[test]
    fn a_fold_ignores_series_for_other_tokens() {
        let mut store = stocked();
        let touched = store.fold_tick(738_561, 105.0, None, |_, _| Roll::Extend);
        assert!(!touched);
        assert_eq!(
            (store
                .candles(SeriesKey::new(INFY, ChartInterval::M5))
                .unwrap()[0]
                .close)
                .to_bits(),
            100.0_f64.to_bits()
        );
    }

    #[test]
    fn the_queue_deduplicates_and_keeps_both_charts_wants() {
        let mut requests = CandleRequests::default();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let day = SeriesKey::new(INFY, ChartInterval::Day);

        requests.want(five, true);
        requests.want(five, false);
        requests.want(day, true);
        assert_eq!(requests.queued(), vec![five, day]);

        assert_eq!(requests.take_next(), Some(five));
        assert_eq!(requests.queued(), vec![day]);
        assert_eq!(
            requests.take_next(),
            None,
            "one request in flight at a time"
        );
        assert!(requests.is_loading(day), "still wanted, still loading");

        requests.finish(five, true);
        assert!(!requests.is_loading(five));
        assert_eq!(requests.take_next(), Some(day));
    }

    #[test]
    fn only_a_settled_or_immediate_want_is_released() {
        let mut requests = CandleRequests::default();
        let five = SeriesKey::new(INFY, ChartInterval::M5);

        requests.want(five, false);
        assert_eq!(requests.take_next(), None, "still settling");
        requests.settle_for_test();
        assert_eq!(requests.take_next(), Some(five));

        let mut requests = CandleRequests::default();
        requests.want(five, true);
        assert_eq!(requests.take_next(), Some(five), "asked for outright");
    }

    #[test]
    fn pruning_drops_queued_keys_but_not_the_request_in_flight() {
        let mut requests = CandleRequests::default();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        let day = SeriesKey::new(INFY, ChartInterval::Day);
        requests.want(five, true);
        requests.want(day, true);
        requests.take_next();

        requests.retain(|_| false);
        assert_eq!(requests.queued().len(), 0);
        assert_eq!(requests.inflight(), Some(five));
        assert!(requests.is_loading(five));
    }

    #[test]
    fn a_failed_key_is_requeued_only_by_an_explicit_want() {
        let mut requests = CandleRequests::default();
        let five = SeriesKey::new(INFY, ChartInterval::M5);
        requests.want(five, true);
        requests.take_next();
        requests.finish(five, false);
        assert!(requests.has_failed(five));

        requests.want(five, false);
        assert!(
            requests.queued().is_empty(),
            "a cursor that keeps arriving here must not keep refetching"
        );

        requests.want(five, true);
        assert_eq!(requests.queued(), vec![five], "an explicit want retries");
        assert!(!requests.has_failed(five));
    }
}
