use super::super::{App, Modal, Side, ToastLevel, cmp_desc};
use crate::tui::data::Position;
use crate::tui::pane_state::{self, PositionKey};
use crate::tui::ticket::PositionConversionState;
use crate::tui::workspace::PaneId;
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PositionsSort {
    #[default]
    Pnl,
    Qty,
    DayPct,
    Symbol,
}

impl PositionsSort {
    pub const ALL: [Self; 4] = [Self::Pnl, Self::Qty, Self::DayPct, Self::Symbol];

    pub(in crate::tui::app) fn next(self) -> Self {
        match self {
            PositionsSort::Pnl => PositionsSort::Qty,
            PositionsSort::Qty => PositionsSort::DayPct,
            PositionsSort::DayPct => PositionsSort::Symbol,
            PositionsSort::Symbol => PositionsSort::Pnl,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            PositionsSort::Pnl => "P&L ▼",
            PositionsSort::Qty => "qty ▼",
            PositionsSort::DayPct => "day% ▼",
            PositionsSort::Symbol => "symbol ▲",
        }
    }
}

pub(in crate::tui::app) fn position_key(position: &Position) -> PositionKey {
    PositionKey {
        exchange: position.exchange.clone(),
        symbol: position.tradingsymbol.clone(),
        product: position.product.clone(),
    }
}

pub(in crate::tui::app) fn position_identity(position: &Position) -> String {
    format!(
        "position:{}:{}:{}",
        position.exchange, position.tradingsymbol, position.product
    )
}

pub fn positions_order(
    positions: &[Position],
    live: impl Fn(u32) -> Option<f64>,
    mode: PositionsSort,
) -> Vec<usize> {
    let mut order: Vec<usize> = (0..positions.len()).collect();
    match mode {
        PositionsSort::Symbol => {
            order.sort_by(|&a, &b| positions[a].tradingsymbol.cmp(&positions[b].tradingsymbol));
        }
        PositionsSort::Pnl => {
            let key =
                |p: &Position| crate::tui::data::position_live_pnl(p, live(p.instrument_token));
            let keys: Vec<f64> = positions.iter().map(key).collect();
            order.sort_by(|&a, &b| cmp_desc(keys[a], keys[b]));
        }
        PositionsSort::Qty => {
            order.sort_by(|&a, &b| {
                positions[b]
                    .quantity
                    .abs()
                    .cmp(&positions[a].quantity.abs())
                    .then_with(|| positions[a].tradingsymbol.cmp(&positions[b].tradingsymbol))
            });
        }
        PositionsSort::DayPct => {
            let key = |p: &Position| -> f64 {
                let ltp = live(p.instrument_token).unwrap_or(p.last_price);
                if p.close_price > 0.0 {
                    (ltp - p.close_price) / p.close_price * 100.0
                } else {
                    f64::NEG_INFINITY
                }
            };
            let keys: Vec<f64> = positions.iter().map(key).collect();
            order.sort_by(|&a, &b| cmp_desc(keys[a], keys[b]));
        }
    }
    order
}

impl App {
    pub fn positions_pane_of(&self, pane: PaneId) -> Option<&pane_state::PositionsPane> {
        (pane == PaneId::PortfolioPositions).then_some(&self.books.portfolio_positions)
    }

    pub(in crate::tui::app) fn key_positions(&mut self, pane: PaneId, key: KeyEvent) {
        let Some(position) = self.selected_position_in(pane).cloned() else {
            return;
        };
        match key.code {
            KeyCode::Char('c') => {
                if self.trading_disabled() {
                    return;
                }
                let lot_size = self
                    .instruments
                    .as_ref()
                    .and_then(|store| store.by_token(position.instrument_token))
                    .map(|instrument| instrument.lot_size)
                    .filter(|lot_size| *lot_size > 0);
                let Some(lot_size) = lot_size else {
                    self.toast(
                        ToastLevel::Info,
                        "instrument details are still loading; try conversion again shortly",
                    );
                    return;
                };
                match PositionConversionState::new(&position, lot_size) {
                    Some(state) => self.modals.push(Modal::PositionConvert(Box::new(state))),
                    None => self.toast(
                        ToastLevel::Info,
                        "no DAY or OVERNIGHT quantity is available to convert",
                    ),
                }
            }

            KeyCode::Char('a') => self.open_alert_form(
                &position.exchange,
                &position.tradingsymbol,
                position.instrument_token,
                "",
            ),
            KeyCode::Char('b' | 's') => self.open_ticket(
                if key.code == KeyCode::Char('b') {
                    Side::Buy
                } else {
                    Side::Sell
                },
                position.exchange,
                position.tradingsymbol,
                position.instrument_token,
                position.quantity.abs(),
                Some(&position.product),
            ),
            _ => {}
        }
    }

