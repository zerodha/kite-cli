use super::super::{App, Modal, Side, ToastLevel, cmp_desc};
use crate::tui::data::{self, Holding};

use crate::tui::pane_state::{self, HoldingKey};
use crate::tui::ticket::{ConfirmState, PendingAction};
use crate::tui::workspace::PaneId;
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldingsSort {
    #[default]
    PnlPct,
    PnlAbs,
    Value,
    DayPct,
    Symbol,
}

impl HoldingsSort {
    pub const ALL: [Self; 5] = [
        Self::PnlPct,
        Self::PnlAbs,
        Self::Value,
        Self::DayPct,
        Self::Symbol,
    ];

    pub(in crate::tui::app) fn next(self) -> Self {
        match self {
            HoldingsSort::PnlPct => HoldingsSort::PnlAbs,
            HoldingsSort::PnlAbs => HoldingsSort::Value,
            HoldingsSort::Value => HoldingsSort::DayPct,
            HoldingsSort::DayPct => HoldingsSort::Symbol,
            HoldingsSort::Symbol => HoldingsSort::PnlPct,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HoldingsSort::PnlPct => "P&L% ▼",
            HoldingsSort::PnlAbs => "P&L ▼",
            HoldingsSort::Value => "value ▼",
            HoldingsSort::DayPct => "day% ▼",
            HoldingsSort::Symbol => "symbol ▲",
        }
    }
}

pub(in crate::tui::app) fn holding_key(holding: &Holding) -> HoldingKey {
    HoldingKey {
        exchange: holding.exchange.clone(),
        symbol: holding.tradingsymbol.clone(),
    }
}

pub(in crate::tui::app) fn holding_identity(holding: &Holding) -> String {
    format!("holding:{}:{}", holding.exchange, holding.tradingsymbol)
}

pub fn holdings_order(
    holdings: &[Holding],
    live: impl Fn(u32) -> Option<f64>,
    mode: HoldingsSort,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..holdings.len()).collect();
    if mode == HoldingsSort::Symbol {
        order.sort_by(|&a, &b| holdings[a].tradingsymbol.cmp(&holdings[b].tradingsymbol));
    } else {
        let key = |h: &Holding| -> f64 {
            let ltp = live(h.instrument_token).unwrap_or(h.last_price);
            #[expect(
                clippy::cast_precision_loss,
                reason = "display sorting uses the same approximate quantity as the rendered value"
            )]
            let qty = crate::tui::data::holding_total_quantity(h) as f64;
            match mode {
                HoldingsSort::PnlAbs => crate::tui::data::holding_live_pnl(h, Some(ltp)),
                HoldingsSort::Value => ltp * qty,

                HoldingsSort::PnlPct if h.average_price > 0.0 => {
                    (ltp - h.average_price) / h.average_price * 100.0
                }
                HoldingsSort::DayPct if h.close_price > 0.0 => {
                    (ltp - h.close_price) / h.close_price * 100.0
                }
                _ => f64::NEG_INFINITY,
            }
        };
        let keys: Vec<f64> = holdings.iter().map(key).collect();
        order.sort_by(|&a, &b| cmp_desc(keys[a], keys[b]));
    }
    order
}

pub fn overview_holdings_order(
    holdings: &[Holding],
    live: impl Fn(u32) -> Option<f64>,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..holdings.len()).collect();
    #[expect(
        clippy::cast_precision_loss,
        reason = "display sorting uses the same approximate quantity as the rendered value"
    )]
    let value = |h: &Holding| {
        live(h.instrument_token).unwrap_or(h.last_price)
            * crate::tui::data::holding_total_quantity(h) as f64
    };
    let values: Vec<f64> = holdings.iter().map(value).collect();
    order.sort_by(|&a, &b| cmp_desc(values[a], values[b]));
    order
}

impl App {
    pub fn holdings_pane_of(&self, pane: PaneId) -> Option<&pane_state::HoldingsPane> {
        (pane == PaneId::PortfolioHoldings).then_some(&self.books.portfolio_holdings)
    }

