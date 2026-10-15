use super::super::{App, Modal, Side, ToastLevel, segment_class};
use super::products::{
    ORDER_TYPES, PRODUCTS, allowed_gtt_products, allowed_products, clamp_product_index,
    default_product_index, validity_indices,
};
use crate::tui::data::{AuctionHolding, Order};
use crate::tui::form::FormViewport;
use crate::tui::ticket::{
    ModifyTarget, TicketField, TicketKind, TicketState, gtt_default_trigger_percent,
    gtt_seed_trigger, seed_qty, tick_price,
};
use kiteconnect::OrderType;

impl App {
    pub(in crate::tui::app) fn contract_segment(&self, token: u32) -> &str {
        self.instruments
            .as_ref()
            .and_then(|store| store.by_token(token))
            .map(|inst| inst.segment.as_str())
            .unwrap_or_default()
    }

    pub(in crate::tui::app) fn seed_gtt_trigger(
        &self,
        token: u32,
        tick_size: f64,
    ) -> Option<String> {
        let ltp = self.live_ltp(token)?;
        let percent = gtt_default_trigger_percent(self.contract_segment(token));
        gtt_seed_trigger(ltp, percent, tick_size)
    }

    pub(in crate::tui::app) fn contract_specs(
        &self,
        token: u32,
    ) -> (f64, u32, Option<u32>, Option<u32>) {
        self.known_contract_specs(token)
            .unwrap_or((0.05, 1, None, None))
    }

    fn known_contract_specs(&self, token: u32) -> Option<(f64, u32, Option<u32>, Option<u32>)> {
        self.instruments
            .as_ref()
            .and_then(|store| store.by_token(token))
            .map(|inst| {
                (
                    inst.tick_size,
                    inst.lot_size,
                    inst.freeze_quantity.filter(|&fz| fz > 0),
                    inst.max_order_quantity.filter(|&mx| mx > 0),
                )
            })
    }

    pub(in crate::tui::app) fn modify_specs(
        &mut self,
        token: u32,
    ) -> Option<(f64, u32, Option<u32>, Option<u32>)> {
        let specs = self.known_contract_specs(token);
        if specs.is_none() {
            self.toast(
                ToastLevel::Info,
                if self.instruments.is_some() {
                    "instrument is missing from the loaded master; exact tick and lot sizes are unavailable"
                } else {
                    "instrument details are not loaded; Modify needs the exact tick and lot sizes"
                },
            );
        }
        specs
    }

    pub(in crate::tui) fn open_ticket(
        &mut self,
        side: Side,
        exchange: String,
        tradingsymbol: String,
        instrument_token: u32,
        qty: i64,
        product: Option<&str>,
    ) {
        if self.trading_disabled() {
            return;
        }
        let (tick_size, lot_size, freeze_qty, max_order_qty) =
            self.contract_specs(instrument_token);
        let ltp = self
            .ticks
            .get(&instrument_token)
            .and_then(|tick| tick.last_price);

        let prefs = self.ticket_prefs.get(&segment_class(&exchange)).copied();
        let product_idx = product
            .and_then(|p| PRODUCTS.iter().position(|candidate| *candidate == p))
            .or_else(|| prefs.map(|p| p.product))
            .unwrap_or_else(|| self.resolve_default_product(&exchange));
        let product_idx = clamp_product_index(product_idx, allowed_products(&exchange));

        self.modals.push(Modal::Ticket(Box::new(TicketState {
            side,
            exchange,
            tradingsymbol,
            instrument_token,
            modify: None,
            kind: TicketKind::Regular,
            qty: seed_qty(qty, lot_size),
            price: ltp.map(|p| tick_price(p, tick_size)).unwrap_or_default(),
            trigger: String::new(),
            gtt_oco: false,
            gtt_trailing: false,
            trailing_points: String::new(),
            trigger2: String::new(),
            price2: String::new(),

            order_type: prefs.map_or(OrderType::Limit, |p| p.order_type),
            product: product_idx,
            validity: prefs.map_or(0, |p| p.validity),
            validity_ttl: prefs.map_or(0, |p| p.validity_ttl),
            focus: TicketField::Quantity,
            issue: None,
            viewport: FormViewport::default(),
            tick_size,
            lot_size,
            freeze_quantity: freeze_qty,
            max_order_quantity: max_order_qty,
            autoslice: prefs.is_some_and(|p| p.autoslice),
            market_protection: false,
            variety: 0,
            iceberg_legs: "2".to_string(),
        })));
    }

