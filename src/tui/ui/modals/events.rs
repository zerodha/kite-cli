use super::layout::{
    DETAIL_ROWS, SELECT_MARKER, UTILITY_BODY_ROWS, UTILITY_TOO_SMALL, draw_body_line, draw_detail,
    draw_select_list, list_rows,
};
use crate::tui::app::{App, EventLogState, ToastLevel};
use crate::tui::format;
use crate::tui::ui::forms;
use crate::tui::ui::{HitList, always, nav};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

const EVENT_TIME_WIDTH: u16 = 8;
const EVENT_LEVEL_WIDTH: u16 = 7;
const EVENT_MESSAGE_COLUMN: u16 = 2 + EVENT_TIME_WIDTH + 1 + EVENT_LEVEL_WIDTH + 1;

fn event_log_line(unix: u64, level: ToastLevel, text: &str, message: usize) -> Line<'static> {
    let (label, style) = match level {
        ToastLevel::Info => ("INFO", Style::default().fg(format::info())),
        ToastLevel::Success => ("SUCCESS", Style::default().fg(format::up())),
        ToastLevel::Error => (
            "ERROR",
            Style::default()
                .fg(format::error())
                .add_modifier(Modifier::BOLD),
        ),
    };
    Line::from(vec![
        Span::styled(format!("{} ", format::clock_ist(unix)), format::dim()),
        Span::styled(
            format!(
                "{} ",
                format::pad_or_clip(label, usize::from(EVENT_LEVEL_WIDTH))
            ),
            style,
        ),
        Span::raw(format::pad_or_clip(text, message)),
    ])
}

fn event_log_header(content: u16) -> Line<'static> {
    Line::from(Span::styled(
        format::pad_or_clip(
            &format!(
                "  {} {} MESSAGE",
                format::pad_or_clip("TIME", usize::from(EVENT_TIME_WIDTH)),
                format::pad_or_clip("LEVEL", usize::from(EVENT_LEVEL_WIDTH))
            ),
            usize::from(content),
        ),
        format::dim(),
    ))
}

pub(in crate::tui::ui) fn event_log_footer() -> &'static forms::FormFooter {
    const {
        &forms::FormFooter {
            help: &[
                "Up/Down or j/k Select",
                "g/G First/Last",
                "PgUp/PgDn Detail",
            ],
            actions: &[forms::FormAction {
                key: "Esc/q",
                label: "Close",
                kind: forms::ActionKind::Secondary,
            }],
            hints: &[
                nav(&["j/k:select", "g/G:first/last", "PgUp/PgDn:detail"]),
                always(&["Esc/q:close"]),
            ],
        }
    }
}

pub(super) fn draw_event_log(frame: &mut Frame, state: &EventLogState, app: &App) {
    let area = frame.area();
    let toasts = app.toasts();
    let layout = forms::ModalLayout::measure(area, UTILITY_BODY_ROWS);
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &format!("Events / {}", toasts.len()),
        Style::default().fg(format::accent()),
        "Newest first | times in IST | session history",
        UTILITY_TOO_SMALL,
    ) else {
        return;
    };
    let body = layout.body;
    draw_body_line(frame, body, 0, event_log_header(layout.content));
    if toasts.is_empty() {
        draw_body_line(
            frame,
            body,
            1,
            Line::from(Span::styled("No events yet.", format::dim())),
        );
        forms::draw_footer(frame, app, &layout, event_log_footer());
        return;
    }

    let (rows, detail) = list_rows(body);
    let message = usize::from(layout.content.saturating_sub(EVENT_MESSAGE_COLUMN));
    let selected = state.selected.min(toasts.len() - 1);
    if rows > 0 {
        let items: Vec<ListItem> = toasts
            .iter()
            .rev()
            .map(|toast| {
                ListItem::new(event_log_line(
                    toast.unix,
                    toast.level,
                    &toast.text,
                    message,
                ))
            })
            .collect();
        let list_area = Rect {
            y: body.y + 1,
            height: rows,
            ..body
        };
        draw_select_list(
            frame,
            app,
            list_area,
            items,
            Some(selected),
            HitList::EventLog,
            SELECT_MARKER,
        );
    }
    if detail > 0 {
        let toast = &toasts[toasts.len() - 1 - selected];
        let wrapped = forms::wrap_cells(&toast.text, usize::from(layout.content));
        let scroll = state.detail.resolve(
            u16::try_from(wrapped.len()).unwrap_or(u16::MAX),
            DETAIL_ROWS - 1,
            None,
        );
        draw_detail(
            frame,
            body,
            layout.content,
            "Selected event",
            &wrapped[usize::from(scroll).min(wrapped.len())..],
        );
    }
    forms::draw_footer(frame, app, &layout, event_log_footer());
}

