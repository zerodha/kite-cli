use super::super::{
    AlertFormFocus, AlertFormState, App, Modal, SearchPurpose, SearchState, Side, ToastLevel,
};
use crate::tui::alerts;
use crate::tui::form::{ControlIntent, FormIssue, FormViewport};
use crate::tui::instruments::Instrument;
use crate::tui::ticket::{edit_numeric, edit_quantity, step_signed_number, tick_price};
use crossterm::event::{KeyCode, KeyEvent};
use kiteconnect::BasketOrderParams;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtoItemFocus {
    Side,
    Quantity,
    Product,
    OrderType,
    Price,
    Trigger,
    Validity,
    Ttl,
}

impl AtoItemFocus {
    pub fn rows(order_type: &str, validity: &str) -> Vec<Self> {
        let mut rows = vec![Self::Side, Self::Quantity, Self::Product, Self::OrderType];
        if order_type != "MARKET" {
            rows.push(Self::Price);
        }
        if order_type == "SL" {
            rows.push(Self::Trigger);
        }
        rows.push(Self::Validity);
        if validity == "TTL" {
            rows.push(Self::Ttl);
        }
        rows
    }

    fn cycle(self, delta: isize, order_type: &str, validity: &str) -> Self {
        let rows = Self::rows(order_type, validity);
        let index = rows
            .iter()
            .position(|row| *row == self)
            .unwrap_or(0)
            .cast_signed();
        rows[(index + delta).rem_euclid(rows.len().cast_signed()) as usize]
    }
}

#[derive(Debug)]
pub struct AtoItemFormState {
    pub exchange: String,
    pub tradingsymbol: String,
    pub instrument_token: u32,

    pub editing: Option<usize>,
    pub side: Side,

    pub product: usize,

    pub order_type: usize,

    pub validity: usize,
    pub quantity: String,
    pub price: String,
    pub trigger: String,
    pub ttl: String,
    pub focus: AtoItemFocus,
    pub lot_size: u32,
    pub tick_size: f64,
    pub freeze_quantity: Option<u32>,

    pub issue: Option<alerts::ItemIssue>,

    pub viewport: FormViewport,
}

impl AtoItemFormState {
    pub fn label(&self) -> String {
        format!("{}:{}", self.exchange, self.tradingsymbol)
    }

    pub fn products(&self) -> &'static [&'static str] {
        alerts::ato_products(&self.exchange)
    }

    pub fn product(&self) -> &'static str {
        self.products()[self.product.min(self.products().len() - 1)]
    }

    pub fn order_type(&self) -> &'static str {
        alerts::ATO_ORDER_TYPES[self.order_type]
    }

    pub fn validity(&self) -> &'static str {
        alerts::ATO_VALIDITIES[self.validity]
    }

    pub fn visible(&self) -> Vec<AtoItemFocus> {
        AtoItemFocus::rows(self.order_type(), self.validity())
    }

    pub fn select(&mut self, field: AtoItemFocus, intent: ControlIntent) {
        if !self.visible().contains(&field) {
            return;
        }
        self.focus = field;
        self.viewport.request_reveal();
        self.clear_issue(field);
        let stepped = match field {
            AtoItemFocus::Side => {
                intent
                    .pick(usize::from(self.side == Side::Sell), 2)
                    .map(|next| {
                        self.side = if next == 1 { Side::Sell } else { Side::Buy };
                    })
            }
            AtoItemFocus::Product => intent
                .pick(self.product, self.products().len())
                .map(|next| self.product = next),

            AtoItemFocus::OrderType => intent
                .pick(self.order_type, alerts::ATO_ORDER_TYPES.len())
                .map(|next| self.order_type = next),
            AtoItemFocus::Validity => intent
                .pick(self.validity, alerts::ATO_VALIDITIES.len())
                .map(|next| self.validity = next),
            AtoItemFocus::Quantity
            | AtoItemFocus::Price
            | AtoItemFocus::Trigger
            | AtoItemFocus::Ttl => None,
        };
        if stepped.is_some() {
            self.issue = None;
        }
    }

    pub fn clear_issue(&mut self, field: AtoItemFocus) {
        if self
            .issue
            .as_ref()
            .is_some_and(|issue| issue.field == Some(field))
        {
            self.issue = None;
        }
    }

    pub fn validated(&self) -> Result<BasketOrderParams, alerts::ItemIssue> {
        alerts::validate_item(
            &self.exchange,
            self.instrument_token,
            self.side.label(),
            self.product(),
            self.order_type(),
            self.validity(),
            &self.ttl,
            &self.quantity,
            &self.price,
            &self.trigger,
            self.lot_size,
            self.tick_size,
            self.freeze_quantity,
        )
    }
}