    pub fn holdings_authorisation_available(&self) -> bool {
        self.profile
            .as_ref()
            .is_some_and(crate::tui::data::profile_requires_holdings_authorisation)
    }

    pub(in crate::tui::app) fn key_holdings(&mut self, pane: PaneId, key: KeyEvent) {
        if key.code == KeyCode::Char('a') {
            if self.trading_disabled() {
                return;
            }
            if !self.holdings_authorisation_available() {
                let message = match self.profile.as_ref() {
                    None => "account authorisation status is still loading".to_string(),
                    Some(_) => "CDSL authorisation is not required for this account".to_string(),
                };
                self.toast(ToastLevel::Info, message);
                return;
            }
            self.modals
                .push(Modal::Confirm(Box::new(ConfirmState::plain(
                    "Authorise holdings with CDSL".to_string(),
                    vec![
                        ("Holdings".to_string(), "all demat holdings".to_string()),
                        (
                            "Next".to_string(),
                            "opens Kite Web for CDSL TPIN/OTP authorisation".to_string(),
                        ),
                        (
                            "Security".to_string(),
                            "TPIN and OTP are entered only in the external CDSL flow".to_string(),
                        ),
                    ],
                    PendingAction::AuthoriseHoldings,
                ))));
            return;
        }
        if !matches!(key.code, KeyCode::Char('b' | 's' | 'g' | 'B')) {
            return;
        }
        let Some(holding) = self.selected_holding_in(pane).cloned() else {
            return;
        };
        let free_quantity = data::holding_free_quantity(&holding);
        let mtf_only =
            free_quantity == 0 && holding.mtf.as_ref().is_some_and(|mtf| mtf.quantity > 0);
        let product = if mtf_only { "MTF" } else { "CNC" };
        let sell_quantity = if mtf_only {
            holding.mtf.as_ref().map_or(0, |mtf| mtf.quantity)
        } else {
            free_quantity
        };
        match key.code {
            KeyCode::Char('b' | 's') => {
                let side = if key.code == KeyCode::Char('b') {
                    Side::Buy
                } else {
                    Side::Sell
                };
                let quantity = if side == Side::Sell { sell_quantity } else { 0 };
                self.open_ticket(
                    side,
                    holding.exchange,
                    holding.tradingsymbol,
                    holding.instrument_token,
                    quantity,
                    Some(product),
                );
            }
            KeyCode::Char('g') => {
                let quantity = sell_quantity;
                self.open_gtt_ticket(
                    Side::Sell,
                    holding.exchange,
                    holding.tradingsymbol,
                    holding.instrument_token,
                    quantity,
                    Some(product),
                    None,
                    None,
                    None,
                );
            }

            KeyCode::Char('B') => self.open_alert_form(
                &holding.exchange,
                &holding.tradingsymbol,
                holding.instrument_token,
                "",
            ),
            _ => {}
        }
    }

    pub fn holdings_order_in(&self, pane: PaneId) -> Vec<usize> {
        let Some(instance) = self.holdings_pane_of(pane) else {
            return Vec::new();
        };
        let Some(holdings) = self.holdings.as_deref() else {
            return Vec::new();
        };
        match instance.sort() {
            HoldingsSort::Symbol => instance.cached_order(self.holdings_gen, |sort| {
                holdings_order(holdings, |token| self.live_ltp(token), sort)
            }),
            sort => holdings_order(holdings, |token| self.live_ltp(token), sort),
        }
    }

    pub fn overview_holdings_display_order(&self) -> Vec<usize> {
        match &self.holdings {
            Some(holdings) => overview_holdings_order(holdings, |t| self.live_ltp(t)),
            None => Vec::new(),
        }
    }

    pub fn overview_holdings_row(&self, display: &[usize]) -> usize {
        let holdings = self.holdings.as_deref().unwrap_or_default();
        self.books
            .overview_holdings
            .display_row(display, |i| holdings.get(i).map(holding_key))
    }

