mod alert_history;
mod alerts;
mod auctions;
mod bids;
mod book_state;
mod corporate_actions;
mod depth;
mod gsec;
mod gtt;
mod headers;
mod holdings;
mod inline_chart;
mod ipo;
mod mf;
mod orders;
mod positions;
mod quote;
mod quote_price;
mod quote_range;
mod table;
mod watch;
mod watch_detail;

use crate::tui::app::{App, ChartTarget};
use crate::tui::pane_state::ChartPane;
use crate::tui::ui::pane::{BidsInput, BookList, InlineChart, InlineChartInput, PaneHits};
use crate::tui::ui::{self, chart};
use crate::tui::workspace::PaneId;
use alerts::draw_alerts_pane;
use auctions::draw_auctions;
use bids::draw_bids_pane;
use bids::{bid_kind_cell, bid_status_cell, bid_symbol_cell, fit_amount, offer_price};
use book_state::{state_body, state_panel};
use corporate_actions::draw_corporate_action_table;
use gsec::draw_gsec_table;
use gtt::draw_gtts_pane;
use holdings::draw_holdings_pane;
use inline_chart::chart_request;
use ipo::{draw_ipo_applications, draw_ipo_table};
use mf::draw_mf_pane;
use orders::draw_orders_pane;
use positions::draw_positions_pane;
use ratatui::Frame;
use ratatui::layout::Rect;
pub use watch::draw_watch_list;
pub use watch_detail::draw_depth_leaf;

pub fn draw_orders_leaf(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let display = app.orders_order_in(pane);
    let Some(instance) = app.orders_pane_of(pane) else {
        return;
    };
    let list = BookList {
        selected: app.pane_row(pane),
        sort: instance.list.sort(),
        display: &display,
        viewport: instance.list.viewport(),
        focus: focused,
    };
    let target = chart_target(app, pane);
    draw_orders_pane(
        frame,
        area,
        app,
        &list,
        &inline_chart(app, pane, target.as_ref()),
        &PaneHits::new(&app.hit, pane),
    );
}

pub fn draw_positions_leaf(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let display = app.positions_order_in(pane);
    let Some(instance) = app.positions_pane_of(pane) else {
        return;
    };
    let list = BookList {
        selected: app.pane_row(pane),
        sort: instance.sort(),
        display: &display,
        viewport: instance.viewport(),
        focus: focused,
    };
    let target = chart_target(app, pane);
    draw_positions_pane(
        frame,
        area,
        app,
        &list,
        &inline_chart(app, pane, target.as_ref()),
        &PaneHits::new(&app.hit, pane),
    );
}

pub fn draw_holdings_leaf(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let display = app.holdings_order_in(pane);
    let Some(instance) = app.holdings_pane_of(pane) else {
        return;
    };
    let list = BookList {
        selected: app.pane_row(pane),
        sort: instance.sort(),
        display: &display,
        viewport: instance.viewport(),
        focus: focused,
    };
    let target = chart_target(app, pane);
    draw_holdings_pane(
        frame,
        area,
        app,
        &list,
        &inline_chart(app, pane, target.as_ref()),
        &PaneHits::new(&app.hit, pane),
    );
}

pub fn draw_mf_leaf(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let display = app.mf_display_order();
    let list = BookList {
        selected: app.mf_display_row(&display),
        sort: app.books.mf.sort(),
        display: &display,
        viewport: app.books.mf.viewport(),
        focus: focused,
    };
    draw_mf_pane(frame, area, app, &list, &PaneHits::new(&app.hit, pane));
}

pub fn draw_gtt_leaf(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let display = app.gtts_display_order();
    let list = BookList {
        selected: app.gtts_display_row(&display),
        sort: app.books.gtt.sort(),
        display: &display,
        viewport: app.books.gtt.viewport(),
        focus: focused,
    };
    draw_gtts_pane(frame, area, app, &list, &PaneHits::new(&app.hit, pane));
}

pub fn draw_alerts_leaf(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let display = app.alerts_display_order();
    let list = BookList {
        selected: app.alerts_display_row(&display),
        sort: app.books.alerts.sort(),
        display: &display,
        viewport: app.books.alerts.viewport(),
        focus: focused,
    };
    draw_alerts_pane(frame, area, app, &list, &PaneHits::new(&app.hit, pane));
}

