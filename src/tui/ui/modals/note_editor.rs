use super::layout::{UTILITY_BODY_ROWS, UTILITY_TOO_SMALL, body_row, draw_body_line, section_line};
use super::notes::draw_blocked_warning;
use crate::tui::app::App;
use crate::tui::format;
use crate::tui::notes::{self, NoteBuffer, NoteEditor};
use crate::tui::ui::forms;
use crate::tui::ui::{HitAction, always, nav};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

fn note_body_lines(
    buffer: &NoteBuffer,
    width: u16,
    height: u16,
    scroll: usize,
) -> Vec<Line<'static>> {
    let width = width.max(1) as usize;
    let (line, col) = buffer.cursor();
    let rows = notes::wrap(buffer.lines(), width);
    let (caret_row, caret_col) = notes::cursor_row(buffer.lines(), width, line, col);
    let caret_style = format::filled_style(format::accent());

    rows.iter()
        .enumerate()
        .skip(scroll)
        .take(height.max(1) as usize)
        .map(|(idx, row)| {
            let text: Vec<char> = buffer.lines()[row.line]
                .chars()
                .skip(row.start)
                .take(row.end - row.start)
                .collect();
            if idx != caret_row {
                return Line::from(Span::raw(text.into_iter().collect::<String>()));
            }
            let before: String = text.iter().take(caret_col).collect();
            let remaining: String = text.iter().skip(caret_col).collect();
            let span = Span::raw(remaining.as_str());
            let at = span
                .styled_graphemes(Style::default())
                .next()
                .map_or(" ", |grapheme| grapheme.symbol);
            let after = remaining.get(at.len()..).unwrap_or_default();
            Line::from(vec![
                Span::raw(before),
                Span::styled(at.to_string(), caret_style),
                Span::raw(after.to_string()),
            ])
        })
        .collect()
}

pub(in crate::tui::ui) fn note_editor_footer(blocked: bool) -> &'static forms::FormFooter {
    const HELP: &[&str] = &["Arrows Move", "Home/End Line", "Enter New line"];
    if blocked {
        return const {
            &forms::FormFooter {
                help: HELP,
                actions: &[
                    forms::FormAction {
                        key: "Esc",
                        label: "Back (cannot save)",
                        kind: forms::ActionKind::Secondary,
                    },
                    forms::FormAction {
                        key: "",
                        label: "Nothing typed here reaches disk",
                        kind: forms::ActionKind::Note,
                    },
                ],
                hints: &[
                    nav(&["type to write", "←→↑↓/Home/End:move", "Enter:newline"]),
                    always(&["Esc:back (cannot save)"]),
                ],
            }
        };
    }
    const {
        &forms::FormFooter {
            help: HELP,
            actions: &[
                forms::FormAction {
                    key: "Esc",
                    label: "Save & back",
                    kind: forms::ActionKind::Primary,
                },
                forms::FormAction {
                    key: "",
                    label: "Empty body discards the note",
                    kind: forms::ActionKind::Note,
                },
            ],
            hints: &[
                nav(&["type to write", "←→↑↓/Home/End:move", "Enter:newline"]),
                always(&["Esc:save & back"]),
            ],
        }
    }
}

pub(super) fn draw_note_editor(frame: &mut Frame, editor: &NoteEditor, blocked: bool, app: &App) {
    let area = frame.area();
    let layout = forms::ModalLayout::measure(area, UTILITY_BODY_ROWS);
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &notes::editor_title(&editor.label()),
        Style::default().fg(format::accent()),
        &notes::editor_subtitle(editor.stamps),
        UTILITY_TOO_SMALL,
    ) else {
        return;
    };
    let footer = note_editor_footer(blocked);
    let mut body = layout.body;
    if blocked {
        let taken = draw_blocked_warning(frame, body, layout.content);
        body = Rect {
            y: body.y + taken.min(body.height),
            height: body.height.saturating_sub(taken),
            ..body
        };
    }
    draw_body_line(frame, body, 0, section_line("Body", layout.content));

    let Some(text_area) = body_row(body, 1).map(|row| Rect {
        height: body.height - 1,
        ..row
    }) else {
        forms::draw_footer(frame, app, &layout, footer);
        return;
    };
    let scroll = notes::editor_scroll(&editor.buffer, text_area.width, text_area.height);
    frame.render_widget(
        Paragraph::new(note_body_lines(
            &editor.buffer,
            text_area.width,
            text_area.height,
            scroll,
        )),
        text_area,
    );
    app.hit
        .borrow_mut()
        .push(text_area, HitAction::NoteCaret { scroll });
    forms::draw_footer(frame, app, &layout, footer);
}

