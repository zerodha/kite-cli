mod chain;
mod chart;
mod columns;
mod dashboard;
mod forms;
mod header;
mod help;
mod hints;
pub(crate) mod hits;
mod modals;
mod pane;
mod panels;
mod text;
mod toast;
mod views;

use crate::tui::app::App;
pub use crate::tui::form::{ControlIntent, ModalField};
use crate::tui::format;
use crate::tui::settings;
use crate::tui::workspace::{self, PaneId, Preset};
pub use chart::ChartHit;
use header::draw_header;
use hints::{HintGroup, act, always, draw_hints, fit_hints, hint_key, nav};
pub use hits::{ChainBand, HitAction, HitList, HitMap, HitRegion, HitText, Surface};
pub use panels::{
    centered, focus_marker, focus_panel, focus_title, panel, push_divider, table_row_hit,
};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use std::cell::RefCell;
pub use text::{MessageTone, state_message};
use toast::{draw_toast, toast_display};

pub fn paint_root(frame: &mut Frame) {
    let theme = settings::theme();
    if theme.background.is_none() && theme.text.is_none() {
        return;
    }
    let style = Style {
        fg: theme.text,
        bg: theme.background,
        ..Style::default()
    };
    frame.render_widget(Block::default().style(style), frame.area());
}

pub(crate) fn bar_style() -> Style {
    let theme = settings::theme();
    Style {
        fg: theme.text,
        bg: theme.surface,
        ..Style::default()
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    {
        let mut hit = app.hit.borrow_mut();
        hit.invalidate();
        hit.surface = Some(Surface::of(app));
    }

    paint_root(frame);

    let toast = toast_display(app, frame.area().width);
    let toast_rows = toast
        .as_ref()
        .map_or(0, |display| {
            u16::try_from(display.rows.len()).unwrap_or(u16::MAX)
        })
        .min(frame.area().height.saturating_sub(2));

    let [header, body, toast_line, hints] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(toast_rows),
        Constraint::Length(1),
    ])
    .areas(frame.area());

    draw_header(frame, header, app);
    draw_workspace(frame, body, app);
    draw_toast(frame, toast_line, toast.as_ref());
    draw_hints(frame, hints, app);

    for modal in &app.modals {
        let mut hit = app.hit.borrow_mut();
        hit.regions.clear();
        hit.charts.clear();
        drop(hit);
        modals::draw_modal(frame, modal, app);
    }
    app.hit.borrow_mut().record_row_identities(app);
}

fn draw_workspace(frame: &mut Frame, body: Rect, app: &App) {
    let preset = app.workspace();
    let focus = app.focused_pane();

    let indices: RefCell<Vec<Line<'static>>> = RefCell::new(Vec::new());
    let index_rows = |outer: u16| {
        let lines = dashboard::index_lines(app, outer.saturating_sub(2));
        let rows = lines.len();
        *indices.borrow_mut() = lines;
        rows
    };
    let content = workspace::OverviewContent {
        positions: app.positions.as_ref().map_or(0, |p| p.net.len()),
        index_rows: &index_rows,
    };
    let resolved = workspace::resolve(
        body,
        &workspace::Request {
            preset,
            focused: focus,
            maximized: app.workspaces.maximized(preset),
            ratios: &app.splits,
            overview: (preset == Preset::Overview).then_some(&content),
        },
    );

    if let Some(kpi) = resolved.band(workspace::Band::Kpi) {
        dashboard::draw_kpis(frame, kpi, app);
    }
    for pane in resolved.panes() {
        if !pane.outer.is_empty() {
            app.hit
                .borrow_mut()
                .push(pane.outer, HitAction::Pane(pane.id));
        }
        draw_pane(frame, pane.outer, app, pane.id, pane.id == focus, &indices);
    }
    for seam in resolved.seams() {
        push_divider(app, seam.split, seam.span, seam.first, seam.second);
    }
    if let Some(account) = resolved.band(workspace::Band::Account) {
        dashboard::draw_account(frame, account, app, resolved.compact());
    }
    app.hit
        .borrow_mut()
        .hidden
        .extend_from_slice(resolved.hidden());
    if let Some(rect) = resolved.rect(focus) {
        draw_hidden_badge(frame, rect, resolved.hidden().len());
    }
}

