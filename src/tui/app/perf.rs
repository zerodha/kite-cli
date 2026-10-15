use super::test_support::test_app;
use super::{App, Preset, ui};
use crate::tui::candles::SeriesKey;
use crate::tui::data::{
    Candle, ChartInterval, Holding, Movers, Position, PositionsData, ScreenerRow,
};
use crate::tui::event::AppEvent;
use crate::tui::instruments::{Instrument, InstrumentStore};
use crate::tui::marketwatch::{State as WatchState, WatchItem};
use crate::tui::workspace::PaneId;
use kiteconnect::ticker::{Tick, TickerEvent, parse_binary};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use std::sync::Arc;
use std::time::{Duration, Instant};

const UNIVERSE: u32 = 1_000;
const EQUITIES: u32 = 200;
const INDICES: u32 = 20;
const FUTURES: u32 = 20;
const WATCH_EQUITIES: u32 = 100;
const HOLDINGS: u32 = 80;
const CHARTED: u32 = 24;
const SEED_CANDLES: usize = 60;

const BATCHES: u32 = 200;
const FULL_PER_BATCH: u32 = 20;
const QUOTE_PER_BATCH: u32 = 8;
const LTP_PER_BATCH: u32 = 2;
const INDEX_PER_BATCH: u32 = 2;
const PACKETS_PER_BATCH: u32 = FULL_PER_BATCH + QUOTE_PER_BATCH + LTP_PER_BATCH + INDEX_PER_BATCH;

const WIDTH: u16 = 160;
const HEIGHT: u16 = 40;

const SESSION_START: u32 = 1_791_345_600;

const SECONDS_PER_BATCH: u32 = 2;

const SEG_NSE: u32 = 1;
const SEG_NFO: u32 = 2;
const SEG_INDICES: u32 = 9;

fn equity(n: u32) -> u32 {
    ((10_000 + n) << 8) | SEG_NSE
}

fn index(n: u32) -> u32 {
    ((20_000 + n) << 8) | SEG_INDICES
}

fn future(n: u32) -> u32 {
    ((30_000 + n) << 8) | SEG_NFO
}

fn unsubscribed(n: u32) -> u32 {
    ((40_000 + n) << 8) | SEG_NSE
}

fn equity_base(n: u32) -> u32 {
    50_000 + 1_000 * n
}

fn index_base(n: u32) -> u32 {
    1_000_000 + 50_000 * n
}

fn future_base(n: u32) -> u32 {
    2_000_000 + 10_000 * n
}

fn streamed(slot: u32) -> (u32, u32) {
    if slot < EQUITIES {
        (equity(slot), equity_base(slot))
    } else {
        let n = slot - EQUITIES;
        (future(n), future_base(n))
    }
}

fn wiggle(batch: u32, slot: u32) -> u32 {
    (batch * 13 + slot * 7) % 61
}

fn price(base: u32, batch: u32, slot: u32) -> u32 {
    base + wiggle(batch, slot) - 30
}

fn volume(slot: u32, batch: u32) -> u32 {
    100_000 * (slot + 1) + 250 * batch
}

fn exchange_time(batch: u32) -> u32 {
    SESSION_START + SECONDS_PER_BATCH * batch
}

