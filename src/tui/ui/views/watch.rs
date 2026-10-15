use super::book_state::state_body;
use super::table::{TableRows, right, right_styled, selectable_table};
use crate::tui::app::App;
use crate::tui::format;
use crate::tui::marketwatch;
use crate::tui::ticks;
use crate::tui::ui;
use crate::tui::ui::columns::{
    self, FitCol, col, fit_constraints, fit_header, fit_row, table_budget,
};
use crate::tui::ui::pane::{PaneHits, PlainList, WatchInput};
use crate::tui::workspace::PaneId;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Row, Table};
use std::fmt::Write;

pub fn draw_watch_list(frame: &mut Frame, area: Rect, app: &App, pane: PaneId, focused: bool) {
    let input = WatchInput {
        rows: app.watch_rows_of(pane),
        watch: app.watch_view_of(pane),
        selected: app.pane_row(pane),
        viewport: app.watch_viewport_of(pane),
        focused,
    };
    draw_watch_pane(frame, area, app, &input, &PaneHits::new(&app.hit, pane));
}

fn watchlist_title(watch: marketwatch::Watch<'_, '_>) -> String {
    let mut title = format!("Watchlist · {}", watch.title());
    if watch.count() > 1 {
        write!(title, " ({}/{})", watch.position() + 1, watch.count())
            .expect("writing to a String");
    }
    if let Some(pending) = watch.pending() {
        write!(title, " · {pending}…").expect("writing to a String");
    } else if watch.is_stale() {
        title.push_str(" · stale, r to reload");
    } else if watch.is_static() {
        title.push_str(" · read-only");
    }
    title
}

fn watch_unavailable_state<'a>(watch: marketwatch::Watch<'a, '_>) -> Option<WatchState<'a>> {
    use crate::tui::marketwatch::Unavailable;
    let state = match watch.unavailable()? {
        Unavailable::NeedsLogin => WatchState {
            tone: ui::MessageTone::Error,
            headline: "Log in again to load your marketwatch",
            detail: Some("This session predates the marketwatch support."),
            hint: None,
        },
        Unavailable::Loading => WatchState {
            tone: ui::MessageTone::Normal,
            headline: "Loading marketwatch...",
            detail: None,
            hint: None,
        },
        Unavailable::Failed => WatchState {
            tone: ui::MessageTone::Error,
            headline: "Marketwatch unavailable",
            detail: Some(watch.error().unwrap_or("no response")),
            hint: Some("r Retry"),
        },
        Unavailable::Empty => WatchState {
            tone: ui::MessageTone::Normal,
            headline: "No watchlists",
            detail: None,
            hint: Some(": wl <name> Create a watchlist"),
        },
    };
    Some(state)
}

struct WatchState<'a> {
    tone: ui::MessageTone,
    headline: &'static str,
    detail: Option<&'a str>,
    hint: Option<&'static str>,
}

pub fn draw_watch_pane(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    input: &WatchInput<'_>,
    hits: &PaneHits<'_>,
) {
    if let Some(state) = watch_unavailable_state(input.watch) {
        state_body(
            frame,
            area,
            ui::focus_panel("Watchlist", input.focused),
            state.tone,
            state.headline,
            state.detail,
            state.hint,
        );
    } else if input.rows.is_empty() {
        let state = if input.watch.detail().is_loading() {
            WatchState {
                tone: ui::MessageTone::Normal,
                headline: "Loading marketwatch...",
                detail: None,
                hint: None,
            }
        } else if let Some(err) = input.watch.error() {
            WatchState {
                tone: ui::MessageTone::Error,
                headline: "Marketwatch unavailable",
                detail: Some(err),
                hint: Some("r Retry"),
            }
        } else {
            WatchState {
                tone: ui::MessageTone::Normal,
                headline: "Watchlist is empty",
                detail: None,
                hint: Some("/ Search and add instruments"),
            }
        };
        state_body(
            frame,
            area,
            ui::focus_panel(watchlist_title(input.watch), input.focused),
            state.tone,
            state.headline,
            state.detail,
            state.hint,
        );
    } else {
        draw_watch_table(
            frame,
            area,
            app,
            input,
            &PlainList {
                selected: input.selected,
                focused: input.focused,
                viewport: Some(input.viewport),
            },
            hits,
        );
    }
}

const WATCH_SYM_MIN: u16 = 12;
const WATCH_SYM_MAX: u16 = 24;
const WATCH_BELL_W: u16 = 3;

pub(super) const WATCH_COLS: [FitCol; 7] = [
    col(WATCH_SYM_MIN, 0), // SYMBOL
    col(11, 2),            // LTP
    col(8, 1),             // CHG%
    col(10, 3),            // CHG
    col(10, 5),            // HIGH
    col(10, 6),            // LOW
    col(8, 4),             // VOL
];

pub(super) const WATCH_LABELS: [(&str, bool); 7] = [
    ("SYMBOL", false),
    ("LTP", true),
    ("CHG%", true),
    ("CHG", true),
    ("HIGH", true),
    ("LOW", true),
    ("VOL", true),
];

