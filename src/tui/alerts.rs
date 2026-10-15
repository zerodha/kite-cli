pub use crate::alert_attributes::Attribute;
use crate::tui::app::AtoItemFocus;
use crate::tui::data::FetchedBasketItem;
use crate::tui::form::FormIssue;
use kiteconnect::{
    AlertParams, AlertStatus, BasketCreate, BasketGtt, BasketItemParams, BasketModify,
    BasketOperation, BasketOrderParams, BasketWrite,
};
use serde_json::Value;

pub type ItemIssue = FormIssue<AtoItemFocus>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Equal,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

impl Operator {
    pub const ALL: [Self; 5] = [
        Self::Equal,
        Self::GreaterThan,
        Self::GreaterThanOrEqual,
        Self::LessThan,
        Self::LessThanOrEqual,
    ];

    pub fn wire(self) -> &'static str {
        match self {
            Self::Equal => "==",
            Self::GreaterThan => ">",
            Self::GreaterThanOrEqual => ">=",
            Self::LessThan => "<",
            Self::LessThanOrEqual => "<=",
        }
    }

    pub fn from_wire(wire: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|operator| operator.wire() == wire)
    }

    pub fn matches(self, lhs: f64, rhs: f64) -> bool {
        match self {
            #[expect(
                clippy::float_cmp,
                reason = "wire equality operator requires exact IEEE equality"
            )]
            Self::Equal => lhs == rhs,
            Self::GreaterThan => lhs > rhs,
            Self::GreaterThanOrEqual => lhs >= rhs,
            Self::LessThan => lhs < rhs,
            Self::LessThanOrEqual => lhs <= rhs,
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::GreaterThan | Self::GreaterThanOrEqual => "▲",
            Self::LessThan | Self::LessThanOrEqual => "▼",
            Self::Equal => "\u{2022}",
        }
    }
}

pub fn operator_glyph(operator: &str) -> &'static str {
    Operator::from_wire(operator).map_or("\u{2022}", Operator::glyph)
}

pub fn validate_level(
    attribute: Attribute,
    operator: Operator,
    level: f64,
    current_value: Option<f64>,
) -> Result<(), String> {
    if !matches!(
        attribute,
        Attribute::DayChange
            | Attribute::DayChangePercent
            | Attribute::IntraDayChange
            | Attribute::IntraDayChangePercent
    ) && level <= 0.0
    {
        return Err(format!("{} level must be positive", attribute.label()));
    }
    match current_value {
        Some(value) if operator.matches(value, level) => Err(format!(
            "condition is already true at current {} ({value:.2})",
            attribute.label()
        )),
        _ => Ok(()),
    }
}

pub fn lhs_matches(
    lhs_exchange: &str,
    lhs_tradingsymbol: &str,
    instrument_exchange: &str,
    instrument_segment: &str,
    instrument_tradingsymbol: &str,
) -> bool {
    if !lhs_tradingsymbol.eq_ignore_ascii_case(instrument_tradingsymbol) {
        return false;
    }
    if lhs_exchange.eq_ignore_ascii_case("INDICES") {
        return instrument_segment.eq_ignore_ascii_case("INDICES");
    }
    lhs_exchange.eq_ignore_ascii_case(instrument_exchange)
}

fn cmp_ignore_ascii_case(left: &str, right: &str) -> std::cmp::Ordering {
    left.bytes()
        .map(|byte| byte.to_ascii_uppercase())
        .cmp(right.bytes().map(|byte| byte.to_ascii_uppercase()))
}

#[derive(Debug, Default)]
pub struct AlertIndex {
    by_symbol: Vec<(String, Vec<usize>)>,
}