pub(in crate::tui::app) enum BasketKey {
    NotOurs,

    Handled,
    AddOrder,
    EditOrder,
}

pub(in crate::tui::app) fn basket_key(key: KeyEvent, state: &mut AlertFormState) -> BasketKey {
    if state.draft.baseline_unavailable()
        && matches!(
            key.code,
            KeyCode::Char('a' | 'e' | 'd' | 'c' | '[' | ']') | KeyCode::Enter
        )
    {
        state.fail(FormIssue::at(
            AlertFormFocus::Basket,
            "this alert's saved orders did not load, so they cannot be changed here",
        ));
        return BasketKey::Handled;
    }
    let last = state.draft.len().saturating_sub(1);
    state.basket_selected = state.basket_selected.min(last);
    match key.code {
        KeyCode::Up | KeyCode::Char('k') => {
            state.basket_selected = state.basket_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.basket_selected = (state.basket_selected + 1).min(last);
        }

        KeyCode::PageUp => {
            state.viewport.page(-1);
            return BasketKey::Handled;
        }
        KeyCode::PageDown => {
            state.viewport.page(1);
            return BasketKey::Handled;
        }
        KeyCode::Home => state.basket_selected = 0,
        KeyCode::End => state.basket_selected = last,
        KeyCode::Char('a') => return BasketKey::AddOrder,
        KeyCode::Char('e') | KeyCode::Enter => return BasketKey::EditOrder,
        KeyCode::Char('d') => {
            if state.draft.remove(state.basket_selected).is_some() {
                state.basket_selected = state
                    .basket_selected
                    .min(state.draft.len().saturating_sub(1));
                state.issue = None;
            }
        }
        KeyCode::Char('c') => match state.draft.duplicate(state.basket_selected) {
            Ok(()) => {
                if !state.draft.is_empty() {
                    state.basket_selected += 1;
                }
                state.issue = None;
            }
            Err(err) => state.fail(FormIssue::at(AlertFormFocus::Basket, err)),
        },
        KeyCode::Char('[') => {
            state.basket_selected = state.draft.move_item(state.basket_selected, -1);
            state.issue = None;
        }
        KeyCode::Char(']') => {
            state.basket_selected = state.draft.move_item(state.basket_selected, 1);
            state.issue = None;
        }
        _ => return BasketKey::NotOurs,
    }

    state.viewport.request_reveal();
    BasketKey::Handled
}

impl App {
    pub(in crate::tui::app) fn open_ato_item_search(&mut self) {
        let Some(Modal::Alert(form)) = self.modals.last_mut() else {
            return;
        };
        if form.draft.is_full() {
            form.fail(FormIssue::at(
                AlertFormFocus::Basket,
                format!("a basket holds at most {} orders", alerts::MAX_BASKET_ITEMS),
            ));
            return;
        }
        form.issue = None;
        self.modals.push(Modal::Search(SearchState {
            purpose: SearchPurpose::AtoBasketItem,
            ..SearchState::default()
        }));
    }