fn draw_watch_table(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    input: &WatchInput<'_>,
    list: &PlainList<'_>,
    hits: &PaneHits<'_>,
) {
    let PlainList {
        selected,
        focused,
        viewport,
    } = *list;
    let labels: Vec<_> = input
        .rows
        .iter()
        .map(|item| (item.label(), app.alert_state(item.instrument_token)))
        .collect();
    let fit = columns::fit_columns(table_budget(area.width), 1, &WATCH_COLS);
    let layout = columns::spread(
        &fit,
        &WATCH_COLS,
        &[columns::text(
            0,
            columns::widest(labels.iter().map(|(label, bell)| {
                format::display_width(label) + usize::from(bell.is_some()) * WATCH_BELL_W as usize
            })),
            WATCH_SYM_MAX,
        )],
    );
    let sym_w = layout.width(0);

    let rows: Vec<Row> = input
        .rows
        .iter()
        .zip(&labels)
        .map(|(item, (label, bell))| {
            let tick = app.tick(item.instrument_token);
            let ltp = tick.and_then(|t| t.last_price);
            let change = tick.and_then(ticks::change);
            let change_pct = tick.and_then(ticks::change_percent);
            let change_style = change.map(format::change_style).unwrap_or_default();

            let ltp_style = match app.flash_dir(item.instrument_token) {
                Some(dir) if dir > 0 => Style::default()
                    .fg(format::up())
                    .add_modifier(Modifier::BOLD),
                Some(_) => Style::default()
                    .fg(format::down())
                    .add_modifier(Modifier::BOLD),
                None => change_style,
            };

            let fmt = |value: Option<f64>| value.map_or_else(|| ".".into(), format::price);
            let symbol_cell = || watch_symbol_cell(label, *bell, sym_w);
            fit_row(
                &fit,
                &[
                    &symbol_cell,
                    &|| right_styled(fmt(ltp), ltp_style),
                    &|| {
                        right_styled(
                            change_pct.map_or_else(|| ".".into(), format::percent),
                            change_style,
                        )
                    },
                    &|| {
                        right_styled(
                            change.map_or_else(|| ".".into(), format::signed_price),
                            change_style,
                        )
                    },
                    &|| right(fmt(tick.and_then(|t| t.high))),
                    &|| right(fmt(tick.and_then(|t| t.low))),
                    &|| {
                        right(
                            tick.and_then(|t| t.volume)
                                .map_or_else(|| ".".into(), format::compact_qty),
                        )
                    },
                ],
            )
        })
        .collect();

    let table = Table::new(rows, fit_constraints(&fit, &layout))
        .header(fit_header(&fit, &WATCH_LABELS))
        .column_spacing(layout.spacing)
        .block(ui::focus_panel(watchlist_title(input.watch), focused));

    selectable_table(
        frame,
        area,
        table,
        &layout,
        TableRows {
            selected,
            count: input.rows.len(),
            viewport,
        },
        hits,
    );
}

fn watch_symbol_cell(label: &str, bell: Option<bool>, sym_w: u16) -> Cell<'static> {
    let label_w = if bell.is_some() {
        sym_w.saturating_sub(WATCH_BELL_W)
    } else {
        sym_w
    };
    let mut symbol_spans = vec![Span::raw(format::pad_or_clip(label, label_w as usize))];
    if let Some(any_armed) = bell {
        let bell_style = if any_armed {
            Style::default().fg(format::warning())
        } else {
            format::dim()
        };
        symbol_spans.push(Span::styled(" 🔔", bell_style));
    }
    Cell::from(Line::from(symbol_spans))
}

#[cfg(test)]
mod watch_flash_render_tests {
    use super::super::draw_watch;
    use crate::tui::app::App;
    use crate::tui::event::AppEvent;
    use crate::tui::format;
    use crate::tui::marketwatch::WatchItem;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use kiteconnect::KiteConnect;
    use kiteconnect::ticker::{Tick, TickerEvent};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Modifier;
    use tokio::sync::mpsc;

