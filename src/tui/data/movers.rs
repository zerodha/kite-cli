use super::{DataEvent, api_err, send};
use crate::app_api::screener::{self as screener_api, MoverPreset, ScreenerPreset, ScreenerQuery};
use crate::tui::event::AppEvent;
use kiteconnect::KiteConnect;
use serde::Deserialize;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenerRow {
    pub tradingsymbol: String,
    pub last_price: f64,
    pub change_percent: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MoversUniverse {
    #[default]
    Nifty50,
    Nifty100,
    Nifty500,
    NiftyFno,
    All,
}

impl MoversUniverse {
    pub const ALL: &'static [MoversUniverse] = &[
        MoversUniverse::Nifty50,
        MoversUniverse::Nifty100,
        MoversUniverse::Nifty500,
        MoversUniverse::NiftyFno,
        MoversUniverse::All,
    ];

    pub fn tag(self) -> Option<&'static str> {
        let idx = Self::ALL.iter().position(|universe| *universe == self)?;
        crate::app_api::screener::UNIVERSE_TAGS.get(idx).copied()
    }

    pub fn label(self) -> &'static str {
        self.tag().unwrap_or("All (any volume)")
    }

    pub fn config_token(self) -> &'static str {
        match self {
            MoversUniverse::Nifty50 => "nifty50",
            MoversUniverse::Nifty100 => "nifty100",
            MoversUniverse::Nifty500 => "nifty500",
            MoversUniverse::NiftyFno => "nifty_fno",
            MoversUniverse::All => "all",
        }
    }

    pub fn cycle(self) -> MoversUniverse {
        crate::util::cycle_next(Self::ALL, self)
    }
}

#[derive(Debug, Clone)]
pub struct Movers {
    pub gainers: Vec<ScreenerRow>,
    pub losers: Vec<ScreenerRow>,
}

pub const MOVERS_ROWS: usize = 5;

async fn fetch_screener_presets(client: &KiteConnect) -> Result<Vec<ScreenerPreset>, String> {
    let value = screener_api::presets(client).await.map_err(api_err)?;
    let presets = value
        .get("presets")
        .filter(|presets| presets.is_array())
        .ok_or("bad screener presets payload: no presets array")?;
    Vec::<ScreenerPreset>::deserialize(presets)
        .map_err(|err| format!("bad screener presets payload: {err}"))
}

#[derive(Default)]
pub struct MoverPresets(Mutex<Option<(MoverPreset, MoverPreset)>>);

impl MoverPresets {
    async fn resolve_with<Fut>(&self, fetch: impl FnOnce() -> Fut) -> (MoverPreset, MoverPreset)
    where
        Fut: Future<Output = Result<Vec<ScreenerPreset>, String>>,
    {
        if let Some(cached) = self.0.lock().ok().and_then(|slot| slot.clone()) {
            return cached;
        }
        let Ok(presets) = fetch().await else {
            return screener_api::mover_presets(&[]);
        };
        let resolved = screener_api::mover_presets(&presets);
        if let Ok(mut slot) = self.0.lock() {
            *slot = Some(resolved.clone());
        }
        resolved
    }

    async fn resolve(&self, client: &KiteConnect) -> (MoverPreset, MoverPreset) {
        self.resolve_with(|| fetch_screener_presets(client)).await
    }
}

async fn fetch_screener_rows(
    client: &KiteConnect,
    preset: &MoverPreset,
    tags: &[String],
) -> Result<Vec<ScreenerRow>, String> {
    let query = ScreenerQuery {
        query: preset.query.clone(),
        order_by: preset.order_by.clone(),
        order: preset.order.clone(),
        per_page: screener_api::DEFAULT_PER_PAGE,
        page: 1,
        tags: tags.to_vec(),
    };
    let value = screener_api::instruments(client, &query)
        .await
        .map_err(api_err)?;
    screener_rows(&value)
}

fn screener_rows(value: &Value) -> Result<Vec<ScreenerRow>, String> {
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or("bad screener payload: no results array")?;
    results
        .iter()
        .take(MOVERS_ROWS)
        .enumerate()
        .map(|(index, row)| {
            screener_row(row)
                .map_err(|reason| format!("bad screener payload: row {index}: {reason}"))
        })
        .collect()
}

