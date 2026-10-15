use crate::tui::app::App;
use crate::tui::data::{MOVERS_ROWS, Movers, ScreenerRow};
use crate::tui::format;
use crate::tui::ui;
use crate::tui::ui::columns::{self, ColumnFit, FitCol, col};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const MOVERS_LTP_W: u16 = 11;
const MOVERS_PCT_W: u16 = 9;
const MOVERS_VOL_W: u16 = 9;
const MOVERS_ARROW_W: u16 = 3;
const MOVERS_SYM_MIN: u16 = 6;
const MOVERS_SYM_MAX: u16 = 24;

const MOVERS_COLS: [FitCol; 4] = [
    col(MOVERS_SYM_MIN, 0), // SYMBOL
    col(MOVERS_LTP_W, 3),   // LTP
    col(MOVERS_PCT_W, 1),   // CHG%
    col(MOVERS_VOL_W, 2),   // VOL
];

pub(in crate::tui::ui) fn draw_movers(frame: &mut Frame, area: Rect, app: &App, focused: bool) {
    let block = ui::focus_panel(format!("Movers · {}", app.movers_universe.label()), focused);
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(state) = movers_state(app.movers.as_ref(), app.movers_failed.as_deref()) {
        let (tone, headline, detail, hint) = match &state {
            MoversState::Loading => (
                ui::MessageTone::Normal,
                "Loading market movers...",
                None,
                None,
            ),
            MoversState::Failed(reason) => (
                ui::MessageTone::Error,
                "Market movers unavailable",
                Some(*reason),
                Some("r Retry"),
            ),
            MoversState::Empty => (
                ui::MessageTone::Normal,
                "No movers in this universe yet",
                None,
                None,
            ),
        };
        ui::state_message(frame, inner, tone, headline, detail, hint);
        return;
    }
    let Some(movers) = app.movers.as_ref() else {
        return;
    };

    let fit = columns::fit_columns(inner.width.saturating_sub(MOVERS_ARROW_W), 0, &MOVERS_COLS);
    let symbol_w = fit.flex(MOVERS_SYM_MIN, MOVERS_SYM_MAX) as usize;

    let mut lines: Vec<Line> = vec![movers_header(&fit, symbol_w)];
    for row in movers.gainers.iter().take(MOVERS_ROWS) {
        lines.push(mover_row("▲", row, &fit, symbol_w));
    }
    for row in movers.losers.iter().take(MOVERS_ROWS) {
        lines.push(mover_row("▼", row, &fit, symbol_w));
    }
    frame.render_widget(Paragraph::new(lines), inner);
}

#[derive(Debug)]
enum MoversState<'a> {
    Loading,
    Failed(&'a str),
    Empty,
}

fn movers_state<'a>(movers: Option<&Movers>, failed: Option<&'a str>) -> Option<MoversState<'a>> {
    match movers {
        None => Some(match failed {
            Some(reason) => MoversState::Failed(reason),
            None => MoversState::Loading,
        }),
        Some(movers) if movers.gainers.is_empty() && movers.losers.is_empty() => {
            Some(MoversState::Empty)
        }
        Some(_) => None,
    }
}

fn movers_header(fit: &ColumnFit, symbol_w: usize) -> Line<'static> {
    let mut text = format!("   {}", format::pad_or_clip("SYMBOL", symbol_w));
    if fit.shows(1) {
        text.push_str(&format::pad_or_clip_right("LTP", usize::from(MOVERS_LTP_W)));
    }
    if fit.shows(2) {
        text.push_str(&format::pad_or_clip_right(
            "CHG%",
            usize::from(MOVERS_PCT_W),
        ));
    }
    if fit.shows(3) {
        text.push_str(&format::pad_or_clip_right("VOL", usize::from(MOVERS_VOL_W)));
    }
    Line::from(Span::styled(text, format::header_style()))
}

