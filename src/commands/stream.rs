use crate::app::output::Output;
use crate::app::render::{self, Term};
use crate::app::runtime::Runtime;
use crate::cli::{StreamCommands, StreamTicksArgs, TuiArgs};
use crate::util::stream_tokens;
use anyhow::{Context, Result, bail};
use kiteconnect::ticker::{CloseReason, Tick, TickerEvent};
use serde_json::json;
use std::io::{self, Write};
use std::time::{Duration, Instant};

pub async fn handle(rt: &Runtime, command: StreamCommands) -> Result<Output> {
    match command {
        StreamCommands::Ticks(args) => ticks(rt, args).await,
        StreamCommands::Tui(args) => tui(rt, args).await,
    }
}

pub async fn tui(rt: &Runtime, args: TuiArgs) -> Result<Output> {
    crate::tui::run(rt, args).await?;
    Ok(Output::None)
}

async fn ticks(rt: &Runtime, args: StreamTicksArgs) -> Result<Output> {
    let (tokens, labels) = resolve_tokens(rt, &args).await?;
    if tokens.is_empty() {
        bail!(
            "at least one instrument token is required; use `kite market resolve NSE:INFY` to find tokens"
        );
    }

    let client = rt.client()?;
    let url =
        url::Url::parse(rt.resolve_credentials()?.ticker_url()?).context("invalid ticker URL")?;
    let (ticker, mut rx) = client.ticker().url(url).spawn()?;
    if let Err(error) = ticker.subscribe(&tokens, args.mode.into()) {
        if let Some(TickerEvent::Closed(reason)) = rx.recv().await {
            bail!(reason);
        }
        return Err(error.into());
    }

    let deadline = (args.timeout_seconds > 0)
        .then(|| Instant::now() + Duration::from_secs(args.timeout_seconds));

    let mut connected = false;
    let mut connection_failure = None;
    let mut ticks = Vec::new();
    let mut count = 0;
    let mut header = false;
    let symbol_width = symbol_width(&tokens, &labels);
    let started = Instant::now();
    if !rt.json {
        stream_status(&tokens, &labels, &args);
    }
    'events: loop {
        let next = match deadline {
            Some(deadline) => {
                if Instant::now() >= deadline {
                    break;
                }
                tokio::time::timeout_at(deadline.into(), rx.recv())
                    .await
                    .ok()
                    .flatten()
            }
            None => rx.recv().await,
        };
        let Some(event) = next else { break };
        match event {
            TickerEvent::Connected => connected = true,
            TickerEvent::Reconnecting { reason, .. } => connection_failure = Some(reason),
            TickerEvent::Ticks(batch) => {
                let mut out = io::stdout().lock();
                for tick in batch {
                    if !rt.json {
                        write_human_tick(&mut out, &tick, &labels, symbol_width, &mut header)?;
                        count += 1;
                        if args.limit > 0 && count >= args.limit {
                            break 'events;
                        }
                    } else if args.limit == 0 {
                        if !write_tick(&mut out, &tick)? {
                            return Ok(Output::None);
                        }
                    } else {
                        ticks.push(tick);
                        if ticks.len() >= args.limit {
                            break 'events;
                        }
                    }
                }
            }
            TickerEvent::Closed(CloseReason::Requested) => break,
            TickerEvent::Closed(CloseReason::AuthRejected(message)) => {
                bail!("{message}; run `kite auth login`")
            }
            TickerEvent::Closed(reason) => bail!(reason),
            _ => {}
        }
    }

    if !connected {
        bail!(
            "ticker never connected: {}",
            connection_failure
                .as_deref()
                .unwrap_or("connection did not complete before the stream ended")
        );
    }

    if !rt.json {
        eprintln!("{count} ticks in {:.1}s", started.elapsed().as_secs_f64());
        Ok(Output::None)
    } else if args.limit == 0 {
        Ok(Output::None)
    } else {
        Ok(Output::view(
            json!({
                "mode": args.mode,
                "count": ticks.len(),
                "ticks": ticks,
            }),
            render::generic,
        ))
    }
}

fn symbol_width(tokens: &[u32], labels: &std::collections::HashMap<u32, String>) -> usize {
    tokens
        .iter()
        .map(|token| {
            labels
                .get(token)
                .cloned()
                .unwrap_or_else(|| format!("token={token}"))
        })
        .map(|symbol| crate::tui::format::display_width(&symbol))
        .max()
        .unwrap_or(10)
        .max(10)
}

fn write_human_tick(
    out: &mut impl Write,
    tick: &Tick,
    labels: &std::collections::HashMap<u32, String>,
    symbol_width: usize,
    header: &mut bool,
) -> io::Result<()> {
    if !*header {
        render::tick_header(out, symbol_width, Term::stdout())?;
        *header = true;
    }
    let received = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let symbol = labels
        .get(&tick.instrument_token)
        .cloned()
        .unwrap_or_else(|| format!("token={}", tick.instrument_token));
    render::tick_line(out, tick, &symbol, symbol_width, received, Term::stdout())?;
    Ok(())
}

fn stream_status(
    tokens: &[u32],
    labels: &std::collections::HashMap<u32, String>,
    args: &StreamTicksArgs,
) {
    let instruments = tokens
        .iter()
        .map(|token| {
            labels
                .get(token)
                .cloned()
                .unwrap_or_else(|| format!("token={token}"))
        })
        .collect::<Vec<_>>()
        .join(", ");
    let limit = if args.limit == 0 {
        "runs until interrupted".to_string()
    } else {
        format!("stops after {} ticks", args.limit)
    };
    eprintln!(
        "Streaming {instruments} in {} mode; {limit}",
        kiteconnect::ticker::Mode::from(args.mode)
    );
}

async fn resolve_tokens(
    rt: &Runtime,
    args: &StreamTicksArgs,
) -> Result<(Vec<u32>, std::collections::HashMap<u32, String>)> {
    let mut tokens = stream_tokens(args);
    let mut labels = std::collections::HashMap::new();
    if !args.instruments.is_empty() {
        let store = crate::instruments::load(&rt.app_uuid()?)
            .await
            .map_err(anyhow::Error::msg)?;
        for requested in &args.instruments {
            let Some((exchange, symbol)) =
                requested.split_once(':').filter(|(exchange, symbol)| {
                    !exchange.trim().is_empty() && !symbol.trim().is_empty()
                })
            else {
                bail!("expected EXCHANGE:SYMBOL, got `{requested}`");
            };
            let instrument = store
                .by_exact_symbol(symbol)
                .into_iter()
                .find(|instrument| instrument.exchange.eq_ignore_ascii_case(exchange.trim()))
                .with_context(|| format!("instrument not found: {requested}"))?;
            if !tokens.contains(&instrument.instrument_token) {
                tokens.push(instrument.instrument_token);
            }
            labels.insert(
                instrument.instrument_token,
                render::Cell::from(instrument.label()).text,
            );
        }
    }
    Ok((tokens, labels))
}

fn write_tick(out: &mut impl Write, tick: &Tick) -> io::Result<bool> {
    let result = serde_json::to_writer(&mut *out, tick)
        .map_err(io::Error::from)
        .and_then(|()| writeln!(out));
    match result {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_output_preserves_other_write_errors() {
        let mut out = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
        let error = write_tick(&mut out, &Tick::default()).unwrap_err();
        assert_ne!(error.kind(), io::ErrorKind::BrokenPipe);
    }

    #[test]
    fn unlimited_stream_exits_when_reader_closes() {
        let (reader, mut writer) = io::pipe().unwrap();
        drop(reader);
        assert!(!write_tick(&mut writer, &Tick::default()).unwrap());
    }
}
