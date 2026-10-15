use super::layout::{
    SELECT_MARKER, UTILITY_BODY_ROWS, UTILITY_TOO_SMALL, draw_body_line, draw_detail,
    draw_select_list, list_rows,
};
use super::note_editor::draw_note_editor;
use crate::tui::app::App;
use crate::tui::format;
use crate::tui::notes::{self, NotesState};
use crate::tui::ui::columns;
use crate::tui::ui::forms;
use crate::tui::ui::{HintGroup, HitList, act, always, nav};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

pub(super) fn draw_notes(frame: &mut Frame, state: &NotesState, app: &App) {
    let blocked = app.notes.blocked();
    match &state.editor {
        Some(editor) => draw_note_editor(frame, editor, blocked, app),
        None => draw_note_list(frame, state, app, blocked),
    }
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "fixed note timestamp and instrument columns fit u16"
)]
const NOTE_PREVIEW_COLUMN: u16 =
    columns::SELECT_GUTTER + notes::STAMP_WIDTH as u16 + 1 + notes::INSTRUMENT_WIDTH as u16 + 1;

fn note_list_header(content: u16) -> Line<'static> {
    Line::from(Span::styled(
        format::pad_or_clip(
            &format!(
                "  {} {} NOTE",
                format::pad_or_clip("WRITTEN", notes::STAMP_WIDTH),
                format::pad_or_clip("INSTRUMENT", notes::INSTRUMENT_WIDTH)
            ),
            usize::from(content),
        ),
        format::dim(),
    ))
}

pub(in crate::tui::ui) fn notes_footer(blocked: bool, attach: bool) -> &'static forms::FormFooter {
    const EDIT: forms::FormAction = forms::FormAction {
        key: "e/Enter",
        label: "Edit",
        kind: forms::ActionKind::Primary,
    };
    const CLOSE: forms::FormAction = forms::FormAction {
        key: "Esc/q",
        label: "Close",
        kind: forms::ActionKind::Secondary,
    };
    const DELETE: forms::FormAction = forms::FormAction {
        key: "d",
        label: "Delete",
        kind: forms::ActionKind::Secondary,
    };
    const HINTS: &[HintGroup] = &[
        act(&["e/Enter:edit", "n:new", "d:delete", "N:new general"]),
        nav(&["j/k:select"]),
        always(&["Esc/q:close"]),
    ];
    if blocked {
        return const {
            &forms::FormFooter {
                help: &["Up/Down or j/k Select", "Saving is refused"],
                actions: &[
                    EDIT,
                    forms::FormAction {
                        key: "d",
                        label: "Delete",
                        kind: forms::ActionKind::Disabled,
                    },
                    CLOSE,
                ],
                hints: &[
                    act(&["e/Enter:edit"]),
                    nav(&["j/k:select"]),
                    always(&["Esc/q:close"]),
                ],
            }
        };
    }
    if attach {
        return const {
            &forms::FormFooter {
                help: &["Up/Down or j/k Select", "n Attached", "N General"],
                actions: &[EDIT, DELETE, CLOSE],
                hints: HINTS,
            }
        };
    }
    const {
        &forms::FormFooter {
            help: &["Up/Down or j/k Select", "n/N General"],
            actions: &[EDIT, DELETE, CLOSE],
            hints: HINTS,
        }
    }
}

pub(super) fn draw_blocked_warning(frame: &mut Frame, body: Rect, content: u16) -> u16 {
    let rows = forms::wrap_cells(notes::BLOCKED_WARNING, usize::from(content));
    for (offset, text) in rows.iter().enumerate() {
        draw_body_line(
            frame,
            body,
            u16::try_from(offset).unwrap_or(u16::MAX),
            Line::from(Span::styled(
                text.clone(),
                Style::default().fg(format::error()),
            )),
        );
    }
    u16::try_from(rows.len()).unwrap_or(u16::MAX) + 1
}