    pub(in crate::tui::app) fn open_auction_ticket(&mut self, auction: AuctionHolding) {
        if self.trading_disabled() {
            return;
        }
        let holding = auction;
        let specs = self
            .instruments
            .as_ref()
            .and_then(|store| {
                store.by_token(holding.instrument_token).or_else(|| {
                    store
                        .by_exact_symbol(&holding.tradingsymbol)
                        .into_iter()
                        .find(|instrument| {
                            instrument.exchange.eq_ignore_ascii_case(&holding.exchange)
                        })
                })
            })
            .map(|instrument| {
                (
                    instrument.tick_size,
                    instrument.lot_size,
                    instrument.freeze_quantity.filter(|quantity| *quantity > 0),
                    instrument
                        .max_order_quantity
                        .filter(|quantity| *quantity > 0),
                )
            })
            .filter(|(_, lot_size, _, _)| *lot_size > 0);
        let Some((tick_size, lot_size, freeze_quantity, max_order_quantity)) = specs else {
            self.toast(
                ToastLevel::Info,
                "instrument details are still loading; try the auction order again shortly"
                    .to_string(),
            );
            return;
        };
        let price = self
            .live_ltp(holding.instrument_token)
            .unwrap_or(holding.last_price);
        self.modals.push(Modal::Ticket(Box::new(TicketState {
            side: Side::Sell,
            exchange: holding.exchange,
            tradingsymbol: holding.tradingsymbol,
            instrument_token: holding.instrument_token,
            modify: None,
            kind: TicketKind::Auction {
                auction_number: holding.auction_number,
            },
            qty: seed_qty(holding.quantity.abs(), lot_size),
            price: if price > 0.0 {
                tick_price(price, tick_size)
            } else {
                String::new()
            },
            trigger: String::new(),
            gtt_oco: false,
            gtt_trailing: false,
            trailing_points: String::new(),
            trigger2: String::new(),
            price2: String::new(),
            order_type: OrderType::Limit,
            product: 0,
            validity: 0,
            validity_ttl: 0,
            focus: TicketField::Quantity,
            issue: None,
            viewport: FormViewport::default(),
            tick_size,
            lot_size,
            freeze_quantity,
            max_order_quantity,
            autoslice: false,
            market_protection: false,
            variety: 0,
            iceberg_legs: "2".to_string(),
        })));
    }

    pub(in crate::tui::app) fn open_modify_ticket(&mut self, order: &Order) {
        if self.trading_disabled() {
            return;
        }
        let Some((tick_size, lot_size, freeze_qty, max_order_qty)) =
            self.modify_specs(order.instrument_token)
        else {
            return;
        };
        let side = if order.transaction_type == "SELL" {
            Side::Sell
        } else {
            Side::Buy
        };
        let variety = if order.variety.is_empty() {
            "regular".to_string()
        } else {
            order.variety.clone()
        };
        let quantity_editable = !matches!(variety.as_str(), "iceberg" | "co");
        let (validity, validity_ttl) = validity_indices(&order.validity, order.validity_ttl);
        self.modals.push(Modal::Ticket(Box::new(TicketState {
            side,
            exchange: order.exchange.clone(),
            tradingsymbol: order.tradingsymbol.clone(),
            instrument_token: order.instrument_token,
            modify: Some(ModifyTarget {
                parent_order_id: order.parent_order_id.clone(),
                order_id: order.order_id.clone(),
                variety,
                quantity_editable,
            }),
            kind: TicketKind::Regular,
            qty: order.quantity.to_string(),
            price: if order.price > 0.0 {
                tick_price(order.price, tick_size)
            } else {
                String::new()
            },
            trigger: if order.trigger_price > 0.0 {
                tick_price(order.trigger_price, tick_size)
            } else {
                String::new()
            },
            gtt_oco: false,
            gtt_trailing: false,
            trailing_points: String::new(),
            trigger2: String::new(),
            price2: String::new(),
            order_type: ORDER_TYPES
                .iter()
                .copied()
                .find(|t| t.to_string() == order.order_type)
                .unwrap_or(OrderType::Limit),
            product: PRODUCTS
                .iter()
                .position(|p| *p == order.product)
                .unwrap_or(0),
            validity,
            validity_ttl,
            focus: TicketField::Quantity,
            issue: None,
            viewport: FormViewport::default(),
            tick_size,
            lot_size,
            freeze_quantity: freeze_qty,
            max_order_quantity: max_order_qty,
            autoslice: false,
            market_protection: false,
            variety: 0,
            iceberg_legs: "2".to_string(),
        })));
    }