pub fn draw_bids_leaf(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let input = BidsInput {
        section: app.bids_pane.section(),
        application_focus: app.bids_pane.application_focus(),
        selected: app.pane_row(pane),
        viewport: app.bids_pane.viewport(),
        focused,
    };
    draw_bids_pane(frame, area, app, &input, &PaneHits::new(&app.hit, pane));
}

fn chart_target(app: &App, pane: PaneId) -> Option<ChartTarget> {
    (app.chart_panel_pane() == Some(pane))
        .then(|| app.selected_chart_target())
        .flatten()
}

fn inline_chart<'a>(
    app: &'a App,
    pane: PaneId,
    target: Option<&'a ChartTarget>,
) -> InlineChartInput<'a> {
    let open = if app.chart_panel_pane() == Some(pane) {
        InlineChart::Open {
            maximized: app.chart_maximized,
        }
    } else {
        InlineChart::Closed
    };
    InlineChartInput {
        open,
        target,
        chart: &app.chart_pane,
    }
}

pub fn draw_chart_leaf(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    chart: &ChartPane,
    source: &str,
    focused: bool,
    hits: &PaneHits<'_>,
) {
    let target = chart.target();
    let title = match target {
        Some(target) => format!("Chart | {}", target.label()),
        None => "Chart".to_string(),
    };
    let Some(target) = target.filter(|target| target.instrument_token != 0) else {
        state_body(
            frame,
            area,
            ui::focus_panel(title, focused),
            ui::MessageTone::Normal,
            "No instrument to chart",
            None,
            Some(source),
        );
        return;
    };
    let block = ui::focus_panel(format!("{title} | {}", chart.interval().label()), focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let token = target.instrument_token;
    let series = chart.indicator_series(token, app.candle_store());
    if let Some(hit) = chart::draw_chart(frame, inner, chart_request(app, chart, token, &series)) {
        hits.push_chart(chart.id(), hit);
    }
}

#[cfg(test)]
mod tests {
    use super::{Cell, Constraint, Row, Table, col, columns, fit_header, right};

    #[test]
    fn numeric_header_aligns_with_numeric_cell_content() {
        use ratatui::Terminal;
        use ratatui::backend::TestBackend;

        let fit = columns::fit_columns(100, 0, &[col(10, 0), col(6, 1)]);
        let headers = fit_header(&fit, &[("SYMBOL", false), ("QTY", true)]);
        let rows = vec![Row::new(vec![Cell::from("INFY"), right("42".to_string())])];
        let table = Table::new(rows, [Constraint::Length(10), Constraint::Length(6)])
            .header(headers)
            .column_spacing(0);

        let mut terminal = Terminal::new(TestBackend::new(16, 3)).unwrap();
        terminal
            .draw(|frame| frame.render_widget(table, frame.area()))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();

        let last_nonblank = |y: u16| -> u16 {
            (0..16)
                .rev()
                .find(|&x| buffer[(x, y)].symbol() != " ")
                .expect("row has content")
        };
        assert_eq!(last_nonblank(0), last_nonblank(1));
    }
}

#[cfg(test)]
mod pane_focus_tests {

    use super::{draw_bids, draw_watch, draw_watch_list};
    use crate::tui::app::App;
    use crate::tui::bids::{Application, Instrument, Section};
    use crate::tui::format;
    use crate::tui::marketwatch::{State, WatchItem};
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings, Theme};
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;
    use kiteconnect::ticker::Tick;
    use ratatui::Frame;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    const SIZES: [(u16, u16); 2] = [(80, 24), (120, 35)];

    fn app(items: Vec<WatchItem>) -> App {
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

    fn watch_app() -> App {
        let mut app = app(vec![
            WatchItem {
                exchange: "NSE".into(),
                tradingsymbol: "INFY".into(),
                instrument_token: 408_065,
                ..WatchItem::default()
            },
            WatchItem {
                exchange: "NSE".into(),
                tradingsymbol: "TCS".into(),
                instrument_token: 2_953_217,
                ..WatchItem::default()
            },
        ]);
        app.set_pane(PaneId::TradingWatch);
        app.ticks.insert(408_065, {
            let mut value = Tick::default();
            value.instrument_token = 408_065;
            value.last_price = Some(1_420.5);
            value.close = Some(1_400.0);
            value
        });
        app
    }

    fn bids_app() -> App {
        let mut app = app(Vec::new());
        app.set_pane(PaneId::InvestBids);
        app.bids_pane.set_section(Section::Ipo);
        app.bids.ipo = Some(vec![Instrument {
            symbol: "ACME".into(),
            name: "Acme Limited".into(),
            status: "ongoing".into(),
            end_at: "2026-09-09 17:00:00".into(),
            min_qty: 120,
            ..Default::default()
        }]);
        app.bids.applications = Some(vec![Application {
            symbol: "ACME".into(),
            status: "allotted".into(),
            created_at: "2026-09-05 11:00:00".into(),
            ..Default::default()
        }]);
        app
    }

    fn buffer(app: &App, width: u16, height: u16, draw: fn(&mut Frame, Rect, &App)) -> Buffer {
        let mut terminal = ratatui::Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw(frame, frame.area(), app))
            .unwrap();
        terminal.backend().buffer().clone()
    }

    fn rows(buffer: &Buffer) -> Vec<String> {
        let area = buffer.area;
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect()
    }

    fn find_all(rows: &[String], needle: &str) -> Vec<(u16, u16)> {
        rows.iter()
            .enumerate()
            .filter_map(|(y, row)| {
                row.find(needle).map(|byte| {
                    (
                        u16::try_from(y).unwrap(),
                        u16::try_from(row[..byte].chars().count()).unwrap(),
                    )
                })
            })
            .collect()
    }

    fn find_once(rows: &[String], needle: &str) -> (u16, u16) {
        let found = find_all(rows, needle);
        assert_eq!(
            found.len(),
            1,
            "{needle:?} is not on exactly one row of\n{}",
            rows.join("\n")
        );
        found[0]
    }

    fn one_marker(rows: &[String], at: &str) {
        let markers: usize = rows.iter().map(|row| row.matches("> ").count()).sum();
        assert_eq!(markers, 1, "{at}: {rows:#?}");
    }

    #[test]
    fn the_focused_watch_list_carries_the_one_marker_and_the_accent_border() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        for (width, height) in SIZES {
            let app = watch_app();
            let focused = buffer(&app, width, height, draw_watch);
            let focused_rows = rows(&focused);
            find_once(&focused_rows, "> Watchlist · ");
            one_marker(&focused_rows, &format!("watch list at {width}x{height}"));
            assert_eq!(focused[(0, 0)].fg, format::accent(), "focused list border");

            let unfocused = buffer(&app, width, height, |frame, area, app| {
                draw_watch_list(frame, area, app, PaneId::TradingWatch, false);
            });
            let unfocused_rows = rows(&unfocused);
            find_once(&unfocused_rows, "   Watchlist · ");
            assert_eq!(
                unfocused[(0, 0)].fg,
                format::border_style().fg.expect("a border colour"),
                "unfocused list border"
            );
        }
    }

    #[test]
    fn the_bids_instruments_table_owns_the_view_until_a_moves_focus() {
        for (width, height) in SIZES {
            let mut app = bids_app();
            let instruments = rows(&buffer(&app, width, height, draw_bids));
            find_once(&instruments, "> IPO · 1 open");
            one_marker(&instruments, &format!("bids table at {width}x{height}"));
            if height >= 21 {
                find_once(&instruments, "   IPO · applications ");
            }

            app.bids_pane.focus_applications();
            let applications = rows(&buffer(&app, width, height, draw_bids));
            find_once(&applications, "> IPO · applications ");
            one_marker(&applications, &format!("bids pane at {width}x{height}"));
        }
    }

    #[test]
    fn the_bids_application_title_does_not_move_with_focus() {
        let mut app = bids_app();
        let unfocused = rows(&buffer(&app, 120, 35, draw_bids));
        let (_, blank_x) = find_once(&unfocused, "   IPO · applications ");
        app.bids_pane.focus_applications();
        let focused = rows(&buffer(&app, 120, 35, draw_bids));
        let (_, marked_x) = find_once(&focused, " > IPO · applications ");
        assert_eq!(blank_x, marked_x, "the title moved with focus");
    }

    #[test]
    fn the_focused_bids_panel_takes_the_accent_border() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let inactive = format::border_style()
            .fg
            .expect("the border style names a colour");
        let mut app = bids_app();
        let unfocused = buffer(&app, 120, 35, draw_bids);
        let (table_y, _) = find_once(&rows(&unfocused), "> IPO · 1 open");
        let (pane_y, _) = find_once(&rows(&unfocused), "   IPO · applications ");
        assert_eq!(unfocused[(0, table_y)].fg, format::accent());
        assert_eq!(unfocused[(0, pane_y)].fg, inactive);

        app.bids_pane.focus_applications();
        let focused = buffer(&app, 120, 35, draw_bids);
        let (focused_y, _) = find_once(&rows(&focused), "> IPO · applications ");
        assert_eq!(focused[(0, focused_y)].fg, format::accent());
    }
}

