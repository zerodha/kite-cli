use crate::app::output::Output;
use crate::app::render;
use crate::app::runtime::Runtime;
use crate::cli::{InstrumentDumpArgs, MarketCacheCommands, MarketCommands, QuoteArgs};
use crate::instruments;
use crate::util::quote_instruments;
use anyhow::{Context, Result};
use serde_json::json;
use std::fs;
use std::io::{BufWriter, Write};

pub(super) enum QuoteKind {
    Full,
    Ohlc,
    Ltp,
}

fn qualify_search(query: &str, exchange: Option<&str>) -> Option<String> {
    let query = query.trim();
    let Some(exchange) = exchange.map(str::trim) else {
        return Some(query.to_string());
    };
    match query.split_once(':') {
        Some((qualified, _)) if !qualified.trim().eq_ignore_ascii_case(exchange) => None,
        Some(_) => Some(query.to_string()),
        None => Some(format!("{exchange}:{query}")),
    }
}

pub async fn handle(rt: &Runtime, command: MarketCommands) -> Result<Output> {
    match command {
        MarketCommands::Instruments(args) => instruments_dump(rt, args).await,
        MarketCommands::Cache { command } => cache(rt, command).await,
        MarketCommands::Search(args) => {
            let store = instruments::load(&rt.app_uuid()?)
                .await
                .map_err(anyhow::Error::msg)?;
            let search_query = qualify_search(&args.query, args.exchange.as_deref());
            let matches = search_query
                .as_deref()
                .map(|query| store.search(query, args.limit))
                .unwrap_or_default();
            Ok(Output::view(
                json!({
                    "query": args.query,
                    "exchange": args.exchange,
                    "matches": matches,
                }),
                instrument_records,
            ))
        }
        MarketCommands::Resolve(args) => {
            let store = instruments::load(&rt.app_uuid()?)
                .await
                .map_err(anyhow::Error::msg)?;
            let matches = store.resolve(&args.symbol, args.exchange.as_deref(), args.limit);
            Ok(Output::check(
                !matches.is_empty(),
                json!({
                    "symbol": args.symbol,
                    "exchange": args.exchange,
                    "matches": matches,
                }),
                instrument_records,
            ))
        }
        MarketCommands::Quote(args) => quote(rt, QuoteKind::Full, args).await,
        MarketCommands::Ohlc(args) => quote(rt, QuoteKind::Ohlc, args).await,
        MarketCommands::Ltp(args) => quote(rt, QuoteKind::Ltp, args).await,
    }
}

pub async fn quote(rt: &Runtime, kind: QuoteKind, args: QuoteArgs) -> Result<Output> {
    let instruments = quote_instruments(args)?;
    let requested = instruments.clone();
    let instruments: Vec<&str> = instruments.iter().map(String::as_str).collect();
    let value = match kind {
        QuoteKind::Ltp => rt.with_session(|api| api.ltp(&instruments).raw()).await?,
        QuoteKind::Ohlc => rt.with_session(|api| api.ohlc(&instruments).raw()).await?,
        QuoteKind::Full => rt.with_session(|api| api.quote(&instruments).raw()).await?,
    };
    Ok(Output::view(value, move |value, doc| {
        quote_view(value, &requested, &kind, doc);
    }))
}

async fn cache(rt: &Runtime, command: MarketCacheCommands) -> Result<Output> {
    match command {
        MarketCacheCommands::Path => {
            let Some(path) = instruments::cache_path() else {
                return Ok(Output::text(
                    "cache directory unavailable".to_string(),
                    |_, doc| {
                        doc.empty("cache directory unavailable");
                    },
                ));
            };
            Ok(Output::view(json!({ "path": path }), render::path))
        }
        MarketCacheCommands::Info => {
            let Some(info) = instruments::cache_info().await else {
                return Ok(Output::text(
                    "cache directory unavailable".to_string(),
                    |_, doc| {
                        doc.empty("cache directory unavailable");
                    },
                ));
            };
            Ok(Output::view(json!(info), |value, doc| {
                let mut value = value.clone();
                for key in ["modified_unix_seconds", "fresh_until_unix_seconds"] {
                    if let Some(timestamp) = value[key].as_u64() {
                        let (year, month, day) = crate::tui::format::date_ist(timestamp);
                        value[key] = json!(format!(
                            "{year:04}-{month:02}-{day:02} {} IST",
                            crate::tui::format::clock_ist(timestamp)
                        ));
                    }
                }
                render::generic(&value, doc);
            }))
        }
        MarketCacheCommands::Refresh => {
            let _status = render::Status::start("Refreshing instrument cache…", rt.json);
            let store = instruments::refresh(&rt.app_uuid()?)
                .await
                .map_err(anyhow::Error::msg)?;
            let path = instruments::cache_path();
            Ok(Output::view(
                json!({
                    "path": path,
                    "records": store.len(),
                    "source": "/instruments.json",
                    "compressed_http": true,
                }),
                render::generic,
            ))
        }
        MarketCacheCommands::Clear => {
            let path = instruments::cache_path();
            let cleared = instruments::clear_cache()
                .await
                .map_err(anyhow::Error::msg)?
                .is_some();
            Ok(Output::view(
                json!({ "path": path, "cleared": cleared }),
                render::generic,
            ))
        }
    }
}