    pub(in crate::tui::app) fn open_ato_item_edit(&mut self) {
        let Some(Modal::Alert(form)) = self.modals.last_mut() else {
            return;
        };
        let selected = form.basket_selected;
        let Some(item) = form.draft.items().get(selected) else {
            form.fail(FormIssue::at(
                AlertFormFocus::Basket,
                "no order to edit; add one with a",
            ));
            return;
        };
        let Some(params) = item.editable() else {
            let message = format!(
                "{} keeps order settings this form cannot edit; reorder, duplicate or remove it, or edit it on Kite Web",
                item.label()
            );
            form.fail(FormIssue::at(AlertFormFocus::Basket, message));
            return;
        };
        let params = params.clone();
        let (exchange, tradingsymbol, token) = (
            item.exchange.clone(),
            item.tradingsymbol.clone(),
            item.instrument_token,
        );
        form.issue = None;
        let Some((tick_size, lot_size, freeze_quantity, _)) = self.modify_specs(token) else {
            return;
        };
        let products = alerts::ato_products(&exchange);
        let index_of = |values: &[&str], wanted: &str| {
            values
                .iter()
                .position(|value| *value == wanted)
                .unwrap_or(0)
        };
        self.modals.push(Modal::AtoItem(Box::new(AtoItemFormState {
            exchange,
            tradingsymbol,
            instrument_token: token,
            editing: Some(selected),
            side: if params.transaction_type == "SELL" {
                Side::Sell
            } else {
                Side::Buy
            },

            product: index_of(products, &params.product),
            order_type: index_of(&alerts::ATO_ORDER_TYPES, &params.order_type),
            validity: index_of(&alerts::ATO_VALIDITIES, &params.validity),
            quantity: params.quantity.to_string(),
            price: tick_price(params.price, tick_size),
            trigger: tick_price(params.trigger_price, tick_size),
            ttl: if params.validity_ttl > 0 {
                params.validity_ttl.to_string()
            } else {
                "1".to_string()
            },
            focus: AtoItemFocus::Side,
            lot_size,
            tick_size,
            freeze_quantity,
            issue: None,
            viewport: FormViewport::default(),
        })));
    }

    pub(in crate::tui::app) fn open_ato_item_new(&mut self, instrument: &Instrument) {
        if !alerts::ATO_ORDER_EXCHANGES
            .iter()
            .any(|allowed| instrument.exchange.eq_ignore_ascii_case(allowed))
        {
            self.toast(
                ToastLevel::Error,
                format!(
                    "{} orders are not available in a basket here",
                    instrument.exchange
                ),
            );
            return;
        }
        if instrument.is_index() {
            self.toast(ToastLevel::Error, "indices are not tradable");
            return;
        }
        let (tick_size, lot_size, freeze_quantity, _) =
            self.contract_specs(instrument.instrument_token);
        let ltp = self.live_ltp(instrument.instrument_token);
        self.modals.push(Modal::AtoItem(Box::new(AtoItemFormState {
            exchange: instrument.exchange.clone(),
            tradingsymbol: instrument.tradingsymbol.clone(),
            instrument_token: instrument.instrument_token,
            editing: None,
            side: Side::Buy,

            product: 0,
            order_type: 1,
            validity: 0,
            quantity: lot_size.max(1).to_string(),
            price: ltp
                .map(|price| tick_price(price, tick_size))
                .unwrap_or_default(),
            trigger: String::new(),
            ttl: "1".to_string(),
            focus: AtoItemFocus::Side,
            lot_size,
            tick_size,
            freeze_quantity,
            issue: None,
            viewport: FormViewport::default(),
        })));
    }

