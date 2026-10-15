use crate::tui::app::SortTarget;
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, FitCol};
pub(super) use crate::tui::ui::columns::{right, right_styled};
use crate::tui::ui::pane::PaneHits;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::Span;
use ratatui::widgets::{Cell, Table, TableState};

pub(super) fn titles_fit(width: u16, title: &str, totals: &[Span]) -> bool {
    let title_w = format::display_width(title);
    let totals_w: usize = totals.iter().map(Span::width).sum();
    title_w + totals_w + 3 <= width as usize
}

pub(super) fn symbol_cells(exchange: &str, tradingsymbol: &str) -> usize {
    format::display_width(exchange) + 1 + format::display_width(tradingsymbol)
}

pub(super) fn symbol_cell(exchange: &str, symbol: &str, width: u16) -> Cell<'static> {
    Cell::from(format::pad_or_clip(
        &format!("{exchange}:{symbol}"),
        usize::from(width),
    ))
}

pub(super) fn sorted_title(title: &str, sort: &str, marked: bool) -> String {
    if marked {
        format!(" {title} ")
    } else {
        format!(" {title} · {sort} ")
    }
}

pub(super) fn selectable_table(
    frame: &mut Frame,
    area: Rect,
    table: Table<'static>,
    layout: &columns::TableLayout,
    rows: TableRows<'_>,
    hits: &PaneHits<'_>,
) {
    let mut state = TableState::default().with_offset(rows.offset());
    if rows.count > 0 {
        state.select(Some(rows.selected.min(rows.count - 1)));
    }
    frame.render_stateful_widget(
        table
            .row_highlight_style(format::selection_style())
            .highlight_symbol(layout.highlight_symbol()),
        area,
        &mut state,
    );
    rows.store_offset(state.offset());
    let drawn = u16::try_from(rows.count.saturating_sub(state.offset())).unwrap_or(u16::MAX);
    let hit_rect = Rect {
        x: area.x,
        y: area.y.saturating_add(2),
        width: area.width,
        height: area.height.saturating_sub(3).min(drawn),
    };
    if !hit_rect.is_empty() {
        hits.push_rows(hit_rect, state.offset());
    }
}

#[derive(Clone, Copy)]
pub(super) struct TableRows<'a> {
    pub(super) selected: usize,
    pub(super) count: usize,
    pub(super) viewport: Option<&'a std::cell::Cell<usize>>,
}

impl TableRows<'_> {
    fn offset(&self) -> usize {
        self.viewport.map_or(0, std::cell::Cell::get)
    }

    fn store_offset(&self, offset: usize) {
        if let Some(viewport) = self.viewport {
            viewport.set(offset);
        }
    }
}

pub(super) fn record_header_hits(
    hits: &PaneHits<'_>,
    area: Rect,
    fit: &columns::ColumnFit,
    layout: &columns::TableLayout,
    cols: &[FitCol],
    targets: &[(usize, SortTarget)],
) {
    if area.height < 3 {
        return;
    }
    let y = area.y.saturating_add(1);
    let mut x = area
        .x
        .saturating_add(1 + layout.inset + columns::SELECT_GUTTER);
    let right = area.right().saturating_sub(1);
    for idx in 0..cols.len() {
        if !fit.shows(idx) {
            continue;
        }
        let width = layout.width(idx).min(right.saturating_sub(x));
        if width == 0 {
            break;
        }
        if let Some((_, target)) = targets.iter().find(|(target_idx, _)| *target_idx == idx) {
            hits.push(
                Rect {
                    x,
                    y,
                    width,
                    height: 1,
                },
                ui::HitAction::Header {
                    owner: hits.owner(),
                    target: *target,
                },
            );
        }
        x = x.saturating_add(width).saturating_add(layout.spacing);
    }
}

#[cfg(test)]
mod width_render_tests {