fn instrument_records(value: &serde_json::Value, doc: &mut render::Doc) {
    render::records(
        value,
        &[
            "exchange",
            "tradingsymbol",
            "name",
            "instrument_token",
            "instrument_type",
            "segment",
            "expiry",
            "strike",
            "tick_size",
            "lot_size",
        ],
        doc,
    );
}

async fn instruments_dump(rt: &Runtime, args: InstrumentDumpArgs) -> Result<Output> {
    let store = instruments::load(&rt.app_uuid()?)
        .await
        .map_err(anyhow::Error::msg)?;
    let mut records = store.into_vec();

    if let Some(out) = args.out {
        let file =
            fs::File::create(&out).with_context(|| format!("failed to write {}", out.display()))?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer_pretty(&mut writer, &records)
            .context("failed to serialize instruments JSON")?;
        writer
            .flush()
            .with_context(|| format!("failed to write {}", out.display()))?;
        return Ok(Output::view(
            json!({
                "path": out,
                "endpoint": "/instruments.json",
                "records": records.len(),
                "format": "json",
                "truncated": false,
            }),
            render::generic,
        ));
    }

    let truncated = args.limit > 0 && records.len() > args.limit;
    if args.limit > 0 {
        records.truncate(args.limit);
    }

    Ok(Output::view(
        json!({
            "endpoint": "/instruments.json",
            "format": "json",
            "limit": args.limit,
            "truncated": truncated,
            "records": records,
        }),
        instrument_records,
    ))
}

fn quote_view(
    value: &serde_json::Value,
    requested: &[String],
    kind: &QuoteKind,
    doc: &mut render::Doc,
) {
    use render::{Col, Table};
    let mut table = Table::new(vec![
        Col::text("INSTRUMENT"),
        Col::decimal("LTP"),
        Col::text("TOKEN"),
    ]);
    let mut sections = 0;
    for symbol in requested {
        let Some(quote) = value.get(symbol) else {
            doc.warn(&format!("no quote returned for {symbol}"));
            continue;
        };
        let exchange = symbol.split_once(':').map_or("", |(exchange, _)| exchange);
        match kind {
            QuoteKind::Ltp => table.row(vec![
                symbol.clone().into(),
                render::nullable_price(&quote["last_price"], exchange),
                render::field(quote, "instrument_token"),
            ]),
            QuoteKind::Ohlc | QuoteKind::Full => {
                if sections > 0 {
                    doc.blank();
                }
                quote_detail(
                    symbol,
                    exchange,
                    quote,
                    matches!(kind, QuoteKind::Full),
                    doc,
                );
                sections += 1;
            }
        }
    }
    if matches!(kind, QuoteKind::Ltp) && requested.iter().any(|symbol| value.get(symbol).is_some())
    {
        doc.table(table);
    }
}