    pub(in crate::tui::app) fn selected_holding_in(&self, pane: PaneId) -> Option<&Holding> {
        let instance = self.holdings_pane_of(pane)?;
        let order = self.holdings_order_in(pane);
        let holdings = self.holdings.as_deref().unwrap_or_default();
        instance
            .target(&order, |i| holdings.get(i).map(holding_key))
            .and_then(|natural| holdings.get(natural))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, char_key, holding, holdings_app, ltp_tick, press_at, region_of, render, test_app,
    };
    use super::super::super::{HoldingsSort, Modal, ui};
    use super::{holdings_order, overview_holdings_order};
    use crate::tui::workspace::PaneId;
    use kiteconnect::ticker::TickerEvent;

    fn no_live(_token: u32) -> Option<f64> {
        None
    }

    #[test]
    fn holdings_sort_pnl_pct_descends() {
        let hs = [
            holding("AAA", 1, 100.0, 110.0, 100.0, 10),
            holding("BBB", 2, 100.0, 130.0, 100.0, 1),
            holding("CCC", 3, 100.0, 90.0, 100.0, 10),
        ];
        assert_eq!(
            holdings_order(&hs, no_live, HoldingsSort::PnlPct),
            vec![1, 0, 2]
        );
    }

    #[test]
    fn holdings_sort_pnl_abs_and_value_descend() {
        let hs = [
            holding("AAA", 1, 100.0, 110.0, 100.0, 10),
            holding("BBB", 2, 100.0, 130.0, 100.0, 1),
            holding("CCC", 3, 100.0, 90.0, 100.0, 10),
        ];
        assert_eq!(
            holdings_order(&hs, no_live, HoldingsSort::PnlAbs),
            vec![0, 1, 2]
        );
        assert_eq!(
            holdings_order(&hs, no_live, HoldingsSort::Value),
            vec![0, 2, 1]
        );
    }

    #[test]
    fn holdings_sort_symbol_ascends() {
        let hs = [
            holding("CCC", 1, 100.0, 90.0, 100.0, 10),
            holding("AAA", 2, 100.0, 110.0, 100.0, 10),
            holding("BBB", 3, 100.0, 130.0, 100.0, 10),
        ];
        assert_eq!(
            holdings_order(&hs, no_live, HoldingsSort::Symbol),
            vec![1, 2, 0]
        );
    }

