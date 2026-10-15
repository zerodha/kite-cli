use crate::{Call, KiteConnect, Request};
use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// OHLC prices returned by the Kite quote endpoints.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Ohlc {
    /// Opening price.
    pub open: f64,
    /// Highest price.
    pub high: f64,
    /// Lowest price.
    pub low: f64,
    /// Closing price (previous session close in quotes).
    pub close: f64,
}

/// A depth level returned by GET /quote.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct DepthItem {
    /// Price of the depth level.
    pub price: f64,
    /// Total quantity at this price.
    pub quantity: i64,
    /// Number of orders at this price.
    pub orders: u32,
}

/// Market depth returned by GET /quote.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Depth {
    /// Buy (bid) depth levels.
    #[serde(default)]
    pub buy: Vec<DepthItem>,
    /// Sell (offer) depth levels.
    #[serde(default)]
    pub sell: Vec<DepthItem>,
}

/// An instrument quote returned by GET /quote.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Quote {
    /// Numeric instrument identifier.
    pub instrument_token: u32,
    /// Time the quote was generated.
    #[serde(with = "crate::time::datetime")]
    pub timestamp: NaiveDateTime,
    /// Last traded price.
    pub last_price: f64,
    /// Quantity of the last trade.
    pub last_quantity: Option<i64>,
    /// Time of the last trade.
    #[serde(default, with = "crate::time::optional_datetime")]
    pub last_trade_time: Option<NaiveDateTime>,
    /// Volume weighted average traded price.
    pub average_price: Option<f64>,
    /// Total volume traded for the day.
    pub volume: Option<i64>,
    /// Total quantity pending on the buy side.
    pub buy_quantity: Option<i64>,
    /// Total quantity pending on the sell side.
    pub sell_quantity: Option<i64>,
    /// Day open, high, low and close prices.
    pub ohlc: Ohlc,
    /// Absolute change from the previous close.
    pub net_change: f64,
    /// Open interest.
    pub oi: Option<f64>,
    /// Day high of open interest.
    pub oi_day_high: Option<f64>,
    /// Day low of open interest.
    pub oi_day_low: Option<f64>,
    /// Lower circuit price limit.
    pub lower_circuit_limit: Option<f64>,
    /// Upper circuit price limit.
    pub upper_circuit_limit: Option<f64>,
    /// Market depth.
    pub depth: Option<Depth>,
}

/// An instrument last price returned by GET /quote/ltp.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct QuoteLtp {
    /// Numeric instrument identifier.
    pub instrument_token: u32,
    /// Last traded price.
    pub last_price: f64,
}

/// An instrument OHLC quote returned by GET /quote/ohlc.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct QuoteOhlc {
    /// Numeric instrument identifier.
    pub instrument_token: u32,
    /// Last traded price.
    pub last_price: f64,
    /// Day open, high, low and close prices.
    pub ohlc: Ohlc,
}

/// An instrument returned by GET /instruments or GET /instruments/{exchange}.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Instrument {
    /// Numeric identifier used to subscribe to market data.
    pub instrument_token: u32,
    /// Exchange's numeric identifier for the instrument.
    pub exchange_token: u32,
    /// Trading symbol of the instrument.
    pub tradingsymbol: String,
    /// Name of the company or instrument.
    pub name: String,
    /// Last traded price at the time of the dump.
    pub last_price: f64,
    /// Expiry date, for derivatives.
    #[serde(default, with = "crate::time::optional_date")]
    pub expiry: Option<NaiveDate>,
    /// Strike price, for options.
    pub strike: f64,
    /// Minimum price movement.
    pub tick_size: f64,
    /// Number of units in one lot.
    pub lot_size: f64,
    /// Instrument type (for example `EQ`, `FUT`, `CE`, `PE`).
    pub instrument_type: String,
    /// Exchange segment of the instrument.
    pub segment: String,
    /// Exchange the instrument trades on.
    pub exchange: String,
}

impl KiteConnect {
    /// `GET /quote`.
    /// Keyed by `EXCHANGE:SYMBOL`.
    pub fn quote(&self, instruments: &[&str]) -> Call<HashMap<String, Quote>> {
        Call::new(
            self.clone(),
            Request::get("/quote").query(instruments.iter().map(|instrument| ("i", *instrument))),
        )
    }

    /// `GET /quote/ltp`.
    /// Keyed by `EXCHANGE:SYMBOL`.
    pub fn ltp(&self, instruments: &[&str]) -> Call<HashMap<String, QuoteLtp>> {
        Call::new(
            self.clone(),
            Request::get("/quote/ltp")
                .query(instruments.iter().map(|instrument| ("i", *instrument))),
        )
    }

    /// `GET /quote/ohlc`.
    /// Keyed by `EXCHANGE:SYMBOL`.
    pub fn ohlc(&self, instruments: &[&str]) -> Call<HashMap<String, QuoteOhlc>> {
        Call::new(
            self.clone(),
            Request::get("/quote/ohlc")
                .query(instruments.iter().map(|instrument| ("i", *instrument))),
        )
    }

    /// `GET /instruments`.
    /// Decoded from CSV.
    pub fn instruments(&self) -> Call<Vec<Instrument>> {
        Call::with_decoder(
            self.clone(),
            Request::get("/instruments"),
            crate::call::decode_csv,
        )
    }

    /// `GET /instruments/{exchange}`.
    /// Decoded from CSV.
    pub fn instruments_by_exchange(&self, exchange: &str) -> Call<Vec<Instrument>> {
        Call::with_decoder(
            self.clone(),
            Request::get(format!(
                "/instruments/{}",
                crate::client::path_segment(exchange)
            )),
            crate::call::decode_csv,
        )
    }
}