    pub(in crate::tui::app) fn key_ato_item(
        &mut self,
        key: KeyEvent,
        mut state: Box<AtoItemFormState>,
    ) {
        let numeric_focus = matches!(
            state.focus,
            AtoItemFocus::Quantity
                | AtoItemFocus::Price
                | AtoItemFocus::Trigger
                | AtoItemFocus::Ttl
        );
        match key.code {
            KeyCode::Esc => return,
            KeyCode::Tab => {
                let next = state.focus.cycle(1, state.order_type(), state.validity());
                state.focus = next;
                state.viewport.request_reveal();
            }
            KeyCode::BackTab => {
                let next = state.focus.cycle(-1, state.order_type(), state.validity());
                state.focus = next;
                state.viewport.request_reveal();
            }
            KeyCode::Enter => match state.validated() {
                Ok(params) => {
                    self.stage_ato_item(&state, params);
                    return;
                }

                Err(issue) => {
                    if let Some(field) = issue.field {
                        state.focus = field;
                    }
                    state.viewport.request_reveal();
                    state.issue = Some(issue);
                }
            },
            KeyCode::PageUp => state.viewport.page(-1),
            KeyCode::PageDown => state.viewport.page(1),
            KeyCode::Left | KeyCode::Right | KeyCode::Char(' ') if !numeric_focus => {
                let intent = if key.code == KeyCode::Left {
                    ControlIntent::Previous
                } else {
                    ControlIntent::Next
                };
                let focus = state.focus;
                state.select(focus, intent);
            }
            KeyCode::Up | KeyCode::Down if numeric_focus => {
                let direction: i8 = if key.code == KeyCode::Up { 1 } else { -1 };
                match state.focus {
                    AtoItemFocus::Quantity => {
                        let lot = state.lot_size.max(1);
                        let current: i64 = state.quantity.trim().parse().unwrap_or(0);
                        let next = current + i64::from(direction) * i64::from(lot);
                        state.quantity = next.max(i64::from(lot)).to_string();
                    }
                    AtoItemFocus::Price => {
                        let ltp = self.live_ltp(state.instrument_token).unwrap_or(0.0);
                        state.price =
                            step_signed_number(&state.price, state.tick_size, direction, ltp);
                    }
                    AtoItemFocus::Trigger => {
                        let ltp = self.live_ltp(state.instrument_token).unwrap_or(0.0);
                        state.trigger =
                            step_signed_number(&state.trigger, state.tick_size, direction, ltp);
                    }
                    AtoItemFocus::Ttl => {
                        let (low, high) = alerts::ATO_TTL_MINUTES;
                        let current: i64 = state.ttl.trim().parse().unwrap_or(i64::from(low));
                        state.ttl = (current + i64::from(direction))
                            .clamp(i64::from(low), i64::from(high))
                            .to_string();
                    }
                    _ => {}
                }
                let focus = state.focus;
                state.clear_issue(focus);
                state.viewport.request_reveal();
            }
            code if numeric_focus => {
                let focus = state.focus;
                let field = match focus {
                    AtoItemFocus::Quantity => &mut state.quantity,
                    AtoItemFocus::Price => &mut state.price,
                    AtoItemFocus::Trigger => &mut state.trigger,
                    _ => &mut state.ttl,
                };

                let before = field.clone();
                if matches!(focus, AtoItemFocus::Quantity | AtoItemFocus::Ttl) {
                    edit_quantity(field, code);
                } else {
                    edit_numeric(field, code);
                }
                let changed = *field != before;
                if changed {
                    state.clear_issue(focus);
                    state.viewport.request_reveal();
                }
            }
            _ => {}
        }
        self.modals.push(Modal::AtoItem(state));
    }