#[cfg(test)]
mod pane_input_tests {

    use super::{
        BookList, InlineChart, InlineChartInput, PaneHits, WatchInput, draw_positions,
        draw_positions_pane, draw_watch, draw_watch_pane,
    };
    use crate::tui::app::App;
    use crate::tui::data::Position;
    use crate::tui::event::AppEvent;
    use crate::tui::marketwatch::{State, WatchItem};
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings, Theme};
    use crate::tui::ui;
    use crate::tui::workspace::PaneId;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use kiteconnect::KiteConnect;
    use kiteconnect::ticker::{Tick, TickerEvent};
    use ratatui::Frame;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

    const SIZES: [(u16, u16); 2] = [(80, 24), (120, 35)];

    fn app(watch: Vec<WatchItem>) -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        App::new(
            client,
            events_tx,
            State::test_loaded(watch),
            Pad::default(),
            false,
            &Settings::default(),
        )
    }

    fn position(tradingsymbol: &str, token: u32, pnl: f64, last_price: f64) -> Position {
        {
            let mut row = crate::tui::data::test_models::position();
            row.tradingsymbol = tradingsymbol.into();
            row.exchange = "NSE".into();
            row.instrument_token = token;
            row.product = "CNC".into();
            row.overnight_quantity = 0;
            row.day_buy_quantity = 0;
            row.day_sell_quantity = 0;
            row.quantity = 25;
            row.average_price = 100.0;
            row.last_price = last_price;
            row.close_price = 110.0;
            row.pnl = pnl;
            row.realised = 0.0;
            row.multiplier = 1.0;
            row
        }
    }

    fn positions_app() -> App {
        let mut app = app(Vec::new());
        app.set_pane(PaneId::PortfolioPositions);
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("GAINER", 1, 1_250.00, 120.0),
                position("LOSER", 2, -980.50, 80.0),
            ];
            row
        });
        tick(&mut app, 1, 120.0);
        tick(&mut app, 2, 80.0);
        app
    }

    fn watch_app() -> App {
        let item = |tradingsymbol: &str, token: u32| WatchItem {
            tradingsymbol: tradingsymbol.into(),
            exchange: "NSE".into(),
            instrument_token: token,
            ..Default::default()
        };
        let mut app = app(vec![item("INFY", 408_065), item("TCS", 2_953_217)]);
        app.circuits.insert(408_065, None);
        app.circuits.insert(2_953_217, None);
        app.set_pane(PaneId::TradingWatch);
        tick(&mut app, 408_065, 1_500.25);
        tick(&mut app, 2_953_217, 3_210.75);
        app
    }

    fn tick(app: &mut App, token: u32, last_price: f64) {
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(vec![{
            let mut value = Tick::default();
            value.instrument_token = token;
            value.last_price = Some(last_price);
            value.open = Some(last_price - 5.0);
            value.high = Some(last_price + 3.0);
            value.low = Some(last_price - 8.0);
            value.close = Some(last_price - 2.0);
            value.volume = Some(1_000_000);
            value
        }])));
    }

    fn press(app: &mut App, code: KeyCode) {
        app.on_event(AppEvent::Key(KeyEvent::new(code, KeyModifiers::NONE)));
    }

    fn frame_of(
        app: &App,
        (width, height): (u16, u16),
        draw: impl FnOnce(&mut Frame, Rect, &App),
    ) -> (Buffer, String) {
        app.hit.borrow_mut().invalidate();
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                ui::paint_root(frame);
                draw(frame, frame.area(), app);
            })
            .unwrap();
        let hits = format!("{:?}", app.hit.borrow().regions);
        (terminal.backend().buffer().clone(), hits)
    }

    fn positions_pane(
        frame: &mut Frame,
        area: Rect,
        app: &App,
        selected: usize,
        open: InlineChart,
    ) {
        let display = app.positions_order_in(PaneId::PortfolioPositions);
        let target = matches!(open, InlineChart::Open { .. })
            .then(|| app.selected_chart_target())
            .flatten();
        draw_positions_pane(
            frame,
            area,
            app,
            &BookList {
                focus: true,
                selected,
                sort: app.books.portfolio_positions.sort(),
                display: &display,
                viewport: app.books.portfolio_positions.viewport(),
            },
            &InlineChartInput {
                open,
                target: target.as_ref(),
                chart: &app.chart_pane,
            },
            &PaneHits::new(&app.hit, PaneId::PortfolioPositions),
        );
    }

    fn watch_pane(frame: &mut Frame, area: Rect, app: &App, selected: usize) {
        draw_watch_pane(
            frame,
            area,
            app,
            &WatchInput {
                rows: app.watch_items(),
                watch: app.watch_view(),
                selected,
                viewport: app.watch_pane.viewport(),
                focused: true,
            },
            &PaneHits::new(&app.hit, PaneId::TradingWatch),
        );
    }

    #[test]
    fn positions_renders_identically_through_its_pane_inputs() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = positions_app();
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.pane_row(PaneId::PortfolioPositions), 1);

        for size in SIZES {
            let (through_view, view_hits) = frame_of(&app, size, draw_positions);
            let (through_pane, pane_hits) = frame_of(&app, size, |frame, area, app| {
                positions_pane(frame, area, app, 1, InlineChart::Closed);
            });
            assert_eq!(through_pane, through_view, "positions cells at {size:?}");
            assert_eq!(pane_hits, view_hits, "positions targets at {size:?}");
        }
    }

    #[test]
    fn watch_renders_identically_through_its_pane_inputs() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = watch_app();
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(app.pane_row(PaneId::TradingWatch), 1);

        for size in SIZES {
            let (through_view, view_hits) = frame_of(&app, size, draw_watch);
            let (through_pane, pane_hits) = frame_of(&app, size, |frame, area, app| {
                watch_pane(frame, area, app, 1);
            });
            assert_eq!(through_pane, through_view, "watch cells at {size:?}");
            assert_eq!(pane_hits, view_hits, "watch targets at {size:?}");
        }
    }

    #[test]
    fn the_book_panes_select_the_row_they_are_handed() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = positions_app();
        let (row_zero, _) = frame_of(&app, SIZES[1], |frame, area, app| {
            positions_pane(frame, area, app, 0, InlineChart::Closed);
        });
        let (row_one, _) = frame_of(&app, SIZES[1], |frame, area, app| {
            positions_pane(frame, area, app, 1, InlineChart::Closed);
        });
        assert_ne!(row_one, row_zero, "the marked row did not move");

        assert_eq!(frame_of(&app, SIZES[1], draw_positions).0, row_zero);
        press(&mut app, KeyCode::Char('j'));
        assert_eq!(frame_of(&app, SIZES[1], draw_positions).0, row_one);
    }

    #[test]
    fn the_positions_pane_takes_its_chart_from_its_input() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = positions_app();

        let (closed_by_app, _) = frame_of(&app, SIZES[1], draw_positions);
        let (closed_by_input, _) = frame_of(&app, SIZES[1], |frame, area, app| {
            positions_pane(frame, area, app, 0, InlineChart::Closed);
        });
        assert_eq!(closed_by_input, closed_by_app);

        app.open_chart_panel_for_test(PaneId::PortfolioPositions);
        let (split, _) = frame_of(&app, SIZES[1], draw_positions);
        assert_ne!(closed_by_input, split, "the input's Closed drew a chart");
    }

    #[test]
    fn the_chart_panel_titles_the_interval_of_the_instance_it_is_handed() {
        use crate::tui::data::ChartInterval;
        use crate::tui::indicators::ChartIndicator;
        use crate::tui::pane_state::{ChartBinding, ChartId, ChartPane};
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let app = watch_app();
        let target = app
            .link_target(PaneId::TradingWatch)
            .expect("the top watch row charts");
        let titled = |interval: ChartInterval| -> String {
            let chart = ChartPane::new(
                ChartId::new(7),
                ChartBinding::SelectionOf(PaneId::TradingWatch),
                interval,
                true,
                ChartIndicator::None,
            );
            let (buffer, _) = frame_of(&app, SIZES[1], |frame, area, app| {
                super::draw_chart_panel(
                    frame,
                    area,
                    app,
                    Some(&target),
                    &chart,
                    &PaneHits::new(&app.hit, PaneId::TradingWatch),
                );
            });
            (0..buffer.area.width)
                .map(|x| buffer[(x, 0)].symbol())
                .collect()
        };
        assert!(
            titled(ChartInterval::Day).contains("1D"),
            "no 1D in the title"
        );
        assert!(
            titled(ChartInterval::M5).contains("5m"),
            "no 5m in the title"
        );
    }
}

