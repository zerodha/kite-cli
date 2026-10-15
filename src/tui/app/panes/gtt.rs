use super::super::{App, Modal, Side, ToastLevel};
use crate::tui::data::{self, Gtt};
use crate::tui::ticket::{ConfirmState, PendingAction, tick_price};
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GttSort {
    #[default]
    Created,
    Status,
    Symbol,
    Type,
}

impl GttSort {
    pub const ALL: [Self; 4] = [Self::Created, Self::Status, Self::Symbol, Self::Type];

    pub(in crate::tui::app) fn next(self) -> Self {
        match self {
            GttSort::Created => GttSort::Status,
            GttSort::Status => GttSort::Symbol,
            GttSort::Symbol => GttSort::Type,
            GttSort::Type => GttSort::Created,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            GttSort::Created => "created ▼",
            GttSort::Status => "status ▲",
            GttSort::Symbol => "symbol ▲",
            GttSort::Type => "type ▲",
        }
    }
}

pub(in crate::tui::app) fn gtt_key(gtt: &Gtt) -> i64 {
    gtt.id
}

pub fn gtts_order(gtts: &[Gtt], mode: GttSort) -> Vec<usize> {
    let mut order: Vec<usize> = (0..gtts.len()).collect();
    match mode {
        GttSort::Created => order.sort_by(|&a, &b| {
            gtts[b]
                .created_at
                .cmp(&gtts[a].created_at)
                .then_with(|| gtts[b].id.cmp(&gtts[a].id))
        }),
        GttSort::Status => {
            order.sort_by(|&a, &b| gtts[a].status.cmp(&gtts[b].status));
        }
        GttSort::Symbol => order.sort_by(|&a, &b| {
            gtts[a]
                .condition
                .tradingsymbol
                .cmp(&gtts[b].condition.tradingsymbol)
        }),
        GttSort::Type => {
            order.sort_by(|&a, &b| gtts[a].trigger_type.cmp(&gtts[b].trigger_type));
        }
    }
    order
}

impl App {
    fn resolve_token(&self, exchange: &str, tradingsymbol: &str) -> u32 {
        let Some(store) = &self.instruments else {
            return 0;
        };
        store
            .by_exact_symbol(tradingsymbol)
            .into_iter()
            .find(|inst| inst.exchange == exchange && inst.tradingsymbol == tradingsymbol)
            .map_or(0, |inst| inst.instrument_token)
    }

    pub(in crate::tui::app) fn on_gtt_result(
        &mut self,
        verb: &str,
        result: Result<String, String>,
    ) {
        self.pending_action = false;
        match result {
            Ok(trigger_id) => {
                self.toast(ToastLevel::Success, format!("GTT {verb}: {trigger_id}"));
                data::spawn_refresh_gtts(self.client.clone(), self.events_tx.clone());
            }
            Err(err) => {
                self.toast(
                    ToastLevel::Error,
                    format!("GTT {verb} request failed: {err}"),
                );
                data::spawn_refresh_gtts(self.client.clone(), self.events_tx.clone());
            }
        }
    }

    pub(in crate::tui::app) fn key_gtt(&mut self, key: KeyEvent) {
        if !matches!(key.code, KeyCode::Char('x' | 'm')) {
            return;
        }
        let Some(gtt) = self.selected_gtt() else {
            return;
        };
        match key.code {
            KeyCode::Char('x') => {
                if self.trading_disabled() {
                    return;
                }
                let triggers = gtt
                    .condition
                    .trigger_values
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                let order_line = gtt.orders.first().map_or_else(
                    || ".".to_string(),
                    |order| {
                        format!(
                            "{} {} @ {}",
                            order.transaction_type, order.quantity, order.price
                        )
                    },
                );
                self.modals
                    .push(Modal::Confirm(Box::new(ConfirmState::plain(
                        "Delete GTT".to_string(),
                        vec![
                            ("Trigger".to_string(), gtt.id.to_string()),
                            (
                                "Symbol".to_string(),
                                format!(
                                    "{}:{}",
                                    gtt.condition.exchange, gtt.condition.tradingsymbol
                                ),
                            ),
                            ("Trigger at".to_string(), triggers),
                            ("Order".to_string(), order_line),
                            ("Status".to_string(), gtt.status.clone()),
                        ],
                        PendingAction::DeleteGtt { trigger_id: gtt.id },
                    ))));
            }
            KeyCode::Char('m') => {
                if gtt.status != "active" {
                    self.toast(ToastLevel::Error, format!("GTT {} is not active", gtt.id));
                    return;
                }
                self.open_gtt_modify(&gtt);
            }
            _ => {}
        }
    }