    pub fn positions_order_in(&self, pane: PaneId) -> Vec<usize> {
        let Some(instance) = self.positions_pane_of(pane) else {
            return Vec::new();
        };
        let net = self.positions_net();
        match instance.sort() {
            PositionsSort::Symbol | PositionsSort::Qty => instance
                .cached_order(self.positions_gen, |sort| {
                    positions_order(net, |token| self.live_ltp(token), sort)
                }),
            sort => positions_order(net, |token| self.live_ltp(token), sort),
        }
    }

    pub fn overview_positions_row(&self) -> usize {
        let natural = self.natural_positions_order();
        let net = self.positions_net();
        self.books
            .overview_positions
            .display_row(&natural, |i| net.get(i).map(position_key))
    }

    pub(in crate::tui::app) fn natural_positions_order(&self) -> Vec<usize> {
        (0..self.positions_net().len()).collect()
    }

    pub(in crate::tui::app) fn positions_net(&self) -> &[Position] {
        self.positions
            .as_ref()
            .map(|positions| positions.net.as_slice())
            .unwrap_or_default()
    }

    pub(in crate::tui::app) fn selected_position_in(&self, pane: PaneId) -> Option<&Position> {
        let instance = self.positions_pane_of(pane)?;
        let order = self.positions_order_in(pane);
        let net = self.positions_net();
        instance
            .target(&order, |i| net.get(i).map(position_key))
            .and_then(|natural| net.get(natural))
    }
}

#[cfg(test)]
pub(in crate::tui::app) mod tests {
    use super::super::super::test_support::{position, test_app};
    use super::super::super::{App, PositionsSort};
    use super::{position_key, positions_order};
    use crate::tui::data::DataEvent;
    use crate::tui::pane_state::PositionsPane;
    use crate::tui::workspace::PaneId;
    use crate::tui::workspace::Preset;
    use kiteconnect::ticker::Tick;

    fn no_live(_token: u32) -> Option<f64> {
        None
    }

    #[test]
    fn positions_sort_pnl_desc_and_symbol_asc() {
        let ps = [
            position("BBB", 1, 10, 100.0, 50.0, 50.0),
            position("AAA", 2, 10, 300.0, 50.0, 50.0),
            position("CCC", 3, 10, -50.0, 50.0, 50.0),
        ];
        assert_eq!(
            positions_order(&ps, no_live, PositionsSort::Pnl),
            vec![1, 0, 2]
        );
        assert_eq!(
            positions_order(&ps, no_live, PositionsSort::Symbol),
            vec![1, 0, 2]
        );
    }

    #[test]
    fn positions_sort_qty_groups_open_positions_by_absolute_size() {
        let ps = [
            position("SMALL", 1, -5, 0.0, 100.0, 100.0),
            position("CLOSED", 2, 0, 0.0, 100.0, 100.0),
            position("LARGE", 3, 20, 0.0, 100.0, 100.0),
        ];
        assert_eq!(
            positions_order(&ps, no_live, PositionsSort::Qty),
            vec![2, 0, 1]
        );
    }

    #[test]
    fn positions_sort_day_pct_desc() {
        let ps = [
            position("AAA", 1, 10, 0.0, 120.0, 100.0),
            position("BBB", 2, 10, 0.0, 110.0, 100.0),
            position("CCC", 3, 10, 0.0, 90.0, 0.0),
        ];
        assert_eq!(
            positions_order(&ps, no_live, PositionsSort::DayPct),
            vec![0, 1, 2]
        );
    }