    use super::super::{
        GTT_COLS, GTT_LABELS, HOLDINGS_COLS, HOLDINGS_LABELS, MF_COLS, MF_LABELS, ORD_COLS,
        ORD_LABELS, POS_COLS, POS_LABELS, WATCH_COLS, WATCH_LABELS, draw_holdings, draw_positions,
        holdings_sort_targets, table_budget,
    };
    use crate::tui::app::App;
    use crate::tui::data::Holding;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::ui::columns;
    use crate::tui::ui::columns::FitCol;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const WIDTHS: [u16; 6] = [60, 70, 80, 100, 120, 200];

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

    fn reported_holding() -> Holding {
        {
            let mut row = crate::tui::data::test_models::holding();
            row.tradingsymbol = "DIVISLAB".into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.quantity = 15;
            row.t1_quantity = 0;
            row.collateral_quantity = 0;
            row.mtf = None;
            row.average_price = 6_077.93;
            row.last_price = 30_000.0;
            row.close_price = 25_000.0;
            row.day_change = None;
            row.discrepancy = false;
            row
        }
    }

    const PNL_FULL: &str = "+3,58,831.05";
    const PNL_FRAGMENT: &str = "58,831";

    fn holdings_cells() -> Vec<(&'static str, String)> {
        vec![
            ("SYMBOL", "NSE:DIVISLAB".to_string()),
            ("QTY", "15".to_string()),
            ("AVG", "6,077.93".to_string()),
            ("LTP", "30,000.00".to_string()),
            ("VALUE", "₹4,50,000".to_string()),
            ("P&L", PNL_FULL.to_string()),
            ("P&L%", "+393.59%".to_string()),
            ("DAY%", "+20.00%".to_string()),
        ]
    }

    fn rows_at(
        width: u16,
        app: &App,
        draw: fn(&mut ratatui::Frame, ratatui::layout::Rect, &App),
    ) -> Vec<String> {
        let height = 8;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
            .collect()
    }

    fn holdings_app() -> App {
        let mut app = test_app();
        app.holdings = Some(vec![reported_holding()]);
        app
    }

    #[test]
    fn holdings_pnl_is_whole_or_absent_at_every_width() {
        let app = holdings_app();
        for width in WIDTHS {
            for (y, row) in rows_at(width, &app, draw_holdings).iter().enumerate() {
                assert!(
                    row.contains(PNL_FULL) || !row.contains(PNL_FRAGMENT),
                    "width {width} row {y} rendered a partial P&L: {row:?}"
                );
            }
        }
    }

