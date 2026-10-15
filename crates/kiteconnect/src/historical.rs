use crate::{Call, Error, KiteConnect, RawResponse, Request, Result};
use chrono::{DateTime, FixedOffset, NaiveDateTime};
use serde::{Deserialize, Serialize};

/// A candle returned by GET /instruments/historical/{token}/{interval}.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[non_exhaustive]
pub struct Candle {
    /// Start time of the candle.
    pub date: DateTime<FixedOffset>,
    /// Opening price of the candle.
    pub open: f64,
    /// Highest price of the candle.
    pub high: f64,
    /// Lowest price of the candle.
    pub low: f64,
    /// Closing price of the candle.
    pub close: f64,
    /// Traded volume in the candle.
    pub volume: u64,
    /// Open interest, present when requested.
    pub oi: Option<i64>,
}

impl Candle {
    /// Start an OHLCV bar at a single price.
    #[must_use]
    pub fn new(date: DateTime<FixedOffset>, price: f64) -> Self {
        Self {
            date,
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 0,
            oi: None,
        }
    }
}

impl KiteConnect {
    /// `GET /instruments/historical/{token}/{interval}`.
    pub fn historical_data(
        &self,
        instrument_token: u32,
        interval: &str,
        from: NaiveDateTime,
        to: NaiveDateTime,
        continuous: bool,
        oi: bool,
    ) -> Call<Vec<Candle>> {
        let path_interval = crate::client::path_segment(interval);
        Call::with_decoder(
            self.clone(),
            Request::get(format!(
                "/instruments/historical/{instrument_token}/{path_interval}"
            ))
            .query([
                ("from", from.format("%Y-%m-%d %H:%M:%S").to_string()),
                ("to", to.format("%Y-%m-%d %H:%M:%S").to_string()),
                ("continuous", u8::from(continuous).to_string()),
                ("oi", u8::from(oi).to_string()),
                ("instrument_token", instrument_token.to_string()),
                ("interval", interval.to_string()),
            ]),
            decode_candles,
        )
    }
}

fn decode_candles(client: &KiteConnect, response: &RawResponse, path: &str) -> Result<Vec<Candle>> {
    #[derive(Deserialize)]
    struct HistoricalData {
        candles: Vec<Vec<serde_json::Value>>,
    }

    let history: HistoricalData = crate::call::decode(client, response, path)?;
    history.candles
        .into_iter()
        .map(|mut row| {
            row.truncate(7);
            if row.len() == 6 {
                row.push(serde_json::Value::Null);
            }
            let (date, open, high, low, close, volume, oi): (
                String,
                f64,
                f64,
                f64,
                f64,
                serde_json::Value,
                Option<i64>,
            ) = serde_json::from_value(serde_json::Value::Array(row))
                .map_err(|source| Error::decode(path, source))?;
            let volume = volume
                .as_u64()
                .or_else(|| {
                    let value = volume.as_f64()?;
                    #[expect(
                        clippy::cast_precision_loss,
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "accepted volume is integral and nonnegative below the rounded exclusive 2^64 bound"
                    )]
                    (value >= 0.0 && value.fract() == 0.0 && value < u64::MAX as f64)
                        .then_some(value as u64)
                })
                .ok_or_else(|| {
                    use serde::de::Error as _;
                    Error::decode(
                        path,
                        serde_json::Error::custom("volume is not a nonnegative integer"),
                    )
                })?;
            let date = DateTime::parse_from_rfc3339(&date)
                .or_else(|_| DateTime::parse_from_str(&date, "%Y-%m-%dT%H:%M:%S%.f%z"))
                .map_err(|source| {
                    use serde::de::Error as _;
                    Error::decode(path, serde_json::Error::custom(source))
                })?;
            Ok(Candle {
                date,
                open,
                high,
                low,
                close,
                volume,
                oi,
            })
        })
        .collect()
}