fn draw_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    pane: PaneId,
    focused: bool,
    indices: &RefCell<Vec<Line<'static>>>,
) {
    match pane {
        PaneId::OverviewPositions => dashboard::draw_positions_panel(frame, area, app, focused),
        PaneId::OverviewHoldings => dashboard::draw_holdings_panel(frame, area, app, focused),
        PaneId::OverviewIndices => {
            dashboard::render_index_panel(frame, area, indices.take(), focused);
        }
        PaneId::OverviewMovers => dashboard::draw_movers(frame, area, app, focused),
        PaneId::TradingWatch => views::draw_watch_list(frame, area, app, pane, focused),
        PaneId::TradingChart => views::draw_chart_leaf(
            frame,
            area,
            app,
            &app.trading_chart,
            "Follows the Trading watch list",
            focused,
            &pane::PaneHits::new(&app.hit, pane),
        ),
        PaneId::TradingDepth => views::draw_depth_leaf(
            frame,
            area,
            app,
            app.watch_pane.selected(&app.watch),
            app.workspaces.maximized(Preset::Trading) == Some(pane),
            focused,
        ),
        PaneId::TradingOrders | PaneId::OrdersBook => {
            views::draw_orders_leaf(frame, area, app, pane, focused);
        }
        PaneId::PortfolioPositions => views::draw_positions_leaf(frame, area, app, pane, focused),
        PaneId::PortfolioHoldings => views::draw_holdings_leaf(frame, area, app, pane, focused),
        PaneId::PortfolioFunds => modals::draw_funds_pane(frame, area, app, focused),
        PaneId::DerivativesChain => chain::draw_chain_leaf(frame, area, app, pane),
        PaneId::DerivativesChart => views::draw_chart_leaf(
            frame,
            area,
            app,
            &app.derivatives_chart,
            "Follows the chain underlying",
            focused,
            &pane::PaneHits::new(&app.hit, pane),
        ),
        PaneId::OrdersGtt => views::draw_gtt_leaf(frame, area, app, pane, focused),
        PaneId::OrdersAlerts => views::draw_alerts_leaf(frame, area, app, pane, focused),
        PaneId::InvestMf => views::draw_mf_leaf(frame, area, app, pane, focused),
        PaneId::InvestBids => views::draw_bids_leaf(frame, area, app, pane, focused),
    }
}

fn draw_hidden_badge(frame: &mut Frame, area: Rect, hidden: usize) {
    if hidden == 0 || area.height == 0 {
        return;
    }
    let text = format!(" +{hidden} hidden ");
    let width = u16::try_from(format::display_width(&text)).unwrap_or(u16::MAX);
    if area.width < width.saturating_add(4) {
        return;
    }
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(text, format::dim()))),
        Rect {
            x: area.right() - width - 2,
            y: area.y,
            width,
            height: 1,
        },
    );
}

