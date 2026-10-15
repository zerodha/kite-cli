use super::quote_price::{MISSING, fit_value, ltp_rows};
use super::quote_range::{circuit_line, range_line};
use crate::tui::app::App;
use crate::tui::candles::SeriesKey;
use crate::tui::data::ChartInterval;
use crate::tui::format;
use kiteconnect::ticker::Tick;
use ratatui::text::{Line, Span};

pub(super) fn quote_width(inner: u16) -> usize {
    let inset = usize::from(inner >= 4);
    inner as usize - inset * 2
}

const QUOTE_LABEL_W: usize = 5;
pub(super) const QUOTE_GAP: &str = "  ";
const QUOTE_PAIR_MIN: usize = 38;

fn quote_fields(width: usize) -> (usize, usize) {
    let gap = format::display_width(QUOTE_GAP);
    let left = (width - gap) / 2;
    (left, width - gap - left)
}

fn quote_field(label: &str, value: &str, width: usize) -> Vec<Span<'static>> {
    let label_w = QUOTE_LABEL_W.min(width);
    let mut spans = vec![Span::styled(
        format::pad_or_clip(label, label_w),
        format::dim(),
    )];
    if width > label_w {
        spans.push(Span::raw(format::pad_or_clip_right(value, width - label_w)));
    }
    spans
}

fn quote_pair_rows(
    left: Option<(&str, String)>,
    right: Option<(&str, String)>,
    width: usize,
) -> Vec<Line<'static>> {
    let fields: Vec<(&str, String)> = [left, right].into_iter().flatten().collect();
    if fields.is_empty() {
        return Vec::new();
    }
    if width >= QUOTE_PAIR_MIN {
        let (left_w, right_w) = quote_fields(width);
        let widths = [left_w, right_w];
        let fits = |(_, value): &(&str, String), field: &usize| {
            QUOTE_LABEL_W + format::display_width(value) <= *field
        };
        if fields
            .iter()
            .zip(&widths)
            .all(|(field, width)| fits(field, width))
        {
            let mut spans: Vec<Span<'static>> = Vec::new();
            for ((label, value), width) in fields.iter().zip(&widths) {
                if !spans.is_empty() {
                    spans.push(Span::raw(QUOTE_GAP));
                }
                spans.extend(quote_field(label, value, *width));
            }
            return vec![Line::from(spans)];
        }
    }
    fields
        .into_iter()
        .map(|(label, value)| {
            let value = fit_value(&value, width.saturating_sub(QUOTE_LABEL_W));
            Line::from(quote_field(label, &value, width))
        })
        .collect()
}

struct QuoteText {
    ohlc: Vec<Line<'static>>,
    meta: Vec<Line<'static>>,
}

pub(super) fn quote_panel_lines(
    app: &App,
    item: &crate::tui::marketwatch::WatchItem,
    tick: &Tick,
    width: usize,
    interval: ChartInterval,
) -> Vec<Line<'static>> {
    let QuoteText { ohlc, meta } = quote_lines(tick, width);
    let mut lines = ohlc;

    if !item.is_index() {
        match app.circuits.get(&item.instrument_token) {
            Some(Some((lower, upper))) => {
                if let Some(ltp) = tick.last_price {
                    lines.extend(fitted_range(
                        circuit_line(*lower, ltp, *upper),
                        (("LC", *lower), ("UC", *upper)),
                        width,
                    ));
                }
            }
            Some(None) => {
                lines.push(Line::from(Span::styled(
                    "circuit limits unavailable",
                    format::dim(),
                )));
            }
            None => {} // fetch hasn't landed yet
        }
    }
    if let (Some(low), Some(high), Some(ltp)) = (tick.low, tick.high, tick.last_price)
        && high > low
    {
        lines.extend(fitted_range(
            range_line("Day", low, ltp, high),
            (("Day", low), ("high", high)),
            width,
        ));
    }
    if interval == ChartInterval::Day
        && let Some(candles) = app
            .candle_store()
            .candles(SeriesKey::new(item.instrument_token, interval))
            .filter(|candles| !candles.is_empty())
    {
        let (low, high) = candles
            .iter()
            .fold((f64::MAX, f64::MIN), |(low, high), candle| {
                (low.min(candle.low), high.max(candle.high))
            });
        if high > low {
            let value = tick.last_price.unwrap_or(low.midpoint(high));
            lines.extend(fitted_range(
                range_line("52wk", low, value, high),
                (("52wk", low), ("high", high)),
                width,
            ));
        }
    }
    lines.extend(meta);
    lines
}

