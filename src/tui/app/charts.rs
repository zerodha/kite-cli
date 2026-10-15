use super::panes::holdings::holding_identity;
use super::panes::orders::order_identity;
use super::panes::positions::position_identity;
use super::{App, ChartTarget, PaneId};
use crate::tui::candles::{CandleStore, Roll, SeriesKey};
use crate::tui::data::{self, ChartInterval};
use crate::tui::pane_state::{ChartBinding, ChartPane};
use kiteconnect::ticker::Tick;
use std::time::SystemTime;

impl App {
    pub(in crate::tui::app) fn resolve_chart_binding(
        &self,
        binding: ChartBinding,
    ) -> Option<ChartTarget> {
        match binding {
            ChartBinding::UnderlyingOf(pane) => {
                let chain = self.chain_pane_of(pane)?;
                let token = chain.spot_token()?;
                let underlying = chain.underlying();
                Some(ChartTarget {
                    exchange: "NSE".to_string(),
                    tradingsymbol: underlying.to_string(),
                    instrument_token: token,
                    row_identity: format!("underlying:{underlying}"),
                })
            }
            ChartBinding::SelectionOf(pane) => match pane {
                PaneId::TradingWatch => {
                    self.watch_instance(pane)?
                        .selected(&self.watch)
                        .map(|item| ChartTarget {
                            exchange: item.exchange.clone(),
                            tradingsymbol: item.tradingsymbol.clone(),
                            instrument_token: item.instrument_token,
                            row_identity: format!("watch:{}", item.instrument_token),
                        })
                }
                PaneId::PortfolioPositions => {
                    self.selected_position_in(pane).map(|position| ChartTarget {
                        exchange: position.exchange.clone(),
                        tradingsymbol: position.tradingsymbol.clone(),
                        instrument_token: position.instrument_token,
                        row_identity: position_identity(position),
                    })
                }
                PaneId::OrdersBook | PaneId::TradingOrders => {
                    self.selected_order_in(pane).map(|order| ChartTarget {
                        exchange: order.exchange.clone(),
                        tradingsymbol: order.tradingsymbol.clone(),
                        instrument_token: order.instrument_token,
                        row_identity: order_identity(order),
                    })
                }
                PaneId::PortfolioHoldings => {
                    self.selected_holding_in(pane).map(|holding| ChartTarget {
                        exchange: holding.exchange.clone(),
                        tradingsymbol: holding.tradingsymbol.clone(),
                        instrument_token: holding.instrument_token,
                        row_identity: holding_identity(holding),
                    })
                }

                _ => None,
            },
        }
    }

    #[cfg(test)]
    pub fn want_candles_for_test(&mut self, key: SeriesKey) {
        self.candle_requests.want(key, false);
    }

    pub(in crate::tui::app) fn refresh_linked_charts(&mut self) {
        for pane in [PaneId::TradingChart, PaneId::DerivativesChart] {
            let Some(binding) = self.chart_of(pane).map(ChartPane::binding) else {
                continue;
            };
            let resolved = self.resolve_chart_binding(binding);
            let chart = match pane {
                PaneId::TradingChart => &mut self.trading_chart,
                _ => &mut self.derivatives_chart,
            };
            chart.resolve(resolved);
        }
    }

    pub(in crate::tui::app) fn publish_chart_target(&mut self) {
        let resolved = self.resolve_chart_binding(self.chart_pane.binding());
        self.chart_pane.resolve(resolved);
    }

    pub(in crate::tui::app) fn ensure_candles(&mut self, immediate: bool) {
        self.refresh_linked_charts();
        let wanted = self.demanded_series();
        self.candle_requests.retain(|key| wanted.contains(&key));
        for key in wanted {
            if !self.candles.contains(key) {
                self.candle_requests.want(key, immediate);
            }
        }
        self.pump_candles();
    }

    fn prune_candle_requests(&mut self) {
        let wanted = self.demanded_series();
        self.candle_requests.retain(|key| wanted.contains(&key));
    }

    fn demanded_series(&self) -> Vec<SeriesKey> {
        let mut keys: Vec<SeriesKey> = Vec::new();
        let mut want = |key: Option<SeriesKey>| {
            if let Some(key) = key
                && !keys.contains(&key)
            {
                keys.push(key);
            }
        };
        want(self.open_chart().and_then(ChartPane::key));
        for pane in self.workspace.panes().iter().copied() {
            if matches!(pane, PaneId::TradingChart | PaneId::DerivativesChart) {
                want(self.chart_of(pane).and_then(ChartPane::key));
            }
        }
        keys
    }

    pub(in crate::tui::app) fn pump_candles(&mut self) {
        if let Some(key) = self.candle_requests.take_next() {
            self.spawn_candles(key);
        }
    }

    fn spawn_candles(&mut self, key: SeriesKey) {
        data::spawn_fetch_candles(
            self.client.clone(),
            self.events_tx.clone(),
            key.token,
            key.interval,
        );
    }

    pub(in crate::tui::app) fn tick_candles(&mut self) {
        self.prune_candle_requests();
        self.pump_candles();
    }

    pub fn candle_store(&self) -> &CandleStore {
        &self.candles
    }

