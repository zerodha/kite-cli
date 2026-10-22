use super::{HitAction, bar_style};
use crate::tui::app::App;
use crate::tui::event::WsState;
use crate::tui::format;
use crate::tui::workspace::Preset;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const BRAND: &str = " kite ";

const TAB_GAP: &str = "  ";

const MORE_LEFT: &str = "‹ ";
const MORE_RIGHT: &str = " ›";

const STATUS_FLOOR: u16 = 14 + 2;

#[derive(Clone, Copy, PartialEq)]
enum Squeeze {
    Keys,
    Brand,
}

const SQUEEZES: [Squeeze; 2] = [Squeeze::Keys, Squeeze::Brand];

#[derive(Debug, PartialEq)]
enum Piece {
    Brand,
    Edge,
    Gap,
    MoreLeft,
    MoreRight,
    Tab {
        target: Preset,
        key: Option<char>,
        active: bool,
    },
}

impl Piece {
    fn width(&self) -> u16 {
        let cells = match self {
            Piece::Brand => format::display_width(BRAND),
            Piece::Edge => 1,
            Piece::Gap => format::display_width(TAB_GAP),
            Piece::MoreLeft => format::display_width(MORE_LEFT),
            Piece::MoreRight => format::display_width(MORE_RIGHT),
            Piece::Tab { target, key, .. } => {
                format::display_width(target.title()) + key.map_or(0, |_| 2)
            }
        };
        u16::try_from(cells).unwrap_or(u16::MAX)
    }
}

fn tab_pieces(
    range: std::ops::Range<usize>,
    active: Preset,
    applied: &[Squeeze],
    pieces: &mut Vec<Piece>,
) {
    let keys = !applied.contains(&Squeeze::Keys);
    for idx in range.clone() {
        let target = Preset::ALL[idx];
        if idx > range.start {
            pieces.push(Piece::Gap);
        }
        pieces.push(Piece::Tab {
            target,
            key: keys.then(|| target.key()),
            active: target == active,
        });
    }
}

fn pieces_width(pieces: &[Piece]) -> u16 {
    pieces.iter().map(Piece::width).sum()
}

fn strip_pieces(active: Preset, width: u16) -> Vec<Piece> {
    let budget = width.saturating_sub(STATUS_FLOOR);
    let count = Preset::ALL.len();
    for level in 0..=SQUEEZES.len() {
        let applied = &SQUEEZES[..level];
        let mut pieces = Vec::new();
        if !applied.contains(&Squeeze::Brand) {
            pieces.push(Piece::Brand);
        }
        pieces.push(Piece::Edge);
        tab_pieces(0..count, active, applied, &mut pieces);
        if pieces_width(&pieces) <= budget {
            return pieces;
        }
    }

    let window = |lo: usize, hi: usize| {
        let mut pieces = vec![if lo > 0 { Piece::MoreLeft } else { Piece::Edge }];
        tab_pieces(lo..hi, active, &SQUEEZES, &mut pieces);
        if hi < count {
            pieces.push(Piece::MoreRight);
        }
        pieces
    };
    let at = Preset::ALL
        .iter()
        .position(|preset| *preset == active)
        .unwrap_or(0);
    let (mut lo, mut hi) = (at, at + 1);
    if pieces_width(&window(lo, hi)) > budget {
        return Vec::new();
    }
    loop {
        let right = hi < count && pieces_width(&window(lo, hi + 1)) <= budget;
        if right {
            hi += 1;
        }
        let left = lo > 0 && pieces_width(&window(lo - 1, hi)) <= budget;
        if left {
            lo -= 1;
        }
        if !right && !left {
            return window(lo, hi);
        }
    }
}

const STATUS_SEP: &str = " | ";