fn fitted_range(
    bar: Line<'static>,
    endpoints: ((&str, f64), (&str, f64)),
    width: usize,
) -> Vec<Line<'static>> {
    if bar.width() <= width {
        return vec![bar];
    }
    let ((low_label, low), (high_label, high)) = endpoints;
    quote_pair_rows(
        Some((low_label, format::price(low))),
        Some((high_label, format::price(high))),
        width,
    )
}

fn quote_lines(tick: &Tick, width: usize) -> QuoteText {
    let price = |value: Option<f64>| value.map(format::price);
    let mut ohlc = ltp_rows(tick, width);
    ohlc.extend(quote_pair_rows(
        Some(("Open", price(tick.open).unwrap_or_else(|| MISSING.into()))),
        Some(("High", price(tick.high).unwrap_or_else(|| MISSING.into()))),
        width,
    ));
    ohlc.extend(quote_pair_rows(
        Some(("Low", price(tick.low).unwrap_or_else(|| MISSING.into()))),
        Some(("Close", price(tick.close).unwrap_or_else(|| MISSING.into()))),
        width,
    ));

    let mut meta = quote_pair_rows(
        tick.volume
            .map(|volume| ("Vol", format::compact_qty(volume))),
        tick.average_price.map(|avg| ("Avg", format::price(avg))),
        width,
    );
    meta.extend(quote_pair_rows(
        tick.oi
            .filter(|oi| *oi > 0)
            .map(|oi| ("OI", format::compact_qty(oi))),
        tick.last_trade_time
            .filter(|time| *time > 0)
            .map(|time| ("LTT", format::time_ist(time))),
        width,
    ));

    QuoteText { ohlc, meta }
}

#[cfg(test)]
mod quote_panel_tests {
    use super::quote_panel_lines;
    use crate::tui::app::App;
    use crate::tui::marketwatch::WatchItem;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use kiteconnect::KiteConnect;
    use kiteconnect::ticker::Tick;

    fn test_app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        )
    }

    fn watch_item(token: u32) -> WatchItem {
        WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: token,
            segment: String::new(),
            ..WatchItem::default()
        }
    }

    fn tick(ltp: f64) -> Tick {
        let mut tick = Tick::default();
        tick.instrument_token = 408_065;
        tick.last_price = Some(ltp);
        tick
    }

    #[test]
    fn circuit_row_renders_on_success() {
        let mut app = test_app();
        app.circuits.insert(408_065, Some((100.0, 200.0)));
        let item = watch_item(408_065);
        let lines = quote_panel_lines(&app, &item, &tick(150.0), 40, app.chart_pane.interval());
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(
            rendered.iter().any(|l| l.contains("LC")),
            "no LC label in: {rendered:?}"
        );
        assert!(
            rendered.iter().any(|l| l.contains("UC")),
            "no UC label in: {rendered:?}"
        );
    }

    #[test]
    fn circuit_row_shows_unavailable_on_known_failure() {
        let mut app = test_app();
        app.circuits.insert(408_065, None);
        let item = watch_item(408_065);
        let lines = quote_panel_lines(&app, &item, &tick(150.0), 40, app.chart_pane.interval());
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(
            rendered
                .iter()
                .any(|l| l.contains("circuit limits unavailable")),
            "no unavailable row in: {rendered:?}"
        );
    }

    #[test]
    fn circuit_row_absent_before_fetch_lands() {
        let app = test_app();
        let item = watch_item(408_065);
        let lines = quote_panel_lines(&app, &item, &tick(150.0), 40, app.chart_pane.interval());
        let rendered: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(
            !rendered
                .iter()
                .any(|l| l.contains("LC") || l.contains("UC")),
            "unexpected Circuit row before the fetch has landed: {rendered:?}"
        );
        assert!(!rendered.iter().any(|l| l.contains("unavailable")));
    }

    #[test]
    fn quote_line_count_grows_with_circuit_row_dynamic_height() {
        let mut app = test_app();
        let item = watch_item(408_065);
        let without =
            quote_panel_lines(&app, &item, &tick(150.0), 40, app.chart_pane.interval()).len();
        app.circuits.insert(408_065, Some((100.0, 200.0)));
        let with =
            quote_panel_lines(&app, &item, &tick(150.0), 40, app.chart_pane.interval()).len();
        assert_eq!(with, without + 1);
    }
}