    pub(in crate::tui::app) fn positions_book_app() -> App {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioPositions);
        app.on_data(DataEvent::Positions(Ok({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("AAA", 1, 10, 100.0, 100.0, 100.0),
                position("BBB", 2, 10, -50.0, 100.0, 100.0),
                position("CCC", 3, 10, 300.0, 100.0, 100.0),
            ];
            row
        })));
        app
    }

    #[test]
    fn two_positions_instances_keep_their_own_sort_and_selection() {
        let app = positions_book_app();
        let net = app.positions_net();
        let symbol_of = |pane: &PositionsPane| {
            let order = positions_order(net, |token| app.live_ltp(token), pane.sort());
            pane.target(&order, |i| net.get(i).map(position_key))
                .map(|natural| net[natural].tradingsymbol.clone())
        };
        let order_for =
            |pane: &PositionsPane| positions_order(net, |token| app.live_ltp(token), pane.sort());

        let mut by_pnl = PositionsPane::default();
        let mut by_symbol = PositionsPane::default();
        by_symbol.set_sort(PositionsSort::Symbol);
        assert_eq!(order_for(&by_pnl), vec![2, 0, 1], "CCC, AAA, BBB");
        assert_eq!(order_for(&by_symbol), vec![0, 1, 2], "AAA, BBB, CCC");

        by_pnl.select(1, &order_for(&by_pnl), |i| net.get(i).map(position_key));
        by_symbol.select(1, &order_for(&by_symbol), |i| net.get(i).map(position_key));
        assert_eq!(symbol_of(&by_pnl).as_deref(), Some("AAA"));
        assert_eq!(symbol_of(&by_symbol).as_deref(), Some("BBB"));

        by_pnl.set_sort(PositionsSort::Qty);
        assert_eq!(by_symbol.sort(), PositionsSort::Symbol);
        assert_eq!(symbol_of(&by_symbol).as_deref(), Some("BBB"));

        by_symbol.step(1, &order_for(&by_symbol), |i| net.get(i).map(position_key));
        assert_eq!(symbol_of(&by_symbol).as_deref(), Some("CCC"));
        assert_eq!(symbol_of(&by_pnl).as_deref(), Some("AAA"));
    }

    #[test]
    fn the_overview_snapshot_and_the_positions_table_select_independently() {
        let mut app = positions_book_app();

        app.set_selection(2);
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("BBB".to_string())
        );

        app.set_workspace(Preset::Overview);
        app.set_selection(1);
        assert_eq!(app.pane_row(app.focused_pane()), 1, "natural row BBB");

        app.set_pane(PaneId::PortfolioPositions);
        assert_eq!(
            app.selected_position_in(PaneId::PortfolioPositions)
                .map(|p| p.tradingsymbol.clone()),
            Some("BBB".to_string()),
            "the full table kept its own cursor"
        );
        app.set_selection(0);
        app.set_workspace(Preset::Overview);
        assert_eq!(
            app.pane_row(app.focused_pane()),
            1,
            "and moving the full table did not move the snapshot"
        );
    }

    #[test]
    fn positions_display_order_pnl_sort_is_never_served_stale() {
        let mut app = test_app();
        let position = |token: u32, avg: f64| {
            let mut row = crate::tui::data::test_models::position();
            row.tradingsymbol = "INFY".into();
            row.exchange = "NSE".into();
            row.instrument_token = token;
            row.product = "CNC".into();
            row.overnight_quantity = 0;
            row.day_buy_quantity = 0;
            row.day_sell_quantity = 0;
            row.quantity = 1;
            row.average_price = avg;
            row.last_price = avg;
            row.close_price = avg;
            row.pnl = 0.0;
            row.realised = 0.0;
            row.multiplier = 1.0;
            row
        };
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![position(1, 100.0), position(2, 100.0)];
            row
        });
        assert_eq!(app.books.portfolio_positions.sort(), PositionsSort::Pnl);

        app.ticks.insert(2, {
            let mut value = Tick::default();
            value.instrument_token = 2;
            value.last_price = Some(200.0);
            value
        });
        assert_eq!(
            app.positions_order_in(PaneId::PortfolioPositions),
            vec![1, 0],
            "live tick must reorder Pnl sort without any cache invalidation hook firing"
        );
    }
}