#[cfg(test)]
mod chrome_tests {
    use super::toast::MAX_TOAST_ROWS;
    use super::{
        MessageTone, draw_header, draw_hints, draw_toast, paint_root, panel, state_message,
        toast_display,
    };
    use crate::tui::app::{App, ToastLevel};
    use crate::tui::format;
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings, Theme};
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Constraint, Layout, Rect};
    use std::time::{Duration, Instant};

    pub(super) const FIXTURE_NOW: u64 = 1_788_757_200;

    pub(super) const FIXTURE_UPTIME: u64 = 80;

    pub(super) fn test_app() -> App {
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

    pub(super) fn row(buffer: &Buffer, y: u16) -> String {
        let mut out = String::new();
        let mut skip = 0u16;
        for x in 0..buffer.area.width {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let symbol = buffer[(x, y)].symbol();
            let cells = format::display_width(symbol);
            skip = u16::try_from(cells.saturating_sub(1)).unwrap();
            out.push_str(symbol);
        }
        out
    }

    fn rows(buffer: &Buffer) -> Vec<String> {
        (0..buffer.area.height).map(|y| row(buffer, y)).collect()
    }

    #[test]
    fn the_root_bars_use_surface_only_when_the_theme_has_one() {
        for name in ["kite", "solarized-light", "system"] {
            let theme = settings::preset(name).expect("preset exists");
            let _guard = settings::theme_guard(theme);
            let mut app = test_app();
            app.toast(ToastLevel::Info, "filled");
            let buffer = root_buffer(&app, 80, 10);
            for y in [0u16, 8, 9] {
                let cell = &buffer[(60, y)];
                match theme.surface {
                    Some(surface) => assert_eq!(cell.bg, surface, "{name} row {y}"),
                    None => assert_eq!(
                        cell.bg,
                        ratatui::style::Color::Reset,
                        "{name} row {y} painted a background"
                    ),
                }
            }
        }
    }

    #[test]
    fn every_preset_renders_the_same_chrome_cells() {
        let mut seen: Option<Vec<String>> = None;
        for name in settings::preset_names() {
            let _guard = settings::theme_guard(settings::preset(name).expect("preset exists"));
            let mut app = test_app();
            app.toast(ToastLevel::Error, "the same text everywhere");
            let buffer = root_buffer(&app, 80, 10);
            let bars = vec![row(&buffer, 8), row(&buffer, 9)];
            match &seen {
                None => seen = Some(bars),
                Some(first) => assert_eq!(&bars, first, "preset {name} changed the cells"),
            }
        }
    }

    pub(super) fn root_buffer(app: &App, width: u16, height: u16) -> Buffer {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|frame| super::draw(frame, app)).unwrap();
        terminal.backend().buffer().clone()
    }

    type GalleryPanel<'a> = (
        Rect,
        &'a str,
        MessageTone,
        &'a str,
        Option<&'a str>,
        Option<&'a str>,
    );

    fn state_gallery(width: u16, height: u16, panel_heights: [u16; 3]) -> Vec<String> {
        let mut app = test_app();
        app.set_pane(PaneId::TradingWatch);
        app.toast(
            ToastLevel::Error,
            "Request failed: the connection closed before a response.\nThe last displayed quote remains on screen.\nOpen the event log for the complete error.",
        );
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                paint_root(frame);
                let toast = toast_display(&app, width);
                let toast_rows = toast
                    .as_ref()
                    .map_or(0, |d| u16::try_from(d.rows.len()).unwrap());
                let [header, body, band, hints] = Layout::vertical([
                    Constraint::Length(1),
                    Constraint::Min(0),
                    Constraint::Length(toast_rows),
                    Constraint::Length(1),
                ])
                .areas(frame.area());
                draw_header(frame, header, &app);
                let [first, second, third] = Layout::vertical([
                    Constraint::Length(panel_heights[0]),
                    Constraint::Length(panel_heights[1]),
                    Constraint::Length(panel_heights[2]),
                ])
                .areas(body);
                let panels: [GalleryPanel; 3] = [
                    (
                        first,
                        " Watchlist ",
                        MessageTone::Normal,
                        "Watchlist is empty",
                        None,
                        Some("/ Search and add instruments"),
                    ),
                    (
                        second,
                        " > Positions (0) ",
                        MessageTone::Normal,
                        "Positions not available yet",
                        None,
                        None,
                    ),
                    (
                        third,
                        " Movers · All (any volume) ",
                        MessageTone::Error,
                        "Market movers unavailable",
                        Some("Connection timed out"),
                        Some("r Retry"),
                    ),
                ];
                for (rect, title, tone, headline, detail, hint) in panels {
                    let block = panel(title.to_string());
                    let inner = block.inner(rect);
                    frame.render_widget(block, rect);
                    state_message(frame, inner, tone, headline, detail, hint);
                }
                draw_toast(frame, band, toast.as_ref());
                draw_hints(frame, hints, &app);
            })
            .unwrap();
        rows(terminal.backend().buffer())
    }

    #[test]
    fn the_state_gallery_matches_the_spec_canvas_at_eighty_by_twenty_four() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let rendered = state_gallery(80, 24, [6, 6, 7]);
        let expected = [
            "┌ Watchlist ───────────────────────────────────────────────────────────────────┐",
            "│                                                                              │",
            "│ Watchlist is empty                                                           │",
            "│ / Search and add instruments                                                 │",
            "│                                                                              │",
            "└──────────────────────────────────────────────────────────────────────────────┘",
            "┌ > Positions (0) ─────────────────────────────────────────────────────────────┐",
            "│                                                                              │",
            "│ Positions not available yet                                                  │",
            "│                                                                              │",
            "│                                                                              │",
            "└──────────────────────────────────────────────────────────────────────────────┘",
            "┌ Movers · All (any volume) ───────────────────────────────────────────────────┐",
            "│                                                                              │",
            "│ Market movers unavailable                                                    │",
            "│ Connection timed out                                                         │",
            "│ r Retry                                                                      │",
            "│                                                                              │",
            "└──────────────────────────────────────────────────────────────────────────────┘",
            " [ERR ] Request failed: the connection closed before a response.     Ctrl-L log ",
            "        The last displayed quote remains on screen.                             ",
            "        Open the event log for the complete error.                              ",
        ];
        assert_eq!(&rendered[1..23], &expected[..]);
    }

    #[test]
    fn the_state_gallery_matches_the_spec_canvas_at_one_twenty_by_thirty_five() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let rendered = state_gallery(120, 35, [10, 10, 10]);
        let expected = [
            "┌ Watchlist ───────────────────────────────────────────────────────────────────────────────────────────────────────────┐",
            "│                                                                                                                      │",
            "│ Watchlist is empty                                                                                                   │",
            "│ / Search and add instruments                                                                                         │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘",
            "┌ > Positions (0) ─────────────────────────────────────────────────────────────────────────────────────────────────────┐",
            "│                                                                                                                      │",
            "│ Positions not available yet                                                                                          │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘",
            "┌ Movers · All (any volume) ───────────────────────────────────────────────────────────────────────────────────────────┐",
            "│                                                                                                                      │",
            "│ Market movers unavailable                                                                                            │",
            "│ Connection timed out                                                                                                 │",
            "│ r Retry                                                                                                              │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "│                                                                                                                      │",
            "└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘",
            " [ERR ] Request failed: the connection closed before a response.                                             Ctrl-L log ",
            "        The last displayed quote remains on screen.                                                                     ",
            "        Open the event log for the complete error.                                                                      ",
        ];
        assert_eq!(&rendered[1..34], &expected[..]);
    }

    #[test]
    fn a_narrow_overview_draws_whichever_pane_tab_focused() {
        use crate::tui::marketwatch::{State, WatchItem};
        use crate::tui::workspace::{PaneId, Preset};
        use kiteconnect::ticker::Tick;
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        app.set_watch_for_test(State::test_loaded(vec![WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "NIFTY BANK".into(),
            instrument_token: 260_105,
            segment: "INDICES".into(),
            ..WatchItem::default()
        }]));
        app.ticks.insert(260_105, {
            let mut value = Tick::default();
            value.instrument_token = 260_105;
            value.last_price = Some(56_789.45);
            value.close = Some(56_666.40);
            value.is_index = true;
            value
        });
        for (width, height) in [(60, 24), (80, 24), (120, 35), (160, 45)] {
            for (pane, title) in [
                (PaneId::OverviewIndices, "Indices"),
                (PaneId::OverviewMovers, "Movers"),
            ] {
                app.workspaces.focus(Preset::Overview, pane);
                let text = rows(&root_buffer(&app, width, height)).join("\n");
                assert!(text.contains(title), "{width}x{height} {pane:?}:\n{text}");
                if pane == PaneId::OverviewIndices {
                    assert!(text.contains("NIFTY BANK"), "{width}x{height}:\n{text}");
                }
            }
        }
    }

    #[test]
    fn tiny_and_unicode_geometry_never_panics() {
        let _guard = settings::theme_guard(settings::preset("kite").expect("preset exists"));
        let texts = [
            "plain",
            "line one\nline two\nline three\nline four",
            "e\u{301}e\u{301}e\u{301} combining",
            "株式会社のマージンが不足しています",
            "Averyveryverylongsinglewordwithnobreaksatallanywhere",
        ];
        for text in texts {
            for width in [0u16, 1, 23, 24, 69, 70, 80, 120] {
                for height in 0..=5u16 {
                    let mut app = test_app();
                    app.toast(ToastLevel::Error, text);
                    app.started_at = Instant::now()
                        .checked_sub(Duration::from_secs(FIXTURE_UPTIME))
                        .unwrap();
                    let buffer = root_buffer(&app, width, height);
                    assert_eq!(buffer.area.width, width);
                    assert_eq!(buffer.area.height, height);
                    if let Some(display) = toast_display(&app, width) {
                        assert!(display.rows.len() <= MAX_TOAST_ROWS);
                    }
                }
            }
        }
    }
}