#[cfg(test)]
mod test_support {
    use super::*;
    pub(super) use crate::tui::ui::columns::{self, col, fit_header, table_budget};
    pub(super) use crate::tui::ui::pane::{PlainList, WatchInput};
    pub(super) use alerts::ALERT_LABELS;
    pub(super) use gtt::{GTT_COLS, GTT_LABELS};
    pub(super) use headers::holdings_sort_targets;
    pub(super) use holdings::{HOLDINGS_COLS, HOLDINGS_LABELS};
    pub(super) use inline_chart::draw_chart_panel;
    pub(super) use mf::{MF_COLS, MF_LABELS};
    pub(super) use orders::{ORD_COLS, ORD_LABELS};
    pub(super) use positions::{POS_COLS, POS_LABELS, PositionsSummary, positions_summary_spans};
    pub(super) use ratatui::layout::Constraint;
    pub(super) use ratatui::widgets::{Cell, Row, Table};
    pub(super) use table::right;
    pub(super) use watch::{WATCH_COLS, WATCH_LABELS, draw_watch_pane};
    pub(super) fn draw_positions(frame: &mut Frame, area: Rect, app: &App) {
        draw_positions_leaf(frame, area, app, PaneId::PortfolioPositions, true);
    }

    pub(super) fn draw_orders(frame: &mut Frame, area: Rect, app: &App) {
        draw_orders_leaf(frame, area, app, PaneId::OrdersBook, true);
    }

    pub(super) fn draw_holdings(frame: &mut Frame, area: Rect, app: &App) {
        draw_holdings_leaf(frame, area, app, PaneId::PortfolioHoldings, true);
    }

    pub(super) fn draw_gtts(frame: &mut Frame, area: Rect, app: &App) {
        draw_gtt_leaf(frame, area, app, PaneId::OrdersGtt, true);
    }

    pub(super) fn draw_mf(frame: &mut Frame, area: Rect, app: &App) {
        draw_mf_leaf(frame, area, app, PaneId::PortfolioMf, true);
    }

    pub(super) fn draw_alerts(frame: &mut Frame, area: Rect, app: &App) {
        draw_alerts_leaf(frame, area, app, PaneId::OrdersAlerts, true);
    }

    pub(super) fn draw_bids(frame: &mut Frame, area: Rect, app: &App) {
        draw_bids_leaf(frame, area, app, PaneId::InvestBids, true);
    }

    pub(super) fn draw_watch(frame: &mut Frame, area: Rect, app: &App) {
        draw_watch_list(frame, area, app, PaneId::TradingWatch, true);
    }
}
#[cfg(test)]
use test_support::*;