fn status_line(
    ws_state: WsState,
    fatal_reason: Option<&str>,
    margin: Option<f64>,
    now: u64,
    uptime_secs: u64,
    width: u16,
) -> Line<'static> {
    let (ws_label, ws_style) = if let Some(reason) = fatal_reason {
        let label = if reason == crate::tui::app::SESSION_EXPIRED {
            "auth expired"
        } else {
            reason
        };
        (format!("○ {label}"), Style::default().fg(format::down()))
    } else {
        let (label, style) = match ws_state {
            WsState::Connected => ("● live", Style::default().fg(format::up())),
            WsState::Connecting => ("◌ connecting", Style::default().fg(format::warning())),
            WsState::Reconnecting => ("◌ reconnecting", Style::default().fg(format::warning())),
            WsState::Closed => ("○ offline", Style::default().fg(format::down())),
        };
        (label.to_string(), style)
    };
    let ws_label = if fatal_reason.is_some()
        && format::display_width(&ws_label) > usize::from(width.saturating_sub(1))
    {
        format::pad_or_clip(&ws_label, usize::from(width.saturating_sub(1)))
    } else {
        ws_label
    };
    let ws_width = u16::try_from(format::display_width(&ws_label)).unwrap_or(u16::MAX);
    let Some(mut budget) = width.checked_sub(ws_width + 1) else {
        return Line::default();
    };

    let phase = format::market_phase(now);
    let phase_style = match phase {
        "open" => Style::default().fg(format::up()),
        "pre-open" => Style::default().fg(format::warning()),
        _ => format::dim(),
    };
    let mut fields = vec![
        (phase.to_string(), phase_style),
        (format::clock_ist(now), format::dim()),
    ];
    if let Some(available) = margin {
        fields.push((
            format!("{} available", format::inr(available)),
            format::dim(),
        ));
    }
    fields.push((format::uptime(uptime_secs), format::dim()));

    let sep_width = u16::try_from(format::display_width(STATUS_SEP)).unwrap_or(u16::MAX);
    let mut spans = vec![Span::styled(ws_label, ws_style)];
    for (text, style) in fields {
        let cost = sep_width + u16::try_from(format::display_width(&text)).unwrap_or(u16::MAX);
        if cost > budget {
            continue;
        }
        budget -= cost;
        spans.push(Span::styled(STATUS_SEP, format::dim()));
        spans.push(Span::styled(text, style));
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

fn active_label_style() -> Style {
    format::filled_style(format::accent()).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}

pub(super) fn draw_header(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = Vec::new();
    let mut x = area.x;
    for piece in strip_pieces(app.workspace(), area.width) {
        let width = piece.width();
        match piece {
            Piece::Brand => spans.push(Span::styled(BRAND, format::bold().fg(format::accent()))),
            Piece::Edge => spans.push(Span::raw(" ")),
            Piece::Gap => spans.push(Span::raw(TAB_GAP)),
            Piece::MoreLeft => spans.push(Span::styled(MORE_LEFT, format::dim())),
            Piece::MoreRight => spans.push(Span::styled(MORE_RIGHT, format::dim())),
            Piece::Tab {
                target,
                key,
                active,
            } => {
                let style = if active {
                    active_label_style()
                } else {
                    format::dim()
                };
                if let Some(key) = key {
                    spans.push(Span::styled(
                        key.to_string(),
                        style.add_modifier(Modifier::DIM),
                    ));
                    spans.push(Span::styled(" ", style));
                }
                spans.push(Span::styled(target.title(), style));
                app.hit.borrow_mut().push(
                    Rect {
                        x,
                        y: area.y,
                        width,
                        height: 1,
                    },
                    HitAction::Workspace(target),
                );
            }
        }
        x += width;
    }
    let drawn = x - area.x;

    let [strip_area, status_area] =
        Layout::horizontal([Constraint::Length(drawn), Constraint::Min(0)]).areas(area);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(bar_style()),
        strip_area,
    );

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    frame.render_widget(
        Paragraph::new(status_line(
            app.ws_state,
            app.ws_fatal.as_deref(),
            app.margin_available,
            now,
            app.started_at.elapsed().as_secs(),
            status_area.width.saturating_sub(1),
        ))
        .style(bar_style())
        .alignment(Alignment::Right),
        status_area,
    );
}