    pub(in crate::tui::app) fn open_gtt_modify(&mut self, gtt: &Gtt) {
        let Some(order) = gtt.orders.first() else {
            self.toast(ToastLevel::Error, "GTT has no order leg");
            return;
        };
        let side = if order.transaction_type == "SELL" {
            Side::Sell
        } else {
            Side::Buy
        };
        let token = self.resolve_token(&gtt.condition.exchange, &gtt.condition.tradingsymbol);

        let product = (!order.product.is_empty()).then_some(order.product.as_str());

        let oco = (gtt.orders.len() >= 2 && gtt.condition.trigger_values.len() >= 2)
            .then(|| (gtt.orders[1].price, gtt.condition.trigger_values[1]));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "GTT wire quantity uses f64 and keeps the existing integer conversion"
        )]
        let quantity = order.quantity as i64;
        self.open_gtt_ticket(
            side,
            gtt.condition.exchange.clone(),
            gtt.condition.tradingsymbol.clone(),
            token,
            quantity,
            product,
            Some(order.price),
            gtt.condition.trigger_values.first().copied(),
            Some(gtt.id),
        );
        if let Some((price2, trigger2)) = oco
            && let Some(Modal::Ticket(state)) = self.modals.last_mut()
        {
            state.gtt_oco = true;
            state.price2 = tick_price(price2, state.tick_size);
            state.trigger2 = tick_price(trigger2, state.tick_size);
        }
        if gtt.trigger_type.starts_with("trailing-")
            && let Some(Modal::Ticket(state)) = self.modals.last_mut()
        {
            state.gtt_trailing = true;
            state.trailing_points = gtt
                .condition
                .trailing_points
                .first()
                .map_or_else(|| "2.00".to_string(), |points| format!("{points:.2}"));
        }
    }

    pub fn gtts_display_order(&self) -> Vec<usize> {
        let Some(gtts) = self.gtts.as_deref() else {
            return Vec::new();
        };
        self.books
            .gtt
            .cached_order(self.gtts_gen, |sort| gtts_order(gtts, sort))
    }

    pub fn gtts_display_row(&self, display: &[usize]) -> usize {
        let gtts = self.gtts.as_deref().unwrap_or_default();
        self.books
            .gtt
            .display_row(display, |i| gtts.get(i).map(gtt_key))
    }

    fn selected_gtt(&self) -> Option<Gtt> {
        let order = self.gtts_display_order();
        let gtts = self.gtts.as_deref().unwrap_or_default();
        self.books
            .gtt
            .target(&order, |i| gtts.get(i).map(gtt_key))
            .and_then(|natural| gtts.get(natural))
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{master_inst, test_app};
    use super::super::super::{GttSort, Modal};
    use super::gtts_order;
    use crate::tui::data::Gtt;
    use crate::tui::instruments::InstrumentStore;
    use std::sync::Arc;

    fn gtt(id: i64, created: &str, status: &str, symbol: &str, trigger_type: &str) -> Gtt {
        {
            let mut row = crate::tui::data::test_models::gtt();
            row.id = id;
            row.trigger_type = trigger_type.into();
            row.status = status.into();
            row.created_at = created.replace(' ', "T").parse().ok();
            row.condition = {
                let mut row = crate::tui::data::test_models::gtt_condition();
                row.exchange = "NSE".into();
                row.tradingsymbol = symbol.into();
                row.trigger_values = vec![100.0];
                row.last_price = 95.0;
                row.trailing_points = Vec::new();
                row
            };
            row.orders = vec![];
            row
        }
    }

    #[test]
    fn gtt_sort_supports_created_status_symbol_and_type() {
        let gtts = [
            gtt(1, "2026-07-20 10:00:00", "triggered", "ZZZ", "single"),
            gtt(2, "2026-07-22 10:00:00", "active", "AAA", "two-leg"),
            gtt(3, "2026-07-21 10:00:00", "disabled", "MMM", "trailing"),
        ];
        assert_eq!(gtts_order(&gtts, GttSort::Created), vec![1, 2, 0]);
        assert_eq!(gtts_order(&gtts, GttSort::Status), vec![1, 2, 0]);
        assert_eq!(gtts_order(&gtts, GttSort::Symbol), vec![1, 2, 0]);
        assert_eq!(gtts_order(&gtts, GttSort::Type), vec![0, 2, 1]);
    }

    #[test]
    fn gtt_modify_clamps_leg_to_delivery_product() {
        use super::super::super::PRODUCTS;
        use crate::tui::ticket::TicketKind;
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NFO",
            "NFO-OPT",
            "NIFTY24JUL24000CE",
            1001,
        )])));

        let gtt = {
            let mut row = crate::tui::data::test_models::gtt();
            row.id = 42;
            row.trigger_type = "single".into();
            row.status = "active".into();
            row.created_at = (String::new()).replace(' ', "T").parse().ok();
            row.condition = {
                let mut row = crate::tui::data::test_models::gtt_condition();
                row.exchange = "NFO".into();
                row.tradingsymbol = "NIFTY24JUL24000CE".into();
                row.trigger_values = vec![100.0];
                row.last_price = 95.0;
                row.trailing_points = Vec::new();
                row
            };
            row.orders = vec![{
                let mut row = crate::tui::data::test_models::gtt_order();
                row.transaction_type = "BUY".into();
                row.quantity = 50.0;
                row.price = 90.0;
                row.product = "MIS".into();
                row
            }];
            row
        };
        app.open_gtt_modify(&gtt);
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("open_gtt_modify must push a GTT ticket modal");
        };
        assert!(
            matches!(&state.kind, TicketKind::Gtt { modify: Some(42) }),
            "ticket must be a modify of the original GTT"
        );
        assert_eq!(
            PRODUCTS[state.product], "NRML",
            "a GTT modify must clamp an intraday leg product to the exchange delivery product"
        );
    }

    #[test]
    fn gtt_modify_preserves_trailing_type_and_points() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "INFY", 408_065,
        )])));
        let gtt = {
            let mut row = crate::tui::data::test_models::gtt();
            row.id = 77;
            row.trigger_type = "trailing-single".into();
            row.status = "active".into();
            row.created_at = (String::new()).replace(' ', "T").parse().ok();
            row.condition = {
                let mut row = crate::tui::data::test_models::gtt_condition();
                row.exchange = "NSE".into();
                row.tradingsymbol = "INFY".into();
                row.trigger_values = vec![1500.0];
                row.last_price = 1520.0;
                row.trailing_points = vec![2.0];
                row
            };
            row.orders = vec![{
                let mut row = crate::tui::data::test_models::gtt_order();
                row.transaction_type = "SELL".into();
                row.quantity = 10.0;
                row.price = 1499.0;
                row.product = "CNC".into();
                row
            }];
            row
        };
        app.open_gtt_modify(&gtt);
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("expected modify ticket");
        };
        assert!(state.gtt_trailing);
        assert_eq!(state.trailing_points, "2.00");
        let confirm = crate::tui::ticket::build_confirm(state, Some(1520.0), 0).unwrap();
        let crate::tui::ticket::PendingAction::ModifyGtt { form, .. } = confirm.action else {
            panic!("expected modify GTT action");
        };
        assert_eq!(form[0].1, "trailing-single");
    }
    #[test]
    fn simplification_regression_exact_gtt_symbol_survives_normalized_aliases() {
        use crate::tui::app::test_support::master_inst;
        use crate::tui::instruments::InstrumentStore;
        use std::sync::Arc;

        let mut app = crate::tui::app::test_support::test_app();
        let mut rows = (0..11)
            .map(|token| master_inst("NSE", "NSE", "NIFTY50", token))
            .collect::<Vec<_>>();
        rows.push(master_inst("NSE", "NSE", "NIFTY 50", 256_265));
        app.instruments = Some(Arc::new(InstrumentStore::new(rows)));
        assert_eq!(app.resolve_token("NSE", "NIFTY 50"), 256_265);
        assert_eq!(app.resolve_token("BSE", "NIFTY 50"), 0);
    }
}