    #[test]
    fn holdings_sort_is_stable_for_equal_keys() {
        let hs = [
            holding("AAA", 1, 100.0, 110.0, 100.0, 10),
            holding("BBB", 2, 100.0, 110.0, 100.0, 10),
            holding("CCC", 3, 100.0, 110.0, 100.0, 10),
        ];
        assert_eq!(
            holdings_order(&hs, no_live, HoldingsSort::PnlPct),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn holdings_sort_sinks_undefined_metric() {
        let hs = [
            holding("AAA", 1, 0.0, 110.0, 100.0, 10),
            holding("BBB", 2, 100.0, 130.0, 100.0, 10),
        ];
        assert_eq!(
            holdings_order(&hs, no_live, HoldingsSort::PnlPct),
            vec![1, 0]
        );
    }

    #[test]
    fn holdings_sort_then_index_map_round_trips() {
        let hs = [
            holding("AAA", 1, 100.0, 110.0, 100.0, 10),
            holding("BBB", 2, 100.0, 130.0, 100.0, 1),
            holding("CCC", 3, 100.0, 90.0, 100.0, 10),
        ];
        let order = holdings_order(&hs, no_live, HoldingsSort::PnlPct);

        assert_eq!(hs[order[0]].tradingsymbol, "BBB");
        assert_eq!(hs[order[2]].tradingsymbol, "CCC");
    }

    #[test]
    fn holdings_sort_uses_live_price_over_last() {
        let hs = [
            holding("AAA", 1, 100.0, 110.0, 100.0, 10),
            holding("BBB", 2, 100.0, 105.0, 100.0, 10),
        ];

        let live = |token: u32| (token == 2).then_some(200.0);
        assert_eq!(holdings_order(&hs, live, HoldingsSort::PnlPct), vec![1, 0]);
    }

    #[test]
    fn holdings_authorisation_is_only_offered_to_consent_accounts() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.profile = Some({
            let mut row = crate::tui::data::test_models::profile();
            row.meta = {
                let mut row = crate::tui::data::test_models::profile_meta();
                row.poa = Some("ddpi".to_string());
                row
            };
            row
        });
        app.on_key(char_key('a'));
        assert!(app.modals.is_empty());

        app.profile = Some({
            let mut row = crate::tui::data::test_models::profile();
            row.meta = {
                let mut row = crate::tui::data::test_models::profile_meta();
                row.poa = Some("consent".to_string());
                row
            };
            row
        });
        app.on_key(char_key('a'));
        assert!(matches!(app.modals.last(), Some(Modal::Confirm(_))));
    }

    #[test]
    fn overview_holdings_order_includes_pledged_and_ranks_by_total_value() {
        let mut fully_pledged = holding("PLEDGED", 1, 100.0, 100.0, 100.0, 0);
        fully_pledged.collateral_quantity = 50;
        let mixed = {
            let mut h = holding("MIXED", 2, 100.0, 100.0, 100.0, 5);
            h.collateral_quantity = 20;
            h
        };
        let free = holding("FREE", 3, 100.0, 200.0, 100.0, 10);
        let holdings = [fully_pledged, mixed, free];

        let order = overview_holdings_order(&holdings, |_| None);

        assert_eq!(order, vec![0, 1, 2]);
    }

    #[test]
    fn the_sort_key_still_cycles_every_mode_in_order() {
        let mut app = holdings_app();
        for expected in [
            HoldingsSort::PnlAbs,
            HoldingsSort::Value,
            HoldingsSort::DayPct,
            HoldingsSort::Symbol,
            HoldingsSort::PnlPct,
        ] {
            app.on_key(char_key('S'));
            assert_eq!(app.books.portfolio_holdings.sort(), expected);
        }
    }

    #[test]
    fn a_row_click_queued_behind_a_reranking_tick_selects_nothing() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.holdings = Some(vec![
            holding("FIRST", 100, 100.0, 120.0, 100.0, 1),
            holding("SECOND", 101, 100.0, 110.0, 100.0, 1),
        ]);
        app.set_selection(0);
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Pane(PaneId::PortfolioHoldings),
                    ..
                }
            )
        });
        assert_eq!(app.holdings_order_in(PaneId::PortfolioHoldings), vec![0, 1]);

        app.on_ws(TickerEvent::Ticks(vec![ltp_tick(101, 150.0)]));
        assert_eq!(app.holdings_order_in(PaneId::PortfolioHoldings), vec![1, 0]);
        press_at(&mut app, rows.rect.x + 2, rows.rect.y);
        let selected = app
            .selected_holding_in(PaneId::PortfolioHoldings)
            .expect("a holding stays selected");
        assert_eq!(
            selected.tradingsymbol, "FIRST",
            "the stale row target selected the holding the tick moved up"
        );
    }

    #[test]
    fn a_row_click_queued_behind_a_repricing_tick_still_selects() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioHoldings);
        app.holdings = Some(vec![
            holding("FIRST", 100, 100.0, 120.0, 100.0, 1),
            holding("SECOND", 101, 100.0, 110.0, 100.0, 1),
        ]);
        app.set_selection(0);
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Pane(PaneId::PortfolioHoldings),
                    ..
                }
            )
        });

        app.on_ws(TickerEvent::Ticks(vec![ltp_tick(101, 115.0)]));
        assert_eq!(app.holdings_order_in(PaneId::PortfolioHoldings), vec![0, 1]);
        press_at(&mut app, rows.rect.x + 2, rows.rect.y + 1);
        let selected = app
            .selected_holding_in(PaneId::PortfolioHoldings)
            .expect("a holding is selected");
        assert_eq!(selected.tradingsymbol, "SECOND");
    }
}
