use crate::tui::app::{
    ORDER_TYPES, PRODUCTS, SegmentClass, Side, TTL_MINUTES, TicketPrefs, VALIDITIES,
    allowed_gtt_products, allowed_products, cycle_product, product_label_index, segment_class,
    validity_params,
};
use crate::tui::data::{CancelOrderTarget, MarginPreview, Position};
use crate::tui::form::{ControlIntent, FormIssue, FormViewport};
use crossterm::event::KeyCode;
use kiteconnect::OrderType;

pub type TicketIssue = FormIssue<TicketField>;

pub type ConversionIssue = FormIssue<ConversionField>;

#[derive(Debug, Clone)]
pub struct ModifyTarget {
    pub parent_order_id: Option<String>,
    pub order_id: String,
    pub variety: String,
    pub quantity_editable: bool,
}

const TICKET_VARIETIES: [&str; 3] = ["regular", "iceberg", "co"];

pub fn ticket_varieties(exchange: &str) -> &'static [&'static str] {
    match exchange {
        "NSE" => &TICKET_VARIETIES,
        "MCX" | "NCO" | "BCD" => &TICKET_VARIETIES[..1],
        _ => &TICKET_VARIETIES[..2],
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TicketKind {
    Regular,
    Gtt { modify: Option<i64> },
    Auction { auction_number: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionType {
    Day,
    Overnight,
}

impl PositionType {
    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "DAY",
            Self::Overnight => "OVERNIGHT",
        }
    }

    fn wire(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Overnight => "overnight",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionConversionChoice {
    pub position_type: PositionType,
    pub signed_quantity: i64,
}

fn position_conversion_choices(position: &Position) -> Vec<PositionConversionChoice> {
    let day_quantity = position.day_buy_quantity - position.day_sell_quantity;
    [
        (PositionType::Day, day_quantity),
        (PositionType::Overnight, position.overnight_quantity),
    ]
    .into_iter()
    .filter_map(|(position_type, signed_quantity)| {
        (signed_quantity != 0).then_some(PositionConversionChoice {
            position_type,
            signed_quantity,
        })
    })
    .collect()
}

fn conversion_target(old_product: &str, exchange: &str) -> &'static str {
    if matches!(old_product, "CNC" | "NRML") {
        "MIS"
    } else if matches!(exchange, "NSE" | "BSE") {
        "CNC"
    } else {
        "NRML"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConversionField {
    Bucket,
    Quantity,
}

impl ConversionField {
    pub const ALL: [Self; 2] = [Self::Bucket, Self::Quantity];
}

#[derive(Debug)]
pub struct PositionConversionState {
    pub exchange: String,
    pub tradingsymbol: String,
    pub old_product: String,
    pub choices: Vec<PositionConversionChoice>,
    pub selected: usize,
    pub qty: String,
    pub focus: ConversionField,
    pub lot_size: u32,
    pub issue: Option<ConversionIssue>,
    pub viewport: FormViewport,
}

impl PositionConversionState {
    pub fn new(position: &Position, lot_size: u32) -> Option<Self> {
        if matches!(position.product.as_str(), "CO" | "BO") {
            return None;
        }
        let choices = position_conversion_choices(position);
        let qty = choices.first()?.signed_quantity.unsigned_abs().to_string();
        Some(Self {
            exchange: position.exchange.clone(),
            tradingsymbol: position.tradingsymbol.clone(),
            old_product: position.product.clone(),
            choices,
            selected: 0,
            qty,
            focus: ConversionField::Bucket,
            lot_size: lot_size.max(1),
            issue: None,
            viewport: FormViewport::default(),
        })
    }

    pub fn choice(&self) -> Option<PositionConversionChoice> {
        self.choices.get(self.selected).copied()
    }

    pub fn new_product(&self) -> &'static str {
        conversion_target(&self.old_product, &self.exchange)
    }

    pub fn move_focus(&mut self, delta: isize) {
        let index = ConversionField::ALL
            .iter()
            .position(|field| *field == self.focus)
            .unwrap_or(0)
            .cast_signed();
        self.focus = ConversionField::ALL
            [(index + delta).rem_euclid(ConversionField::ALL.len().cast_signed()) as usize];
        self.viewport.request_reveal();
    }

    pub fn select(&mut self, field: ConversionField, intent: ControlIntent) {
        self.focus = field;
        self.viewport.request_reveal();
        self.clear_issue(field);
        if field != ConversionField::Bucket || self.choices.is_empty() {
            return;
        }
        let Some(next) = intent.pick(self.selected, self.choices.len()) else {
            return;
        };
        self.selected = next;
        if let Some(choice) = self.choice() {
            self.qty = choice.signed_quantity.unsigned_abs().to_string();
        }
        self.clear_issue(ConversionField::Quantity);
    }

    pub fn clear_issue(&mut self, field: ConversionField) {
        if self
            .issue
            .as_ref()
            .is_some_and(|issue| issue.field == Some(field))
        {
            self.issue = None;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketField {
    Quantity,
    Side,
    OrderType,
    Price,
    Trigger,
    Product,
    GttType,
    Validity,
    TtlMinutes,
    Autoslice,
    Variety,
    Protection,
    Legs,
    Trailing,
    TrailPoints,
    TargetPrice,
    TargetTrigger,
}

impl TicketField {
    pub fn is_choice(self) -> bool {
        matches!(
            self,
            Self::Side
                | Self::OrderType
                | Self::Product
                | Self::GttType
                | Self::Validity
                | Self::TtlMinutes
                | Self::Autoslice
                | Self::Variety
                | Self::Protection
                | Self::Trailing
        )
    }

    fn controls(self, other: TicketField) -> bool {
        match self {
            Self::OrderType => matches!(other, Self::Price | Self::Trigger | Self::Protection),
            Self::Validity => other == Self::TtlMinutes,
            Self::Variety => matches!(other, Self::Legs | Self::Product | Self::Trigger),
            Self::GttType => matches!(other, Self::TargetPrice | Self::TargetTrigger),
            Self::Side => matches!(other, Self::Trailing | Self::TrailPoints),
            Self::Trailing => other == Self::TrailPoints,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GttSeed {
    pub ltp: Option<f64>,
    pub percent: f64,
}

#[expect(
    clippy::struct_excessive_bools,
    reason = "independent order options can be combined"
)]
#[derive(Debug)]
pub struct TicketState {
    pub side: Side,
    pub exchange: String,
    pub tradingsymbol: String,
    pub instrument_token: u32,
    pub modify: Option<ModifyTarget>,
    pub kind: TicketKind,
    pub qty: String,
    pub price: String,
    pub trigger: String,
    pub gtt_oco: bool,
    pub gtt_trailing: bool,
    pub trailing_points: String,
    pub trigger2: String,
    pub price2: String,
    pub order_type: OrderType,
    pub product: usize,
    pub validity: usize,
    pub validity_ttl: usize,
    pub focus: TicketField,
    pub issue: Option<TicketIssue>,
    pub viewport: FormViewport,
    pub tick_size: f64,
    pub lot_size: u32,
    pub freeze_quantity: Option<u32>,
    pub max_order_quantity: Option<u32>,
    pub autoslice: bool,
    pub market_protection: bool,
    pub variety: usize,
    pub iceberg_legs: String,
}

impl TicketState {
    pub fn effective_autoslice(&self) -> bool {
        if self.is_gtt()
            || self.is_auction()
            || self.modify.is_some()
            || matches!(self.active_variety(), "iceberg" | "co")
        {
            return false;
        }
        self.autoslice || self.autoslice_forced()
    }

    pub fn autoslice_forced(&self) -> bool {
        if self.is_gtt()
            || self.is_auction()
            || self.modify.is_some()
            || matches!(self.active_variety(), "iceberg" | "co")
        {
            return false;
        }
        let qty: u64 = self.qty.parse().unwrap_or(0);
        self.freeze_quantity.is_some_and(|fz| qty > u64::from(fz))
    }

    pub fn label(&self) -> String {
        format!("{}:{}", self.exchange, self.tradingsymbol)
    }

    pub fn is_gtt(&self) -> bool {
        matches!(&self.kind, TicketKind::Gtt { .. })
    }

    pub fn is_auction(&self) -> bool {
        matches!(&self.kind, TicketKind::Auction { .. })
    }

    pub fn variety_is_iceberg(&self) -> bool {
        !self.is_gtt()
            && !self.is_auction()
            && self.modify.is_none()
            && TICKET_VARIETIES[self.variety] == "iceberg"
    }

    pub fn variety_is_co(&self) -> bool {
        !self.is_gtt() && !self.is_auction() && self.active_variety() == "co"
    }

    pub fn active_variety(&self) -> &str {
        if self.is_auction() {
            return "auction";
        }
        self.modify
            .as_ref()
            .map_or(TICKET_VARIETIES[self.variety], |target| {
                target.variety.as_str()
            })
    }

    pub fn quantity_editable(&self) -> bool {
        self.modify
            .as_ref()
            .is_none_or(|target| target.quantity_editable)
    }

    fn market_protection_enabled(&self) -> bool {
        !self.is_gtt()
            && !self.is_auction()
            && self.modify.is_none()
            && matches!(self.order_type, OrderType::Market | OrderType::SlM)
    }

    pub fn visible_fields(&self) -> Vec<TicketField> {
        use TicketField as F;
        if self.is_auction() {
            return vec![F::Quantity, F::Price];
        }
        if self.is_gtt() {
            let mut fields = vec![F::Quantity, F::Side, F::GttType, F::Price, F::Trigger];
            if self.gtt_oco {
                fields.push(F::TargetPrice);
                fields.push(F::TargetTrigger);
            }
            if self.side == Side::Sell {
                fields.push(F::Trailing);
                if self.gtt_trailing {
                    fields.push(F::TrailPoints);
                }
            }
            return fields;
        }
        let order_type = self.order_type;
        let co = self.variety_is_co();
        let mut fields = Vec::new();
        if self.quantity_editable() {
            fields.push(F::Quantity);
        }
        if !(co && self.modify.is_some()) {
            fields.push(F::OrderType);
        }
        if matches!(order_type, OrderType::Limit | OrderType::Sl) {
            fields.push(F::Price);
        }
        if (co && self.modify.is_none()) || matches!(order_type, OrderType::Sl | OrderType::SlM) {
            fields.push(F::Trigger);
        }
        if self.modify.is_some() {
            return fields;
        }
        if !co {
            fields.push(F::Product);
            fields.push(F::Validity);
            if self.validity == 2 {
                fields.push(F::TtlMinutes);
            }
            if !self.autoslice_forced() && !self.variety_is_iceberg() {
                fields.push(F::Autoslice);
            }
        }
        if self.market_protection_enabled() {
            fields.push(F::Protection);
        }
        fields.push(F::Variety);
        if self.variety_is_iceberg() {
            fields.push(F::Legs);
        }
        fields
    }

    pub fn focused(&self) -> TicketField {
        let fields = self.visible_fields();
        if fields.contains(&self.focus) {
            self.focus
        } else {
            fields.first().copied().unwrap_or(self.focus)
        }
    }

    pub fn move_focus(&mut self, delta: isize) {
        let fields = self.visible_fields();
        if fields.is_empty() {
            return;
        }
        let current = fields
            .iter()
            .position(|field| *field == self.focus)
            .unwrap_or(0)
            .cast_signed();
        self.focus = fields[(current + delta).rem_euclid(fields.len().cast_signed()) as usize];
        self.viewport.request_reveal();
    }

    pub fn focus_field(&mut self, field: TicketField) {
        if self.visible_fields().contains(&field) {
            self.focus = field;
            self.viewport.request_reveal();
        }
    }

    pub fn clear_issue(&mut self, field: TicketField) {
        if self.issue.as_ref().is_some_and(|issue| {
            issue
                .field
                .is_some_and(|failed| failed == field || field.controls(failed))
        }) {
            self.issue = None;
        }
    }

    pub fn select(&mut self, field: TicketField, intent: ControlIntent, seed: GttSeed) {
        self.focus_field(field);
        if field.is_choice() {
            self.clear_issue(field);
        }
        match field {
            TicketField::OrderType => self.select_order_type(intent),
            TicketField::Product => self.select_product(intent),
            TicketField::Validity => {
                if let Some(next) = intent.pick(self.validity, VALIDITIES.len()) {
                    self.validity = next;
                }
            }
            TicketField::TtlMinutes => {
                if let Some(next) = intent.pick(self.validity_ttl, TTL_MINUTES.len()) {
                    self.validity_ttl = next;
                }
            }
            TicketField::Autoslice => {
                if let Some(next) = intent.pick(usize::from(self.autoslice), 2) {
                    self.autoslice = next == 1;
                }
            }
            TicketField::Protection => {
                if self.market_protection_enabled()
                    && let Some(next) = intent.pick(usize::from(self.market_protection), 2)
                {
                    self.market_protection = next == 1;
                }
            }
            TicketField::Variety => self.select_variety(intent),
            TicketField::Side => self.select_side(intent),
            TicketField::GttType => self.select_gtt_type(intent, seed),
            TicketField::Trailing => self.select_trailing(intent),
            TicketField::Quantity
            | TicketField::Price
            | TicketField::Trigger
            | TicketField::Legs
            | TicketField::TrailPoints
            | TicketField::TargetPrice
            | TicketField::TargetTrigger => {}
        }
        if !self.visible_fields().contains(&self.focus) {
            self.focus = field;
        }
    }

    fn select_order_type(&mut self, intent: ControlIntent) {
        let count = if self.variety_is_co() && self.modify.is_none() {
            2
        } else {
            ORDER_TYPES.len()
        };
        let Some(next) = intent.pick(
            ORDER_TYPES
                .iter()
                .position(|t| *t == self.order_type)
                .unwrap_or(0),
            count,
        ) else {
            return;
        };
        self.order_type = ORDER_TYPES[next];
        if !self.market_protection_enabled() {
            self.market_protection = false;
        }
        self.clear_issue(TicketField::OrderType);
    }

    fn select_product(&mut self, intent: ControlIntent) {
        if self.modify.is_some() {
            return;
        }
        let allowed = if self.is_gtt() {
            allowed_gtt_products(&self.exchange)
        } else {
            allowed_products(&self.exchange)
        };
        match intent {
            ControlIntent::Focus => {}
            ControlIntent::Option(index) => {
                if let Some(label) = allowed.get(index) {
                    self.product = product_label_index(label);
                }
            }
            ControlIntent::Previous => self.product = cycle_product(self.product, allowed, -1),
            ControlIntent::Next => self.product = cycle_product(self.product, allowed, 1),
        }
    }

    fn select_variety(&mut self, intent: ControlIntent) {
        let varieties = ticket_varieties(&self.exchange);
        let Some(next) = intent.pick(self.variety, varieties.len()) else {
            return;
        };
        self.variety = next;
        self.issue = None;
        if self.variety_is_co() {
            if !matches!(self.order_type, OrderType::Market | OrderType::Limit) {
                self.order_type = OrderType::Limit;
            }
            self.product = product_label_index("MIS");
            self.validity = 0;
            self.autoslice = false;
        } else if !matches!(self.order_type, OrderType::Sl | OrderType::SlM) {
            self.trigger.clear();
        }
    }

    fn select_side(&mut self, intent: ControlIntent) {
        let current = usize::from(self.side == Side::Sell);
        let Some(next) = intent.pick(current, 2) else {
            return;
        };
        self.side = if next == 1 { Side::Sell } else { Side::Buy };
        if self.side == Side::Buy {
            self.gtt_trailing = false;
        }
    }

    fn select_gtt_type(&mut self, intent: ControlIntent, seed: GttSeed) {
        let Some(next) = intent.pick(usize::from(self.gtt_oco), 2) else {
            return;
        };
        self.gtt_oco = next == 1;
        if !self.gtt_oco || !self.trigger2.trim().is_empty() {
            return;
        }
        let Some(ltp) = seed.ltp else { return };
        let lower = gtt_seed_trigger(ltp, -seed.percent, self.tick_size).unwrap_or_default();
        let upper = gtt_seed_trigger(ltp, seed.percent, self.tick_size).unwrap_or_default();
        if self.trigger.trim().is_empty()
            || self
                .trigger
                .parse::<f64>()
                .is_ok_and(|trigger| trigger >= ltp)
        {
            self.trigger = lower;
        }
        self.trigger2 = upper;
        if self.price.parse::<f64>().ok() == tick_price(ltp, self.tick_size).parse::<f64>().ok() {
            self.price = self.trigger.clone();
        }
        if self.price2.trim().is_empty() {
            self.price2 = self.trigger2.clone();
        }
    }

    fn buffer_mut(&mut self, field: TicketField) -> Option<&mut String> {
        match field {
            TicketField::Quantity => Some(&mut self.qty),
            TicketField::Price => Some(&mut self.price),
            TicketField::Trigger => Some(&mut self.trigger),
            TicketField::TargetPrice => Some(&mut self.price2),
            TicketField::TargetTrigger => Some(&mut self.trigger2),
            TicketField::TrailPoints => Some(&mut self.trailing_points),
            TicketField::Legs => Some(&mut self.iceberg_legs),
            _ => None,
        }
    }

    pub fn edit(&mut self, field: TicketField, code: KeyCode) {
        match field {
            TicketField::Quantity => edit_quantity(&mut self.qty, code),
            TicketField::Legs => edit_legs(&mut self.iceberg_legs, code),
            _ => match self.buffer_mut(field) {
                Some(buffer) => edit_numeric(buffer, code),
                None => return,
            },
        }
        self.clear_issue(field);
        self.viewport.request_reveal();
    }

    pub fn step(&mut self, field: TicketField, dir: i8, ltp: f64) {
        let tick = self.tick_size;
        let lot = self.lot_size;
        match field {
            TicketField::Quantity => self.qty = step_qty(&self.qty, lot, dir),
            TicketField::Legs => self.iceberg_legs = step_legs(&self.iceberg_legs, dir),
            TicketField::TtlMinutes => {
                self.validity_ttl = (self.validity_ttl.cast_signed() + isize::from(dir))
                    .rem_euclid(TTL_MINUTES.len().cast_signed())
                    as usize;
            }
            TicketField::TrailPoints => {
                self.trailing_points = step_price(&self.trailing_points, tick, dir, tick);
            }
            _ => match self.buffer_mut(field) {
                Some(buffer) => *buffer = step_price(buffer, tick, dir, ltp),
                None => return,
            },
        }
        self.clear_issue(field);
        self.viewport.request_reveal();
    }

    fn select_trailing(&mut self, intent: ControlIntent) {
        if self.side != Side::Sell {
            return;
        }
        let Some(next) = intent.pick(usize::from(self.gtt_trailing), 2) else {
            return;
        };
        self.gtt_trailing = next == 1;
        if self.gtt_trailing && self.trailing_points.is_empty() {
            self.trailing_points = "2.00".to_string();
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmEffect {
    Logout,
    Local,
    KiteRequest,
}

#[derive(Debug)]
pub enum PendingAction {
    Logout,
    DiscardAlertDraft,
    SetAlertStatus {
        uuid: String,
        label: String,
        status: kiteconnect::AlertStatus,
        alert_type: kiteconnect::AlertType,
    },
    Place {
        variety: String,
        form: Vec<(String, String)>,
    },
    Modify {
        variety: String,
        order_id: String,
        form: Vec<(String, String)>,
    },
    ConvertPosition {
        form: Vec<(String, String)>,
    },
    AuthoriseHoldings,
    CancelOrders {
        targets: Vec<CancelOrderTarget>,
    },
    DeleteGtt {
        trigger_id: i64,
    },
    PlaceGtt {
        form: Vec<(String, String)>,
    },
    ModifyGtt {
        trigger_id: i64,
        form: Vec<(String, String)>,
    },
    DeleteAlert {
        uuid: String,
        label: String,
    },
    DeleteNote {
        id: u64,
        label: String,
    },
    DeleteWatchlist {
        watch: String,
        name: String,
    },
}

impl PendingAction {
    pub fn effect(&self) -> ConfirmEffect {
        match self {
            Self::Logout => ConfirmEffect::Logout,
            Self::DeleteNote { .. } | Self::DiscardAlertDraft => ConfirmEffect::Local,
            Self::Place { .. }
            | Self::Modify { .. }
            | Self::ConvertPosition { .. }
            | Self::AuthoriseHoldings
            | Self::CancelOrders { .. }
            | Self::DeleteGtt { .. }
            | Self::PlaceGtt { .. }
            | Self::ModifyGtt { .. }
            | Self::DeleteAlert { .. }
            | Self::SetAlertStatus { .. }
            | Self::DeleteWatchlist { .. } => ConfirmEffect::KiteRequest,
        }
    }
}

#[derive(Debug)]
pub struct ConfirmState {
    pub title: String,
    pub lines: Vec<(String, String)>,
    pub action: PendingAction,
    pub preview_request: Option<serde_json::Value>,
    pub preview_seq: Option<u64>,
    pub margin: Option<Result<MarginPreview, String>>,
    pub ticket_prefs: Option<(SegmentClass, TicketPrefs)>,
    pub viewport: FormViewport,
}

impl ConfirmState {
    pub(crate) fn plain(
        title: String,
        lines: Vec<(String, String)>,
        action: PendingAction,
    ) -> Self {
        Self {
            title,
            lines,
            action,
            preview_request: None,
            preview_seq: None,
            margin: None,
            ticket_prefs: None,
            viewport: FormViewport::default(),
        }
    }
}

pub fn edit_numeric(buffer: &mut String, code: KeyCode) {
    match code {
        KeyCode::Backspace => {
            buffer.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() => buffer.push(c),
        KeyCode::Char('.') if !buffer.contains('.') => buffer.push('.'),
        _ => {}
    }
}

pub fn edit_signed_numeric(buffer: &mut String, code: KeyCode) {
    if code == KeyCode::Char('-') && buffer.is_empty() {
        buffer.push('-');
    } else {
        edit_numeric(buffer, code);
    }
}

const MAX_QTY_DIGITS: usize = 10;

pub fn edit_quantity(buffer: &mut String, code: KeyCode) {
    match code {
        KeyCode::Backspace => {
            buffer.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            if buffer.len() >= MAX_QTY_DIGITS {
                return;
            }
            let mut candidate = buffer.clone();
            candidate.push(c);
            let collapsed = candidate.trim_start_matches('0');
            let collapsed = if collapsed.is_empty() { "0" } else { collapsed };
            if collapsed.parse::<u32>().is_ok() {
                *buffer = collapsed.to_string();
            }
        }
        _ => {}
    }
}

fn edit_legs(buffer: &mut String, code: KeyCode) {
    match code {
        KeyCode::Backspace => {
            buffer.pop();
        }
        KeyCode::Char(c) if c.is_ascii_digit() && buffer.chars().count() < 2 => buffer.push(c),
        _ => {}
    }
}

pub fn seed_qty(qty: i64, lot_size: u32) -> String {
    if qty > 0 {
        qty.to_string()
    } else {
        lot_size.max(1).to_string()
    }
}

fn step_qty(current: &str, lot: u32, dir: i8) -> String {
    let lot = lot.max(1);
    let trimmed = current.trim();
    let next = match trimmed.parse::<u32>() {
        Ok(qty) if dir >= 0 => qty.saturating_add(lot),
        Ok(qty) => qty.saturating_sub(lot).max(lot),
        Err(_) if !trimmed.is_empty() && trimmed.bytes().all(|b| b.is_ascii_digit()) => {
            if dir >= 0 {
                u32::MAX
            } else {
                u32::MAX.saturating_sub(lot).max(lot)
            }
        }
        Err(_) => lot,
    };
    next.to_string()
}

fn step_price(current: &str, tick: f64, dir: i8, fallback: f64) -> String {
    let tick = if tick > 0.0 { tick } else { 0.05 };
    let value = match current.trim().parse::<f64>() {
        Ok(v) => {
            let snapped = (v / tick).round() * tick;
            (snapped + f64::from(dir) * tick).max(0.0)
        }
        Err(_) if fallback > 0.0 => ((fallback / tick).round() * tick).max(0.0),
        Err(_) => (f64::from(dir) * tick).max(0.0),
    };
    tick_price(value, tick)
}

pub fn tick_price(price: f64, tick: f64) -> String {
    let tick = if tick > 0.0 { tick } else { 0.05 };
    let decimals = (0_usize..=8)
        .find(|decimals| {
            let scale = 10_f64.powi(i32::try_from(*decimals).expect("bounded TUI value fits i32"));
            ((tick * scale).round() - tick * scale).abs() < 1e-9
        })
        .unwrap_or(8)
        .max(2);
    format!("{price:.decimals$}")
}

pub fn step_signed_number(current: &str, step: f64, dir: i8, fallback: f64) -> String {
    let step = if step > 0.0 { step } else { 0.05 };
    let value = match current.trim().parse::<f64>() {
        Ok(value) if value.is_finite() => {
            let snapped = (value / step).round() * step;
            snapped + f64::from(dir) * step
        }
        _ if fallback.is_finite() => (fallback / step).round() * step,
        _ => f64::from(dir) * step,
    };
    let value = if value.abs() < 1e-12 { 0.0 } else { value };
    tick_price(value, step)
}

const GTT_MIN_TRIGGER: f64 = 0.05;

pub fn gtt_default_trigger_percent(segment: &str) -> f64 {
    if segment.ends_with("-OPT") {
        10.0
    } else if segment.ends_with("-FUT") {
        2.0
    } else {
        5.0
    }
}

pub fn gtt_seed_trigger(ltp: f64, percent: f64, tick: f64) -> Option<String> {
    if ltp <= 0.0 {
        return None;
    }
    let tick = if tick > 0.0 { tick } else { 0.05 };
    let raw = ltp * (1.0 + percent / 100.0);
    let value = ((raw / tick) + 1e-9).floor() * tick;
    (value >= GTT_MIN_TRIGGER).then(|| tick_price(value, tick))
}

fn step_legs(current: &str, dir: i8) -> String {
    let base = current.trim().parse::<i64>().unwrap_or(2);
    (base + i64::from(dir)).clamp(2, 50).to_string()
}

fn on_tick(value: f64, tick: f64) -> bool {
    if tick <= 0.0 {
        return true;
    }
    let steps = (value / tick).round();
    (value - steps * tick).abs() < 1e-6
}

fn require_max_qty(state: &TicketState, quantity: u64) -> Result<(), TicketIssue> {
    if let Some(max) = state.max_order_quantity
        && quantity > u64::from(max)
    {
        return Err(FormIssue::at(
            TicketField::Quantity,
            format!("quantity {quantity} exceeds max order quantity {max}"),
        ));
    }
    Ok(())
}

fn require_on_tick(
    value: f64,
    name: &str,
    field: TicketField,
    tick: f64,
) -> Result<(), TicketIssue> {
    if on_tick(value, tick) {
        Ok(())
    } else {
        Err(FormIssue::at(
            field,
            format!("{name} must be a multiple of tick size {tick}"),
        ))
    }
}

fn parse_price(raw: &str, name: &str, field: TicketField) -> Result<f64, TicketIssue> {
    raw.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| FormIssue::at(field, format!("{name} must be a positive number")))
}

fn parse_gtt_trigger(raw: &str, label: &str, field: TicketField) -> Result<f64, TicketIssue> {
    if raw.trim().is_empty() {
        return Err(FormIssue::at(field, format!("{label} is required")));
    }
    raw.trim()
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value >= GTT_MIN_TRIGGER)
        .ok_or_else(|| {
            FormIssue::at(
                field,
                format!("Trigger cannot be created with {label} less than {GTT_MIN_TRIGGER}"),
            )
        })
}

fn parse_qty(state: &TicketState) -> Result<u64, TicketIssue> {
    let qty: u64 = state
        .qty
        .parse()
        .ok()
        .filter(|qty| *qty > 0)
        .ok_or_else(|| {
            FormIssue::at(TicketField::Quantity, "quantity must be a positive integer")
        })?;
    if state.lot_size > 1 && !qty.is_multiple_of(u64::from(state.lot_size)) {
        return Err(FormIssue::at(
            TicketField::Quantity,
            format!("quantity must be a multiple of lot size {}", state.lot_size),
        ));
    }
    Ok(qty)
}

pub fn build_position_conversion_confirm(
    state: &PositionConversionState,
) -> Result<ConfirmState, ConversionIssue> {
    let choice = state.choice().ok_or_else(|| {
        FormIssue::at(
            ConversionField::Bucket,
            "no convertible position quantity is available",
        )
    })?;
    let available = choice.signed_quantity.unsigned_abs();
    let quantity: u64 = state
        .qty
        .parse()
        .ok()
        .filter(|quantity| *quantity > 0)
        .ok_or_else(|| {
            FormIssue::at(
                ConversionField::Quantity,
                "quantity must be a positive integer",
            )
        })?;
    if quantity > available {
        return Err(FormIssue::at(
            ConversionField::Quantity,
            format!("quantity must not exceed available quantity {available}"),
        ));
    }
    if state.lot_size > 1 && !quantity.is_multiple_of(u64::from(state.lot_size)) {
        return Err(FormIssue::at(
            ConversionField::Quantity,
            format!("quantity must be a multiple of lot size {}", state.lot_size),
        ));
    }
    let new_product = state.new_product();
    let transaction_type = if choice.signed_quantity > 0 {
        "BUY"
    } else {
        "SELL"
    };
    let form = vec![
        ("exchange".to_string(), state.exchange.clone()),
        ("tradingsymbol".to_string(), state.tradingsymbol.clone()),
        ("transaction_type".to_string(), transaction_type.to_string()),
        (
            "position_type".to_string(),
            choice.position_type.wire().to_string(),
        ),
        ("quantity".to_string(), quantity.to_string()),
        ("old_product".to_string(), state.old_product.clone()),
        ("new_product".to_string(), new_product.to_string()),
    ];
    Ok(ConfirmState::plain(
        format!("Convert {}:{}", state.exchange, state.tradingsymbol),
        vec![
            ("Type".to_string(), choice.position_type.label().to_string()),
            ("Side".to_string(), transaction_type.to_string()),
            (
                "Product".to_string(),
                format!("{} → {new_product}", state.old_product),
            ),
            ("Quantity".to_string(), quantity.to_string()),
        ],
        PendingAction::ConvertPosition { form },
    ))
}

fn build_auction_confirm(
    state: &TicketState,
    auction_number: &str,
) -> Result<ConfirmState, TicketIssue> {
    let quantity = parse_qty(state)?;
    require_max_qty(state, quantity)?;
    let price = parse_price(&state.price, "price", TicketField::Price)?;
    require_on_tick(price, "price", TicketField::Price, state.tick_size)?;
    if auction_number.trim().is_empty() {
        return Err(FormIssue::general("auction number is required"));
    }
    let form = vec![
        ("exchange".to_string(), state.exchange.clone()),
        ("tradingsymbol".to_string(), state.tradingsymbol.clone()),
        ("transaction_type".to_string(), "SELL".to_string()),
        ("product".to_string(), "CNC".to_string()),
        ("order_type".to_string(), "LIMIT".to_string()),
        ("quantity".to_string(), quantity.to_string()),
        ("price".to_string(), format!("{price}")),
        ("validity".to_string(), "DAY".to_string()),
        ("auction_number".to_string(), auction_number.to_string()),
    ];
    Ok(ConfirmState::plain(
        format!("Auction SELL {}", state.label()),
        vec![
            ("Auction".to_string(), auction_number.to_string()),
            ("Qty".to_string(), quantity.to_string()),
            ("Type".to_string(), format!("LIMIT @ {price}")),
            ("Product".to_string(), "CNC".to_string()),
            ("Validity".to_string(), "DAY".to_string()),
        ],
        PendingAction::Place {
            variety: "auction".to_string(),
            form,
        },
    ))
}

fn resolve_product(state: &TicketState) -> Result<&'static str, TicketIssue> {
    if state.variety_is_co() {
        if state.exchange != "NSE" {
            return Err(FormIssue::at(
                TicketField::Variety,
                "cover orders are available only on NSE equity",
            ));
        }
        return Ok("MIS");
    }
    let product = PRODUCTS[state.product];
    let allowed = if state.is_gtt() {
        allowed_gtt_products(&state.exchange)
    } else {
        allowed_products(&state.exchange)
    };
    if !allowed.contains(&product) {
        return Err(FormIssue::at(
            TicketField::Product,
            format!("{product} is not a valid product for {}", state.exchange),
        ));
    }
    if state.active_variety() == "amo" && product == "MTF" {
        return Err(FormIssue::at(
            TicketField::Product,
            "AMO supports CNC, NRML, or MIS; MTF is not available",
        ));
    }
    Ok(product)
}

fn build_gtt_confirm(
    state: &TicketState,
    modify: Option<i64>,
    ltp: Option<f64>,
) -> Result<ConfirmState, TicketIssue> {
    if state.gtt_oco {
        return build_gtt_oco_confirm(state, modify, ltp);
    }
    let qty = parse_qty(state)?;
    require_max_qty(state, qty)?;
    let price = parse_price(&state.price, "price", TicketField::Price)?;
    let trigger = parse_gtt_trigger(&state.trigger, "trigger price", TicketField::Trigger)?;
    require_on_tick(price, "price", TicketField::Price, state.tick_size)?;
    require_on_tick(trigger, "trigger", TicketField::Trigger, state.tick_size)?;
    let product = resolve_product(state)?;
    let trailing_points = gtt_trailing_points(state)?;
    let fallback_ref = ltp.is_none();
    let last_price = ltp.unwrap_or(price);
    let mut condition = serde_json::json!({
        "exchange": state.exchange,
        "tradingsymbol": state.tradingsymbol,
        "trigger_values": [trigger],
        "last_price": last_price,
    });
    if let Some(points) = trailing_points {
        condition["trailing_points"] = serde_json::json!([points]);
    }
    let orders = serde_json::json!([{
        "exchange": state.exchange,
        "tradingsymbol": state.tradingsymbol,
        "transaction_type": state.side.label(),
        "quantity": qty,
        "order_type": "LIMIT",
        "product": product,
        "price": price,
    }]);
    let form = vec![
        (
            "type".to_string(),
            if state.gtt_trailing {
                "trailing-single"
            } else {
                "single"
            }
            .to_string(),
        ),
        ("condition".to_string(), condition.to_string()),
        ("orders".to_string(), orders.to_string()),
    ];
    let mut lines = vec![
        ("Symbol".to_string(), state.label()),
        ("Side".to_string(), state.side.label().to_string()),
        ("Qty".to_string(), qty.to_string()),
        ("Price".to_string(), format!("{price}")),
        ("Trigger".to_string(), format!("{trigger}")),
        ("Product".to_string(), product.to_string()),
    ];
    if let Some(points) = trailing_points {
        lines.push(("Trailing".to_string(), format!("{points} points")));
    }
    if fallback_ref {
        lines.push(("Ref price".to_string(), format!("{price} (no live tick)")));
    }
    Ok(gtt_confirmation(
        modify,
        format!("GTT {} {}", state.side.label(), state.label()),
        lines,
        form,
    ))
}

fn build_gtt_oco_confirm(
    state: &TicketState,
    modify: Option<i64>,
    ltp: Option<f64>,
) -> Result<ConfirmState, TicketIssue> {
    let qty = parse_qty(state)?;
    require_max_qty(state, qty)?;
    let price_a = parse_price(&state.price, "price", TicketField::Price)?;
    let trigger_a = parse_gtt_trigger(&state.trigger, "trigger price", TicketField::Trigger)?;
    let price_b = parse_price(&state.price2, "second price", TicketField::TargetPrice)?;
    let trigger_b = parse_gtt_trigger(
        &state.trigger2,
        "second trigger price",
        TicketField::TargetTrigger,
    )?;
    for (value, name, field) in [
        (price_a, "price", TicketField::Price),
        (trigger_a, "trigger", TicketField::Trigger),
        (price_b, "second price", TicketField::TargetPrice),
        (trigger_b, "second trigger", TicketField::TargetTrigger),
    ] {
        require_on_tick(value, name, field, state.tick_size)?;
    }
    if (trigger_a - trigger_b).abs() < f64::EPSILON {
        return Err(FormIssue::at(
            TicketField::TargetTrigger,
            "OCO triggers must be different values",
        ));
    }
    let product = resolve_product(state)?;
    let trailing_points = gtt_trailing_points(state)?;
    let (lower, upper) = if trigger_a < trigger_b {
        ((trigger_a, price_a), (trigger_b, price_b))
    } else {
        ((trigger_b, price_b), (trigger_a, price_a))
    };
    let fallback_ref = ltp.is_none();
    let last_price = ltp.unwrap_or(lower.0.midpoint(upper.0));
    let leg = |price: f64| {
        serde_json::json!({
            "exchange": state.exchange,
            "tradingsymbol": state.tradingsymbol,
            "transaction_type": state.side.label(),
            "quantity": qty,
            "order_type": "LIMIT",
            "product": product,
            "price": price,
        })
    };
    let mut condition = serde_json::json!({
        "exchange": state.exchange,
        "tradingsymbol": state.tradingsymbol,
        "trigger_values": [lower.0, upper.0],
        "last_price": last_price,
    });
    if let Some(points) = trailing_points {
        condition["trailing_points"] = serde_json::json!([points]);
    }
    let orders = serde_json::json!([leg(lower.1), leg(upper.1)]);
    let form = vec![
        (
            "type".to_string(),
            if state.gtt_trailing {
                "trailing-two-leg"
            } else {
                "two-leg"
            }
            .to_string(),
        ),
        ("condition".to_string(), condition.to_string()),
        ("orders".to_string(), orders.to_string()),
    ];
    let mut lines = vec![
        ("Symbol".to_string(), state.label()),
        ("Side".to_string(), state.side.label().to_string()),
        ("Qty".to_string(), qty.to_string()),
        ("Type".to_string(), "OCO (two-leg)".to_string()),
        (
            "Lower leg".to_string(),
            format!("trg {} @ {}", lower.0, lower.1),
        ),
        (
            "Upper leg".to_string(),
            format!("trg {} @ {}", upper.0, upper.1),
        ),
        ("Product".to_string(), product.to_string()),
    ];
    if let Some(points) = trailing_points {
        lines.push(("Trailing".to_string(), format!("{points} points")));
    }
    if fallback_ref {
        lines.push((
            "Ref price".to_string(),
            format!("{last_price} (no live tick)"),
        ));
    }
    Ok(gtt_confirmation(
        modify,
        format!("GTT OCO {} {}", state.side.label(), state.label()),
        lines,
        form,
    ))
}

fn gtt_trailing_points(state: &TicketState) -> Result<Option<f64>, TicketIssue> {
    if state.gtt_trailing {
        if state.side != Side::Sell {
            return Err(FormIssue::at(
                TicketField::Trailing,
                "trailing GTT is available only for SELL triggers",
            ));
        }
        parse_price(
            &state.trailing_points,
            "trailing points",
            TicketField::TrailPoints,
        )
        .map(Some)
    } else {
        Ok(None)
    }
}

fn gtt_confirmation(
    modify: Option<i64>,
    title: String,
    mut lines: Vec<(String, String)>,
    form: Vec<(String, String)>,
) -> ConfirmState {
    match modify {
        Some(trigger_id) => {
            lines.insert(0, ("Trigger ID".to_string(), trigger_id.to_string()));
            ConfirmState::plain(
                format!("MODIFY GTT {trigger_id}"),
                lines,
                PendingAction::ModifyGtt { trigger_id, form },
            )
        }
        None => ConfirmState::plain(title, lines, PendingAction::PlaceGtt { form }),
    }
}

const ICEBERG_MIN_EQUITY_VALUE: f64 = 100_000.0;
const ICEBERG_MIN_FNO_LOTS: u64 = 5;

fn iceberg_size_rule(
    state: &TicketState,
    qty: u64,
    price: Option<f64>,
    ltp: Option<f64>,
) -> Result<(), TicketIssue> {
    let lot = u64::from(state.lot_size.max(1));
    if matches!(segment_class(&state.exchange), SegmentClass::Equity) {
        let Some(ltp) = ltp else { return Ok(()) };
        let mark = price.unwrap_or(ltp);
        if mark <= 0.0 {
            return Ok(());
        }
        #[expect(
            clippy::cast_precision_loss,
            reason = "minimum order value compares quantity with an f64 price"
        )]
        let value = qty as f64 * mark;
        if value < ICEBERG_MIN_EQUITY_VALUE {
            let min_qty = (ICEBERG_MIN_EQUITY_VALUE / mark).ceil();
            return Err(FormIssue::at(
                TicketField::Quantity,
                format!(
                    "Equity Iceberg orders require minimum 1,00,000 value, set order qty. to \
                     atleast {min_qty:.0} (current value: {value:.2})"
                ),
            ));
        }
        return Ok(());
    }
    if qty / lot < ICEBERG_MIN_FNO_LOTS {
        return Err(FormIssue::at(
            TicketField::Quantity,
            if lot == 1 {
                "F&O Iceberg orders require minimum 5 lots/qty.".to_string()
            } else {
                format!(
                    "F&O Iceberg orders require minimum 5 lots (or {} order qty.)",
                    lot * ICEBERG_MIN_FNO_LOTS
                )
            },
        ));
    }
    Ok(())
}

fn iceberg_quantities(
    state: &TicketState,
    qty: u64,
    price: Option<f64>,
    ltp: Option<f64>,
) -> Result<Option<(u64, u64)>, TicketIssue> {
    if state.variety_is_iceberg() {
        if !ticket_varieties(&state.exchange).contains(&"iceberg") {
            return Err(FormIssue::at(
                TicketField::Variety,
                format!("iceberg orders are not available on {}", state.exchange),
            ));
        }
        let legs: u64 = state
            .iceberg_legs
            .trim()
            .parse()
            .ok()
            .filter(|legs| (2..=50).contains(legs))
            .ok_or_else(|| {
                FormIssue::at(TicketField::Legs, "iceberg legs must be between 2 and 50")
            })?;
        let lot = u64::from(state.lot_size.max(1));
        let lots = qty / lot;
        iceberg_size_rule(state, qty, price, ltp)?;
        if lots < legs {
            let unit = if matches!(state.exchange.as_str(), "NSE" | "BSE") {
                "qty."
            } else {
                "lots"
            };
            return Err(FormIssue::at(
                TicketField::Legs,
                format!("Order {unit} should be greater than Iceberg legs"),
            ));
        }
        let per_leg = lots.div_ceil(legs) * lot;
        if let Some(fz) = state.freeze_quantity
            && per_leg > u64::from(fz)
        {
            return Err(FormIssue::at(
                TicketField::Legs,
                format!("iceberg per-leg quantity {per_leg} exceeds freeze limit {fz}"),
            ));
        }
        if let Some(max) = state.max_order_quantity
            && per_leg > u64::from(max)
        {
            return Err(FormIssue::at(
                TicketField::Legs,
                format!("iceberg per-leg quantity {per_leg} exceeds max order quantity {max}"),
            ));
        }
        Ok(Some((legs, per_leg)))
    } else {
        Ok(None)
    }
}

fn regular_prices(state: &TicketState) -> Result<(Option<f64>, Option<f64>), TicketIssue> {
    let order_type = state.order_type;
    let fresh_cover = state.variety_is_co() && state.modify.is_none();
    if fresh_cover && !matches!(order_type, OrderType::Market | OrderType::Limit) {
        return Err(FormIssue::at(
            TicketField::OrderType,
            "cover orders support MARKET or LIMIT entry orders",
        ));
    }
    let needs_price = matches!(order_type, OrderType::Limit | OrderType::Sl);
    let needs_trigger = fresh_cover || matches!(order_type, OrderType::Sl | OrderType::SlM);

    let price = needs_price
        .then(|| parse_price(&state.price, "price", TicketField::Price))
        .transpose()?;
    let trigger = needs_trigger
        .then(|| parse_price(&state.trigger, "trigger price", TicketField::Trigger))
        .transpose()?;

    if fresh_cover
        && order_type == OrderType::Limit
        && let (Some(price), Some(trigger)) = (price, trigger)
    {
        let invalid = match state.side {
            Side::Buy => trigger > price,
            Side::Sell => trigger < price,
        };
        if invalid {
            return Err(FormIssue::at(
                TicketField::Trigger,
                match state.side {
                    Side::Buy => "cover-order trigger cannot be higher than price",
                    Side::Sell => "cover-order trigger cannot be lower than price",
                },
            ));
        }
    }

    if let Some(price) = price {
        require_on_tick(price, "price", TicketField::Price, state.tick_size)?;
    }
    if let Some(trigger) = trigger {
        require_on_tick(
            trigger,
            "trigger price",
            TicketField::Trigger,
            state.tick_size,
        )?;
    }

    Ok((price, trigger))
}

struct OrderFields<'a> {
    qty: u64,
    price: Option<f64>,
    trigger: Option<f64>,
    product: &'static str,
    validity: &'static str,
    validity_ttl: Option<u16>,
    iceberg: Option<(u64, u64)>,
    variety: &'a str,
    protection: bool,
}

fn regular_order_form(state: &TicketState, fields: &OrderFields<'_>) -> Vec<(String, String)> {
    let mut form: Vec<(String, String)> = Vec::new();
    let mut push = |key: &str, value: String| form.push((key.to_string(), value));
    if state.modify.is_none() {
        push("exchange", state.exchange.clone());
        push("tradingsymbol", state.tradingsymbol.clone());
        push("transaction_type", state.side.label().to_string());
        push("product", fields.product.to_string());
    }
    push("order_type", state.order_type.to_string());
    if state.quantity_editable() {
        push("quantity", fields.qty.to_string());
    }
    if let Some(price) = fields.price {
        push("price", format!("{price}"));
    }
    if let Some(trigger) = fields.trigger {
        push("trigger_price", format!("{trigger}"));
    }
    if let Some(parent) = state
        .modify
        .as_ref()
        .and_then(|target| target.parent_order_id.as_ref())
    {
        push("parent_order_id", parent.clone());
    }
    if state.modify.is_none() {
        push("validity", fields.validity.to_string());
        if let Some(minutes) = fields.validity_ttl {
            push("validity_ttl", minutes.to_string());
        }
        if state.effective_autoslice() {
            push("autoslice", "true".to_string());
        }
        if fields.protection {
            push("market_protection", "-1".to_string());
        }
        if let Some((legs, per_leg)) = fields.iceberg {
            push("iceberg_legs", legs.to_string());
            push("iceberg_quantity", per_leg.to_string());
        }
    }
    form
}

fn regular_confirmation_lines(
    state: &TicketState,
    fields: &OrderFields<'_>,
) -> Vec<(String, String)> {
    let validity_label = fields.validity_ttl.map_or_else(
        || fields.validity.to_string(),
        |minutes| format!("TTL {minutes}m"),
    );
    let order_type = state.order_type;
    let type_line = match (fields.price, fields.trigger) {
        (Some(price), Some(trigger)) => format!("{order_type} @ {price} trg {trigger}"),
        (Some(price), None) => format!("{order_type} @ {price}"),
        (None, Some(trigger)) => format!("{order_type} trg {trigger}"),
        (None, None) => state.order_type.to_string(),
    };
    let mut lines = vec![
        ("Symbol".to_string(), state.label()),
        ("Side".to_string(), state.side.label().to_string()),
        ("Qty".to_string(), fields.qty.to_string()),
        ("Type".to_string(), type_line),
        ("Product".to_string(), fields.product.to_string()),
        ("Validity".to_string(), validity_label),
    ];
    if let Some((legs, per_leg)) = fields.iceberg {
        lines.push(("Iceberg".to_string(), format!("{legs} legs × {per_leg}")));
    }
    if state.modify.is_none() && state.effective_autoslice() {
        lines.push(("Autoslice".to_string(), "on".to_string()));
    }
    if state.modify.is_none() && fields.protection {
        lines.push(("Protection".to_string(), "automatic".to_string()));
    }
    if state.modify.is_none() && fields.variety != "regular" {
        lines.push(("Variety".to_string(), fields.variety.to_ascii_uppercase()));
    }
    lines
}

pub fn build_confirm(
    state: &TicketState,
    ltp: Option<f64>,
    seq: u64,
) -> Result<ConfirmState, TicketIssue> {
    if let TicketKind::Gtt { modify } = &state.kind {
        return build_gtt_confirm(state, *modify, ltp);
    }
    if let TicketKind::Auction { auction_number } = &state.kind {
        return build_auction_confirm(state, auction_number);
    }
    let qty = parse_qty(state)?;
    let (price, trigger) = regular_prices(state)?;
    let product = resolve_product(state)?;
    let (validity, validity_ttl) = if state.variety_is_co() {
        ("DAY", None)
    } else {
        validity_params(state.validity, state.validity_ttl)
    };
    let iceberg = iceberg_quantities(state, qty, price, ltp)?;
    if iceberg.is_none()
        && !state.effective_autoslice()
        && let Some(max) = state.max_order_quantity
        && qty > u64::from(max)
    {
        return Err(FormIssue::at(
            TicketField::Quantity,
            format!("quantity {qty} exceeds max order quantity {max}"),
        ));
    }
    let fields = OrderFields {
        qty,
        price,
        trigger,
        product,
        validity,
        validity_ttl,
        iceberg,
        variety: if iceberg.is_some() {
            "iceberg"
        } else {
            state.active_variety()
        },
        protection: state.market_protection && state.market_protection_enabled(),
    };
    let form = regular_order_form(state, &fields);
    let mut lines = regular_confirmation_lines(state, &fields);

    if let Some(target) = &state.modify {
        lines.insert(0, ("Order".to_string(), target.order_id.clone()));
        Ok(ConfirmState::plain(
            format!("Modify {} {}", state.side.label(), state.label()),
            lines,
            PendingAction::Modify {
                variety: target.variety.clone(),
                order_id: target.order_id.clone(),
                form,
            },
        ))
    } else {
        let preview_request = serde_json::json!({
            "exchange": state.exchange,
            "tradingsymbol": state.tradingsymbol,
            "transaction_type": state.side.label(),
            "variety": fields.variety,
            "product": fields.product,
            "order_type": state.order_type,
            "quantity": fields.qty,
            "price": fields.price.unwrap_or(0.0),
            "trigger_price": fields.trigger.unwrap_or(0.0),
            "market_protection": if fields.protection { -1 } else { 0 },
        });
        Ok(ConfirmState {
            title: format!("{} {}", state.side.label(), state.label()),
            lines,
            action: PendingAction::Place {
                variety: fields.variety.to_string(),
                form,
            },
            preview_request: Some(preview_request),
            preview_seq: Some(seq),
            margin: None,
            ticket_prefs: Some((
                segment_class(&state.exchange),
                TicketPrefs {
                    product: state.product,
                    autoslice: state.autoslice,
                    order_type: match state.order_type {
                        OrderType::Sl | OrderType::SlM => OrderType::Limit,
                        _ => state.order_type,
                    },
                    validity: state.validity,
                    validity_ttl: state.validity_ttl,
                },
            )),
            viewport: FormViewport::default(),
        })
    }
}

#[cfg(test)]
#[path = "ticket/tests.rs"]
mod tests;