fn quote_detail(
    symbol: &str,
    exchange: &str,
    value: &serde_json::Value,
    full: bool,
    doc: &mut render::Doc,
) {
    use render::{Cell, Col, Table, Tone};
    let close = value["ohlc"]["close"].as_f64();
    let last = value["last_price"].as_f64();
    let price = |value: &serde_json::Value| render::nullable_price(value, exchange);
    let delta = last.zip(close).map(|(last, close)| last - close);
    doc.line(vec![
        Cell::from(format!("{symbol} ")).tone(Tone::Bold),
        price(&value["last_price"]).tone(Tone::Bold),
        delta.map_or_else(|| "–".into(), render::change),
        delta
            .zip(close)
            .filter(|(_, close)| *close > 0.0)
            .map_or_else(
                || "–".into(),
                |(delta, close)| render::change_pct(delta / close * 100.0),
            ),
    ])
    .blank();
    let mut table = Table::new(vec![
        Col::decimal("OPEN"),
        Col::decimal("HIGH"),
        Col::decimal("LOW"),
        Col::decimal("CLOSE"),
        Col::number("VOLUME"),
        Col::decimal("AVG PRICE"),
    ]);
    table.row(vec![
        price(&value["ohlc"]["open"]),
        price(&value["ohlc"]["high"]),
        price(&value["ohlc"]["low"]),
        price(&value["ohlc"]["close"]),
        render::qty_field(value, "volume"),
        price(&value["average_price"]),
    ]);
    doc.table(table);
    if !full {
        return;
    }
    doc.blank().fields(vec![
        (
            "Last trade",
            format!(
                "{} @ {}",
                render::qty_field(value, "last_quantity").text,
                render::s(value, "last_trade_time")
                    .get(11..19)
                    .unwrap_or("–")
            )
            .into(),
        ),
        (
            "Buy / sell",
            format!(
                "{} / {}",
                render::qty_field(value, "buy_quantity").text,
                render::qty_field(value, "sell_quantity").text
            )
            .into(),
        ),
        (
            "Circuits",
            format!(
                "{} – {}",
                price(&value["lower_circuit_limit"]).text,
                price(&value["upper_circuit_limit"]).text
            )
            .into(),
        ),
        (
            "Updated",
            if value["timestamp"].is_null() {
                "–".into()
            } else {
                format!("{} IST", render::s(value, "timestamp")).into()
            },
        ),
    ]);
    if matches!(exchange, "NFO" | "BFO" | "MCX" | "NCO" | "CDS" | "BCD") {
        doc.fields(vec![
            ("OI", render::qty_field(value, "oi")),
            (
                "OI high / low",
                format!(
                    "{} / {}",
                    render::qty_field(value, "oi_day_high").text,
                    render::qty_field(value, "oi_day_low").text
                )
                .into(),
            ),
        ]);
    }
    quote_depth(value, exchange, doc);
}

fn quote_depth(value: &serde_json::Value, exchange: &str, doc: &mut render::Doc) {
    use render::{Col, Table};
    let price = |value: &serde_json::Value| render::nullable_price(value, exchange);
    let buy = value["depth"]["buy"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let sell = value["depth"]["sell"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    let mut depth = Table::new(vec![
        Col::number("BID QTY"),
        Col::number("ORDERS"),
        Col::decimal("BID"),
        Col::decimal("ASK"),
        Col::number("ORDERS"),
        Col::number("ASK QTY"),
    ]);
    let mut populated = false;
    for index in 0..buy.len().max(sell.len()) {
        let bid = buy
            .get(index)
            .filter(|row| render::f(row, "quantity") > 0.0);
        let ask = sell
            .get(index)
            .filter(|row| render::f(row, "quantity") > 0.0);
        if bid.is_none() && ask.is_none() {
            continue;
        }
        let quantity = |row: Option<&serde_json::Value>, key| {
            row.map_or_else(|| "–".into(), |row| render::qty_field(row, key))
        };
        depth.row(vec![
            quantity(bid, "quantity"),
            quantity(bid, "orders"),
            bid.map_or_else(|| "–".into(), |row| price(&row["price"])),
            ask.map_or_else(|| "–".into(), |row| price(&row["price"])),
            quantity(ask, "orders"),
            quantity(ask, "quantity"),
        ]);
        populated = true;
    }
    if populated {
        doc.blank().table(depth);
    }
}

#[cfg(test)]
mod tests {
    use super::qualify_search;

    #[test]
    fn exchange_filter_is_applied_before_search_ranking() {
        assert_eq!(
            qualify_search("INFY", Some("BSE")).as_deref(),
            Some("BSE:INFY")
        );
        assert_eq!(
            qualify_search("bse:INFY", Some("BSE")).as_deref(),
            Some("bse:INFY")
        );
        assert_eq!(qualify_search("NSE:INFY", Some("BSE")), None);
        assert_eq!(
            qualify_search(" nse : INFY ", Some("NSE")).as_deref(),
            Some("nse : INFY")
        );
        assert_eq!(qualify_search("INFY", None).as_deref(), Some("INFY"));
    }
}