impl AlertIndex {
    pub fn build<'a>(symbols: impl Iterator<Item = &'a str>) -> Self {
        let mut by_symbol: Vec<(String, Vec<usize>)> = Vec::new();
        for (idx, symbol) in symbols.enumerate() {
            let key = symbol.to_ascii_uppercase();
            match by_symbol.binary_search_by(|(known, _)| cmp_ignore_ascii_case(known, &key)) {
                Ok(at) => by_symbol[at].1.push(idx),
                Err(at) => by_symbol.insert(at, (key, vec![idx])),
            }
        }
        Self { by_symbol }
    }

    pub fn candidates(&self, tradingsymbol: &str) -> &[usize] {
        match self
            .by_symbol
            .binary_search_by(|(known, _)| cmp_ignore_ascii_case(known, tradingsymbol))
        {
            Ok(at) => &self.by_symbol[at].1,
            Err(_) => &[],
        }
    }
}

pub const BASKET_NAME: &str = "alerts-basket";

pub const MAX_BASKET_ITEMS: usize = 20;

pub const ATO_VARIETY: &str = "regular";

pub const ATO_PRODUCTS: [&str; 3] = ["CNC", "MIS", "NRML"];

pub const ATO_ORDER_TYPES: [&str; 3] = ["MARKET", "LIMIT", "SL"];

pub const ATO_VALIDITIES: [&str; 3] = ["DAY", "IOC", "TTL"];

pub const ATO_TTL_MINUTES: (u32, u32) = (1, 180);

const MODELLED_KEYS: [&str; 15] = [
    "variety",
    "transaction_type",
    "product",
    "order_type",
    "validity",
    "validity_ttl",
    "quantity",
    "disclosed_quantity",
    "price",
    "trigger_price",
    "squareoff",
    "stoploss",
    "trailing_stoploss",
    "gtt",
    "tags",
];

pub fn authorable_params(raw: &Value) -> Option<BasketOrderParams> {
    let object = raw.as_object()?;
    if object
        .keys()
        .any(|key| !MODELLED_KEYS.contains(&key.as_str()))
    {
        return None;
    }
    let params: BasketOrderParams = serde_json::from_value(raw.clone()).ok()?;
    let authorable = params.variety == ATO_VARIETY
        && matches!(params.transaction_type.as_str(), "BUY" | "SELL")
        && ATO_PRODUCTS.contains(&params.product.as_str())
        && ATO_ORDER_TYPES.contains(&params.order_type.as_str())
        && ATO_VALIDITIES.contains(&params.validity.as_str())
        && params.disclosed_quantity == 0
        && params.squareoff == 0.0
        && params.stoploss == 0.0
        && params.trailing_stoploss == 0.0
        && params.gtt == BasketGtt::default()
        && params.tags.is_empty();
    authorable.then_some(params)
}

pub const ATO_EXCLUDED_EXCHANGES: [&str; 4] = ["MCX", "NCO", "CDS", "BCD"];

pub const ATO_EXCLUDED_SYMBOLS: [&str; 13] = [
    "GIFT NIFTY",
    "AUS200",
    "JAPAN225",
    "SHANGHAICHINA",
    "HANGSENG",
    "UK100",
    "FRANCE40",
    "GERMANY40",
    "USCOMPOSITE",
    "US30",
    "US100",
    "US500",
    "US10YRYIELD",
];

pub const ATO_ORDER_EXCHANGES: [&str; 4] = ["NSE", "BSE", "NFO", "BFO"];

pub fn ato_products(exchange: &str) -> &'static [&'static str] {
    const EQUITY: [&str; 2] = ["CNC", "MIS"];
    const DERIVATIVE: [&str; 2] = ["NRML", "MIS"];
    if crate::tui::app::allowed_products(exchange).contains(&"NRML") {
        &DERIVATIVE
    } else {
        &EQUITY
    }
}