#[cfg(test)]
mod render_tests {
    use super::super::draw_notes;
    use super::super::render_tests::{
        assert_mockup, modal_ascii, modal_rect, notes_app, rect_tuple, rendered_text, test_app,
    };
    use super::note_body_lines;
    use crate::tui::notes::{NoteBuffer, NoteEditor, NotesState, Pad};
    use crate::tui::ui::HitAction;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn scroll_of(buffer: &NoteBuffer, width: u16, height: u16) -> usize {
        crate::tui::notes::editor_scroll(buffer, width, height)
    }

    fn text_of(line: &ratatui::text::Line<'_>) -> String {
        line.spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect()
    }

    #[test]
    fn note_body_wraps_at_the_box_width_and_marks_the_cursor() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let buffer = NoteBuffer::from_text(&"a".repeat(25));
        let lines = note_body_lines(&buffer, 10, 5, scroll_of(&buffer, 10, 5));
        assert_eq!(lines.len(), 3);
        assert_eq!(text_of(&lines[0]).chars().count(), 10);
        assert_eq!(text_of(&lines[1]).chars().count(), 10);
        assert_eq!(text_of(&lines[2]), "aaaaa ");
        let caret = &lines[2].spans[1];
        assert_eq!(caret.content.as_ref(), " ");
        assert_eq!(caret.style.bg, Some(crate::tui::format::accent()));
    }

    #[test]
    fn note_body_scrolls_to_keep_the_cursor_visible() {
        let buffer = NoteBuffer::from_text("l1\nl2\nl3\nl4\nl5");
        let lines = note_body_lines(&buffer, 10, 2, scroll_of(&buffer, 10, 2));
        assert_eq!(lines.len(), 2);
        assert_eq!(text_of(&lines[0]), "l4");
        assert_eq!(text_of(&lines[1]), "l5 ");
    }

    #[test]
    fn note_body_caret_lands_on_a_whole_multibyte_char() {
        let mut buffer = NoteBuffer::from_text("₹1520");
        buffer.home();
        buffer.right();
        let lines = note_body_lines(&buffer, 20, 3, scroll_of(&buffer, 20, 3));
        assert_eq!(text_of(&lines[0]), "₹1520");
        assert_eq!(lines[0].spans[0].content.as_ref(), "₹");
        assert_eq!(lines[0].spans[1].content.as_ref(), "1");
        assert_eq!(lines[0].spans[2].content.as_ref(), "520");
    }

    #[test]
    fn note_editor_titles_the_attached_instrument_and_footers_its_stamps() {
        let mut notes = Pad::default();
        let id = notes
            .upsert(
                None,
                "why I am long",
                Some(crate::tui::notes::NoteInstrument {
                    exchange: "NSE".to_string(),
                    tradingsymbol: "INFY".to_string(),
                }),
                0,
            )
            .unwrap();
        let state = NotesState {
            editor: Some(NoteEditor::editing(notes.get(id).unwrap())),
            ..NotesState::default()
        };
        let app = test_app();
        let text = rendered_text(100, 26, |frame| draw_notes(frame, &state, &app));
        assert!(text.contains("Note / NSE:INFY"), "title missing:\n{text}");
        assert!(text.contains("why I am long"), "body missing:\n{text}");
        assert!(
            text.contains("Written 1970-01-01 05:30 | edited 1970-01-01 05:30"),
            "both timestamps must show:\n{text}"
        );
        let general = NotesState {
            editor: Some(NoteEditor::draft(None)),
            ..NotesState::default()
        };
        let text = rendered_text(100, 26, |frame| draw_notes(frame, &general, &app));
        assert!(text.contains("Note / General"), "title missing:\n{text}");
        assert!(
            text.contains("New note | nothing on disk yet"),
            "a draft must say it has no history:\n{text}"
        );
    }

    #[test]
    fn note_editor_matches_the_spec_mockup() {
        let app = notes_app();
        let note = app.notes.get(1).expect("the fixture note");
        let mut editor = NoteEditor::editing(note);
        editor.buffer = NoteBuffer::from_text(
            "Watch the opening range.\n\nKeep position size within the session plan.",
        );
        editor.buffer.up();
        editor.buffer.up();
        editor.buffer.end();
        let state = NotesState {
            editor: Some(editor),
            ..NotesState::default()
        };
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_notes(frame, &state, &app)),
            "
