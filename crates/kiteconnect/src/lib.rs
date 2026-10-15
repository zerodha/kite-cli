#![doc = include_str!("../README.md")]
#![warn(missing_docs)]

mod alerts;
mod call;
mod client;
mod enums;
/// Error types and the secret-key redaction helpers.
pub mod error;
mod gtt;
mod historical;
mod http;
mod margins;
mod market;
mod mf;
mod orders;
mod portfolio;
mod request;
mod secret;
mod session;
/// Kite Connect WebSocket ticker.
pub mod ticker;
mod time;
mod user;

pub use call::Call;
pub use client::{DEFAULT_BASE_URL, KiteConnect, KiteConnectBuilder};
pub use enums::{Exchange, OrderType, Product, TransactionType, Validity, Variety};
pub use error::{Error, Result};
pub use orders::{Order, OrderChild, OrderChildError, OrderParams, OrderResponse, Trade};
pub use request::{Body, RawResponse, Request};
pub use secret::{AccessToken, ApiSecret, RefreshToken};
pub use ticker::{
    CloseReason, DEFAULT_TICKER_URL, KiteTicker, Mode, Tick, TickerBuilder, TickerEvent,
    TickerEvents, parse_binary,
};

pub use alerts::{
    Alert, AlertBasket, AlertBasketItem, AlertHistory, AlertHistoryMeta, AlertOhlc, AlertOperator,
    AlertParams, AlertRhsType, AlertStatus, AlertType, BasketCreate, BasketGtt, BasketItemParams,
    BasketModify, BasketOperation, BasketOrderParams, BasketWrite,
};
pub use gtt::{
    Gtt, GttCondition, GttMeta, GttOrder, GttOrderResult, GttParams, GttResponse, GttResult,
    GttTrigger, GttTriggerParams,
};
pub use historical::Candle;
pub use margins::{
    BasketMargins, BasketParams, Charges, ChargesParams, Gst, MarginParams, MarginPnl,
    OrderCharges, OrderChargesParam, OrderMarginParam, OrderMargins,
};
pub use market::{Depth, DepthItem, Instrument, Ohlc, Quote, QuoteLtp, QuoteOhlc};
pub use mf::{
    MfHolding, MfInstrument, MfOrder, MfOrderParams, MfOrderResponse, MfSip, MfSipModifyParams,
    MfSipParams, MfSipResponse, MfTrade,
};
pub use portfolio::{
    AuctionInstrument, ConvertPositionParams, Holding, HoldingAuthParams, HoldingAuthTransferType,
    HoldingAuthType, HoldingCompact, HoldingSummary, HoldingsAuthInstruments, HoldingsAuthResp,
    MtfHolding, Position, PositionType, Positions,
};
pub use session::{UserSession, UserSessionTokens};
pub use user::{
    AllMargins, AvailableMargins, Bank, FullUserMeta, FullUserProfile, Margins, UsedMargins,
    UserMeta, UserProfile,
};