fn screener_row(row: &Value) -> Result<ScreenerRow, String> {
    let row: ScreenerRow = ScreenerRow::deserialize(row).map_err(|err| err.to_string())?;
    if row.tradingsymbol.trim().is_empty() {
        return Err("blank tradingsymbol".to_string());
    }
    if !row.last_price.is_finite() || !row.change_percent.is_finite() {
        return Err("price or change is not finite".to_string());
    }
    if !(row.volume.is_finite() && row.volume >= 0.0) {
        return Err("volume is not a nonnegative number".to_string());
    }
    Ok(row)
}

async fn fetch_movers(
    client: &KiteConnect,
    universe: MoversUniverse,
    presets: &MoverPresets,
) -> Result<Movers, String> {
    let (gainers_preset, losers_preset) = presets.resolve(client).await;
    let tags: Vec<String> = universe
        .tag()
        .map(|tag| vec![tag.to_string()])
        .unwrap_or_default();
    let (gainers, losers) = tokio::join!(
        fetch_screener_rows(client, &gainers_preset, &tags),
        fetch_screener_rows(client, &losers_preset, &tags),
    );
    Ok(Movers {
        gainers: gainers?,
        losers: losers?,
    })
}

pub fn spawn_refresh_movers(
    client: KiteConnect,
    tx: mpsc::Sender<AppEvent>,
    universe: MoversUniverse,
    presets: Arc<MoverPresets>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_movers(&client, universe, &presets)
            .await
            .map(Box::new);
        send(&tx, DataEvent::Movers { universe, result }).await;
    })
}

#[cfg(test)]
mod tests {
    use super::{
        MOVERS_ROWS, MoverPresets, MoversUniverse, ScreenerRow, fetch_screener_presets,
        screener_row, screener_rows, spawn_refresh_movers,
    };
    use crate::app_api::screener::{MoverPreset, ScreenerPreset};
    use crate::tui::data::DataEvent;
    use crate::tui::data::tests::{connect_client, next_event, reply};
    use httpmock::Method::{GET, POST};
    use httpmock::MockServer;
    use serde_json::json;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::mpsc;

    fn row(symbol: &str) -> serde_json::Value {
        json!({
            "tradingsymbol": symbol,
            "last_price": 1500.0,
            "change_percent": 1.25,
            "volume": 10
        })
    }

    #[test]
    fn screener_rows_errors_on_a_broken_payload() {
        let ok = screener_rows(&serde_json::json!({
            "results": [row("INFY")],
            "page": 1,
            "per_page": 50,
            "total": 1,
            "total_pages": 1
        }))
        .expect("a well-formed payload parses");
        assert_eq!(ok.len(), 1);
        assert_eq!(ok[0].tradingsymbol, "INFY");

        assert!(
            screener_rows(&serde_json::json!({"results": []}))
                .expect("an empty result set is not an error")
                .is_empty()
        );

        for broken in [
            serde_json::json!({}),
            serde_json::json!({"rows": []}),
            serde_json::json!({"results": {"unexpected": "object"}}),
            serde_json::json!({"results": "nope"}),
        ] {
            let err = screener_rows(&broken)
                .expect_err(&format!("{broken} must not read as an empty market"));
            assert!(err.starts_with("bad screener payload"), "{err}");
        }
    }

    #[test]
    fn screener_rows_truncate_to_the_rows_the_panel_draws() {
        let rows: Vec<serde_json::Value> = (0..50).map(|i| row(&format!("SYM{i}"))).collect();
        let parsed = screener_rows(&serde_json::json!({"results": rows})).expect("parses");
        assert_eq!(parsed.len(), MOVERS_ROWS);
    }