+- Note / NSE:INFY ----------------------------------------------------+
| Written 2026-09-10 09:30 | edited 2026-09-10 09:31                   |
+----------------------------------------------------------------------+
| Body                                                                 |
| Watch the opening range.                                             |
|                                                                      |
| Keep position size within the session plan.                          |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
+----------------------------------------------------------------------+
| Arrows Move  Home/End Line  Enter New line                           |
| [Esc Save & back]  Empty body discards the note                      |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 1, 72, 22));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_notes(frame, &state, &app)),
            "
+- Note / NSE:INFY --------------------------------------------------------------------+
| Written 2026-09-10 09:30 | edited 2026-09-10 09:31                                   |
+--------------------------------------------------------------------------------------+
| Body                                                                                 |
| Watch the opening range.                                                             |
|                                                                                      |
| Keep position size within the session plan.                                          |
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
|                                                                                      |
|                                                                                      |
|                                                                                      |
+--------------------------------------------------------------------------------------+
| Arrows Move  Home/End Line  Enter New line                                           |
| [Esc Save & back]  Empty body discards the note                                      |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 3, 88, 29));
    }

    #[test]
    fn wide_glyphs_wrap_by_cells_so_the_tail_and_caret_stay_visible() {
        let _guard = crate::tui::settings::theme_guard(crate::tui::settings::Theme::DEFAULT);
        let app = notes_app();
        let note = app.notes.get(1).expect("the fixture note");
        let mut editor = NoteEditor::editing(note);
        editor.buffer = NoteBuffer::from_text(&format!("{}TAIL", "漢".repeat(40)));
        let state = NotesState {
            editor: Some(editor),
            ..NotesState::default()
        };
        for (width, height) in [(80, 24), (120, 35), (160, 45)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| draw_notes(frame, &state, &app))
                .unwrap();
            let buffer = terminal.backend().buffer().clone();
            let (text_area, scroll) = app
                .hit
                .borrow()
                .regions
                .iter()
                .rev()
                .find_map(|region| match region.action {
                    HitAction::NoteCaret { scroll } => Some((region.rect, scroll)),
                    _ => None,
                })
                .expect("the editor records a caret target");
            let (tail_x, y) = (text_area.y..text_area.bottom())
                .flat_map(|y| (text_area.x..text_area.right()).map(move |x| (x, y)))
                .find(|&(x, y)| {
                    x + 4 <= text_area.right()
                        && (0..4).all(|i| buffer[(x + i, y)].symbol() == &"TAIL"[i as usize..][..1])
                })
                .unwrap_or_else(|| panic!("{width}x{height}: the tail is hidden"));
            let row = usize::from(y - text_area.y);
            let caret = if tail_x + 4 < text_area.right() {
                (tail_x + 4, y)
            } else {
                (text_area.x, y + 1)
            };
            assert_eq!(
                buffer[caret].style().bg,
                Some(crate::tui::format::accent()),
                "{width}x{height}: the caret after the tail must be drawn"
            );
            let mut clicked = state.editor.as_ref().unwrap().buffer.clone();
            clicked.click(
                usize::from(text_area.width),
                scroll + row,
                usize::from(tail_x - text_area.x + 1),
            );
            assert_eq!(clicked.cursor(), (0, 41), "{width}x{height}: click on A");
        }
    }

    #[test]
    fn the_note_caret_and_its_click_target_agree() {
        let app = notes_app();
        let note = app.notes.get(1).expect("the fixture note");
        let state = NotesState {
            editor: Some(NoteEditor::editing(note)),
            ..NotesState::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| draw_notes(frame, &state, &app))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();
        let caret = &buffer[(6 + 24, 1 + 4)];
        assert_ne!(
            caret.style().bg,
            buffer[(6, 1 + 4)].style().bg,
            "the caret cell must be filled"
        );
        let target = app
            .hit
            .borrow()
            .regions
            .iter()
            .rev()
            .find_map(|region| match region.action {
                HitAction::NoteCaret { scroll } => Some((region.rect, scroll)),
                _ => None,
            })
            .expect("the editor records a caret target");
        assert_eq!(target.1, 0, "a short note does not scroll");
        assert_eq!((target.0.x, target.0.y), (6, 5));
        assert_eq!(target.0.height, 14, "the text owns every row below Body");
    }
}
