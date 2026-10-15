use super::{App, PaneId};
use crate::cli::StreamMode;
use crate::tui::subscriptions::{self, Demands, Reserve};
use std::collections::HashMap;

impl App {
    pub fn desired_subscriptions(&self) -> HashMap<u32, StreamMode> {
        self.demands().resolve(subscriptions::TOKEN_BUDGET)
    }

    fn demands(&self) -> Demands {
        let mut demands = Demands::default();

        if let Some(positions) = &self.positions {
            for position in &positions.net {
                demands.want(Reserve::Visible, position.instrument_token, StreamMode::Ltp);
            }
        }
        if let Some(holdings) = &self.holdings {
            for holding in holdings {
                demands.want(Reserve::Visible, holding.instrument_token, StreamMode::Ltp);
            }
        }
        if let Some(auctions) = &self.auctions {
            for auction in auctions {
                demands.want(Reserve::Visible, auction.instrument_token, StreamMode::Ltp);
            }
        }

        demands.absorb(self.watch_pane.demands(
            &self.watch,
            self.workspace.panes().contains(&PaneId::TradingDepth),
        ));

        for pane in self.workspace.panes().iter().copied() {
            match pane {
                PaneId::DerivativesChain => demands.absorb(self.derivatives_chain.demands()),

                PaneId::TradingChart => demands.absorb(self.trading_chart.demands()),
                PaneId::DerivativesChart => demands.absorb(self.derivatives_chart.demands()),
                _ => {}
            }
        }

        if self.open_chart().is_some() {
            demands.absorb(self.chart_pane.demands());
        }
        demands
    }

    pub(in crate::tui::app) fn sync_subscriptions(&mut self) {
        self.refresh_linked_charts();
        let desired = self.desired_subscriptions();

        self.ticks.retain(|token, _| desired.contains_key(token));
        self.flash.retain(|token, _| desired.contains_key(token));
        self.candles.retain(|key| desired.contains_key(&key.token));
        if desired != self.last_subs
            && let Some(ticker) = &self.ticker
            && ticker
                .set_subscriptions(
                    desired
                        .iter()
                        .map(|(token, mode)| (*token, (*mode).into()))
                        .collect(),
                )
                .is_ok()
        {
            self.last_subs = desired;
        }

        self.ensure_candles(false);

        self.ensure_circuits();
    }
}

#[cfg(test)]
mod tests {
    use super::super::navigation::tests::stocked;
    use super::super::test_support::test_app;
    use crate::tui::candles::SeriesKey;
    use crate::tui::data::ChartInterval;
    use crate::tui::marketwatch::WatchItem;
    use crate::tui::workspace::{PaneId, Preset};
    use kiteconnect::ticker::Tick;
    use std::time::Instant;

    #[test]
    fn sync_subscriptions_prunes_undesired_tokens_from_per_token_maps() {
        let mut app = test_app();

        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: 100,
            segment: String::new(),
            ..WatchItem::default()
        });

        app.circuits.insert(100, Some((90.0, 110.0)));
        for token in [100u32, 999u32] {
            app.ticks.insert(token, {
                let mut value = Tick::default();
                value.instrument_token = token;
                value
            });
            app.flash.insert(token, (Instant::now(), 1));
            app.candles
                .insert(SeriesKey::new(token, ChartInterval::M5), vec![]);
        }

        app.sync_subscriptions();

        assert!(app.ticks.contains_key(&100), "still-desired tick kept");
        assert!(!app.ticks.contains_key(&999), "stale tick evicted");
        assert!(app.flash.contains_key(&100), "still-desired flash kept");
        assert!(!app.flash.contains_key(&999), "stale flash evicted");
        assert!(
            app.candles.contains(SeriesKey::new(100, ChartInterval::M5)),
            "still-desired candles kept"
        );
        assert!(
            !app.candles.contains(SeriesKey::new(999, ChartInterval::M5)),
            "stale candles evicted"
        );
    }

    #[test]
    fn a_composite_subscribes_for_every_pane_it_defines() {
        let mut app = stocked();
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingWatch);
        app.move_selection(0);
        let wanted = app.desired_subscriptions();

        assert_eq!(
            wanted.get(&408_065),
            Some(&crate::cli::StreamMode::Full),
            "the linked instrument needs depth: {wanted:?}"
        );
        assert!(wanted.contains_key(&738_561), "and every other watch row");

        assert!(wanted.contains_key(&101), "positions keep their marks live");
    }

    #[test]
    fn the_orders_and_invest_workspaces_need_no_depth() {
        let mut app = stocked();
        for preset in [Preset::Orders, Preset::Invest] {
            app.set_workspace(preset);
            let wanted = app.desired_subscriptions();
            assert_eq!(
                wanted.get(&408_065),
                Some(&crate::cli::StreamMode::Quote),
                "{preset:?}: {wanted:?}"
            );
            assert!(wanted.contains_key(&738_561), "{preset:?}");
            assert!(wanted.contains_key(&101), "{preset:?}");
        }
    }
}