#[cfg(test)]
mod quote_layout_tests {

    use super::super::draw_depth_leaf;
    use super::{quote_lines, quote_panel_lines, quote_width};
    use crate::tui::app::App;
    use crate::tui::data::test_models::depth_entry;
    use kiteconnect::KiteConnect;
    use kiteconnect::ticker::{MarketDepth, Tick, TickerEvent};

    use crate::tui::event::AppEvent;
    use crate::tui::format;
    use crate::tui::marketwatch::{State, WatchItem};
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings, Theme};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::{Color, Modifier};

    const INFY: u32 = 408_065;

    fn watch_app(items: Vec<WatchItem>) -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        App::new(
            client,
            events_tx,
            State::test_loaded(items),
            Pad::default(),
            true,
            &Settings::default(),
        )
    }

    fn infy() -> WatchItem {
        WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: INFY,
            ..WatchItem::default()
        }
    }

    fn spec_tick() -> Tick {
        let mut tick = Tick::default();
        tick.instrument_token = INFY;
        tick.last_price = Some(1_420.50);
        tick.open = Some(1_400.0);
        tick.high = Some(1_430.0);
        tick.low = Some(1_390.0);
        tick.close = Some(1_400.0);
        tick.volume = Some(1_240_000);
        tick
    }

    fn with_tick(tick: Tick) -> App {
        let mut app = watch_app(vec![infy()]);
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(vec![tick])));
        app
    }

    fn depth_buffer(app: &App, width: u16, height: u16, focused: bool) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        let item = app.watch_pane.selected(&app.watch);
        terminal
            .draw(|frame| draw_depth_leaf(frame, frame.area(), app, item, true, focused))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn buffer(app: &App, width: u16, height: u16) -> Buffer {
        depth_buffer(app, width, height, true)
    }

    fn cells(buffer: &Buffer, x: u16, y: u16, count: u16) -> String {
        (x..x + count)
            .map(|column| buffer[(column, y)].symbol())
            .collect()
    }

    fn rows(app: &App, width: u16, height: u16) -> Vec<String> {
        let buffer = buffer(app, width, height);
        (0..height).map(|y| cells(&buffer, 0, y, width)).collect()
    }

    fn column_of(buffer: &Buffer, y: u16, needle: &str) -> u16 {
        let row: Vec<String> = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol().to_string())
            .collect();
        let want: Vec<String> = needle.chars().map(|glyph| glyph.to_string()).collect();
        u16::try_from(
            row.windows(want.len())
                .position(|window| window == want.as_slice())
                .unwrap_or_else(|| panic!("{needle:?} is not in row {y}")),
        )
        .unwrap()
    }

    fn text_rows(tick: &Tick, width: usize) -> Vec<String> {
        let app = watch_app(vec![infy()]);
        quote_panel_lines(&app, &infy(), tick, width, app.chart_pane.interval())
            .iter()
            .map(|line| {
                let row: String = line
                    .spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect();
                row.trim_end().to_string()
            })
            .collect()
    }

    #[test]
    fn the_zoomed_quote_is_a_centred_column_at_eighty_columns() {
        let app = with_tick(spec_tick());
        let buffer = buffer(&app, 80, 22);

        assert_eq!(
            cells(&buffer, 0, 0, 80),
            format!("┌ > Depth | INFY {}┐", "─".repeat(62)),
            "the Depth leaf's identity behind the focus marker"
        );
        assert_eq!(quote_width(48), 46);
        let expected = [
            "LTP 1,420.50                   +20.50 (+1.46%)",
            "Open          1,400.00  High          1,430.00",
            "Low           1,390.00  Close         1,400.00",
            "Day   1,390.00 ├────────●───┤ 1,430.00        ",
            "Vol             12.40L                        ",
            "                                              ",
            "Waiting for depth...                          ",
        ];
        for (offset, row) in expected.iter().enumerate() {
            assert_eq!(
                &cells(&buffer, 17, 1 + u16::try_from(offset).unwrap(), 46),
                row,
                "row {offset}"
            );
        }
        for y in 1..8 {
            assert_eq!(cells(&buffer, 1, y, 16).trim(), "", "left margin, row {y}");
            assert_eq!(
                cells(&buffer, 64, y, 15).trim(),
                "",
                "right margin, row {y}"
            );
        }
    }

    #[test]
    fn the_quote_matches_the_spec_canvas_in_a_forty_four_cell_pane() {
        let app = with_tick(spec_tick());
        let buffer = depth_buffer(&app, 44, 33, false);

        assert_eq!(
            cells(&buffer, 0, 0, 44),
            format!("┌   Depth | INFY {}┐", "─".repeat(26)),
            "the unfocused leaf keeps its marker's width blank"
        );
        assert_eq!(quote_width(42), 40);
        let expected = [
            "LTP 1,420.50             +20.50 (+1.46%)",
            "Open       1,400.00  High       1,430.00",
            "Low        1,390.00  Close      1,400.00",
            "Day   1,390.00 ├────────●───┤ 1,430.00  ",
            "Vol          12.40L                     ",
            "                                        ",
            "Waiting for depth...                    ",
        ];
        for (offset, row) in expected.iter().enumerate() {
            assert_eq!(
                &cells(&buffer, 2, 1 + u16::try_from(offset).unwrap(), 40),
                row,
                "row {offset}"
            );
        }
    }

    #[test]
    fn every_preset_renders_the_same_quote_cells() {
        let app = with_tick(spec_tick());
        let baseline = {
            let _guard = settings::theme_guard(Theme::DEFAULT);
            rows(&app, 80, 22)
        };
        for (name, theme) in settings::PRESETS {
            let _guard = settings::theme_guard(*theme);
            assert_eq!(rows(&app, 80, 22), baseline, "preset {name} moved a cell");
        }
        for name in ["kite", "solarized-light", "system"] {
            let theme = settings::preset(name).unwrap_or_else(|| panic!("preset {name}"));
            let _guard = settings::theme_guard(theme);
            assert_eq!(rows(&app, 80, 22), baseline, "preset {name} moved a cell");
        }
    }

    #[test]
    fn thirty_eight_cells_is_the_last_width_that_pairs_a_row() {
        let tick = spec_tick();
        let paired = text_rows(&tick, 38);
        assert_eq!(
            paired[1], "Open      1,400.00  High      1,430.00",
            "18-cell fields at the boundary: {paired:?}"
        );
        assert_eq!(paired[2], "Low       1,390.00  Close     1,400.00");

        let stacked = text_rows(&tick, 37);
        assert_eq!(
            &stacked[1..5],
            [
                "Open                         1,400.00",
                "High                         1,430.00",
                "Low                          1,390.00",
                "Close                        1,400.00",
            ],
            "{stacked:?}"
        );
        assert_eq!(
            stacked.len(),
            paired.len() + 3,
            "stacking is what grows the reserved height: {stacked:?}"
        );
    }

    #[test]
    fn a_field_whose_own_half_is_too_narrow_stacks_the_whole_pair() {
        let mut tick = spec_tick();
        tick.low = Some(1_000_000_000.0);
        tick.close = Some(1_400.0);
        let rows = text_rows(&tick, 40);
        assert_eq!(
            &rows[2..4],
            [
                "Low                    1,00,00,00,000.00",
                "Close                           1,400.00",
            ],
            "{rows:?}"
        );
    }

    #[test]
    fn a_price_with_no_room_at_all_says_overflow_not_a_clipped_number() {
        let mut tick = spec_tick();
        tick.open = Some(12_345_678.90);
        let rows = text_rows(&tick, 15);
        assert!(
            rows.contains(&"Open   overflow".to_string()),
            "a clipped numeric suffix reads as a smaller real number: {rows:?}"
        );
        let rows = text_rows(&tick, 8);
        assert!(rows.contains(&"Open   ?".to_string()), "{rows:?}");
    }

    #[test]
    fn a_missing_value_is_a_question_mark_and_never_a_zero() {
        let mut tick = Tick::default();
        tick.instrument_token = INFY;
        tick.last_price = None;
        tick.open = None;
        tick.high = Some(1_430.0);
        tick.low = None;
        tick.close = None;
        let rows = text_rows(&tick, 40);
        assert_eq!(rows[0], "LTP ?", "no LTP is not a zero LTP: {rows:?}");
        assert_eq!(rows[1], "Open              ?  High       1,430.00");
        assert_eq!(rows[2], "Low               ?  Close             ?");
        assert_eq!(
            rows.len(),
            3,
            "no low/high range and no volume, so no rows for them: {rows:?}"
        );
    }

    #[test]
    fn the_change_group_drops_to_its_own_row_before_the_price_moves() {
        let tick = spec_tick();
        let rows = text_rows(&tick, 28);
        assert_eq!(rows[0], "LTP 1,420.50");
        assert_eq!(
            rows[1], "             +20.50 (+1.46%)",
            "the change group is right-aligned on its own row: {rows:?}"
        );

        let rows = text_rows(&tick, 12);
        assert_eq!(rows[0], "LTP 1,420.50");
        assert_eq!(rows[1], "      +20.50");
        assert_eq!(rows[2], "      +1.46%");
    }

    #[test]
    fn the_ltp_label_is_given_up_before_the_price_is() {
        let tick = spec_tick();
        let rows = text_rows(&tick, 8);
        assert_eq!(rows[0], "1,420.50", "{rows:?}");
    }

    #[test]
    fn a_quote_with_no_close_has_no_change_group() {
        let mut tick = spec_tick();
        tick.close = None;
        let rows = text_rows(&tick, 40);
        assert_eq!(rows[0], "LTP 1,420.50", "{rows:?}");
        assert!(
            !rows.iter().any(|row| row.contains('%')),
            "no close is no change and no percent: {rows:?}"
        );
    }

    #[test]
    fn the_metadata_rows_carry_only_the_values_the_tick_has() {
        let mut bare = spec_tick();
        bare.volume = None;
        let rows = text_rows(&bare, 40);
        assert!(
            !rows.iter().any(|row| row.starts_with("Vol")),
            "an absent volume is an absent row, not a zero: {rows:?}"
        );

        let mut full = spec_tick();
        full.average_price = Some(1_410.25);
        full.oi = Some(250_000);
        full.last_trade_time = Some(1_757_500_000);
        let rows = text_rows(&full, 40);
        assert!(
            rows.contains(&"Vol          12.40L  Avg        1,410.25".to_string()),
            "{rows:?}"
        );
        assert!(
            rows.contains(&"OI            2.50L  LTT        15:56:40".to_string()),
            "{rows:?}"
        );
    }

    #[test]
    fn a_range_bar_that_does_not_fit_keeps_both_endpoints() {
        let tick = spec_tick();
        let render = |limits: (f64, f64), width: usize| -> Vec<String> {
            let mut app = watch_app(vec![infy()]);
            app.circuits.insert(INFY, Some(limits));
            quote_panel_lines(&app, &infy(), &tick, width, app.chart_pane.interval())
                .iter()
                .map(|line| {
                    let row: String = line
                        .spans
                        .iter()
                        .map(|span| span.content.as_ref())
                        .collect();
                    row.trim_end().to_string()
                })
                .collect()
        };

        let wide = render((1_260.0, 1_540.0), 40);
        assert!(
            wide.contains(&"LC  1,260.00 ├──────●─────┤ 1,540.00 UC".to_string()),
            "{wide:?}"
        );
        assert!(
            wide.contains(&"Day   1,390.00 ├────────●───┤ 1,430.00".to_string()),
            "{wide:?}"
        );

        let huge = render((1_260_000.0, 1_540_000.0), 40);
        assert!(
            huge.contains(&"LC     12,60,000.00  UC     15,40,000.00".to_string()),
            "{huge:?}"
        );
        assert!(
            huge.contains(&"Day   1,390.00 ├────────●───┤ 1,430.00".to_string()),
            "the Day bar still fits, so it keeps its track: {huge:?}"
        );

        let narrow = render((1_260.0, 1_540.0), 30);
        assert!(!narrow.iter().any(|row| row.contains('●')), "{narrow:?}");
        assert_eq!(
            &narrow[5..9],
            [
                "LC                    1,260.00",
                "UC                    1,540.00",
                "Day                   1,390.00",
                "high                  1,430.00",
            ],
            "{narrow:?}"
        );
    }

    #[test]
    fn the_quote_price_never_forces_terminal_white() {
        let ltp_cell = |app: &App| {
            let buffer = buffer(app, 120, 33);
            let at = column_of(&buffer, 1, "1,420.50");
            buffer[(at, 1)].clone()
        };
        let app = with_tick(spec_tick());
        {
            let _guard = settings::theme_guard(Theme::DEFAULT);
            let cell = ltp_cell(&app);
            assert_eq!(
                cell.fg,
                Color::Reset,
                "terminal-native must stay terminal-native, not white"
            );
            assert!(cell.modifier.contains(Modifier::BOLD));
        }
        {
            let light = settings::preset("solarized-light").expect("a light preset");
            let _guard = settings::theme_guard(light);
            assert_eq!(
                ltp_cell(&app).fg,
                light.text.expect("a light preset sets text")
            );
        }
    }

    #[test]
    fn the_quote_labels_are_muted_and_the_change_is_semantic() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let app = with_tick(spec_tick());
        let buffer = depth_buffer(&app, 44, 33, false);
        assert_eq!(buffer[(2, 1)].fg, format::text_muted(), "the `LTP` label");
        let at = column_of(&buffer, 1, "+20.50");
        assert_eq!(buffer[(at, 1)].fg, format::up());
    }

    #[test]
    fn the_depth_ladder_keeps_its_rows_and_its_full_inner_width() {
        let level = |price: f64| depth_entry(100, price, 2);
        let app = with_tick({
            let mut value = spec_tick();
            value.depth = Some({
                let mut value = MarketDepth::default();
                value.buy = (0..5).map(|i| level(1_420.0 - f64::from(i))).collect();
                value.sell = (0..5).map(|i| level(1_421.0 + f64::from(i))).collect();
                value
            });
            value
        });

        for width in [44u16, 80, 120] {
            let rows = rows(&app, width, 22);
            assert!(
                rows.iter()
                    .any(|row| row.contains("QTY") && row.contains("BID")),
                "{width}: {rows:?}"
            );
            assert!(
                rows.iter().any(|row| row.contains("1,416.00  1,425.00")),
                "five bid and five ask levels at {width}: {rows:?}"
            );
        }
    }

    #[test]
    fn an_index_says_depth_is_not_available_for_it() {
        let index = WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "NIFTY 50".into(),
            instrument_token: INFY,
            segment: "INDICES".into(),
            ..WatchItem::default()
        };
        let mut app = watch_app(vec![index]);
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(vec![{
            let mut value = spec_tick();
            value.is_index = true;
            value
        }])));
        let rows = rows(&app, 80, 22);
        assert!(
            rows.iter()
                .any(|row| row.contains("Market depth not available for indices")),
            "{rows:?}"
        );
        assert!(
            !rows.iter().any(|row| row.contains("Waiting for depth")),
            "an index is not a pending depth read: {rows:?}"
        );
        assert!(
            !rows.iter().any(|row| row.contains("LC")),
            "indices have no circuit limits: {rows:?}"
        );
    }

    #[test]
    fn a_quote_that_has_not_arrived_says_it_is_waiting() {
        let app = watch_app(vec![infy()]);
        let rows = rows(&app, 80, 22);
        assert!(
            rows.iter().any(|row| row.contains("Waiting for quote...")),
            "{rows:?}"
        );
        assert!(
            !rows.iter().any(|row| row.contains("LTP")),
            "no tick is no headline, not a zero one: {rows:?}"
        );
    }

    #[test]
    fn a_zero_width_quote_draws_nothing_and_does_not_panic() {
        let tick = spec_tick();
        for width in 0..6usize {
            let lines = quote_lines(&tick, width);
            for line in lines.ohlc.iter().chain(lines.meta.iter()) {
                assert!(
                    line.width() <= width,
                    "width {width} rendered {} cells",
                    line.width()
                );
            }
        }
        let app = with_tick(tick);
        for width in [0u16, 1, 2, 3, 4, 20] {
            for height in 0..5u16 {
                let _ = rows(&app, width.max(1), height.max(1));
            }
        }
    }
}