fn mover_row(arrow: &str, row: &ScreenerRow, fit: &ColumnFit, symbol_w: usize) -> Line<'static> {
    let style = format::change_style(row.change_percent);
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "nonnegative screener volume truncates to displayed whole units"
    )]
    let volume = row.volume.max(0.0) as u64;
    let volume_style = if volume == 0 {
        format::dim()
    } else {
        Style::default()
    };
    let mut spans = vec![
        Span::styled(format!(" {arrow} "), style),
        Span::raw(format::pad_or_clip(&row.tradingsymbol, symbol_w)),
    ];
    if fit.shows(1) {
        spans.push(Span::styled(
            format::pad_or_clip_right(&format::price(row.last_price), usize::from(MOVERS_LTP_W)),
            Style::default(),
        ));
    }
    if fit.shows(2) {
        spans.push(Span::styled(
            format::pad_or_clip_right(
                &format::percent(row.change_percent),
                usize::from(MOVERS_PCT_W),
            ),
            style,
        ));
    }
    if fit.shows(3) {
        spans.push(Span::styled(
            format::pad_or_clip_right(&format::compact_qty(volume), usize::from(MOVERS_VOL_W)),
            volume_style,
        ));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::{MOVERS_COLS, MoversState, draw_movers, mover_row, movers_state};
    use crate::tui::app::App;
    use crate::tui::data::{Movers, MoversUniverse, ScreenerRow};
    use crate::tui::format;
    use crate::tui::ui::columns::{self, ColumnFit};
    use crate::tui::ui::dashboard::fixtures::{buffer_rows, find_once, test_app};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn all_columns() -> ColumnFit {
        columns::fit_columns(200, 0, &MOVERS_COLS)
    }

    fn screener_row(
        symbol: &str,
        last_price: f64,
        change_percent: f64,
        volume: f64,
    ) -> ScreenerRow {
        ScreenerRow {
            tradingsymbol: symbol.into(),
            last_price,
            change_percent,
            volume,
        }
    }

    #[test]
    fn movers_state_distinguishes_loading_failed_and_empty() {
        assert!(
            matches!(movers_state(None, None), Some(MoversState::Loading)),
            "an unfetched panel must say it is loading"
        );
        let failed = movers_state(None, Some("offline, no network route to Kite"));
        match failed {
            Some(MoversState::Failed(reason)) => {
                assert_eq!(reason, "offline, no network route to Kite");
            }
            other => panic!("a failed fetch must not keep claiming it is loading: {other:?}"),
        }

        let empty = Movers {
            gainers: Vec::new(),
            losers: Vec::new(),
        };
        assert!(matches!(
            movers_state(Some(&empty), None),
            Some(MoversState::Empty)
        ));

        let gainers_only = Movers {
            gainers: vec![screener_row("LGEINDIA", 1621.4, 9.59, 241_000.0)],
            losers: Vec::new(),
        };
        assert!(movers_state(Some(&gainers_only), None).is_none());
        assert!(movers_state(Some(&gainers_only), Some("boom")).is_none());
        let losers_only = Movers {
            gainers: Vec::new(),
            losers: vec![screener_row("ZEEL", 100.0, -5.72, 1_000.0)],
        };
        assert!(movers_state(Some(&losers_only), None).is_none());
    }

    #[test]
    fn mover_row_has_stable_width_for_short_and_long_symbols() {
        let width = |line: &ratatui::text::Line| -> usize {
            line.spans.iter().map(|s| s.content.chars().count()).sum()
        };
        let fit = all_columns();
        let short = mover_row("▲", &screener_row("TCS", 3200.0, 1.5, 12_345.0), &fit, 10);
        let long = mover_row(
            "▼",
            &screener_row("SOMEVERYLONGSCREENERSYMBOL", 3200.0, -2.0, 98_765_432.0),
            &fit,
            10,
        );
        assert_eq!(width(&short), width(&long));
    }

    #[test]
    fn mover_row_never_lets_a_huge_percent_push_out_the_volume() {
        let width = |line: &ratatui::text::Line| -> usize {
            line.spans.iter().map(|s| s.content.chars().count()).sum()
        };
        let fit = all_columns();
        let normal = mover_row("▲", &screener_row("INFY", 1500.0, 1.5, 100.0), &fit, 10);
        let absurd = mover_row(
            "▲",
            &screener_row("PHLSRG", 200.05, 99_999.99, 0.0),
            &fit,
            10,
        );
        assert_eq!(width(&normal), width(&absurd));
        assert_eq!(
            absurd.spans.last().expect("volume span").content.trim(),
            "0",
            "the volume column must survive a five-digit percent"
        );
    }

    #[test]
    fn mover_row_reports_zero_volume_for_an_illiquid_row() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let fit = all_columns();
        let line = mover_row(
            "▲",
            &screener_row("PHLSRG-SF", 200.05, 2781.36, 0.0),
            &fit,
            12,
        );
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.contains("PHLSRG-SF"), "{text}");
        assert!(text.contains("2781.36%"), "{text}");
        assert!(text.trim_end().ends_with('0'), "volume column: {text}");
        let volume_span = line.spans.last().expect("volume span");
        assert_eq!(volume_span.style, format::dim());

        let traded = mover_row(
            "▲",
            &screener_row("LGEINDIA", 1621.4, 9.59, 241_000.0),
            &fit,
            12,
        );
        let volume_span = traded.spans.last().expect("volume span");
        assert_ne!(volume_span.style, format::dim());
        assert!(volume_span.content.contains("2.41L"), "{volume_span:?}");
    }

    fn movers_panel_rows(app: &App) -> Vec<String> {
        movers_rows_at(48, app)
    }

    fn movers_rows_at(width: u16, app: &App) -> Vec<String> {
        let height = 10;
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| draw_movers(frame, frame.area(), app, false))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        (0..height)
            .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
            .collect()
    }

    #[test]
    fn movers_panel_title_names_the_universe() {
        let mut app = test_app();
        app.movers = Some(Movers {
            gainers: vec![ScreenerRow {
                tradingsymbol: "LGEINDIA".into(),
                last_price: 1621.4,
                change_percent: 9.59,
                volume: 241_000.0,
            }],
            losers: Vec::new(),
        });
        let title = &movers_panel_rows(&app)[0];
        assert!(title.contains("Movers"), "unexpected title: {title:?}");
        assert!(title.contains("Nifty 50"), "unexpected title: {title:?}");

        app.movers_universe = MoversUniverse::All;
        let title = &movers_panel_rows(&app)[0];
        assert!(title.contains("any volume"), "unexpected title: {title:?}");
    }

    #[test]
    fn movers_panel_shows_volume_so_an_illiquid_row_is_obvious() {
        let mut app = test_app();
        app.movers_universe = MoversUniverse::All;
        app.movers = Some(Movers {
            gainers: vec![ScreenerRow {
                tradingsymbol: "PHLSRG-SF".into(),
                last_price: 200.05,
                change_percent: 2781.36,
                volume: 0.0,
            }],
            losers: Vec::new(),
        });
        let rows = movers_panel_rows(&app);
        assert!(rows[1].contains("VOL"), "missing VOL header: {:?}", rows[1]);
        let data = &rows[2];
        assert!(data.contains("PHLSRG-SF"), "{data:?}");
        assert!(data.contains("2781.36%"), "{data:?}");
        assert!(
            data.trim_end().ends_with("0│"),
            "zero volume must be shown: {data:?}"
        );
    }

    #[test]
    fn movers_panel_reports_loading_before_the_first_fetch() {
        let app = test_app();
        assert!(app.movers.is_none());
        let rows = movers_panel_rows(&app);
        assert!(
            rows[2].contains("Loading market movers..."),
            "{:?}",
            rows[2]
        );
        assert!(
            !rows[2].contains("watchlist"),
            "the panel no longer depends on the watchlist: {:?}",
            rows[2]
        );
    }

    #[test]
    fn movers_panel_reports_a_failed_fetch_instead_of_loading_forever() {
        let mut app = test_app();
        app.movers_failed = Some("offline, no network route to Kite".to_string());
        assert!(app.movers.is_none());
        let rows = movers_panel_rows(&app);
        assert!(
            rows[2].contains("Market movers unavailable"),
            "unexpected headline: {:?}",
            rows[2]
        );
        assert!(
            rows[3].contains("offline"),
            "the panel must keep the reason the toast carried: {:?}",
            rows[3]
        );
        assert!(
            rows[4].contains("r Retry"),
            "the panel must name the key that retries: {:?}",
            rows[4]
        );
        assert!(
            !rows.join("\n").contains("Loading"),
            "a failed fetch must not read as still loading: {rows:#?}"
        );
    }

    const WIDTHS: [u16; 6] = [60, 70, 80, 100, 120, 200];

    fn one_mover_app() -> App {
        let mut app = test_app();
        app.movers = Some(Movers {
            gainers: vec![ScreenerRow {
                tradingsymbol: "LGEINDIA".into(),
                last_price: 1_621.4,
                change_percent: 9.59,
                volume: 241_000.0,
            }],
            losers: Vec::new(),
        });
        app
    }

    #[test]
    fn movers_volume_is_whole_or_absent_at_every_width() {
        let app = one_mover_app();
        for width in 20..=200u16 {
            let rows = movers_rows_at(width, &app);
            let data = &rows[2];
            let has_volume = data.contains("2.41L");
            for fragment in ["2.4", "41L", ".41"] {
                assert!(
                    has_volume || !data.contains(fragment),
                    "width {width} rendered a partial volume ({fragment:?}): {data:?}"
                );
            }
        }
    }

    #[test]
    fn movers_header_and_rows_agree_at_every_width() {
        let app = one_mover_app();
        for width in WIDTHS {
            let rows = movers_rows_at(width, &app);
            let (header, data) = (&rows[1], &rows[2]);
            for (label, value) in [("LTP", "1,621.40"), ("CHG%", "+9.59%"), ("VOL", "2.41L")] {
                assert_eq!(
                    header.contains(label),
                    data.contains(value),
                    "width {width}: {label} header/data mismatch\n{header:?}\n{data:?}"
                );
            }
        }
    }

    #[test]
    fn movers_drops_ltp_before_volume() {
        let app = one_mover_app();
        let data = &movers_rows_at(32, &app)[2];
        assert!(data.contains("2.41L"), "volume must survive LTP: {data:?}");
        assert!(data.contains("+9.59%"), "{data:?}");
        assert!(!data.contains("1,621.40"), "{data:?}");
        assert!(data.contains("LGEINDIA"), "{data:?}");
    }

    #[test]
    fn movers_rows_stay_aligned_at_every_width() {
        let mut app = one_mover_app();
        app.movers = Some(Movers {
            gainers: vec![ScreenerRow {
                tradingsymbol: "LGEINDIA".into(),
                last_price: 1_621.4,
                change_percent: 9.59,
                volume: 241_000.0,
            }],
            losers: vec![ScreenerRow {
                tradingsymbol: "SOMEVERYLONGSCREENERSYMBOL".into(),
                last_price: 12.05,
                change_percent: -5.72,
                volume: 0.0,
            }],
        });
        for width in 20..=200u16 {
            let rows = movers_rows_at(width, &app);
            let gainer = rows[2].trim_end_matches([' ', '│']).chars().count();
            let loser = rows[3].trim_end_matches([' ', '│']).chars().count();
            assert_eq!(
                gainer, loser,
                "width {width}: {:?} vs {:?}",
                rows[2], rows[3]
            );
        }
    }

    #[test]
    fn an_empty_movers_universe_is_not_reported_as_loading() {
        let mut app = test_app();
        app.movers = Some(Movers {
            gainers: Vec::new(),
            losers: Vec::new(),
        });
        let rows = movers_panel_rows(&app);
        assert!(
            rows[2].contains("No movers in this universe yet"),
            "{:?}",
            rows[2]
        );
        assert!(!rows.join("\n").contains("Loading"), "{rows:#?}");
    }

    #[test]
    fn a_movers_failure_colours_only_its_headline() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let mut app = test_app();
        app.movers_failed = Some("Connection timed out".to_string());
        let mut terminal = Terminal::new(TestBackend::new(48, 10)).unwrap();
        terminal
            .draw(|frame| draw_movers(frame, frame.area(), &app, false))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let rows = buffer_rows(&buffer);

        let (y, x) = find_once(&rows, "Market movers unavailable");
        assert_eq!(buffer[(x, y)].fg, format::error());
        for needle in ["Connection timed out", "r Retry"] {
            let (y, x) = find_once(&rows, needle);
            assert_eq!(buffer[(x, y)].fg, format::text_muted(), "{needle}");
        }
    }
}