fn put(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn quote_body(out: &mut Vec<u8>, token: u32, base: u32, batch: u32, slot: u32) {
    let ltp = price(base, batch, slot);
    put(out, token);
    put(out, ltp);
    put(out, 10 + slot % 50);
    put(out, base);
    put(out, volume(slot, batch));
    put(out, 5_000 + batch);
    put(out, 6_000 + batch);
    put(out, base - 100);
    put(out, base + 200);
    put(out, base - 200);
    put(out, base);
}

fn full_packet(token: u32, base: u32, batch: u32, slot: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(184);
    quote_body(&mut out, token, base, batch, slot);
    put(&mut out, exchange_time(batch));
    put(&mut out, 0);
    put(&mut out, 0);
    put(&mut out, 0);
    put(&mut out, exchange_time(batch));
    let ltp = price(base, batch, slot);
    for level in 0..10u32 {
        let (quantity, at) = if level < 5 {
            (100 * (level + 1), ltp - 5 * (level + 1))
        } else {
            (150 * (level - 4), ltp + 5 * (level - 4))
        };
        put(&mut out, quantity);
        put(&mut out, at);
        out.extend_from_slice(
            &(u16::try_from(level + 1).expect("ten depth levels fit u16")).to_be_bytes(),
        );
        out.extend_from_slice(&[0, 0]);
    }
    out
}

fn quote_packet(token: u32, base: u32, batch: u32, slot: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(44);
    quote_body(&mut out, token, base, batch, slot);
    out
}

fn ltp_packet(token: u32, base: u32, batch: u32, slot: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(8);
    put(&mut out, token);
    put(&mut out, price(base, batch, slot) + 5);
    out
}

fn index_packet(n: u32, batch: u32) -> Vec<u8> {
    let base = index_base(n);
    let mut out = Vec::with_capacity(32);
    put(&mut out, index(n));
    put(&mut out, price(base, batch, n));
    put(&mut out, base + 5_000);
    put(&mut out, base - 5_000);
    put(&mut out, base - 1_000);
    put(&mut out, base);
    put(&mut out, 0);
    put(&mut out, exchange_time(batch));
    out
}

fn frame(packets: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(
        &(u16::try_from(packets.len()).expect("fixture packet count fits u16")).to_be_bytes(),
    );
    for packet in packets {
        out.extend_from_slice(
            &(u16::try_from(packet.len()).expect("fixture packet size fits u16")).to_be_bytes(),
        );
        out.extend_from_slice(packet);
    }
    out
}

fn frames() -> Vec<Vec<u8>> {
    let rotation = EQUITIES + FUTURES;
    let quoted = rotation - CHARTED;
    (0..BATCHES)
        .map(|batch| {
            let mut packets = Vec::with_capacity(PACKETS_PER_BATCH as usize);
            for s in 0..FULL_PER_BATCH {
                let slot = (batch * FULL_PER_BATCH + s) % rotation;
                let (token, base) = streamed(slot);
                packets.push(full_packet(token, base, batch, slot));
            }
            for s in 0..QUOTE_PER_BATCH {
                let slot = CHARTED + (batch * QUOTE_PER_BATCH + s) % quoted;
                let (token, base) = streamed(slot);
                packets.push(quote_packet(token, base, batch, slot));
                if s < LTP_PER_BATCH {
                    packets.push(ltp_packet(token, base, batch, slot));
                }
            }
            for s in 0..INDEX_PER_BATCH {
                packets.push(index_packet((batch * INDEX_PER_BATCH + s) % INDICES, batch));
            }
            frame(&packets)
        })
        .collect()
}

fn instrument(token: u32, symbol: String, exchange: &str, segment: &str, kind: &str) -> Instrument {
    Instrument {
        instrument_token: token,
        tradingsymbol: symbol.clone(),
        name: symbol,
        exchange: exchange.into(),
        expiry: None,
        strike: None,
        tick_size: 0.05,
        lot_size: if kind == "FUT" { 50 } else { 1 },
        instrument_type: kind.into(),
        segment: segment.into(),
        freeze_quantity: None,
        max_order_quantity: None,
    }
}

fn universe() -> Vec<Instrument> {
    let mut all = Vec::with_capacity(UNIVERSE as usize);
    all.extend(
        (0..EQUITIES).map(|n| instrument(equity(n), format!("EQ{n:03}"), "NSE", "NSE", "EQ")),
    );
    all.extend(
        (0..INDICES).map(|n| instrument(index(n), format!("IDX{n:02}"), "NSE", "INDICES", "EQ")),
    );
    all.extend(
        (0..FUTURES).map(|n| instrument(future(n), format!("FUT{n:02}"), "NFO", "NFO-FUT", "FUT")),
    );
    let rest = UNIVERSE - EQUITIES - INDICES - FUTURES;
    all.extend(
        (0..rest).map(|n| instrument(unsubscribed(n), format!("UNS{n:03}"), "NSE", "NSE", "EQ")),
    );
    all
}

fn watch_row(token: u32, segment: &str, symbol: String) -> WatchItem {
    WatchItem {
        exchange: "NSE".into(),
        tradingsymbol: symbol,
        instrument_token: token,
        segment: segment.into(),
        ..WatchItem::default()
    }
}

fn watch_rows() -> Vec<WatchItem> {
    let mut rows: Vec<WatchItem> = (0..WATCH_EQUITIES)
        .map(|n| watch_row(equity(n), "NSE", format!("EQ{n:03}")))
        .collect();
    rows.extend((0..INDICES).map(|n| watch_row(index(n), "INDICES", format!("IDX{n:02}"))));
    rows
}

fn movers() -> Movers {
    let row = |n: u32, change_percent: f64| ScreenerRow {
        tradingsymbol: format!("MOV{n}"),
        last_price: 1_000.0 + f64::from(n),
        change_percent,
        volume: 1e6,
    };
    Movers {
        gainers: (0..5).map(|n| row(n, 5.0 - f64::from(n))).collect(),
        losers: (5..10).map(|n| row(n, f64::from(n) - 11.0)).collect(),
    }
}

fn holdings() -> Vec<Holding> {
    (WATCH_EQUITIES..WATCH_EQUITIES + HOLDINGS)
        .map(|n| {
            let close = f64::from(equity_base(n)) / 100.0;
            let i = f64::from(n - WATCH_EQUITIES);
            {
                let mut row = crate::tui::data::test_models::holding();
                row.tradingsymbol = format!("EQ{n:03}");
                row.exchange = "NSE".into();
                row.instrument_token = equity(n);
                row.quantity = 10;
                row.t1_quantity = 0;
                row.collateral_quantity = 0;
                row.mtf = None;
                row.average_price = close * (1.0 - 0.0001 * i);
                row.last_price = close;
                row.close_price = close;
                row.day_change = None;
                row.discrepancy = false;
                row
            }
        })
        .collect()
}

fn position(
    symbol: String,
    exchange: &str,
    token: u32,
    product: &str,
    close: u32,
    pnl: f64,
) -> Position {
    let close = f64::from(close) / 100.0;
    {
        let mut row = crate::tui::data::test_models::position();
        row.tradingsymbol = symbol;
        row.exchange = exchange.into();
        row.instrument_token = token;
        row.product = product.into();
        row.overnight_quantity = 0;
        row.day_buy_quantity = 100;
        row.day_sell_quantity = 0;
        row.quantity = 100;
        row.average_price = close;
        row.last_price = close;
        row.close_price = close;
        row.pnl = pnl;
        row.realised = 0.0;
        row.multiplier = 1.0;
        row
    }
}

fn positions() -> PositionsData {
    let first = WATCH_EQUITIES + HOLDINGS;
    let mut net: Vec<Position> = (first..EQUITIES)
        .map(|n| {
            let pnl = 10.0 * f64::from(n - first);
            position(
                format!("EQ{n:03}"),
                "NSE",
                equity(n),
                "MIS",
                equity_base(n),
                pnl,
            )
        })
        .collect();
    net.extend((0..FUTURES).map(|n| {
        let pnl = 10.0 * f64::from(n + EQUITIES - first);
        position(
            format!("FUT{n:02}"),
            "NFO",
            future(n),
            "NRML",
            future_base(n),
            pnl,
        )
    }));
    {
        let mut row = crate::tui::data::test_models::positions();
        row.net = net;
        row
    }
}

fn seed_candles(base: u32) -> Vec<Candle> {
    let close = f64::from(base) / 100.0;
    (0..SEED_CANDLES)
        .map(|i| {
            let minutes = 9 * 60 + 30 - 5 * (SEED_CANDLES - 1 - i);
            {
                let mut row = crate::tui::data::test_models::candle();
                row.date = format!(
                    "{}+05:30",
                    (format!("2026-10-07 {:02}:{:02}:00", minutes / 60, minutes % 60))
                        .replace(' ', "T")
                )
                .parse()
                .expect("date");
                row.open = close;
                row.high = close + 1.0;
                row.low = close - 1.0;
                row.close = close;
                row.volume = 1_000;
                row
            }
        })
        .collect()
}

fn replay_app(preset: Preset) -> App {
    let mut app = test_app();
    app.instruments = Some(Arc::new(InstrumentStore::new(universe())));
    app.set_watch_for_test(WatchState::test_loaded(watch_rows()));
    app.holdings = Some(holdings());
    app.positions = Some(positions());
    app.orders = Some(Vec::new());
    app.movers = Some(movers());
    for n in 0..CHARTED {
        app.candles.insert(
            SeriesKey::new(equity(n), ChartInterval::M5),
            seed_candles(equity_base(n)),
        );
    }
    app.set_workspace(preset);
    if preset == Preset::Portfolio {
        app.focus_pane_for_test(PaneId::PortfolioPositions);
        app.set_selection(2);
    }
    app
}

#[derive(Default)]
struct Sample {
    parse: Duration,
    update: Duration,
    render: Duration,
    packets: usize,
    dispatched: usize,
    renders: usize,
}

fn replay(app: &mut App, terminal: &mut Terminal<TestBackend>, frames: &[Vec<u8>]) -> Sample {
    let mut sample = Sample::default();
    for bytes in frames {
        let started = Instant::now();
        let ticks: Vec<Tick> = parse_binary(bytes);
        let parsed = Instant::now();
        sample.packets += ticks.len();
        sample.dispatched += ticks.len();
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(ticks)));
        let updated = Instant::now();
        terminal
            .draw(|frame| ui::draw(frame, app))
            .expect("frame draws");
        let drawn = Instant::now();
        sample.renders += 1;
        sample.parse += parsed - started;
        sample.update += updated - parsed;
        sample.render += drawn - updated;
    }
    sample
}