    fn test_app() -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = mpsc::channel(8);
        App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            true,
            &Settings::default(),
        )
    }

    fn tick(price: f64) -> Tick {
        let mut tick = Tick::default();
        tick.instrument_token = 408_065;
        tick.last_price = Some(price);
        tick
    }

    fn rendered(app: &App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(90, 8)).unwrap();
        terminal
            .draw(|frame| draw_watch(frame, frame.area(), app))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn the_trading_watch_leaf_follows_workspace_focus() {
        use crate::tui::workspace::{PaneId, Preset};
        for items in [
            Vec::new(),
            vec![WatchItem {
                tradingsymbol: "INFY".into(),
                exchange: "NSE".into(),
                instrument_token: 408_065,
                ..Default::default()
            }],
        ] {
            let mut app = test_app();
            app.set_watch_for_test(crate::tui::marketwatch::State::test_loaded(items));
            app.set_workspace(Preset::Trading);
            for (pane, marked) in [(PaneId::TradingOrders, false), (PaneId::TradingWatch, true)] {
                app.focus_pane_for_test(pane);
                for (w, h) in [(80, 24), (120, 35), (160, 45)] {
                    let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
                    terminal
                        .draw(|frame| crate::tui::ui::draw(frame, &app))
                        .unwrap();
                    let text: String = terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .map(ratatui::buffer::Cell::symbol)
                        .collect();
                    assert_eq!(
                        text.contains("> Watchlist"),
                        marked,
                        "{w}x{h} focus {pane:?}"
                    );
                    if w > 80 {
                        assert!(text.contains("Watchlist"), "{w}x{h}: the leaf is drawn");
                    }
                }
            }
        }
    }

    #[test]
    fn each_reason_there_are_no_rows_says_which_one_it_is() {
        use crate::tui::marketwatch::State;

        let mut app = test_app();

        app.set_watch_for_test(State::remote(None, String::new(), None));
        let text = rendered(&app);
        assert!(
            text.contains("Log in again to load your marketwatch"),
            "{text}"
        );
        assert!(
            text.contains("This session predates the marketwatch support."),
            "{text}"
        );

        app.set_watch_for_test(State::remote(None, String::new(), None));
        app.watch.test_set_reachable();
        assert!(rendered(&app).contains("Loading marketwatch..."));

        app.watch.set_catalog(Err("offline".to_string()));
        let text = rendered(&app);
        assert!(text.contains("Marketwatch unavailable"), "{text}");
        assert!(text.contains("offline"), "{text}");
        assert!(text.contains("r Retry"), "{text}");

        app.watch
            .set_catalog(Ok(crate::tui::marketwatch::Catalog::default()));
        let text = rendered(&app);
        assert!(text.contains("No watchlists"), "{text}");
        assert!(text.contains(": wl <name> Create a watchlist"), "{text}");

        app.set_watch_for_test(State::test_loaded(Vec::new()));
        let text = rendered(&app);
        assert!(text.contains("Watchlist is empty"), "{text}");
        assert!(text.contains("/ Search and add instruments"), "{text}");
    }

    #[test]
    fn a_failed_read_reads_as_a_failure_and_an_empty_list_does_not() {
        use crate::tui::marketwatch::State;
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);

        let headline_fg = |app: &App, needle: &str| {
            let mut terminal = Terminal::new(TestBackend::new(90, 8)).unwrap();
            terminal
                .draw(|frame| draw_watch(frame, frame.area(), app))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            let first = needle.chars().next().expect("a needle");
            for y in 0..8 {
                for x in 0..90u16 {
                    if buffer[(x, y)].symbol() == first.to_string() {
                        let row: String = (x..90).map(|c| buffer[(c, y)].symbol()).collect();
                        if row.starts_with(needle) {
                            return buffer[(x, y)].fg;
                        }
                    }
                }
            }
            panic!("{needle:?} is not on screen");
        };

        let mut app = test_app();
        app.set_watch_for_test(State::remote(None, String::new(), None));
        app.watch.test_set_reachable();
        app.watch.set_catalog(Err("offline".to_string()));
        assert_eq!(
            headline_fg(&app, "Marketwatch unavailable"),
            format::error()
        );

        app.set_watch_for_test(State::test_loaded(Vec::new()));
        assert_ne!(
            headline_fg(&app, "Watchlist is empty"),
            format::error(),
            "an empty list is a state, not a failure"
        );
    }

    #[test]
    fn the_title_names_the_open_list_and_what_it_is_doing() {
        let mut app = test_app();
        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: 408_065,
            segment: "NSE".into(),
            ..WatchItem::default()
        });
        let text = rendered(&app);
        assert!(text.contains("Watchlist"), "{text}");
        assert!(text.contains("default"), "{text}");

        app.watch.test_set_static();
        assert!(rendered(&app).contains("read-only"));
    }

    #[test]
    fn a_live_ltp_flash_keeps_the_selected_row_highlight_intact() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let mut app = test_app();
        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: "INFY".into(),
            instrument_token: 408_065,
            segment: "NSE".into(),
            ..WatchItem::default()
        });
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(vec![tick(99.0)])));
        app.on_event(AppEvent::Ws(TickerEvent::Ticks(vec![tick(100.0)])));

        let width = 90;
        let mut terminal = Terminal::new(TestBackend::new(width, 8)).unwrap();
        terminal
            .draw(|frame| draw_watch(frame, frame.area(), &app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let row = 2;
        let ltp_start = (0..width.saturating_sub(5))
            .find(|&x| {
                ["1", "0", "0", ".", "0", "0"]
                    .iter()
                    .enumerate()
                    .all(|(offset, symbol)| {
                        buffer[(x + u16::try_from(offset).unwrap(), row)].symbol() == *symbol
                    })
            })
            .expect("rendered LTP");
        let cell = &buffer[(ltp_start, row)];
        assert_eq!(cell.fg, format::up());
        assert!(cell.modifier.contains(Modifier::BOLD));
        assert_ne!(cell.bg, format::up());
        assert_ne!(cell.bg, format::down());
    }
}