#[cfg(test)]
mod tests {
    use super::{bar_style, draw_header, pieces_width, status_line, strip_pieces};
    use crate::tui::app::App;
    use crate::tui::event::WsState;
    use crate::tui::format;
    use crate::tui::settings::{self, Theme};
    use crate::tui::ui::HitAction;
    use crate::tui::ui::chrome_tests::{FIXTURE_NOW, FIXTURE_UPTIME, root_buffer, row, test_app};
    use crate::tui::workspace::Preset;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Alignment, Rect};
    use ratatui::style::Modifier;
    use ratatui::widgets::{Clear, Paragraph};

    fn text(line: &ratatui::text::Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    fn strip(ws: WsState, margin: Option<f64>, width: u16) -> String {
        text(&status_line(
            ws,
            None,
            margin,
            FIXTURE_NOW,
            FIXTURE_UPTIME,
            width,
        ))
    }

    #[test]
    fn the_status_strip_shows_every_field_in_priority_order() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let strip = strip(WsState::Connected, Some(50_000.0), 64);
        assert_eq!(
            strip,
            "● live | open | 10:30:00 | ₹50,000 available | up 1m 20s "
        );
        assert_eq!(format::display_width(&strip), 57);
    }

    #[test]
    fn the_status_strip_keeps_the_connection_and_phase_at_eighty_columns() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let strip = strip(WsState::Connected, Some(50_000.0), 24);
        assert_eq!(strip, "● live | open ");
        assert_eq!(format::display_width(&strip), 14);
    }

    #[test]
    fn the_status_strip_skips_one_field_and_keeps_a_narrower_one() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let strip = strip(WsState::Connected, Some(50_000.0), 37);
        assert_eq!(strip, "● live | open | 10:30:00 | up 1m 20s ");
    }

    #[test]
    fn the_status_strip_omits_absent_funds_without_a_gap() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        assert_eq!(
            strip(WsState::Connected, None, 64),
            "● live | open | 10:30:00 | up 1m 20s "
        );
    }

    #[test]
    fn a_long_uptime_renders_whole_or_not_at_all() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let long = text(&status_line(
            WsState::Connected,
            None,
            None,
            FIXTURE_NOW,
            99 * 3600 + 59 * 60,
            64,
        ));
        assert!(long.ends_with("| up 99h 59m "), "{long:?}");
        let tight = text(&status_line(
            WsState::Connected,
            None,
            None,
            FIXTURE_NOW,
            99 * 3600 + 59 * 60,
            6 + 1 + 7 + 11 + 11,
        ));
        assert_eq!(tight, "● live | open | 10:30:00 ");
    }

    #[test]
    fn the_connection_label_is_whole_or_absent() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        for width in 0..=14u16 {
            assert_eq!(
                strip(WsState::Reconnecting, None, width),
                "",
                "width {width} drew a partial connection label"
            );
        }
        assert_eq!(strip(WsState::Reconnecting, None, 15), "◌ reconnecting ");
    }

    #[test]
    fn every_connection_state_has_its_own_label_and_colour() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let cases = [
            (WsState::Connected, false, "● live", format::up()),
            (
                WsState::Connecting,
                false,
                "◌ connecting",
                format::warning(),
            ),
            (
                WsState::Reconnecting,
                false,
                "◌ reconnecting",
                format::warning(),
            ),
            (WsState::Closed, false, "○ offline", format::down()),
            (WsState::Connected, true, "○ auth expired", format::down()),
        ];
        for (state, expired, label, colour) in cases {
            let line = status_line(
                state,
                expired.then_some(crate::tui::app::SESSION_EXPIRED),
                None,
                FIXTURE_NOW,
                FIXTURE_UPTIME,
                40,
            );
            let first = &line.spans[0];
            assert_eq!(first.content.as_ref(), label);
            assert_eq!(first.style.fg, Some(colour), "{label}");
        }
    }

    #[test]
    fn a_non_auth_fatal_close_shows_its_reason_in_the_badge() {
        let line = status_line(
            WsState::Closed,
            Some("ticker subscription limit exceeded"),
            None,
            FIXTURE_NOW,
            FIXTURE_UPTIME,
            100,
        );
        assert_eq!(
            line.spans[0].content,
            "○ ticker subscription limit exceeded"
        );
        assert_eq!(line.spans[0].style.fg, Some(format::down()));
    }

    #[test]
    fn a_long_fatal_badge_stays_visible_in_a_narrow_header() {
        let mut app = test_app();
        app.ws_state = WsState::Closed;
        app.ws_fatal = Some("ticker subscription limit exceeded".into());
        for width in [80, 100] {
            let header = header_canvas(&app, width);
            assert!(header.contains("○ ticker"), "width {width}: {header:?}");
            assert!(header.contains('…'), "width {width}: {header:?}");
        }
    }

    #[test]
    fn the_phase_keeps_its_own_colour_and_the_rest_is_muted() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let line = status_line(
            WsState::Connected,
            None,
            Some(1.0),
            FIXTURE_NOW,
            FIXTURE_UPTIME,
            64,
        );
        let phase = line
            .spans
            .iter()
            .find(|span| span.content.as_ref() == "open")
            .expect("the strip shows the phase");
        assert_eq!(phase.style.fg, Some(format::up()));
        let clock = line
            .spans
            .iter()
            .find(|span| span.content.as_ref() == "10:30:00")
            .expect("the strip shows the clock");
        assert_eq!(clock.style.fg, Some(format::text_muted()));
        for span in line.spans.iter().filter(|s| s.content.as_ref() == " | ") {
            assert_eq!(span.style.fg, Some(format::text_muted()));
        }
    }

    fn header_canvas(app: &App, width: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|frame| {
                draw_header(frame, frame.area(), app);
                let strip = pieces_width(&strip_pieces(app.workspace(), width));
                let status_area = Rect {
                    x: strip,
                    y: 0,
                    width: width - strip,
                    height: 1,
                };
                frame.render_widget(Clear, status_area);
                frame.render_widget(
                    Paragraph::new(status_line(
                        app.ws_state,
                        app.ws_fatal.as_deref(),
                        app.margin_available,
                        FIXTURE_NOW,
                        FIXTURE_UPTIME,
                        status_area.width - 1,
                    ))
                    .style(bar_style())
                    .alignment(Alignment::Right),
                    status_area,
                );
            })
            .unwrap();
        row(terminal.backend().buffer(), 0)
    }

    fn drawn_tabs(app: &App, width: u16) -> (Buffer, Vec<(Preset, Rect, String)>) {
        app.hit.borrow_mut().invalidate();
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|frame| draw_header(frame, frame.area(), app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let tabs = app
            .hit
            .borrow()
            .regions
            .iter()
            .filter_map(|region| match region.action {
                HitAction::Workspace(preset) => {
                    let text = (region.rect.x..region.rect.right())
                        .map(|x| buffer[(x, 0)].symbol())
                        .collect();
                    Some((preset, region.rect, text))
                }
                _ => None,
            })
            .collect();
        (buffer, tabs)
    }

    fn spellings(preset: Preset) -> Vec<String> {
        vec![
            preset.title().to_string(),
            format!("{} {}", preset.key(), preset.title()),
        ]
    }

    #[test]
    fn every_tab_is_whole_and_the_active_one_is_marked_at_every_width() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        for width in [160u16, 120, 80, 70, 40] {
            for active in Preset::ALL {
                app.set_workspace(active);
                let (buffer, tabs) = drawn_tabs(&app, width);
                let targets: Vec<Preset> = tabs.iter().map(|(preset, ..)| *preset).collect();
                if width >= 80 {
                    assert_eq!(targets, Preset::ALL, "width {width} lost a tab");
                } else {
                    let start = Preset::ALL
                        .iter()
                        .position(|preset| *preset == targets[0])
                        .unwrap();
                    assert_eq!(targets, Preset::ALL[start..start + targets.len()]);
                    assert!(targets.contains(&active), "{active:?} is off the strip");
                    assert!(targets.len() > 1, "width {width} kept no neighbour");
                }
                for (preset, rect, text) in &tabs {
                    assert!(
                        spellings(*preset).contains(text),
                        "width {width}: {preset:?} drew {text:?}"
                    );
                    for x in [rect.x - 1, rect.right()] {
                        let edge = buffer[(x, 0)].symbol();
                        assert!(
                            [" ", "‹", "›"].contains(&edge),
                            "width {width}: {text:?} runs into {edge:?}"
                        );
                    }
                    let cell = &buffer[(rect.x, 0)];
                    let marked = cell.bg == format::accent()
                        && cell
                            .modifier
                            .contains(Modifier::UNDERLINED | Modifier::BOLD);
                    assert_eq!(marked, *preset == active, "width {width}: {text:?}");
                    if !marked {
                        assert_eq!(cell.fg, format::text_muted());
                    }
                }
            }
        }
    }

    #[test]
    fn a_narrow_strip_marks_the_side_that_hides_tabs() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        let cases = [
            (Preset::Overview, false, true),
            (Preset::Portfolio, true, true),
            (Preset::Invest, true, false),
        ];
        for (active, left, right) in cases {
            app.set_workspace(active);
            let line = header_canvas(&app, 40);
            assert_eq!(line.contains('‹'), left, "{line:?}");
            assert_eq!(line.contains('›'), right, "{line:?}");
        }
    }

    #[test]
    fn the_header_row_matches_the_canvas_at_every_target_size() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        app.set_workspace(Preset::Trading);
        app.ws_state = WsState::Connected;
        app.margin_available = Some(50_000.0);
        let keyed =
            " kite  1 Overview  2 Trading  3 Portfolio  4 Option Chain  5 Orders  6 IPO & Bids";
        let row160 = header_canvas(&app, 160);
        assert!(row160.starts_with(keyed), "{row160:?}");
        assert!(row160.ends_with("● live | open | 10:30:00 | ₹50,000 available | up 1m 20s "));
        let row120 = header_canvas(&app, 120);
        assert!(row120.starts_with(keyed), "{row120:?}");
        assert!(
            row120.ends_with("● live | open | 10:30:00 | up 1m 20s "),
            "{row120:?}"
        );
        assert_eq!(
            header_canvas(&app, 80),
            " Overview  Trading  Portfolio  Option Chain  Orders  IPO & Bids   ● live | open "
        );
    }

    #[test]
    fn the_brand_is_dropped_after_the_keys_and_before_any_tab() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        app.set_workspace(Preset::Overview);
        for (ws, label) in [
            (WsState::Connected, "● live"),
            (WsState::Connecting, "◌ connecting"),
            (WsState::Reconnecting, "◌ reconnecting"),
            (WsState::Closed, "○ offline"),
        ] {
            app.ws_state = ws;
            let row = header_canvas(&app, 80);
            assert!(
                row.starts_with(" Overview  Trading  Portfolio  Option Chain  Orders  IPO & Bids"),
                "{row:?}"
            );
            assert!(!row.contains("kite"), "{row:?}");
            assert!(!row.contains('›'), "every tab still fits: {row:?}");
            assert!(row.contains(label), "the connection stays visible: {row:?}");
        }
    }

    #[test]
    fn every_connection_label_is_drawn_at_every_width_from_70_to_100() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        for active in Preset::ALL {
            app.set_workspace(active);
            for (ws, label) in [
                (WsState::Connected, "● live"),
                (WsState::Connecting, "◌ connecting"),
                (WsState::Reconnecting, "◌ reconnecting"),
                (WsState::Closed, "○ offline"),
            ] {
                app.ws_state = ws;
                for width in 70..=100 {
                    let header = row(&drawn_tabs(&app, width).0, 0);
                    assert!(header.contains(label), "{active:?} at {width}: {header:?}");
                }
            }
        }
    }

    #[test]
    fn the_strip_is_the_only_chrome_row_above_the_body() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        for (width, height) in [(80u16, 24u16), (120, 35), (160, 45)] {
            for active in [Preset::Trading, Preset::Orders, Preset::Invest] {
                app.set_workspace(active);
                let buffer = root_buffer(&app, width, height);
                let header = row(&buffer, 0);
                assert!(header.contains("Overview"), "{width}x{height}: {header:?}");
                assert!(
                    header.contains("IPO & Bids"),
                    "{width}x{height}: {header:?}"
                );
                let body = row(&buffer, 1);
                assert!(
                    body.starts_with('┌'),
                    "{width}x{height} {active:?}: row 1 is {body:?}"
                );
            }
        }
    }
}