    #[test]
    fn screener_row_parses_the_full_live_row() {
        let row: ScreenerRow = serde_json::from_value(serde_json::json!({
            "tradingsymbol": "PHLSRG-SF",
            "name": "Phoenix Lamps",
            "segment": "NSE",
            "isin": "INE455O01019",
            "instrument_token": 3_563_265,
            "exchange_token": 13919,
            "last_price": 200.05,
            "open": 200.05,
            "high": 200.05,
            "low": 200.05,
            "close": 6.94,
            "average_price": 200.05,
            "change": 193.11,
            "change_percent": 2781.36,
            "volume": 0,
            "buy_quantity": 0,
            "sell_quantity": 0,
            "trade_value": 0,
            "market_cap": 0,
            "pe": 0,
            "roe": 0,
            "sector": "",
            "tags": [],
            "week_52_high": 200.05,
            "week_52_low": 6.94,
            "upper_circuit_limit": 210.05,
            "lower_circuit_limit": 190.04,
            "debt_to_equity": 0,
            "dividend_yield": 0,
            "free_cash_flow": 0,
            "pat_growth_yoy": 0,
            "revenue_growth_yoy": 0,
            "fundamental_updated_at": "2025-03-31",
            "timestamp": "2026-08-14 15:29:59",
            "last_trade_time": "2026-08-13 09:15:00"
        }))
        .expect("screener row parses");
        assert_eq!(row.tradingsymbol, "PHLSRG-SF");
        assert_eq!(row.last_price.to_bits(), 200.05_f64.to_bits());
        assert_eq!(row.change_percent.to_bits(), 2781.36_f64.to_bits());
        assert_eq!(row.volume.to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn screener_row_rejects_missing_or_invalid_drawn_fields() {
        let with = |key: &str, value: serde_json::Value| {
            let mut row = row("INFY");
            row[key] = value;
            row
        };
        let without = |key: &str| {
            let mut row = row("INFY");
            row.as_object_mut().expect("object").remove(key);
            row
        };
        for bad in [
            without("last_price"),
            without("tradingsymbol"),
            without("volume"),
            with("tradingsymbol", json!("  ")),
            with("volume", json!(-1)),
        ] {
            assert!(screener_row(&bad).is_err(), "{bad} must not parse");
        }

        let float_volume = screener_row(&with("volume", json!(1234.0))).expect("float volume");
        assert_eq!(float_volume.volume.to_bits(), 1234.0_f64.to_bits());
        assert_eq!(
            (screener_row(&with("volume", json!(0)))
                .expect("zero")
                .volume)
                .to_bits(),
            0.0_f64.to_bits()
        );
        let extra = screener_row(&with("isin", json!("INE009A01021"))).expect("extra field");
        assert_eq!(extra.tradingsymbol, "INFY");
    }

    #[test]
    fn a_malformed_row_past_the_drawn_window_is_ignored() {
        let mut rows: Vec<serde_json::Value> =
            (0..MOVERS_ROWS).map(|i| row(&format!("S{i}"))).collect();
        rows.push(json!({ "tradingsymbol": "NOPRICE", "change_percent": 1.0, "volume": 1 }));
        let parsed = screener_rows(&json!({ "results": rows.clone() })).expect("drawn rows parse");
        assert_eq!(parsed.len(), MOVERS_ROWS);
        assert_eq!(parsed[4].tradingsymbol, "S4");

        rows[1]["last_price"] = json!(null);
        let err = screener_rows(&json!({ "results": rows })).expect_err("drawn row 1 is bad");
        assert!(err.starts_with("bad screener payload: row 1:"), "{err}");
    }

    #[test]
    fn movers_universe_cycle_visits_every_universe_and_defaults_to_nifty_50() {
        assert_eq!(MoversUniverse::default(), MoversUniverse::Nifty50);
        assert_ne!(MoversUniverse::default(), MoversUniverse::All);

        let mut seen = Vec::new();
        let mut universe = MoversUniverse::Nifty50;
        for _ in 0..5 {
            seen.push(universe);
            universe = universe.cycle();
        }
        assert_eq!(
            seen,
            vec![
                MoversUniverse::Nifty50,
                MoversUniverse::Nifty100,
                MoversUniverse::Nifty500,
                MoversUniverse::NiftyFno,
                MoversUniverse::All,
            ]
        );
        assert_eq!(universe, MoversUniverse::Nifty50);
    }

    #[test]
    fn movers_universe_tags_and_labels() {
        assert_eq!(MoversUniverse::Nifty50.tag(), Some("Nifty 50"));
        assert_eq!(MoversUniverse::Nifty100.tag(), Some("Nifty 100"));
        assert_eq!(MoversUniverse::Nifty500.tag(), Some("Nifty 500"));
        assert_eq!(MoversUniverse::NiftyFno.tag(), Some("Nifty F&O Stocks"));
        assert_eq!(MoversUniverse::All.tag(), None);

        assert_eq!(MoversUniverse::All.label(), "All (any volume)");
        assert_eq!(MoversUniverse::Nifty500.label(), "Nifty 500");

        let title_width = "Movers · ".len() + MoversUniverse::All.label().len();
        assert!(
            title_width <= 31,
            "movers title is {title_width} cols, clipped at an 80-col terminal"
        );
    }

    #[test]
    fn movers_universe_config_tokens_are_unique() {
        let all = [
            MoversUniverse::Nifty50,
            MoversUniverse::Nifty100,
            MoversUniverse::Nifty500,
            MoversUniverse::NiftyFno,
            MoversUniverse::All,
        ];
        let mut tokens: Vec<&str> = all.iter().map(|u| u.config_token()).collect();
        tokens.sort_unstable();
        let count = tokens.len();
        tokens.dedup();
        assert_eq!(
            tokens.len(),
            count,
            "duplicate movers universe config token"
        );
        assert_eq!(MoversUniverse::Nifty500.config_token(), "nifty500");
        assert_eq!(MOVERS_ROWS, 5);
    }

    fn server_presets() -> Vec<ScreenerPreset> {
        serde_json::from_value(serde_json::json!([
            {
                "key": "top_gainers", "name": "Top gainers", "description": "",
                "query": "change_percent>0 AND volume>1000", "order_by": "change_percent",
                "order": "desc", "type": "preset", "has_computation": false
            },
            {
                "key": "top_losers", "name": "Top losers", "description": "",
                "query": "change_percent<0 AND volume>1000", "order_by": "change_percent",
                "order": "asc", "type": "preset", "has_computation": false
            }
        ]))
        .expect("presets parse")
    }

    #[tokio::test]
    async fn presets_are_resolved_once_across_many_movers_loads() {
        let cache = MoverPresets::default();
        let fetches = AtomicUsize::new(0);
        let fetch = || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(server_presets())
        };

        let first = cache.resolve_with(fetch).await;
        assert_eq!(first.0.query, "change_percent>0 AND volume>1000");
        assert_eq!(first.1.query, "change_percent<0 AND volume>1000");

        for _ in 0..4 {
            assert_eq!(cache.resolve_with(fetch).await, first);
        }
        assert_eq!(
            fetches.load(Ordering::SeqCst),
            1,
            "the preset list was re-fetched for a load that already knew it"
        );
    }

    #[tokio::test]
    async fn a_failed_preset_fetch_falls_back_and_is_retried() {
        let cache = MoverPresets::default();
        let fetches = AtomicUsize::new(0);
        let fetch = || async {
            let attempt = fetches.fetch_add(1, Ordering::SeqCst);
            if attempt >= 2 {
                Ok(server_presets())
            } else {
                Err("request failed".to_string())
            }
        };

        for attempt in 1..=2 {
            let (gainers, losers) = cache.resolve_with(fetch).await;
            assert_eq!(gainers, MoverPreset::gainers(), "attempt {attempt}");
            assert_eq!(losers, MoverPreset::losers(), "attempt {attempt}");
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 2, "a failure is retried");

        let (gainers, _) = cache.resolve_with(fetch).await;
        assert_eq!(gainers.query, "change_percent>0 AND volume>1000");
        cache.resolve_with(fetch).await;
        assert_eq!(fetches.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn a_malformed_preset_list_is_not_cached() {
        let cache = MoverPresets::default();
        let fetches = AtomicUsize::new(0);
        let fetch = || async {
            match fetches.fetch_add(1, Ordering::SeqCst) {
                0 => Err("bad screener presets payload: no presets array".to_string()),
                _ => Ok(server_presets()),
            }
        };

        let (gainers, _) = cache.resolve_with(fetch).await;
        assert_eq!(gainers, MoverPreset::gainers());
        let (gainers, _) = cache.resolve_with(fetch).await;
        assert_eq!(gainers.query, "change_percent>0 AND volume>1000");
        assert_eq!(fetches.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_valid_empty_preset_list_is_cached() {
        let cache = MoverPresets::default();
        let fetches = AtomicUsize::new(0);
        let fetch = || async {
            fetches.fetch_add(1, Ordering::SeqCst);
            Ok(Vec::new())
        };

        for _ in 0..2 {
            assert_eq!(
                cache.resolve_with(fetch).await,
                (MoverPreset::gainers(), MoverPreset::losers())
            );
        }
        assert_eq!(fetches.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn the_preset_fetch_distinguishes_a_malformed_list_from_an_empty_one() {
        for (data, valid) in [
            (json!({ "presets": "x" }), false),
            (json!({}), false),
            (json!({ "presets": [{ "query": 1 }] }), false),
            (json!({ "presets": [] }), true),
        ] {
            let server = MockServer::start_async().await;
            let presets = reply(&server, GET, "/screener/presets", data.clone()).await;

            let result = fetch_screener_presets(&connect_client(&server)).await;

            match result {
                Ok(list) => assert!(valid && list.is_empty(), "{data}: {list:?}"),
                Err(err) => {
                    assert!(!valid, "{data}: {err}");
                    assert!(err.starts_with("bad screener presets payload"), "{err}");
                }
            }
            presets.assert_calls_async(1).await;
        }
    }

    /// The screener rejects any body that is not form-encoded with a 415, so
    /// the mock only answers a form request that carries the universe tag.
    #[tokio::test]
    async fn movers_query_the_screener_with_a_form_body() {
        let server = MockServer::start_async().await;
        reply(&server, GET, "/screener/presets", json!({ "presets": [] })).await;
        let body = json!({ "status": "success", "data": { "results": [row("INFY")] } });
        let instruments = server
            .mock_async(|when, then| {
                when.method(POST)
                    .path("/screener/instruments")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .form_urlencoded_tuple("tags", "Nifty 50");
                then.status(200)
                    .header("content-type", "application/json")
                    .body(body.to_string());
            })
            .await;
        let (tx, mut rx) = mpsc::channel(1);

        spawn_refresh_movers(
            connect_client(&server),
            tx,
            MoversUniverse::Nifty50,
            Arc::default(),
        )
        .await
        .expect("movers task");

        match next_event(&mut rx).await {
            DataEvent::Movers {
                universe: MoversUniverse::Nifty50,
                result: Ok(movers),
            } => assert_eq!(movers.gainers[0].tradingsymbol, "INFY"),
            other => panic!("unexpected {other:?}"),
        }
        instruments.assert_calls_async(2).await;
    }

    #[tokio::test]
    async fn a_drawn_row_without_a_price_fails_the_movers_event() {
        let server = MockServer::start_async().await;
        reply(&server, GET, "/screener/presets", json!({ "presets": [] })).await;
        let instruments = reply(
            &server,
            POST,
            "/screener/instruments",
            json!({ "results": [{ "tradingsymbol": "INFY", "change_percent": 1.0, "volume": 1 }] }),
        )
        .await;
        let (tx, mut rx) = mpsc::channel(1);

        spawn_refresh_movers(
            connect_client(&server),
            tx,
            MoversUniverse::Nifty50,
            Arc::default(),
        )
        .await
        .expect("movers task");

        match next_event(&mut rx).await {
            DataEvent::Movers {
                universe: MoversUniverse::Nifty50,
                result: Err(err),
            } => assert!(err.starts_with("bad screener payload: row 0:"), "{err}"),
            other => panic!("unexpected {other:?}"),
        }
        instruments.assert_calls_async(2).await;
    }
}
