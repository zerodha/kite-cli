use kiteconnect::ticker::Tick;

/// Quote field available for a constant-value alert condition.
///
/// The order is Kite Web's selector order. Keeping the wire name and the display
/// label together means a form selection cannot accidentally submit a label
/// instead of the service's attribute name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Attribute {
    LastTradedPrice,
    HighPrice,
    LowPrice,
    OpenPrice,
    ClosePrice,
    DayChange,
    DayChangePercent,
    IntraDayChange,
    IntraDayChangePercent,
    LastTradedQuantity,
    AverageTradePrice,
    VolumeTraded,
    TotalBuyQuantity,
    TotalSellQuantity,
    OpenInterest,
    OpenInterestDayHigh,
    OpenInterestDayLow,
}

impl Attribute {
    pub const ALL: [Self; 17] = [
        Self::LastTradedPrice,
        Self::HighPrice,
        Self::LowPrice,
        Self::OpenPrice,
        Self::ClosePrice,
        Self::DayChange,
        Self::DayChangePercent,
        Self::IntraDayChange,
        Self::IntraDayChangePercent,
        Self::LastTradedQuantity,
        Self::AverageTradePrice,
        Self::VolumeTraded,
        Self::TotalBuyQuantity,
        Self::TotalSellQuantity,
        Self::OpenInterest,
        Self::OpenInterestDayHigh,
        Self::OpenInterestDayLow,
    ];

    pub fn wire(self) -> &'static str {
        match self {
            Self::LastTradedPrice => "LastTradedPrice",
            Self::HighPrice => "HighPrice",
            Self::LowPrice => "LowPrice",
            Self::OpenPrice => "OpenPrice",
            Self::ClosePrice => "ClosePrice",
            Self::DayChange => "DayChange",
            Self::DayChangePercent => "DayChangePercent",
            Self::IntraDayChange => "IntraDayChange",
            Self::IntraDayChangePercent => "IntraDayChangePercent",
            Self::LastTradedQuantity => "LastTradedQuantity",
            Self::AverageTradePrice => "AverageTradePrice",
            Self::VolumeTraded => "VolumeTraded",
            Self::TotalBuyQuantity => "TotalBuyQuantity",
            Self::TotalSellQuantity => "TotalSellQuantity",
            Self::OpenInterest => "OpenInterest",
            Self::OpenInterestDayHigh => "OpenInterestDayHigh",
            Self::OpenInterestDayLow => "OpenInterestDayLow",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::LastTradedPrice => "Last price",
            Self::HighPrice => "High price",
            Self::LowPrice => "Low price",
            Self::OpenPrice => "Open price",
            Self::ClosePrice => "Close price",
            Self::DayChange => "Day change",
            Self::DayChangePercent => "Day change %",
            Self::IntraDayChange => "Intraday change",
            Self::IntraDayChangePercent => "Intraday change %",
            Self::LastTradedQuantity => "Last traded qty.",
            Self::AverageTradePrice => "Avg. traded price",
            Self::VolumeTraded => "Volume",
            Self::TotalBuyQuantity => "Total buy qty.",
            Self::TotalSellQuantity => "Total sell qty.",
            Self::OpenInterest => "OI",
            Self::OpenInterestDayHigh => "OI day high",
            Self::OpenInterestDayLow => "OI day low",
        }
    }

    pub fn from_wire(wire: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|attribute| attribute.wire() == wire)
    }

    /// Human label for a known wire value, or the untouched server value for an
    /// unknown attribute. The latter is display-only; it must never be sent back
    /// through a form that cannot model it.
    pub fn label_for_wire(wire: &str) -> &str {
        match Self::from_wire(wire) {
            Some(attribute) => attribute.label(),
            None => wire,
        }
    }

    /// Value this condition reads from the latest tick. Derived values intentionally
    /// mirror Kite Web's definitions rather than reuse a price-only convenience.
    pub fn current_value(self, tick: &Tick) -> Option<f64> {
        match self {
            Self::LastTradedPrice => tick.last_price,
            Self::HighPrice => tick.high,
            Self::LowPrice => tick.low,
            Self::OpenPrice => tick.open,
            Self::ClosePrice => tick.close,
            Self::DayChange => Some(tick.last_price? - tick.close?),
            Self::DayChangePercent => percent_change(tick.last_price, tick.close),
            Self::IntraDayChange => Some(tick.last_price? - tick.open?),
            Self::IntraDayChangePercent => percent_change(tick.last_price, tick.open),
            Self::LastTradedQuantity => tick.last_quantity.map(f64::from),
            Self::AverageTradePrice => tick.average_price,
            Self::VolumeTraded => tick.volume.map(quantity_value),
            Self::TotalBuyQuantity => tick.buy_quantity.map(quantity_value),
            Self::TotalSellQuantity => tick.sell_quantity.map(quantity_value),
            Self::OpenInterest => tick.oi.map(quantity_value),
            Self::OpenInterestDayHigh => tick.oi_day_high.map(quantity_value),
            Self::OpenInterestDayLow => tick.oi_day_low.map(quantity_value),
        }
    }

    pub fn cycle(self, delta: isize) -> Self {
        let index = Self::ALL
            .iter()
            .position(|attribute| *attribute == self)
            .expect("all attributes include self")
            .cast_signed();
        Self::ALL[(index + delta).rem_euclid(Self::ALL.len().cast_signed()) as usize]
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "alert levels use f64, matching tick quantity comparisons"
)]
fn quantity_value(value: u64) -> f64 {
    value as f64
}

fn percent_change(last: Option<f64>, base: Option<f64>) -> Option<f64> {
    let (last, base) = (last?, base?);
    (base != 0.0).then(|| (last - base) / base * 100.0)
}
