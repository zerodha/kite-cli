use super::bar_style;
use super::text::{mark_clipped, wrap_prose};
use crate::tui::app::{App, ToastLevel};
use crate::tui::format;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

pub(super) const MAX_TOAST_ROWS: usize = 3;

const TOAST_PREFIX_WIDTH: u16 = 8;

const TOAST_LOG_WIDTH: u16 = 12;

const TOAST_LOG_HINT: &str = " Ctrl-L log ";

const TOAST_MIN_BODY: u16 = 4;

pub(super) struct ToastDisplay {
    prefix: &'static str,
    prefix_style: Style,
    prefix_width: u16,
    pub(super) rows: Vec<String>,
    body_width: u16,
    log_column: bool,
}

pub(super) fn toast_display(app: &App, width: u16) -> Option<ToastDisplay> {
    let toast = app.visible_toast()?;
    let log_column = width >= TOAST_LOG_WIDTH + TOAST_PREFIX_WIDTH + TOAST_MIN_BODY;
    let right = if log_column { TOAST_LOG_WIDTH } else { 0 };
    let prefix_width = if width.saturating_sub(right + TOAST_PREFIX_WIDTH) >= TOAST_MIN_BODY {
        TOAST_PREFIX_WIDTH
    } else {
        2
    };
    let body_width = width.saturating_sub(right + prefix_width);
    if body_width == 0 {
        return None;
    }
    let (long, short, tone) = match toast.level {
        ToastLevel::Info => (" [INFO] ", "i ", format::info()),
        ToastLevel::Success => (" [ OK ] ", "+ ", format::up()),
        ToastLevel::Error => (" [ERR ] ", "! ", format::error()),
    };
    let mut rows = wrap_prose(&toast.text, body_width as usize);
    if rows.len() > MAX_TOAST_ROWS {
        rows.truncate(MAX_TOAST_ROWS);
        let last = rows.len() - 1;
        rows[last] = mark_clipped(&rows[last], body_width as usize);
    }
    Some(ToastDisplay {
        prefix: if prefix_width == TOAST_PREFIX_WIDTH {
            long
        } else {
            short
        },
        prefix_style: Style::default().fg(tone).add_modifier(Modifier::BOLD),
        prefix_width,
        rows,
        body_width,
        log_column,
    })
}

pub(super) fn draw_toast(frame: &mut Frame, area: Rect, display: Option<&ToastDisplay>) {
    let Some(display) = display else {
        return;
    };
    if area.width == 0 || area.height == 0 {
        return;
    }
    let blank_prefix = " ".repeat(display.prefix_width as usize);
    let lines: Vec<Line<'static>> = display
        .rows
        .iter()
        .take(area.height as usize)
        .enumerate()
        .map(|(idx, row)| {
            let mut spans = vec![if idx == 0 {
                Span::styled(display.prefix, display.prefix_style)
            } else {
                Span::raw(blank_prefix.clone())
            }];
            spans.push(Span::raw(format::pad_or_clip(
                row,
                display.body_width as usize,
            )));
            if idx == 0 && display.log_column {
                spans.push(Span::styled(TOAST_LOG_HINT, format::dim()));
            }
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines).style(bar_style()), area);
}

#[cfg(test)]
mod tests {
    use super::{MAX_TOAST_ROWS, TOAST_LOG_HINT, toast_display};
    use crate::tui::app::ToastLevel;
    use crate::tui::format;
    use crate::tui::settings::{self, Theme};
    use crate::tui::ui::chrome_tests::{root_buffer, row, test_app};
    use ratatui::style::Modifier;