    pub(in crate::tui::app) fn resolve_default_product(&self, exchange: &str) -> usize {
        self.default_product
            .unwrap_or_else(|| default_product_index(exchange))
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "a GTT seeds an instrument, an order leg and an optional trigger"
    )]
    pub(in crate::tui::app) fn open_gtt_ticket(
        &mut self,
        side: Side,
        exchange: String,
        tradingsymbol: String,
        instrument_token: u32,
        qty: i64,
        product: Option<&str>,
        price: Option<f64>,
        trigger: Option<f64>,
        modify: Option<i64>,
    ) {
        if self.trading_disabled() {
            return;
        }
        let specs = match modify {
            Some(_) => self.modify_specs(instrument_token),
            None => Some(self.contract_specs(instrument_token)),
        };
        let Some((tick_size, lot_size, freeze_qty, max_order_qty)) = specs else {
            return;
        };
        let ltp = self.live_ltp(instrument_token);

        let remembered_product = modify
            .is_none()
            .then(|| self.ticket_prefs.get(&segment_class(&exchange)))
            .flatten()
            .map(|p| p.product);
        let product_idx = product
            .and_then(|p| PRODUCTS.iter().position(|candidate| *candidate == p))
            .or(remembered_product)
            .unwrap_or_else(|| self.resolve_default_product(&exchange));
        let product_idx = clamp_product_index(product_idx, allowed_gtt_products(&exchange));
        let use_default_price = price.is_none();
        let price = match price {
            Some(p) if p > 0.0 => tick_price(p, tick_size),
            _ => ltp.map(|p| tick_price(p, tick_size)).unwrap_or_default(),
        };

        let trigger = match trigger {
            Some(t) if t > 0.0 => tick_price(t, tick_size),
            _ => self
                .seed_gtt_trigger(instrument_token, tick_size)
                .unwrap_or_default(),
        };
        let price = if side == Side::Buy && modify.is_none() && use_default_price {
            trigger.clone()
        } else {
            price
        };
        self.modals.push(Modal::Ticket(Box::new(TicketState {
            side,
            exchange,
            tradingsymbol,
            instrument_token,
            modify: None,
            kind: TicketKind::Gtt { modify },
            qty: seed_qty(qty, lot_size),
            price,
            trigger,
            gtt_oco: false,
            gtt_trailing: false,
            trailing_points: String::new(),
            trigger2: String::new(),
            price2: String::new(),
            order_type: OrderType::Limit,
            product: product_idx,
            validity: 0,
            validity_ttl: 0,
            focus: TicketField::Quantity,
            issue: None,
            viewport: FormViewport::default(),
            tick_size,
            lot_size,
            freeze_quantity: freeze_qty,
            max_order_quantity: max_order_qty,
            autoslice: false,
            market_protection: false,
            variety: 0,
            iceberg_legs: "2".to_string(),
        })));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{master_inst, sample_order, test_app};
    use super::super::super::{App, Modal, Side};
    use crate::tui::data::{DataEvent, Order};
    use crate::tui::instruments::InstrumentStore;
    use crate::tui::ticket::{PendingAction, build_confirm};
    use std::sync::Arc;

    fn cds_order() -> Order {
        {
            let mut row = sample_order("OPEN");
            row.exchange = "CDS".into();
            row.tradingsymbol = "USDINR26SEPFUT".into();
            row.instrument_token = 7001;
            row.order_type = "LIMIT".into();
            row.product = "NRML".into();
            row.price = 83.1475;
            row
        }
    }

    fn cds_master() -> DataEvent {
        let mut future = master_inst("CDS", "CDS-FUT", "USDINR26SEPFUT", 7001);
        future.tick_size = 0.0025;
        DataEvent::Instruments(Ok(Arc::new(InstrumentStore::new(vec![future]))))
    }

    fn refused_for_specs(app: &App) -> bool {
        app.modals.is_empty()
            && app
                .toasts()
                .iter()
                .any(|toast| toast.text.contains("tick and lot sizes"))
    }

    fn open_cds_gtt_modify(app: &mut App) {
        app.open_gtt_ticket(
            Side::Sell,
            "CDS".to_string(),
            "USDINR26SEPFUT".to_string(),
            7001,
            1,
            Some("NRML"),
            Some(83.1475),
            Some(83.1225),
            Some(5),
        );
    }

    #[test]
    fn a_modify_before_the_master_loads_is_refused_not_rounded() {
        let mut app = test_app();
        assert!(app.instruments.is_none());
        app.open_modify_ticket(&cds_order());
        assert!(refused_for_specs(&app), "Modify opened on a guessed tick");

        app.on_data(cds_master());
        app.open_modify_ticket(&cds_order());
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("Modify opens once the master is loaded");
        };
        assert_eq!(state.price, "83.1475");
        let confirm = build_confirm(state, None, 0).expect("the untouched modify is valid");
        let PendingAction::Modify { form, .. } = confirm.action else {
            panic!("expected a modify");
        };
        assert!(form.contains(&("price".to_string(), "83.1475".to_string())));
    }

    #[test]
    fn a_gtt_modify_without_the_contracts_tick_is_refused() {
        let mut app = test_app();
        app.on_data(DataEvent::Instruments(Err("master download failed".into())));
        open_cds_gtt_modify(&mut app);
        assert!(
            refused_for_specs(&app),
            "GTT modify opened on a guessed tick"
        );

        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "INFY", 408_065,
        )])));
        app.open_modify_ticket(&cds_order());
        assert!(refused_for_specs(&app), "the token is not in the master");

        app.open_gtt_ticket(
            Side::Buy,
            "NSE".to_string(),
            "INFY".to_string(),
            1,
            1,
            None,
            Some(1500.0),
            Some(1490.0),
            None,
        );
        assert!(matches!(app.modals.last(), Some(Modal::Ticket(_))));
        app.modals.clear();

        app.on_data(cds_master());
        open_cds_gtt_modify(&mut app);
        let Some(Modal::Ticket(state)) = app.modals.last() else {
            panic!("GTT modify opens once the tick is known");
        };
        assert_eq!(
            (state.price.as_str(), state.trigger.as_str()),
            ("83.1475", "83.1225")
        );
    }
}