    #[test]
    fn holdings_cells_are_whole_or_absent_at_every_width() {
        let app = holdings_app();
        for width in 30..=200u16 {
            let rows = rows_at(width, &app, draw_holdings);
            for (label, value) in holdings_cells() {
                let tail: String = value.chars().skip(2).collect();
                if tail.chars().count() >= 4 && !value.contains('…') {
                    for (y, row) in rows.iter().enumerate().take(3).skip(1) {
                        assert!(
                            row.contains(&value) || !row.contains(&tail),
                            "width {width} row {y}: {label} rendered only {tail:?}: {row:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn holdings_header_and_data_agree_at_every_width() {
        let app = holdings_app();
        for width in WIDTHS {
            let rows = rows_at(width, &app, draw_holdings);
            let header = &rows[1];
            let data = &rows[2];
            let sortable = holdings_sort_targets().map(|(idx, _)| idx);
            let cols = columns::sort_floors(&HOLDINGS_COLS, &HOLDINGS_LABELS, &sortable);
            let fit = columns::fit_columns(table_budget(width), 1, &cols);
            for (idx, (label, value)) in holdings_cells().into_iter().enumerate() {
                let shown = fit.shows(idx);
                assert_eq!(
                    header.contains(label),
                    shown,
                    "width {width}: header/{label} disagrees with the fit: {header:?}"
                );
                if idx == 0 {
                    continue; // the symbol ellipsises rather than disappearing
                }
                assert_eq!(
                    data.contains(&value),
                    shown,
                    "width {width}: data/{label} disagrees with the fit: {data:?}"
                );
            }
            assert_eq!(
                HOLDINGS_LABELS.len(),
                HOLDINGS_COLS.len(),
                "labels and columns must describe the same table"
            );
        }
    }

    #[test]
    fn holdings_symbol_clips_with_an_ellipsis() {
        let app = holdings_app();
        for width in 30..=200u16 {
            let data = rows_at(width, &app, draw_holdings)[2].clone();
            assert!(
                data.contains("NSE:DIVISLAB") || data.contains('…'),
                "width {width} cut the symbol with no ellipsis: {data:?}"
            );
        }
    }

    #[test]
    fn holdings_totals_strip_is_whole_or_dropped() {
        let app = holdings_app();
        for width in 30..=200u16 {
            let title = rows_at(width, &app, draw_holdings)[0].clone();
            assert!(
                title.contains("Holdings"),
                "width {width} lost the panel title: {title:?}"
            );
            if title.contains("total") {
                assert!(
                    title.contains(PNL_FULL),
                    "width {width} clipped the totals strip: {title:?}"
                );
            }
        }
    }

    fn positions_app() -> App {
        let mut app = test_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![{
                let mut row = crate::tui::data::test_models::position();
                row.tradingsymbol = "DIVISLAB".into();
                row.exchange = "NSE".into();
                row.instrument_token = 1;
                row.product = "CNC".into();
                row.overnight_quantity = 0;
                row.day_buy_quantity = 0;
                row.day_sell_quantity = 0;
                row.quantity = 15;
                row.average_price = 6_077.93;
                row.last_price = 30_000.0;
                row.close_price = 25_000.0;
                row.pnl = 358_831.05;
                row.realised = 0.0;
                row.multiplier = 1.0;
                row
            }];
            row
        });
        app
    }

    #[test]
    fn positions_pnl_is_whole_or_absent_at_every_width() {
        let app = positions_app();
        for width in 30..=200u16 {
            let rows = rows_at(width, &app, draw_positions);
            for (y, row) in rows.iter().enumerate() {
                if !row.contains(PNL_FRAGMENT) {
                    continue;
                }
                assert!(
                    row.contains(PNL_FULL) || row.contains("+₹3,58,831"),
                    "width {width} row {y} rendered a partial P&L: {row:?}"
                );
            }
        }
    }

    #[test]
    fn positions_summary_strip_drops_segments_instead_of_clipping() {
        let summary = super::super::PositionsSummary {
            total: 358_831.05,
            realized: 1_234.0,
            unrealized: 357_597.05,
            open: 1,
            total_count: 1,
        };
        let text_of = |width: u16| -> String {
            super::super::positions_summary_spans(&summary, width)
                .iter()
                .map(|span| span.content.as_ref().to_string())
                .collect()
        };
        let full = text_of(200);
        for part in [
            "Positions P&L",
            "realized",
            "unrealized",
            "1 open / 1 total",
        ] {
            assert!(full.contains(part), "missing {part:?} in {full:?}");
        }
        for width in 10..=200u16 {
            let text = text_of(width);
            assert!(
                text.chars().count() <= width as usize
                    || text.contains("  Positions P&L +₹3,58,831"),
                "width {width}: {text:?}"
            );
            assert!(
                text.contains("+₹3,58,831"),
                "width {width} lost the P&L total: {text:?}"
            );
        }
        let narrow = text_of(25);
        assert!(narrow.contains("Positions P&L"), "{narrow:?}");
        assert!(!narrow.contains("open /"), "{narrow:?}");
        assert!(!narrow.contains("realized"), "{narrow:?}");
    }

    #[test]
    fn column_priorities_are_unique_and_rooted_at_zero() {
        let tables: [(&str, &[FitCol], usize); 6] = [
            ("watch", &WATCH_COLS, WATCH_LABELS.len()),
            ("positions", &POS_COLS, POS_LABELS.len()),
            ("orders", &ORD_COLS, ORD_LABELS.len()),
            ("gtt", &GTT_COLS, GTT_LABELS.len()),
            ("holdings", &HOLDINGS_COLS, HOLDINGS_LABELS.len()),
            ("mf", &MF_COLS, MF_LABELS.len()),
        ];
        for (name, cols, labels) in tables {
            assert_eq!(cols.len(), labels, "{name}: label/column count mismatch");
            let mut priorities: Vec<u8> = cols.iter().map(|c| c.priority).collect();
            priorities.sort_unstable();
            let expected: Vec<u8> = (0..u8::try_from(cols.len()).unwrap()).collect();
            assert_eq!(
                priorities, expected,
                "{name}: priorities must be 0..n with no duplicates"
            );
        }
    }

    #[test]
    fn tables_only_gain_columns_as_the_terminal_widens() {
        let tables: [(&str, &[FitCol]); 6] = [
            ("watch", &WATCH_COLS),
            ("positions", &POS_COLS),
            ("orders", &ORD_COLS),
            ("gtt", &GTT_COLS),
            ("holdings", &HOLDINGS_COLS),
            ("mf", &MF_COLS),
        ];
        for (name, cols) in tables {
            for width in 10..200u16 {
                let narrow = columns::fit_columns(table_budget(width), 1, cols);
                let wide = columns::fit_columns(table_budget(width + 1), 1, cols);
                for idx in 0..cols.len() {
                    assert!(
                        !narrow.shows(idx) || wide.shows(idx),
                        "{name}: column {idx} vanishes between {width} and {}",
                        width + 1
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod theme_selection_render_tests {

    use super::super::draw_positions;
    use crate::tui::app::App;
    use crate::tui::data::Position;
    use crate::tui::event::AppEvent;
    use crate::tui::format::contrast_ratio;
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings, Theme};
    use crate::tui::workspace::PaneId;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::style::Color;

    const WIDTH: u16 = 140;
    const HEIGHT: u16 = 8;
    const INNER: std::ops::Range<u16> = 1..WIDTH - 1;

    fn position(symbol: &str, token: u32, pnl: f64, last_price: f64, close_price: f64) -> Position {
        {
            let mut row = crate::tui::data::test_models::position();
            row.tradingsymbol = symbol.into();
            row.exchange = "NSE".into();
            row.instrument_token = token;
            row.product = "CNC".into();
            row.overnight_quantity = 0;
            row.day_buy_quantity = 0;
            row.day_sell_quantity = 0;
            row.quantity = 10;
            row.average_price = 100.0;
            row.last_price = last_price;
            row.close_price = close_price;
            row.pnl = pnl;
            row.realised = 0.0;
            row.multiplier = 1.0;
            row
        }
    }

    fn positions_app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("GAINER", 1, 1_250.00, 120.0, 110.0),
                position("LOSER", 2, -980.50, 80.0, 90.0),
            ];
            row
        });
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.on_event(AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    fn render(app: &App) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
        terminal
            .draw(|frame| {
                crate::tui::ui::paint_root(frame);
                draw_positions(frame, frame.area(), app);
            })
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn row_text(buffer: &Buffer, y: u16) -> String {
        (0..WIDTH).map(|x| buffer[(x, y)].symbol()).collect()
    }

    fn selected_row(buffer: &Buffer) -> u16 {
        let rows: Vec<u16> = (0..HEIGHT)
            .filter(|&y| row_text(buffer, y).contains('▶'))
            .collect();
        assert_eq!(rows.len(), 1, "expected exactly one marked row");
        rows[0]
    }

    fn foregrounds(buffer: &Buffer, y: u16) -> Vec<(Color, String)> {
        let mut seen: Vec<(Color, String)> = Vec::new();
        for x in INNER {
            let cell = &buffer[(x, y)];
            if cell.symbol().trim().is_empty() {
                continue;
            }
            if !seen.iter().any(|(fg, _)| *fg == cell.fg) {
                seen.push((cell.fg, cell.symbol().to_string()));
            }
        }
        seen
    }

    fn assert_selected_row_reads(app: &App, theme: &Theme, symbol: &str, expected: Color) {
        let buffer = render(app);
        let y = selected_row(&buffer);
        let text = row_text(&buffer, y);
        assert!(text.contains(symbol), "marker is not on {symbol}: {text:?}");

        let selection = theme.selection.expect("an RGB preset defines a selection");
        for x in INNER {
            assert_eq!(
                buffer[(x, y)].bg,
                selection,
                "row {y} col {x} lost the selection fill"
            );
        }

        let pnl = if expected == theme.up {
            "+1,250.00"
        } else {
            "-980.50"
        };
        let start = (0..WIDTH - u16::try_from(pnl.chars().count()).unwrap())
            .find(|&x| {
                pnl.chars().enumerate().all(|(i, ch)| {
                    buffer[(x + u16::try_from(i).unwrap(), y)].symbol() == ch.to_string()
                })
            })
            .unwrap_or_else(|| panic!("{pnl} not rendered in {text:?}"));
        assert_eq!(
            buffer[(start, y)].fg,
            expected,
            "the P&L cell lost its semantic colour on the selected row"
        );

        let muted = foregrounds(&buffer, y)
            .into_iter()
            .find(|(fg, _)| *fg == theme.text_muted);
        assert!(
            muted.is_some(),
            "no muted label survived on the selected row: {text:?}"
        );

        let Color::Rgb(br, bg_, bb) = selection else {
            panic!("an RGB preset's selection must be RGB, got {selection:?}");
        };
        for (fg, glyph) in foregrounds(&buffer, y) {
            let Color::Rgb(fr, fg_, fb) = fg else {
                panic!("row {y} painted {glyph:?} in non-RGB {fg:?} under an RGB preset");
            };
            let ratio = contrast_ratio((fr, fg_, fb), (br, bg_, bb));
            assert!(
                ratio >= 4.5,
                "{glyph:?} renders at {ratio:.2}:1 on the selection fill"
            );
        }
    }

    #[test]
    fn a_selected_gain_and_loss_keep_their_colours_and_stay_readable() {
        for name in ["kite", "solarized-light"] {
            let theme = settings::preset(name).expect("preset exists");
            let _guard = settings::theme_guard(theme);
            let mut app = positions_app();

            press(&mut app, KeyCode::Char('3'));
            assert_eq!(app.focused_pane(), PaneId::PortfolioPositions);
            assert_eq!(app.pane_row(PaneId::PortfolioPositions), 0);
            assert_selected_row_reads(&app, &theme, "GAINER", theme.up);

            press(&mut app, KeyCode::Char('j'));
            assert_eq!(app.pane_row(PaneId::PortfolioPositions), 1);
            assert_selected_row_reads(&app, &theme, "LOSER", theme.down);
        }
    }

    #[test]
    fn the_light_preset_paints_a_selected_row_in_its_own_inks() {
        let dark = settings::preset("kite").expect("kite preset");
        let light = settings::preset("solarized-light").expect("solarized-light preset");
        assert_ne!(dark.up, light.up);
        assert_ne!(dark.selection, light.selection);

        let sample = |theme: Theme| {
            let _guard = settings::theme_guard(theme);
            let app = positions_app();
            let buffer = render(&app);
            let y = selected_row(&buffer);
            (
                buffer[(INNER.start, y)].bg,
                foregrounds(&buffer, y)
                    .into_iter()
                    .map(|(fg, _)| fg)
                    .collect::<Vec<_>>(),
            )
        };

        let (dark_bg, dark_fgs) = sample(dark);
        let (light_bg, light_fgs) = sample(light);
        assert_eq!(Some(dark_bg), dark.selection);
        assert_eq!(Some(light_bg), light.selection);

        let luminance = |color: Color| match color {
            Color::Rgb(r, g, b) => crate::tui::format::relative_luminance((r, g, b)),
            other => panic!("expected RGB, got {other:?}"),
        };
        assert!(luminance(light_bg) > luminance(dark_bg));
        let mean = |fgs: Vec<Color>| -> f64 {
            let n = f64::from(u32::try_from(fgs.len()).unwrap());
            fgs.into_iter().map(luminance).sum::<f64>() / n
        };
        assert!(
            mean(light_fgs) < mean(dark_fgs),
            "the light preset must paint darker text than the dark preset"
        );
    }
}

#[cfg(test)]
mod slack_layout_render_tests {

    use super::super::{draw_alerts, draw_holdings, draw_orders, draw_watch};
    use crate::tui::app::App;
    use crate::tui::data::{Alert, Holding, Order};
    use crate::tui::marketwatch::WatchItem;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const WIDTHS: [u16; 3] = [80, 120, 200];

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

    fn holding(symbol: &str) -> Holding {
        {
            let mut row = crate::tui::data::test_models::holding();
            row.tradingsymbol = symbol.into();
            row.exchange = "NSE".into();
            row.instrument_token = 1;
            row.quantity = 15;
            row.t1_quantity = 0;
            row.collateral_quantity = 0;
            row.mtf = None;
            row.average_price = 6_077.93;
            row.last_price = 30_000.0;
            row.close_price = 25_000.0;
            row.day_change = None;
            row.discrepancy = false;
            row
        }
    }

    fn order(exchange: &str, symbol: &str) -> Order {
        {
            let mut row = crate::tui::data::test_models::order();
            row.order_id = "251009000000001".into();
            row.order_timestamp = "2026-09-09T10:15:00".parse().expect("timestamp");
            row.status = "COMPLETE".into();
            row.status_message = None;
            row.exchange = exchange.into();
            row.tradingsymbol = symbol.into();
            row.instrument_token = 1;
            row.transaction_type = "BUY".into();
            row.order_type = "LIMIT".into();
            row.product = "CNC".into();
            row.validity = "DAY".into();
            row.validity_ttl = None;
            row.variety = "regular".into();
            row.tags = Vec::new();
            row.price = 6_100.0;
            row.trigger_price = 0.0;
            row.average_price = 6_077.93;
            row.quantity = f64::from(15);
            row.filled_quantity = f64::from(15);
            row.pending_quantity = f64::from(0);
            row
        }
    }

    fn alert(name: &str, exchange: &str, symbol: &str) -> Alert {
        {
            let mut row = crate::tui::data::test_models::alert();
            row.alert_type =
                serde_json::from_value(serde_json::json!("simple")).expect("alert_type");
            row.uuid = name.into();
            row.name = name.into();
            row.status = serde_json::from_value(serde_json::json!("enabled")).expect("status");
            row.lhs_attribute = "LastTradedPrice".into();
            row.lhs_exchange = exchange.into();
            row.lhs_tradingsymbol = symbol.into();
            row.operator = ">=".into();
            row.rhs_type = "constant".into();
            row.rhs_constant = 27_000.0;
            row.alert_count = 3;
            row.created_at = "2025-02-17T08:24:10".parse().expect("timestamp");
            row
        }
    }

    fn watch_item(token: u32, exchange: &str, symbol: &str) -> WatchItem {
        WatchItem {
            exchange: exchange.into(),
            tradingsymbol: symbol.into(),
            instrument_token: token,
            segment: exchange.into(),
            ..WatchItem::default()
        }
    }

    fn loaded_app() -> App {
        let mut app = test_app();
        app.holdings = Some(vec![holding("DIVISLAB"), holding("BAJAJFINSV")]);
        app.orders = Some(vec![
            order("NSE", "DIVISLAB"),
            order("NFO", "NIFTY25DEC26000CE"),
        ]);
        app.set_alerts(vec![
            alert("NIFTY upper", "INDICES", "NIFTY NEXT 50"),
            alert("gold dip", "MCX", "GOLDM25DECFUT"),
        ]);
        app.watch.test_push(watch_item(408_065, "NSE", "INFY"));
        app.watch
            .test_push(watch_item(408_066, "INDICES", "NIFTY NEXT 50"));
        app
    }

    fn panel_rows(
        width: u16,
        app: &App,
        draw: fn(&mut ratatui::Frame, ratatui::layout::Rect, &App),
    ) -> Vec<String> {
        let height = 6;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (1..4)
            .map(|y| {
                let row: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
                let inner = row.strip_prefix('│').unwrap_or(&row);
                let end = inner.find('│').unwrap_or(inner.len());
                inner[..end].trim_end().to_string()
            })
            .collect()
    }

    fn assert_rows(
        width: u16,
        app: &App,
        draw: fn(&mut ratatui::Frame, ratatui::layout::Rect, &App),
        want: [&str; 3],
    ) {
        let got = panel_rows(width, app, draw);
        for (idx, want) in want.iter().enumerate() {
            assert_eq!(&got[idx], want, "width {width}, row {idx}");
        }
    }

    #[test]
    fn holdings_spreads_as_the_terminal_widens() {
        let app = loaded_app();
        assert_rows(
            80,
            &app,
            draw_holdings,
            [
                "  SYMBOL          QTY         LTP         VALUE          P&L   P&L% ▼     DAY%",
                "▶ NSE:DIVISL…      15   30,000.00     ₹4,50,000 +3,58,831.05 +393.59%  +20.00%",
                "  NSE:BAJAJF…      15   30,000.00     ₹4,50,000 +3,58,831.05 +393.59%  +20.00%",
            ],
        );
        let spread = [
            "   SYMBOL                         QTY           AVG           LTP           VALUE            P&L     P&L% ▼       DAY%",
            " ▶ NSE:DIVISLAB                    15      6,077.93     30,000.00       ₹4,50,000   +3,58,831.05   +393.59%    +20.00%",
            "   NSE:BAJAJFINSV                  15      6,077.93     30,000.00       ₹4,50,000   +3,58,831.05   +393.59%    +20.00%",
        ];
        assert_rows(120, &app, draw_holdings, spread);
        assert_rows(200, &app, draw_holdings, spread);
    }

    #[test]
    fn orders_spreads_as_the_terminal_widens() {
        let app = loaded_app();
        assert_rows(
            80,
            &app,
            draw_orders,
            [
                "  SEL TIME ▼   SYMBOL     SIDE KIND            QTY       PRICE STATUS",
                "▶ [ ] 10:15:00 NSE:DIVIS… BUY                15/15    6,077.93 COMPLETE",
                "  [ ] 10:15:00 NFO:NIFTY… BUY                15/15    6,077.93 COMPLETE",
            ],
        );
        assert_rows(
            120,
            &app,
            draw_orders,
            [
                "  SEL TIME ▼   SYMBOL                      SIDE TYPE   KIND      PROD        QTY       PRICE   TRIGGER STATUS",
                "▶ [ ] 10:15:00 NSE:DIVISLAB                BUY  LIMIT            CNC       15/15    6,077.93         . COMPLETE",
                "  [ ] 10:15:00 NFO:NIFTY25DEC26000CE       BUY  LIMIT            CNC       15/15    6,077.93         . COMPLETE",
            ],
        );
        assert_rows(
            200,
            &app,
            draw_orders,
            [
                "   SEL   TIME ▼     SYMBOL                         SIDE   TYPE     KIND        PROD          QTY         PRICE     TRIGGER   STATUS",
                " ▶ [ ]   10:15:00   NSE:DIVISLAB                   BUY    LIMIT                CNC         15/15      6,077.93           .   COMPLETE",
                "   [ ]   10:15:00   NFO:NIFTY25DEC26000CE          BUY    LIMIT                CNC         15/15      6,077.93           .   COMPLETE",
            ],
        );
    }

    #[test]
    fn alerts_stop_clipping_the_instrument_they_watch() {
        let app = loaded_app();
        assert_rows(
            80,
            &app,
            draw_alerts,
            [
                "  NAME ▲        SYMBOL                CONDITION           STATUS     TRIGGERED",
                "▶ gold dip      MCX:GOLDM25DECFUT     Last price >= 27000 enabled            3",
                "  NIFTY upper   INDICES:NIFTY NEXT 50 Last price >= 27000 enabled            3",
            ],
        );
        assert_rows(
            120,
            &app,
            draw_alerts,
            [
                "  NAME ▲                    SYMBOL                  CONDITION             STATUS       TRIGGERED   TYPE     CREATED",
                "▶ gold dip                  MCX:GOLDM25DECFUT       Last price >= 27000   enabled              3   SIMPLE   2025-02-17",
                "  NIFTY upper               INDICES:NIFTY NEXT 50   Last price >= 27000   enabled              3   SIMPLE   2025-02-17",
            ],
        );
        assert_rows(
            200,
            &app,
            draw_alerts,
            [
                "   NAME ▲                                 SYMBOL                  CONDITION             STATUS       TRIGGERED   TYPE     CREATED",
                " ▶ gold dip                               MCX:GOLDM25DECFUT       Last price >= 27000   enabled              3   SIMPLE   2025-02-17",
                "   NIFTY upper                            INDICES:NIFTY NEXT 50   Last price >= 27000   enabled              3   SIMPLE   2025-02-17",
            ],
        );
    }

    #[test]
    fn watch_spreads_once_its_pane_can_pay_for_it() {
        let app = loaded_app();
        assert_rows(
            80,
            &app,
            draw_watch,
            [
                "  SYMBOL                LTP     CHG%        CHG       HIGH        LOW      VOL",
                "▶ NSE:INFY                .        .          .          .          .        .",
                "  INDICES:NIFT…           .        .          .          .          .        .",
            ],
        );
        assert_rows(
            120,
            &app,
            draw_watch,
            [
                "   SYMBOL                             LTP       CHG%          CHG         HIGH          LOW        VOL",
                " ▶ NSE:INFY                             .          .            .            .            .          .",
                "   INDICES:NIFTY NEXT 50                .          .            .            .            .          .",
            ],
        );
        assert_rows(
            200,
            &app,
            draw_watch,
            [
                "   SYMBOL                             LTP       CHG%          CHG         HIGH          LOW        VOL",
                " ▶ NSE:INFY                             .          .            .            .            .          .",
                "   INDICES:NIFTY NEXT 50                .          .            .            .            .          .",
            ],
        );
    }

    #[test]
    fn no_text_cell_is_clipped_at_200_columns() {
        let app = loaded_app();
        for draw in [
            draw_holdings as fn(&mut ratatui::Frame, ratatui::layout::Rect, &App),
            draw_orders,
            draw_alerts,
            draw_watch,
        ] {
            for (idx, row) in panel_rows(200, &app, draw).iter().enumerate() {
                assert!(!row.contains('…'), "row {idx} clipped a cell: {row:?}");
            }
        }
        for (draw, symbol) in [
            (
                draw_orders as fn(&mut ratatui::Frame, ratatui::layout::Rect, &App),
                "NFO:NIFTY25DEC26000CE",
            ),
            (draw_alerts, "INDICES:NIFTY NEXT 50"),
            (draw_watch, "INDICES:NIFTY NEXT 50"),
            (draw_holdings, "NSE:BAJAJFINSV"),
        ] {
            let rows = panel_rows(200, &app, draw);
            assert!(
                rows.iter().any(|row| row.contains(symbol)),
                "{symbol} missing at 200 columns: {rows:?}"
            );
        }
    }

    #[test]
    fn the_selection_marker_leaves_the_border_only_when_there_is_room() {
        let app = loaded_app();
        let draws = [
            draw_holdings as fn(&mut ratatui::Frame, ratatui::layout::Rect, &App),
            draw_orders,
            draw_alerts,
            draw_watch,
        ];
        for draw in draws {
            for width in WIDTHS {
                let marker = &panel_rows(width, &app, draw)[1];
                let inset = marker.starts_with(" ▶ ");
                match width {
                    80 => assert!(!inset, "80 columns spent a cell on the inset: {marker:?}"),
                    200 => assert!(
                        inset,
                        "200 columns left the marker on the border: {marker:?}"
                    ),
                    _ => {}
                }
                assert!(
                    inset || marker.starts_with('▶'),
                    "width {width}: no selection marker in {marker:?}"
                );
            }
        }
    }
}
