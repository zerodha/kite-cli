use super::{DataEvent, api_err, send};
use crate::tui::event::AppEvent;
use crate::tui::format;
use kiteconnect::KiteConnect;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChartInterval {
    M1,
    M5,
    M15,
    H1,
    Day,
}

impl ChartInterval {
    pub const ALL: &'static [ChartInterval] = &[
        ChartInterval::M1,
        ChartInterval::M5,
        ChartInterval::M15,
        ChartInterval::H1,
        ChartInterval::Day,
    ];

    pub fn api_name(self) -> &'static str {
        match self {
            ChartInterval::M1 => "minute",
            ChartInterval::M5 => "5minute",
            ChartInterval::M15 => "15minute",
            ChartInterval::H1 => "60minute",
            ChartInterval::Day => "day",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ChartInterval::M1 => "1m",
            ChartInterval::M5 => "5m",
            ChartInterval::M15 => "15m",
            ChartInterval::H1 => "1h",
            ChartInterval::Day => "1D",
        }
    }

    pub fn cycle(self) -> ChartInterval {
        crate::util::cycle_next(Self::ALL, self)
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn candle_range(interval: ChartInterval, now: u64) -> (String, String) {
    let to = format::datetime_ist_string(now);
    let lookback_days = match interval {
        ChartInterval::M1 => 7,
        ChartInterval::M5 => 30,
        ChartInterval::M15 => 90,
        ChartInterval::H1 => 180,
        ChartInterval::Day => 250,
    };
    let from = format::datetime_ist_string(now.saturating_sub(lookback_days * 86_400));
    (from, to)
}

async fn fetch_candles(
    client: &KiteConnect,
    token: u32,
    interval: ChartInterval,
) -> Result<Vec<super::Candle>, String> {
    let (from, to) = candle_range(interval, now_unix());
    let parse = |date: String| {
        date.replace(' ', "T")
            .parse()
            .map_err(|err| format!("bad candle range: {err}"))
    };
    let from = parse(from)?;
    let to = parse(to)?;
    client
        .historical_data(token, interval.api_name(), from, to, false, false)
        .await
        .map_err(api_err)
}

pub fn spawn_fetch_candles(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    token: u32,
    interval: ChartInterval,
) {
    tokio::spawn(async move {
        let result = fetch_candles(&client, token, interval).await;
        send(
            &tx,
            DataEvent::Candles {
                token,
                interval,
                result,
            },
        )
        .await;
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::data::tests::{connect_client, next_event, reply};
    use httpmock::{Method::GET, MockServer};
    use serde_json::{Value, json};

    #[test]
    fn interval_cycle_visits_every_interval_including_hourly() {
        let mut interval = ChartInterval::M1;
        let mut seen = Vec::new();
        for _ in 0..5 {
            seen.push(interval);
            interval = interval.cycle();
        }
        assert_eq!(seen, ChartInterval::ALL);
        assert_eq!(interval, ChartInterval::M1);
        assert_eq!(ChartInterval::H1.label(), "1h");
        assert_eq!(ChartInterval::H1.api_name(), "60minute");
    }

    #[test]
    fn candle_range_keeps_the_existing_lookback_windows() {
        let now = 1_750_000_000;
        for (interval, days) in [
            (ChartInterval::M1, 7),
            (ChartInterval::M5, 30),
            (ChartInterval::M15, 90),
            (ChartInterval::H1, 180),
            (ChartInterval::Day, 250),
        ] {
            let (from, to) = candle_range(interval, now);
            assert_eq!(from, format::datetime_ist_string(now - days * 86_400));
            assert_eq!(to, format::datetime_ist_string(now));
        }
    }

    #[tokio::test]
    async fn real_fixture_reaches_the_chart_through_the_typed_fetch() {
        let server = MockServer::start_async().await;
        let data: Value = serde_json::from_str(include_str!(
            "../../../mock_responses/historical_minute.json"
        ))
        .unwrap();
        let mock = reply(
            &server,
            GET,
            "/instruments/historical/408065/minute",
            data["data"].clone(),
        )
        .await;
        let candles = fetch_candles(&connect_client(&server), 408_065, ChartInterval::M1)
            .await
            .unwrap();
        assert_eq!(candles.len(), 6);
        assert_eq!(
            crate::tui::data::candle_timestamp(&candles[0]),
            "2017-12-15 09:15:00"
        );
        assert_eq!(
            (
                candles[0].open,
                candles[0].high,
                candles[0].low,
                candles[0].close
            ),
            (1704.5, 1705.0, 1699.25, 1702.8)
        );
        assert_eq!(candles[0].volume, 2499);
        mock.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn malformed_history_is_reported_to_the_pane() {
        let server = MockServer::start_async().await;
        let mock = reply(
            &server,
            GET,
            "/instruments/historical/408065/minute",
            json!({}),
        )
        .await;
        let (tx, mut rx) = mpsc::channel(1);
        spawn_fetch_candles(connect_client(&server), tx, 408_065, ChartInterval::M1);
        let DataEvent::Candles {
            result: Err(error), ..
        } = next_event(&mut rx).await
        else {
            panic!("expected pane error")
        };
        assert!(error.contains("candles"), "{error}");
        mock.assert_calls_async(1).await;
    }
    #[tokio::test]
    async fn typed_candles_accept_integral_float_volume_null_oi_and_extra_columns() {
        let server = MockServer::start_async().await;
        let mock = reply(&server, GET, "/instruments/historical/408065/minute", json!({"candles":[["2017-12-15T09:15:00+0530",1704.5,1705.0,1699.25,1702.8,100.0,null,{"future":"metadata"}]]})).await;
        let candles = fetch_candles(&connect_client(&server), 408_065, ChartInterval::M1)
            .await
            .unwrap();
        assert_eq!(candles[0].volume, 100);
        assert_eq!(candles[0].oi, None);
        mock.assert_calls_async(1).await;
    }

    #[tokio::test]
    async fn typed_candles_reject_invalid_volume_and_timestamp() {
        for row in [
            json!(["2017-12-15T09:15:00+0530", 1.0, 2.0, 0.5, 1.5, -1]),
            json!(["2017-12-15T09:15:00+0530", 1.0, 2.0, 0.5, 1.5, 1.5]),
            json!(["2017-12-15T09:15:00+0530", 1.0, 2.0, 0.5, 1.5, 1e20]),
            json!(["garbage", 1.0, 2.0, 0.5, 1.5, 100]),
        ] {
            let server = MockServer::start_async().await;
            let mock = reply(
                &server,
                GET,
                "/instruments/historical/408065/minute",
                json!({"candles":[row]}),
            )
            .await;
            let error = fetch_candles(&connect_client(&server), 408_065, ChartInterval::M1)
                .await
                .unwrap_err();
            assert!(
                error.starts_with("bad /instruments/historical/408065/minute payload"),
                "{error}"
            );
            mock.assert_calls_async(1).await;
        }
    }
}