fn draw_note_list(frame: &mut Frame, state: &NotesState, app: &App, blocked: bool) {
    let area = frame.area();
    let attach = state
        .attach
        .as_ref()
        .map(crate::tui::notes::NoteInstrument::label);
    let layout = forms::ModalLayout::measure(area, UTILITY_BODY_ROWS);
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &notes::list_title(app.notes.len()),
        Style::default().fg(format::accent()),
        &notes::list_subtitle(blocked, attach.as_deref()),
        UTILITY_TOO_SMALL,
    ) else {
        return;
    };
    let footer = notes_footer(blocked, attach.is_some());
    let mut body = layout.body;
    if blocked {
        let taken = draw_blocked_warning(frame, body, layout.content);
        body = Rect {
            y: body.y + taken.min(body.height),
            height: body.height.saturating_sub(taken),
            ..body
        };
    }
    draw_body_line(frame, body, 0, note_list_header(layout.content));

    let ordered = app.notes.newest_first();
    if ordered.is_empty() {
        draw_body_line(
            frame,
            body,
            1,
            Line::from(Span::styled(
                "No notes yet. Press n to write one.",
                format::dim(),
            )),
        );
        forms::draw_footer(frame, app, &layout, footer);
        return;
    }

    let preview_width = usize::from(layout.content.saturating_sub(NOTE_PREVIEW_COLUMN));
    let (rows, detail) = list_rows(body);
    let selected = state.selected.min(ordered.len() - 1);
    if rows > 0 {
        let items: Vec<ListItem> = ordered
            .iter()
            .map(|note| {
                let [when, instrument, preview] = notes::row_columns(note, preview_width);
                ListItem::new(Line::from(vec![
                    Span::styled(when, format::dim()),
                    Span::raw(" "),
                    Span::styled(instrument, Style::default().fg(format::accent())),
                    Span::raw(" "),
                    Span::raw(preview),
                ]))
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
            HitList::Notes,
            SELECT_MARKER,
        );
    }
    if detail > 0 {
        let wrapped = forms::wrap_cells(&ordered[selected].body, usize::from(layout.content));
        draw_detail(frame, body, layout.content, "Selected note", &wrapped);
    }
    forms::draw_footer(frame, app, &layout, footer);
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{
        assert_mockup, modal_ascii, modal_rect, notes_app, notes_list_state, rect_tuple,
        rendered_text, test_app,
    };
    use super::notes_footer;
    use crate::tui::notes::{NoteEditor, NotesState, Pad};
    use crate::tui::ui::HitAction;
    use crate::tui::ui::forms;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn note_list_columns_never_bleed_into_each_other() {
        let mut app = test_app();
        app.notes.upsert(
            None,
            "hold above 1520, else cut, this line is long enough to be clipped",
            Some(crate::tui::notes::NoteInstrument {
                exchange: "NFO".to_string(),
                tradingsymbol: "MIDCPNIFTY26AUG13400CE".to_string(),
            }),
            0,
        );
        let state = NotesState::default();
        let text = rendered_text(100, 26, |frame| super::draw_notes(frame, &state, &app));
        assert!(text.contains("Notes / 1"), "title/count missing:\n{text}");
        assert!(
            text.contains("1970-01-01 05:30 NFO:MIDCPNIFTY26AUG… hold above 1520,"),
            "columns are not laid out as expected:\n{text}"
        );
    }

    #[test]
    fn note_modal_survives_a_tiny_terminal() {
        let mut app = test_app();
        app.notes.upsert(None, "something", None, 0);
        let list = NotesState::default();
        rendered_text(20, 4, |frame| super::draw_notes(frame, &list, &app));
        let editor = NotesState {
            editor: Some(NoteEditor::draft(None)),
            ..NotesState::default()
        };
        rendered_text(6, 2, |frame| super::draw_notes(frame, &editor, &app));
        rendered_text(1, 1, |frame| super::draw_notes(frame, &editor, &app));

        app.notes = Pad::unreadable();
        rendered_text(20, 4, |frame| super::draw_notes(frame, &list, &app));
        rendered_text(6, 2, |frame| super::draw_notes(frame, &list, &app));
        rendered_text(1, 1, |frame| super::draw_notes(frame, &list, &app));
        rendered_text(1, 1, |frame| super::draw_notes(frame, &editor, &app));
    }

    #[test]
    fn note_list_announces_the_attach_target_before_a_note_is_written() {
        let app = test_app();
        let state = NotesState {
            attach: Some(crate::tui::notes::NoteInstrument {
                exchange: "NSE".to_string(),
                tradingsymbol: "INFY".to_string(),
            }),
            ..NotesState::default()
        };
        let text = rendered_text(100, 26, |frame| super::draw_notes(frame, &state, &app));
        assert!(
            text.contains("n attaches NSE:INFY"),
            "the attach offer must be visible before pressing n:\n{text}"
        );
        assert!(
            text.contains("No notes yet. Press n to write one."),
            "empty-pad placeholder missing:\n{text}"
        );
    }

    #[test]
    fn a_pad_that_cannot_be_saved_says_so_every_time_it_is_opened() {
        let mut app = test_app();
        app.notes = Pad::unreadable();

        let list = NotesState::default();
        let text = rendered_text(100, 26, |frame| super::draw_notes(frame, &list, &app));
        assert!(
            text.contains("nothing typed here is saved"),
            "the subtitle must flag a pad that is not being saved:\n{text}"
        );
        assert!(
            text.contains("could not be read"),
            "the standing warning line is missing:\n{text}"
        );
        assert!(
            text.contains("No notes yet"),
            "rows were displaced:\n{text}"
        );

        let editor = NotesState {
            editor: Some(NoteEditor::draft(None)),
            ..NotesState::default()
        };
        let text = rendered_text(100, 26, |frame| super::draw_notes(frame, &editor, &app));
        assert!(
            text.contains("Back (cannot save)"),
            "the editor must not claim a save it cannot make:\n{text}"
        );
        assert!(
            !text.contains("Save & back"),
            "the editor must not claim a save it cannot make:\n{text}"
        );
        assert!(text.contains("could not be read"), "{text}");

        let app = test_app();
        let text = rendered_text(100, 26, |frame| super::draw_notes(frame, &list, &app));
        assert!(!text.contains("could not be read"));
        assert!(text.contains("Press n to write one"));
        assert!(text.contains("[d Delete]"));
    }

    #[test]
    fn notes_list_matches_the_spec_mockup() {
        let app = notes_app();
        let state = notes_list_state();
        assert_mockup(
            &modal_ascii(80, 24, |frame| super::draw_notes(frame, &state, &app)),
            "
+- Notes / 2 ----------------------------------------------------------+
| Local notes | n attaches NSE:INFY | N creates a general note         |
+----------------------------------------------------------------------+
|   WRITTEN          INSTRUMENT           NOTE                         |
| > 2026-09-10 09:30 NSE:INFY             Watch the opening range.     |
|   2026-09-09 15:15 General              Review execution notes.      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
| Selected note                                                        |
| Watch the opening range.                                             |
|                                                                      |
+----------------------------------------------------------------------+
| Up/Down or j/k Select  n Attached  N General                         |
| [e/Enter Edit]  [d Delete]  [Esc/q Close]                            |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 1, 72, 22));

        assert_mockup(
            &modal_ascii(120, 35, |frame| super::draw_notes(frame, &state, &app)),
            "
+- Notes / 2 --------------------------------------------------------------------------+
| Local notes | n attaches NSE:INFY | N creates a general note                         |
+--------------------------------------------------------------------------------------+
|   WRITTEN          INSTRUMENT           NOTE                                         |
| > 2026-09-10 09:30 NSE:INFY             Watch the opening range.                     |
|   2026-09-09 15:15 General              Review execution notes.                      |
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
| Selected note                                                                        |
| Watch the opening range.                                                             |
|                                                                                      |
+--------------------------------------------------------------------------------------+
| Up/Down or j/k Select  n Attached  N General                                         |
| [e/Enter Edit]  [d Delete]  [Esc/q Close]                                            |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 3, 88, 29));
    }

    #[test]
    fn the_notes_preview_follows_the_selected_row() {
        let app = notes_app();
        let state = NotesState {
            selected: 1,
            ..notes_list_state()
        };
        let ascii = modal_ascii(80, 24, |frame| super::draw_notes(frame, &state, &app));
        let rows: Vec<&str> = ascii.lines().collect();
        assert!(rows[5].starts_with("| > 2026-09-09"), "{ascii}");
        assert!(rows[16].contains("Review execution notes."), "{ascii}");
    }

    #[test]
    fn a_blocked_pad_offers_only_what_it_can_deliver() {
        let mut app = notes_app();
        app.notes = Pad::unreadable();
        let state = notes_list_state();
        let text = rendered_text(80, 24, |frame| super::draw_notes(frame, &state, &app));
        assert!(text.contains("notes.json could not be read"), "{text}");
        assert!(text.contains("then restart"), "{text}");
        assert!(text.contains("[Esc/q Close]"), "{text}");
        assert!(text.contains("[e/Enter Edit]"), "{text}");
        assert!(text.contains("[d Delete]"), "{text}");
        assert!(
            !app.hit.borrow().regions.iter().any(|region| region.action
                == HitAction::Key(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE))),
            "a refused delete must record no target"
        );
        assert_eq!(
            notes_footer(true, true).actions[1].kind,
            forms::ActionKind::Disabled
        );
    }
}