    pub fn candles_loading(&self, key: SeriesKey) -> bool {
        self.candle_requests.is_loading(key)
    }
}

pub(in crate::tui::app) fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn interval_secs(interval: ChartInterval) -> u64 {
    match interval {
        ChartInterval::M1 => 60,
        ChartInterval::M5 => 300,
        ChartInterval::M15 => 900,
        ChartInterval::H1 => 3_600,
        ChartInterval::Day => 86_400,
    }
}

pub(in crate::tui::app) fn candle_tick_epoch(tick: &Tick, wall_epoch: u64) -> u64 {
    let event_time = if tick.is_index {
        tick.exchange_time.or(tick.last_trade_time)
    } else {
        tick.last_trade_time.or(tick.exchange_time)
    };
    event_time.map_or(wall_epoch, u64::from)
}

pub(in crate::tui::app) fn candle_bucket(
    last_epoch: i64,
    tick_epoch: u64,
    interval: ChartInterval,
) -> Roll {
    let step = interval_secs(interval);
    let tick_bucket = (tick_epoch + 19_800) / step;
    let last_bucket =
        (last_epoch + 19_800).div_euclid(i64::try_from(step).expect("chart interval fits i64"));
    match (i64::try_from(tick_bucket).expect("chart interval bucket fits i64")).cmp(&last_bucket) {
        std::cmp::Ordering::Less => Roll::Skip,
        std::cmp::Ordering::Equal => Roll::Extend,
        std::cmp::Ordering::Greater => Roll::Open(tick_bucket * step - 19_800),
    }
}

#[cfg(test)]
mod tests {
    use super::super::Preset;
    use super::super::test_support::{char_key, sample_order, test_app};
    use crate::tui::candles::SeriesKey;
    use crate::tui::data::{ChartInterval, DataEvent};
    use crate::tui::workspace::PaneId;

    #[tokio::test]
    async fn a_hidden_inline_chart_demands_no_candles() {
        let mut app = test_app();
        let mut order = sample_order("COMPLETE");
        order.tradingsymbol = "TCS".into();
        order.instrument_token = 2_953_217;
        app.orders = Some(vec![order.clone()]);
        app.show_pane(PaneId::OrdersBook);
        app.on_key(char_key('c'));
        let key = SeriesKey::new(2_953_217, app.chart_pane.interval());
        assert_eq!(app.candle_requests.inflight(), Some(key), "c fetches");

        app.switch_workspace(Preset::Trading);
        app.on_data(DataEvent::Candles {
            token: key.token,
            interval: key.interval,
            result: Ok(Vec::new()),
        });
        app.on_data(DataEvent::Orders(Ok(vec![order])));
        assert_eq!(app.candle_requests.inflight(), None);
        assert!(
            !app.candle_requests.queued().contains(&key),
            "the Orders poll re-queued a chart that is not on screen"
        );
    }

    #[test]
    fn candle_bucket_classifies_earlier_current_and_later_ticks() {
        use super::{Roll, candle_bucket};

        let bucket_start = 1_704_080_700u64;
        let last_epoch = 1_704_080_700;
        assert_eq!(
            candle_bucket(last_epoch, bucket_start - 60, ChartInterval::M1),
            Roll::Skip
        );
        assert_eq!(
            candle_bucket(last_epoch, bucket_start + 30, ChartInterval::M1),
            Roll::Extend
        );
        assert_eq!(
            candle_bucket(last_epoch, bucket_start + 60, ChartInterval::M1),
            Roll::Open(1_704_080_760)
        );
        assert_eq!(
            candle_bucket(last_epoch, bucket_start + 60, ChartInterval::M5),
            Roll::Extend
        );
        assert_eq!(
            candle_bucket(last_epoch, bucket_start + 300, ChartInterval::M5),
            Roll::Open(1_704_081_000)
        );
    }

    #[test]
    fn candle_bucket_day_interval_rejects_prior_days() {
        use super::{Roll, candle_bucket};
        let last_epoch = 1_704_080_700;
        let same_day = 1_704_080_700u64;
        assert_eq!(
            candle_bucket(last_epoch, same_day - 86_400, ChartInterval::Day),
            Roll::Skip
        );
        assert_eq!(
            candle_bucket(last_epoch, same_day, ChartInterval::Day),
            Roll::Extend
        );
        assert!(matches!(
            candle_bucket(last_epoch, same_day + 86_400, ChartInterval::Day),
            Roll::Open(_)
        ));
    }

    #[test]
    fn candle_tick_time_uses_trade_time_for_scrips_and_exchange_time_for_indices() {
        use super::candle_tick_epoch;
        use kiteconnect::ticker::Tick;

        let mut regular = Tick::default();
        regular.last_trade_time = Some(100);
        regular.exchange_time = Some(200);
        assert_eq!(candle_tick_epoch(&regular, 300), 100);

        let mut index = Tick::default();
        index.is_index = true;
        index.last_trade_time = Some(100);
        index.exchange_time = Some(200);
        assert_eq!(candle_tick_epoch(&index, 300), 200);
        assert_eq!(candle_tick_epoch(&Tick::default(), 300), 300);
    }
}