fn screen_text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    (0..HEIGHT)
        .map(|y| {
            (0..WIDTH)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn symbol_order(app: &App) -> Vec<String> {
    app.positions_order_in(PaneId::PortfolioPositions)
        .into_iter()
        .map(|i| app.positions_net()[i].tradingsymbol.clone())
        .collect()
}

fn selected_position(app: &App) -> String {
    app.selected_position_in(PaneId::PortfolioPositions)
        .expect("a position is selected")
        .tradingsymbol
        .clone()
}

fn last_m5(app: &App, token: u32) -> Candle {
    app.candles
        .candles(SeriesKey::new(token, ChartInterval::M5))
        .and_then(|candles| candles.last())
        .cloned()
        .expect("charted series retained")
}

fn assert_replayed_state(app: &App, sample: &Sample) {
    let total = (BATCHES * PACKETS_PER_BATCH) as usize;
    assert_eq!(sample.packets, total, "every packet decoded");
    assert_eq!(sample.dispatched, total, "every tick dispatched");
    assert_eq!(sample.renders, BATCHES as usize, "one draw per frame");
    assert_eq!(app.ticks.len(), 240, "every subscribed token holds a quote");
    assert_eq!(app.desired_subscriptions().len(), 240);

    let first_equity = app.tick(equity(0)).expect("EQ000 quoted");
    assert_eq!(first_equity.last_price, Some(499.82));
    assert_eq!(first_equity.volume, Some(149_500));
    assert_eq!(
        first_equity.depth.as_ref().map(|depth| depth.buy.len()),
        Some(5)
    );

    let repeat_equity = app.tick(equity(40)).expect("EQ040 quoted");
    assert_eq!(repeat_equity.last_price, Some(900.23));
    assert_eq!(repeat_equity.volume, Some(4_149_500));
    assert_eq!(app.live_ltp(future(19)), Some(21_899.77));
    assert_eq!(app.live_ltp(index(19)), Some(19_500.06));

    for n in 0..CHARTED {
        let series = app
            .candles
            .candles(SeriesKey::new(equity(n), ChartInterval::M5))
            .expect("charted series retained");
        assert_eq!(
            series.len(),
            SEED_CANDLES + 1,
            "EQ{n:03} opened the 09:35 bucket"
        );
    }
    let candle = last_m5(app, equity(0));
    assert_eq!(
        crate::tui::data::candle_timestamp(&candle),
        "2026-10-07 09:35:00"
    );
    assert_eq!((candle.open, candle.close), (500.2, 499.82));
    assert_eq!((candle.high, candle.low), (500.22, 499.8));
    assert_eq!(candle.volume, 11_000);
}

fn assert_portfolio(app: &App, terminal: &Terminal<TestBackend>) {
    assert_eq!(
        selected_position(app),
        "FUT17",
        "the cursor followed its record"
    );
    let order = symbol_order(app);
    assert_eq!(&order[..5], ["FUT17", "FUT16", "FUT19", "FUT15", "FUT18"]);
    assert_screen_has(terminal, &["FUT17", "21,700.24"]);
}

fn assert_screen_has(terminal: &Terminal<TestBackend>, needles: &[&str]) {
    let text = screen_text(terminal);
    for needle in needles {
        assert!(text.contains(needle), "screen lacks {needle:?}:\n{text}");
    }
}

fn check_scenario(preset: Preset, app: &App, terminal: &Terminal<TestBackend>, sample: &Sample) {
    assert_replayed_state(app, sample);
    match preset {
        Preset::Portfolio => assert_portfolio(app, terminal),
        Preset::Trading => assert_screen_has(
            terminal,
            &[
                "NSE:EQ000",
                "O 500.20 H 500.22 L 499.80 C 499.82",
                "V 11,000",
                "499.77",
                "5,198",
            ],
        ),
        Preset::Overview => assert_screen_has(
            terminal,
            &[
                "EQ180",
                "2,299.96",
                "EQ179",
                "2,289.76",
                "IDX00",
                "10,000.00",
                "MOV0",
            ],
        ),
        other => unreachable!("no replay scenario for {other:?}"),
    }
}

const SCENARIOS: [(&str, Preset); 3] = [
    ("overview", Preset::Overview),
    ("portfolio", Preset::Portfolio),
    ("trading", Preset::Trading),
];
const WARMUP: usize = 1;
const SAMPLES: usize = 7;

#[test]
#[ignore = "timing harness; run explicitly with --release --ignored --nocapture"]
fn tick_replay() {
    let frames = frames();
    let wire_bytes: usize = frames.iter().map(Vec::len).sum();
    println!(
        "REPLAY frames={BATCHES} packets_per_frame={PACKETS_PER_BATCH} wire_bytes={wire_bytes} \
         universe={UNIVERSE} subscribed=240 charted={CHARTED} screen={WIDTH}x{HEIGHT} \
         warmup={WARMUP} samples={SAMPLES}"
    );

    let portfolio = replay_app(Preset::Portfolio);
    assert_eq!(selected_position(&portfolio), "FUT17");
    assert_eq!(
        &symbol_order(&portfolio)[..5],
        ["FUT19", "FUT18", "FUT17", "FUT16", "FUT15"]
    );

    let rounds = WARMUP + SAMPLES;
    for round in 0..rounds {
        for (name, preset) in SCENARIOS {
            let mut app = replay_app(preset);
            let mut terminal =
                Terminal::new(TestBackend::new(WIDTH, HEIGHT)).expect("test terminal");
            let sample = replay(&mut app, &mut terminal, &frames);
            check_scenario(preset, &app, &terminal, &sample);
            if round < WARMUP {
                continue;
            }
            let total = sample.parse + sample.update + sample.render;
            println!(
                "SAMPLE scenario={name} round={} parse_us={} update_us={} render_us={} total_us={} \
                 packets={} dispatched={} renders={}",
                round - WARMUP,
                sample.parse.as_micros(),
                sample.update.as_micros(),
                sample.render.as_micros(),
                total.as_micros(),
                sample.packets,
                sample.dispatched,
                sample.renders,
            );
        }
    }
}