#[cfg(test)]
mod event_log_tests {
    use super::event_log_line;
    use crate::tui::app::ToastLevel;

    #[test]
    fn formats_timestamp_and_text() {
        let line = event_log_line(25496, ToastLevel::Info, "order update: INFY complete", 30);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "12:34:56 INFO    order update: INFY complete   ");
        let line = event_log_line(25496, ToastLevel::Error, "rejected", 8);
        let rendered: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(rendered, "12:34:56 ERROR   rejected");
    }
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{
        assert_mockup, event_log_app, modal_ascii, modal_rect, rect_tuple, rendered_text, test_app,
    };
    use super::draw_event_log;
    use crate::tui::app::{EventLogState, ToastLevel};

    #[test]
    fn event_log_matches_the_spec_mockup() {
        let app = event_log_app();
        let state = EventLogState::default();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_event_log(frame, &state, &app)),
            "
+- Events / 3 ---------------------------------------------------------+
| Newest first | times in IST | session history                        |
+----------------------------------------------------------------------+
|   TIME     LEVEL   MESSAGE                                           |
| > 09:32:10 ERROR   Order rejected: insufficient margin               |
|   09:31:20 SUCCESS Order update: INFY complete                       |
|   09:30:00 INFO    Connected to Kite                                 |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
| Selected event                                                       |
| Order rejected: insufficient margin                                  |
|                                                                      |
+----------------------------------------------------------------------+
| Up/Down or j/k Select  g/G First/Last  PgUp/PgDn Detail              |
| [Esc/q Close]                                                        |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 1, 72, 22));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_event_log(frame, &state, &app)),
            "
+- Events / 3 -------------------------------------------------------------------------+
| Newest first | times in IST | session history                                        |
+--------------------------------------------------------------------------------------+
|   TIME     LEVEL   MESSAGE                                                           |
| > 09:32:10 ERROR   Order rejected: insufficient margin                               |
|   09:31:20 SUCCESS Order update: INFY complete                                       |
|   09:30:00 INFO    Connected to Kite                                                 |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
| Selected event                                                                       |
| Order rejected: insufficient margin                                                  |
|                                                                                      |
+--------------------------------------------------------------------------------------+
| Up/Down or j/k Select  g/G First/Last  PgUp/PgDn Detail                              |
| [Esc/q Close]                                                                        |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 3, 88, 29));
    }

    #[test]
    fn an_empty_event_log_keeps_its_frame_and_footer() {
        let app = test_app();
        let state = EventLogState::default();
        let text = rendered_text(80, 24, |frame| draw_event_log(frame, &state, &app));
        assert!(text.contains("Events / 0"), "{text}");
        assert!(text.contains("No events yet."), "{text}");
        assert!(text.contains("[Esc/q Close]"), "{text}");
        assert!(
            !text.contains("Selected event"),
            "there is no selection to detail:\n{text}"
        );
    }

    #[test]
    fn the_selected_event_detail_scrolls_without_moving() {
        use std::fmt::Write;
        let mut app = test_app();

        let mut long = String::new();
        for n in 0..40 {
            write!(long, "clause{n} ").unwrap();
        }
        app.push_event(14400, ToastLevel::Error, long.trim());
        let state = EventLogState::default();
        let first = rendered_text(80, 24, |frame| draw_event_log(frame, &state, &app));
        state.detail.page(1);
        let paged = rendered_text(80, 24, |frame| draw_event_log(frame, &state, &app));
        assert_ne!(first, paged, "PgDn must move the detail");
        for text in [&first, &paged] {
            let rows: Vec<&str> = text.lines().collect();
            assert!(
                rows[16].contains("Selected event"),
                "the label moved:\n{text}"
            );
        }
    }
}