    #[test]
    fn measured_toast_rows_equal_the_rendered_rows() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let long = "A very long Kite rejection: ₹1,00,000 のマージンが不足しています and the order was not placed at all today.";
        for width in [24u16, 40, 80, 120] {
            let mut app = test_app();
            app.toast(ToastLevel::Error, long);
            let measured = toast_display(&app, width).expect("a toast is showing").rows;
            let buffer = root_buffer(&app, width, 24);
            let top = 24 - 1 - u16::try_from(measured.len()).unwrap();
            let painted: Vec<String> = (0..u16::try_from(measured.len()).unwrap())
                .map(|idx| row(&buffer, top + idx))
                .collect();
            assert_eq!(
                painted.len(),
                measured.len(),
                "width {width} painted a different number of rows"
            );
            for (idx, (measured_row, painted_row)) in measured.iter().zip(&painted).enumerate() {
                assert!(
                    painted_row.contains(measured_row.trim_end()),
                    "width {width} row {idx}: {painted_row:?} is not {measured_row:?}"
                );
            }
        }
    }

    #[test]
    fn each_toast_tone_has_its_own_prefix_and_colour() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let cases = [
            (ToastLevel::Info, " [INFO] ", format::info()),
            (ToastLevel::Success, " [ OK ] ", format::up()),
            (ToastLevel::Error, " [ERR ] ", format::error()),
        ];
        for (level, prefix, colour) in cases {
            let mut app = test_app();
            app.toast(level, "done");
            let display = toast_display(&app, 80).expect("a toast is showing");
            assert_eq!(display.prefix, prefix);
            assert_eq!(display.prefix_style.fg, Some(colour));
            assert!(display.prefix_style.add_modifier.contains(Modifier::BOLD));
            let buffer = root_buffer(&app, 80, 10);
            assert_eq!(&row(&buffer, 8)[..8], prefix);
            assert_eq!(buffer[(8, 8)].fg, ratatui::style::Color::Reset);
        }
    }

    #[test]
    fn a_one_line_toast_occupies_only_the_row_above_the_hints() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        app.toast(ToastLevel::Error, "order rejected");
        let buffer = root_buffer(&app, 80, 24);
        assert_eq!(
            row(&buffer, 22),
            " [ERR ] order rejected                                               Ctrl-L log "
        );
        assert!(!row(&buffer, 21).contains("order rejected"));
    }

    #[test]
    fn explicit_newlines_start_new_toast_rows() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        app.toast(ToastLevel::Info, "first\nsecond");
        let display = toast_display(&app, 80).expect("a toast is showing");
        assert_eq!(
            display.rows,
            vec!["first".to_string(), "second".to_string()]
        );
    }

    #[test]
    fn an_overlong_toast_shows_three_marked_rows_and_keeps_its_text() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let full = "one\ntwo\nthree\nfour\nfive";
        let mut app = test_app();
        app.toast(ToastLevel::Error, full);
        let display = toast_display(&app, 80).expect("a toast is showing");
        assert_eq!(display.rows.len(), MAX_TOAST_ROWS);
        assert_eq!(&display.rows[..2], ["one".to_string(), "two".to_string()]);
        let marked = &display.rows[2];
        assert!(marked.starts_with("three"), "{marked:?}");
        assert!(marked.ends_with("..."), "{marked:?}");
        assert_eq!(format::display_width(marked), 60);
        assert_eq!(
            app.toasts().back().map(|toast| toast.text.as_str()),
            Some(full)
        );
    }

    #[test]
    fn a_newer_toast_replaces_the_visible_one() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        app.toast(ToastLevel::Info, "first");
        app.toast(ToastLevel::Error, "second");
        let display = toast_display(&app, 80).expect("a toast is showing");
        assert_eq!(display.rows[0], "second");
        assert_eq!(display.prefix, " [ERR ] ");
        assert_eq!(app.toasts().len(), 2);
    }

    #[test]
    fn no_toast_reserves_no_rows() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let app = test_app();
        assert!(toast_display(&app, 80).is_none());
        let buffer = root_buffer(&app, 80, 24);
        assert!(!row(&buffer, 22).contains(TOAST_LOG_HINT.trim()));
    }

    #[test]
    fn a_narrow_toast_drops_the_log_column_then_the_prefix() {
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let mut app = test_app();
        app.toast(ToastLevel::Error, "rejected");
        let wide = toast_display(&app, 24).expect("a toast is showing");
        assert!(wide.log_column);
        assert_eq!(wide.prefix, " [ERR ] ");
        assert_eq!(wide.body_width, 4);

        let narrow = toast_display(&app, 23).expect("a toast is showing");
        assert!(!narrow.log_column);
        assert_eq!(narrow.prefix, " [ERR ] ");
        assert_eq!(narrow.body_width, 15);

        let tiny = toast_display(&app, 11).expect("a toast is showing");
        assert!(!tiny.log_column);
        assert_eq!(tiny.prefix, "! ");
        assert_eq!(tiny.body_width, 9);

        for width in 0..=2u16 {
            assert!(toast_display(&app, width).is_none(), "width {width}");
        }
    }
}