    fn stage_ato_item(&mut self, item: &AtoItemFormState, params: BasketOrderParams) {
        let Some(Modal::Alert(form)) = self.modals.last_mut() else {
            self.toast(
                ToastLevel::Error,
                "the alert form closed; the order was not added",
            );
            return;
        };
        if let Some(index) = item.editing {
            form.draft.replace(index, params);
            form.basket_selected = index;
        } else {
            if let Err(err) = form.draft.push(
                item.exchange.clone(),
                item.tradingsymbol.clone(),
                item.instrument_token,
                params,
            ) {
                form.fail(FormIssue::at(AlertFormFocus::Basket, err));
                return;
            }
            form.basket_selected = form.draft.len().saturating_sub(1);
        }
        form.issue = None;
        form.focus = AlertFormFocus::Basket;
        form.viewport.request_reveal();
    }
}
#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        ato_alert, basket_item, char_key, char_key_code, issue_message,
    };
    use super::super::super::test_support::{master_inst, test_app};
    use super::super::super::{AlertFormFocus, AlertFormStage, AtoItemFocus, Modal, SearchPurpose};

    use crate::tui::alerts;
    use crate::tui::form::ControlIntent;
    use crate::tui::instruments::InstrumentStore;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::sync::Arc;

    #[tokio::test]
    async fn staging_a_basket_order_never_places_an_immediate_order() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![
            master_inst("NSE", "NSE", "INFY", 408_065),
            master_inst("NSE", "NSE", "TCS", 2_953_217),
        ])));
        app.open_alert_form("NSE", "INFY", 408_065, "NSE");
        app.on_key(char_key_code(KeyCode::Right));
        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open");
        };
        assert_eq!(state.focus, AlertFormFocus::Basket);

        app.on_key(char_key('a'));
        let Some(Modal::Search(search)) = app.modals.last() else {
            panic!("a must open the basket instrument picker");
        };
        assert_eq!(search.purpose, SearchPurpose::AtoBasketItem);
        assert_eq!(app.modals.len(), 2, "the alert form stays underneath");

        for ch in "TCS".chars() {
            app.on_key(char_key(ch));
        }
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::AtoItem(item)) = app.modals.last() else {
            panic!("picking an instrument must open the item editor");
        };
        assert_eq!(item.tradingsymbol, "TCS");
        assert_eq!(item.editing, None);

        app.on_key(char_key_code(KeyCode::Tab));
        for _ in 0..3 {
            app.on_key(char_key_code(KeyCode::Tab));
        }
        let Some(Modal::AtoItem(item)) = app.modals.last() else {
            panic!("the editor must stay open");
        };
        assert_eq!(item.focus, AtoItemFocus::Price);
        for ch in "100.00".chars() {
            app.on_key(char_key(ch));
        }
        app.on_key(char_key_code(KeyCode::Enter));

        assert!(
            !app.pending_action,
            "staging a basket order must not take the immediate-order interlock"
        );
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the alert form must be back on top");
        };
        assert_eq!(app.modals.len(), 1, "the item editor closed");
        assert_eq!(state.draft.len(), 1);
        assert_eq!(state.draft.items()[0].label(), "NSE:TCS");
        assert_eq!(state.focus, AlertFormFocus::Basket);
        assert_eq!(state.basket_selected, 0);
    }

    #[tokio::test]
    async fn escape_unwinds_the_nested_ato_modals_one_at_a_time() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![master_inst(
            "NSE", "NSE", "GOLDBEES", 3_693_569,
        )])));
        app.open_alert_edit(&ato_alert());
        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }

        app.on_key(char_key('e'));
        assert!(matches!(app.modals.last(), Some(Modal::AtoItem(_))));
        assert_eq!(app.modals.len(), 2);

        app.on_key(char_key_code(KeyCode::Esc));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("Esc must return to the alert form, not close it");
        };
        assert_eq!(
            state.draft.len(),
            1,
            "a cancelled order edit must leave the basket alone"
        );
        assert!(state.draft.is_unchanged());

        app.on_key(char_key_code(KeyCode::Esc));
        assert!(app.modals.is_empty(), "Esc on the form closes it");
    }

    #[tokio::test]
    async fn the_basket_section_adds_removes_duplicates_and_reorders() {
        let mut app = test_app();
        let mut alert = ato_alert();
        alert.basket = Some({
            let mut row = crate::tui::data::test_models::basket();
            row.items = vec![
                basket_item(1, "GOLDBEES", 1),
                basket_item(2, "SILVERBEES", 2),
            ];
            row
        });
        app.open_alert_edit(&alert);
        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }

        app.on_key(char_key('j'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert_eq!(state.basket_selected, 1);

        app.on_key(char_key(']'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert_eq!(state.basket_selected, 1);

        app.on_key(char_key('['));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert_eq!(state.basket_selected, 0);
        assert_eq!(state.draft.items()[0].label(), "NSE:SILVERBEES");

        app.on_key(char_key('c'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert_eq!(state.draft.len(), 3);
        assert_eq!(state.basket_selected, 1);

        app.on_key(char_key('d'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert_eq!(state.draft.len(), 2);
        assert_eq!(state.basket_selected, 1);
    }

    #[tokio::test]
    async fn a_read_only_basket_order_refuses_to_open_for_editing() {
        let mut app = test_app();
        let mut alert = ato_alert();
        let mut advanced = basket_item(9, "GOLDBEES", 1);
        advanced.params = serde_json::json!({"order_type": "SL-M", "quantity": 10, "price": 0});
        alert.basket = Some({
            let mut row = crate::tui::data::test_models::basket();
            row.items = vec![advanced];
            row
        });
        app.open_alert_edit(&alert);
        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }

        app.on_key(char_key('e'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open");
        };
        assert_eq!(app.modals.len(), 1, "no item editor may open");
        assert!(
            issue_message(state).is_some_and(|err| err.contains("cannot edit")),
            "the refusal must say why: {:?}",
            state.issue
        );

        app.on_key(char_key('c'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("open");
        };
        assert_eq!(state.draft.len(), 2, "a read-only order can be duplicated");
    }

    #[tokio::test]
    async fn a_basket_order_offers_only_the_products_its_exchange_allows() {
        for (exchange, segment, default, refused) in [
            ("NFO", "NFO-OPT", "NRML", "CNC"),
            ("BFO", "BFO-OPT", "NRML", "CNC"),
            ("NSE", "NSE", "CNC", "NRML"),
            ("BSE", "BSE", "CNC", "NRML"),
        ] {
            let mut app = test_app();
            app.open_ato_item_new(&master_inst(exchange, segment, "TESTSYM", 12_345));
            let Some(Modal::AtoItem(item)) = app.modals.last() else {
                panic!("{exchange}: the item editor must open");
            };
            assert_eq!(
                item.product(),
                default,
                "{exchange}: a new order takes the segment's delivery product"
            );
            assert!(
                !item.products().contains(&refused),
                "{exchange}: {refused} must not be selectable: {:?}",
                item.products()
            );

            let Some(Modal::AtoItem(mut item)) = app.modals.pop() else {
                panic!("{exchange}: open");
            };
            for _ in 0..=crate::tui::alerts::ATO_PRODUCTS.len() {
                item.select(AtoItemFocus::Product, ControlIntent::Next);
                assert_ne!(
                    item.product(),
                    refused,
                    "{exchange}: stepped onto {refused}"
                );
            }

            let issue = alerts::validate_item(
                exchange, 12_345, "BUY", refused, "LIMIT", "DAY", "", "1", "100.00", "", 1, 0.05,
                None,
            )
            .expect_err("{exchange}: the pair must be refused");
            assert_eq!(issue.field, Some(AtoItemFocus::Product));
        }
    }

    #[tokio::test]
    async fn an_ato_whose_basket_did_not_load_refuses_a_basket_edit() {
        let unattached = {
            let mut row = ato_alert();
            row.basket = None;
            row
        };
        for key in ['a', 'e', 'd', 'c', '[', ']'] {
            let mut app = test_app();
            app.open_alert_edit(&unattached);
            assert!(
                app.modals.last().is_some_and(|modal| matches!(
                    modal,
                    Modal::Alert(form) if form.draft.baseline_unavailable()
                )),
                "{key}: an unattached basket is not an empty one"
            );
            for _ in 0..4 {
                app.on_key(char_key_code(KeyCode::Tab));
            }
            app.on_key(char_key(key));

            assert_eq!(app.modals.len(), 1, "{key}: nothing may open over the form");
            let Some(Modal::Alert(state)) = app.modals.last() else {
                panic!("{key}: the form must stay open");
            };
            assert!(
                issue_message(state).is_some_and(|err| err.contains("did not load")),
                "{key}: the refusal must say why: {:?}",
                state.issue
            );
            assert!(state.draft.is_empty(), "{key}: nothing was staged");
            assert_eq!(
                state.basket_write(),
                None,
                "{key}: the write still says nothing about the basket"
            );
        }

        let mut app = test_app();
        app.open_alert_edit(&unattached);
        app.on_key(char_key_code(KeyCode::Tab));
        app.on_key(char_key_code(KeyCode::Tab));
        for ch in "125".chars() {
            app.on_key(char_key(ch));
        }
        app.on_key(char_key_code(KeyCode::Enter));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("a condition-only edit must reach the review");
        };
        assert_eq!(state.stage, AlertFormStage::Review);
        assert_eq!(
            state.save_blocker(),
            None,
            "an unstated basket is not empty"
        );
        assert!(state.ato);
        assert_eq!(state.basket_write(), None);
    }

    #[test]
    fn the_basket_selects_by_item_while_the_body_scrolls_by_row() {
        let mut app = test_app();
        app.open_alert_edit(&ato_alert());

        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Tab));
        }
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open");
        };
        assert_eq!(state.focus, AlertFormFocus::Basket);

        app.on_key(char_key('c'));
        let Some(Modal::Alert(state)) = app.modals.last() else {
            panic!("the form must stay open");
        };
        assert_eq!(state.draft.len(), 2);
        assert_eq!(state.basket_selected, 1, "the copy takes the selection");
        let items = state.draft.len();

        for (key, expected) in [
            (KeyCode::End, items - 1),
            (KeyCode::Home, 0),
            (KeyCode::Down, 1),
            (KeyCode::Up, 0),
            (KeyCode::PageDown, 0),
            (KeyCode::PageUp, 0),
        ] {
            app.on_key(KeyEvent::new(key, KeyModifiers::NONE));
            let Some(Modal::Alert(state)) = app.modals.last() else {
                panic!("the form must stay open while its basket moves");
            };
            assert_eq!(state.basket_selected, expected, "{key:?}");
        }
    }
}