pub fn ato_ineligible_reason(lhs_exchange: &str, lhs_tradingsymbol: &str) -> Option<String> {
    if ATO_EXCLUDED_EXCHANGES
        .iter()
        .any(|excluded| lhs_exchange.eq_ignore_ascii_case(excluded))
    {
        return Some(format!("{lhs_exchange} conditions cannot place orders"));
    }
    ATO_EXCLUDED_SYMBOLS
        .iter()
        .any(|excluded| lhs_tradingsymbol.eq_ignore_ascii_case(excluded))
        .then(|| format!("{lhs_tradingsymbol} conditions cannot place orders"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftKey {
    Server(i64),
    Local(u64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DraftItem {
    pub key: DraftKey,
    pub weight: i64,
    pub exchange: String,
    pub tradingsymbol: String,
    pub instrument_token: u32,
    pub params: BasketItemParams,
}

impl DraftItem {
    pub fn editable(&self) -> Option<&BasketOrderParams> {
        match &self.params {
            BasketItemParams::Authored(params) => Some(params),
            _ => None,
        }
    }

    pub fn label(&self) -> String {
        crate::tui::format::instrument_label(&self.exchange, &self.tradingsymbol)
    }

    pub fn summary(&self) -> String {
        let Some(params) = self.editable() else {
            return "Kept as saved".to_string();
        };
        let mut parts = vec![
            params.transaction_type.clone(),
            params.quantity.to_string(),
            params.product.clone(),
            params.order_type.clone(),
        ];
        if params.order_type != "MARKET" && params.price > 0.0 {
            parts.push(format!("{:.2}", params.price));
        }
        if params.trigger_price > 0.0 {
            parts.push(format!("trigger {:.2}", params.trigger_price));
        }
        if params.validity == "TTL" {
            parts.push(format!("TTL {}", params.validity_ttl));
        } else {
            parts.push(params.validity.clone());
        }
        parts.join(" ")
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AtoBasketDraft {
    baseline: Vec<DraftItem>,
    items: Vec<DraftItem>,
    next_local_id: u64,
    baseline_unavailable: bool,
}

impl AtoBasketDraft {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn unavailable() -> Self {
        Self {
            baseline_unavailable: true,
            ..Self::default()
        }
    }

    pub fn baseline_unavailable(&self) -> bool {
        self.baseline_unavailable
    }

    pub fn from_fetched(items: &[FetchedBasketItem]) -> Self {
        let items: Vec<DraftItem> = items
            .iter()
            .map(|item| DraftItem {
                key: DraftKey::Server(item.id.unwrap_or(0)),
                weight: item.weight,
                exchange: item.exchange.clone(),
                tradingsymbol: item.tradingsymbol.clone(),
                instrument_token: item.instrument_token.unwrap_or(0),
                params: match authorable_params(&item.params) {
                    Some(params) => BasketItemParams::Authored(params),
                    None => BasketItemParams::Preserved(item.params.clone()),
                },
            })
            .collect();
        Self {
            baseline: items.clone(),
            items,
            next_local_id: 0,
            baseline_unavailable: false,
        }
    }

    pub fn items(&self) -> &[DraftItem] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn is_full(&self) -> bool {
        self.items.len() >= MAX_BASKET_ITEMS
    }

    pub fn push(
        &mut self,
        exchange: String,
        tradingsymbol: String,
        instrument_token: u32,
        params: BasketOrderParams,
    ) -> Result<(), String> {
        if self.is_full() {
            return Err(format!("a basket holds at most {MAX_BASKET_ITEMS} orders"));
        }
        let key = DraftKey::Local(self.next_local_id);
        self.next_local_id += 1;
        self.items.push(DraftItem {
            key,
            weight: 0,
            exchange,
            tradingsymbol,
            instrument_token,
            params: BasketItemParams::Authored(params),
        });
        Ok(())
    }

    pub fn replace(&mut self, index: usize, params: BasketOrderParams) {
        if let Some(item) = self.items.get_mut(index) {
            item.params = BasketItemParams::Authored(params);
        }
    }

    pub fn remove(&mut self, index: usize) -> Option<DraftItem> {
        (index < self.items.len()).then(|| self.items.remove(index))
    }

    pub fn duplicate(&mut self, index: usize) -> Result<(), String> {
        if self.is_full() {
            return Err(format!("a basket holds at most {MAX_BASKET_ITEMS} orders"));
        }
        let Some(source) = self.items.get(index) else {
            return Ok(());
        };
        let key = DraftKey::Local(self.next_local_id);
        self.next_local_id += 1;
        let copy = DraftItem {
            key,
            ..source.clone()
        };
        self.items.insert(index + 1, copy);
        Ok(())
    }

    pub fn move_item(&mut self, index: usize, delta: isize) -> usize {
        if index >= self.items.len() {
            return index;
        }
        let target = index.saturating_add_signed(delta).min(self.items.len() - 1);
        if target != index {
            let item = self.items.remove(index);
            self.items.insert(target, item);
        }
        target
    }

    pub fn is_unchanged(&self) -> bool {
        self.items == self.baseline
    }

    pub fn removed(&self) -> Vec<&DraftItem> {
        self.baseline
            .iter()
            .filter(|item| !self.items.iter().any(|kept| kept.key == item.key))
            .collect()
    }

    pub fn write(&self, creating: bool) -> Option<BasketWrite> {
        if creating {
            return Some(BasketWrite::Create(BasketCreate::new(
                BASKET_NAME,
                self.insert_operations(),
            )));
        }
        if self.is_unchanged() {
            return None;
        }
        let mut operations = Vec::new();
        for (position, item) in self.items.iter().enumerate() {
            let weight = u32::try_from(position).expect("bounded TUI value fits u32") + 1;
            match item.key {
                DraftKey::Local(_) => operations.push(BasketOperation::Insert {
                    exchange: item.exchange.clone(),
                    tradingsymbol: item.tradingsymbol.clone(),
                    weight,
                    params: item.params.clone(),
                }),
                DraftKey::Server(id) => {
                    let unchanged = self
                        .baseline
                        .iter()
                        .find(|known| known.key == item.key)
                        .is_some_and(|known| {
                            known.params == item.params && known.weight == i64::from(weight)
                        });
                    if !unchanged {
                        operations.push(BasketOperation::Update {
                            id,
                            exchange: item.exchange.clone(),
                            tradingsymbol: item.tradingsymbol.clone(),
                            weight,
                            params: item.params.clone(),
                        });
                    }
                }
            }
        }
        for item in self.removed() {
            if let DraftKey::Server(id) = item.key {
                operations.push(BasketOperation::Delete { id });
            }
        }
        Some(BasketWrite::Modify(BasketModify::new(operations)))
    }

    fn insert_operations(&self) -> Vec<BasketOperation> {
        self.items
            .iter()
            .enumerate()
            .map(|(position, item)| BasketOperation::Insert {
                exchange: item.exchange.clone(),
                tradingsymbol: item.tradingsymbol.clone(),
                weight: u32::try_from(position).expect("bounded TUI value fits u32") + 1,
                params: item.params.clone(),
            })
            .collect()
    }
}

#[expect(clippy::too_many_arguments, reason = "one order's fields, not options")]
pub fn validate_item(
    exchange: &str,
    instrument_token: u32,
    side: &str,
    product: &str,
    order_type: &str,
    validity: &str,
    ttl_minutes: &str,
    quantity: &str,
    price: &str,
    trigger: &str,
    lot_size: u32,
    tick_size: f64,
    freeze_quantity: Option<u32>,
) -> Result<BasketOrderParams, ItemIssue> {
    if !ATO_ORDER_EXCHANGES
        .iter()
        .any(|allowed| exchange.eq_ignore_ascii_case(allowed))
    {
        return Err(FormIssue::general(format!(
            "{exchange} orders are not available in a basket here"
        )));
    }
    if instrument_token == 0 {
        return Err(FormIssue::general(
            "instrument is not in the master yet; try again shortly",
        ));
    }
    if !ato_products(exchange).contains(&product) {
        return Err(FormIssue::at(
            AtoItemFocus::Product,
            format!("{exchange} orders cannot use product {product}"),
        ));
    }
    if !ATO_ORDER_TYPES.contains(&order_type) {
        return Err(FormIssue::at(
            AtoItemFocus::OrderType,
            format!("order type {order_type} is not available here"),
        ));
    }
    if !ATO_VALIDITIES.contains(&validity) {
        return Err(FormIssue::at(
            AtoItemFocus::Validity,
            format!("validity {validity} is not available here"),
        ));
    }

    let quantity = item_quantity(quantity, lot_size, freeze_quantity)?;

    let price = if order_type == "MARKET" {
        0.0
    } else {
        parse_tick_aligned(price, "price", AtoItemFocus::Price, tick_size)?
    };
    let trigger = if order_type == "SL" {
        parse_tick_aligned(trigger, "trigger", AtoItemFocus::Trigger, tick_size)?
    } else {
        0.0
    };
    if order_type == "SL" {
        match side {
            "BUY" if price < trigger => {
                return Err(FormIssue::at(
                    AtoItemFocus::Trigger,
                    "a BUY stop-loss needs its price at or above the trigger",
                ));
            }
            "SELL" if price > trigger => {
                return Err(FormIssue::at(
                    AtoItemFocus::Trigger,
                    "a SELL stop-loss needs its price at or below the trigger",
                ));
            }
            _ => {}
        }
    }
    let validity_ttl = if validity == "TTL" {
        let (low, high) = ATO_TTL_MINUTES;
        ttl_minutes
            .trim()
            .parse()
            .ok()
            .filter(|value| (low..=high).contains(value))
            .ok_or_else(|| {
                FormIssue::at(
                    AtoItemFocus::Ttl,
                    format!("TTL must be {low} to {high} minutes"),
                )
            })?
    } else {
        0
    };

    Ok(BasketOrderParams::new(
        ATO_VARIETY,
        side,
        product,
        order_type,
        validity,
        quantity,
        price,
    )
    .validity_ttl(validity_ttl)
    .trigger_price(trigger))
}

fn item_quantity(
    quantity: &str,
    lot_size: u32,
    freeze_quantity: Option<u32>,
) -> Result<u32, ItemIssue> {
    let quantity: u32 = quantity
        .trim()
        .parse()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            FormIssue::at(
                AtoItemFocus::Quantity,
                "quantity must be a whole number above zero",
            )
        })?;
    let lot = lot_size.max(1);
    if !quantity.is_multiple_of(lot) {
        return Err(FormIssue::at(
            AtoItemFocus::Quantity,
            format!("quantity must be a multiple of the lot size {lot}"),
        ));
    }
    if let Some(freeze) = freeze_quantity
        && quantity > freeze
    {
        return Err(FormIssue::at(
            AtoItemFocus::Quantity,
            format!("quantity {quantity} exceeds the freeze quantity {freeze}"),
        ));
    }

    Ok(quantity)
}

fn parse_tick_aligned(
    raw: &str,
    name: &str,
    field: AtoItemFocus,
    tick_size: f64,
) -> Result<f64, ItemIssue> {
    let value: f64 = raw
        .trim()
        .parse()
        .ok()
        .filter(|value: &f64| value.is_finite() && *value > 0.0)
        .ok_or_else(|| FormIssue::at(field, format!("{name} must be a positive number")))?;
    if tick_size > 0.0 {
        let steps = (value / tick_size).round();
        if (value - steps * tick_size).abs() >= 1e-6 {
            return Err(FormIssue::at(
                field,
                format!("{name} must be a multiple of the tick size {tick_size}"),
            ));
        }
    }
    Ok(value)
}

fn status_for(armed: bool) -> AlertStatus {
    if armed {
        AlertStatus::Enabled
    } else {
        AlertStatus::Disabled
    }
}

pub fn params(
    name: &str,
    exchange: &str,
    tradingsymbol: &str,
    attribute: Attribute,
    operator: Operator,
    level: f64,
    armed: bool,
) -> AlertParams {
    let operator = match operator {
        Operator::LessThanOrEqual => kiteconnect::AlertOperator::Le,
        Operator::GreaterThanOrEqual => kiteconnect::AlertOperator::Ge,
        Operator::LessThan => kiteconnect::AlertOperator::Lt,
        Operator::GreaterThan => kiteconnect::AlertOperator::Gt,
        Operator::Equal => kiteconnect::AlertOperator::Eq,
    };
    AlertParams::new(
        name,
        kiteconnect::AlertType::Simple,
        exchange,
        tradingsymbol,
        attribute.wire(),
        operator,
        kiteconnect::AlertRhsType::Constant,
    )
    .rhs_constant(level)
    .status(status_for(armed))
}

#[cfg(test)]
mod tests;
