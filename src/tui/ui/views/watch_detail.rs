use super::depth::draw_depth;
use super::quote::{quote_panel_lines, quote_width};
use crate::tui::app::App;
use crate::tui::data::ChartInterval;
use crate::tui::marketwatch::WatchItem;
use crate::tui::ui;
use kiteconnect::ticker::Tick;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::widgets::Paragraph;

const ZOOMED_QUOTE_DEPTH_W: u16 = 48;

fn draw_detail_message(frame: &mut Frame, area: Rect, message: &str) {
    ui::state_message(frame, area, ui::MessageTone::Normal, message, None, None);
}

pub fn draw_depth_leaf(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    item: Option<&WatchItem>,
    zoomed: bool,
    focused: bool,
) {
    let title = match item {
        Some(item) => format!("Depth | {}", item.tradingsymbol),
        None => "Depth".to_string(),
    };
    let block = ui::focus_panel(title, focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.is_empty() {
        return;
    }
    let Some(item) = item else {
        draw_detail_message(frame, inner, "Waiting for depth...");
        return;
    };
    if zoomed {
        draw_quote_depth(frame, inner, app, item, app.trading_chart.interval());
        return;
    }
    let Some(tick) = app.tick(item.instrument_token) else {
        draw_detail_message(frame, inner, "Waiting for depth...");
        return;
    };
    let circuit = app.circuits.get(&item.instrument_token).copied().flatten();
    draw_ladder(frame, inner, tick, circuit);
}

fn draw_quote_depth(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    item: &WatchItem,
    interval: ChartInterval,
) {
    let Some(tick) = app.tick(item.instrument_token) else {
        draw_detail_message(frame, area, "Waiting for quote...");
        return;
    };

    let column = area.width.min(ZOOMED_QUOTE_DEPTH_W);
    let area = Rect {
        x: area.x + (area.width - column) / 2,
        width: column,
        ..area
    };
    let width = quote_width(area.width);
    let lines = quote_panel_lines(app, item, tick, width, interval);
    let quote_h = u16::try_from(lines.len()).unwrap_or(u16::MAX);
    let [quote_area, depth_area] =
        Layout::vertical([Constraint::Length(quote_h), Constraint::Min(0)]).areas(area);

    let inset = (area.width - u16::try_from(width).unwrap_or(u16::MAX)) / 2;
    frame.render_widget(
        Paragraph::new(lines),
        Rect {
            x: quote_area.x + inset,
            width: u16::try_from(width).unwrap_or(u16::MAX),
            ..quote_area
        },
    );

    draw_ladder(frame, depth_area, tick, None);
}

fn draw_ladder(frame: &mut Frame, area: Rect, tick: &Tick, circuit: Option<(f64, f64)>) {
    if let Some(depth) = &tick.depth {
        draw_depth(frame, area, depth, tick, circuit);
    } else if tick.is_index {
        draw_detail_message(frame, area, "Market depth not available for indices");
    } else {
        draw_detail_message(frame, area, "Waiting for depth...");
    }
}

#[cfg(test)]
mod watch_detail_tests {
    use crate::tui::app::App;
    use crate::tui::data::test_models::depth_entry;
    use crate::tui::event::AppEvent;
    use crate::tui::marketwatch::{State, WatchItem};
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::{PaneId, Preset};
    use kiteconnect::KiteConnect;
    use kiteconnect::ticker::{MarketDepth, Tick, TickerEvent};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use tokio::sync::mpsc;

    const INFY: u32 = 408_065;

    fn deliver_tick(app: &mut App) {
        let level = |price: f64| depth_entry(100, price, 2);
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(vec![{
            let mut value = Tick::default();
            value.instrument_token = INFY;
            value.last_price = Some(120.0);
            value.open = Some(118.0);
            value.high = Some(121.0);
            value.low = Some(117.0);
            value.close = Some(119.0);
            value.volume = Some(1_000_000);
            value.depth = Some({
                let mut value = MarketDepth::default();
                value.buy = (0..5).map(|i| level(120.0 - f64::from(i))).collect();
                value.sell = (0..5).map(|i| level(121.0 + f64::from(i))).collect();
                value
            });
            value
        }])));
    }

    fn screen(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| crate::tui::ui::draw(frame, app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..h)
            .map(|row| {
                (0..w)
                    .map(|column| buffer[(column, row)].symbol().to_string())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    fn watch_app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = mpsc::channel(8);
        App::new(
            client,
            events_tx,
            State::test_loaded(vec![WatchItem {
                tradingsymbol: "INFY".into(),
                exchange: "NSE".into(),
                instrument_token: INFY,
                ..Default::default()
            }]),
            Pad::default(),
            true,
            &Settings::default(),
        )
    }

    #[test]
    fn the_trading_depth_leaf_keeps_whole_prices_at_every_size() {
        let mut app = watch_app();
        deliver_tick(&mut app);
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(vec![{
            let mut value = Tick::default();
            value.instrument_token = INFY;
            value.last_price = Some(1_235.0);
            value.buy_quantity = Some(12_000);
            value.sell_quantity = Some(9_000);
            value.depth = Some({
                let mut value = MarketDepth::default();
                value.buy = vec![depth_entry(12_345, 1_234.50, 3)];
                value.sell = vec![depth_entry(6_789, 1_235.50, 2)];
                value
            });
            value
        }])));
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingDepth);
        for (w, h) in [(80, 24), (120, 35), (160, 45)] {
            let text = screen(&app, w, h).join("\n");
            assert!(text.contains("Depth | INFY"), "{w}x{h}:\n{text}");
            for price in ["1,234.50", "1,235.50"] {
                assert!(text.contains(price), "{w}x{h}: {price} clipped:\n{text}");
            }
        }
    }

    #[test]
    fn rest_circuit_limits_render_under_the_depth_ladder() {
        let mut app = watch_app();
        deliver_tick(&mut app);
        app.on_event(AppEvent::Data(crate::tui::data::DataEvent::Circuits {
            token: INFY,
            result: Ok(Some((100.0, 200.0))),
        }));
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingDepth);
        let text = screen(&app, 160, 45).join("\n");
        assert!(text.contains("LC"), "{text}");
        assert!(text.contains("100.00"), "{text}");
        assert!(text.contains("UC"), "{text}");
        assert!(text.contains("200.00"), "{text}");
    }

    #[test]
    fn a_zoomed_depth_leaf_draws_the_quote_block_above_the_whole_ladder() {
        let mut app = watch_app();
        deliver_tick(&mut app);
        app.set_workspace(Preset::Trading);
        app.focus_pane_for_test(PaneId::TradingDepth);
        let quote = |rows: &[String]| rows.iter().any(|row| row.contains("+1.00 (+0.84%)"));
        let whole_ladder = |rows: &[String]| rows.iter().any(|row| row.contains("116.00  125.00"));

        let tiled = screen(&app, 160, 45);
        assert!(!quote(&tiled), "the tiled leaf is the ladder alone");

        app.set_pane(PaneId::TradingDepth);
        for (w, h) in [(80, 24), (120, 35), (160, 45)] {
            let rows = screen(&app, w, h);
            assert!(quote(&rows), "{w}x{h}: the quote block drew");
            assert!(whole_ladder(&rows), "{w}x{h}: the ladder is whole");
        }
    }
}
