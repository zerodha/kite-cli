use super::super::{App, Modal, ToastLevel, holding_key, now_epoch, position_key};
use crate::tui::notes::{self, NoteEditor, NoteInstrument, NotesState, Persist};
use crate::tui::pane_state::ChartBinding;
use crate::tui::ticket::{ConfirmState, PendingAction};
use crate::tui::workspace::PaneId;
use crossterm::event::{KeyCode, KeyEvent};

impl App {
    fn note_attach_target(&self) -> Option<NoteInstrument> {
        match self.focused_pane() {
            PaneId::OverviewPositions | PaneId::OverviewHoldings => self.overview_row_instrument(),
            pane @ (PaneId::TradingWatch
            | PaneId::PortfolioPositions
            | PaneId::PortfolioHoldings) => self
                .resolve_chart_binding(ChartBinding::SelectionOf(pane))
                .map(|target| NoteInstrument {
                    exchange: target.exchange,
                    tradingsymbol: target.tradingsymbol,
                }),
            _ => None,
        }
    }

    fn overview_row_instrument(&self) -> Option<NoteInstrument> {
        if self.focused_pane() == PaneId::OverviewHoldings {
            let overview = self.overview_holdings_display_order();
            let holdings = self.holdings.as_deref().unwrap_or_default();
            let holding = self
                .books
                .overview_holdings
                .target(&overview, |i| holdings.get(i).map(holding_key))
                .and_then(|i| holdings.get(i))?;
            Some(NoteInstrument {
                exchange: holding.exchange.clone(),
                tradingsymbol: holding.tradingsymbol.clone(),
            })
        } else {
            let natural = self.natural_positions_order();
            let net = self.positions_net();
            let position = self
                .books
                .overview_positions
                .target(&natural, |i| net.get(i).map(position_key))
                .and_then(|i| net.get(i))?;
            Some(NoteInstrument {
                exchange: position.exchange.clone(),
                tradingsymbol: position.tradingsymbol.clone(),
            })
        }
    }

    pub(in crate::tui::app) fn open_notes(&mut self) {
        let attach = self.note_attach_target();
        self.modals.push(Modal::Notes(NotesState {
            attach,
            ..NotesState::default()
        }));
    }

    pub(in crate::tui::app) fn key_notes(&mut self, key: KeyEvent, state: NotesState) {
        if state.editor.is_some() {
            self.key_note_editor(key, state);
        } else {
            self.key_note_list(key, state);
        }
    }

    fn key_note_list(&mut self, key: KeyEvent, mut state: NotesState) {
        let blocked = self.notes.blocked();
        let ids = self.notes.ids_newest_first();

        state.selected = state.selected.min(ids.len().saturating_sub(1));
        let selected = ids.get(state.selected).copied();
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.persist_notes();
                return;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                state.selected = (state.selected + 1).min(ids.len().saturating_sub(1));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                state.selected = state.selected.saturating_sub(1);
            }

            KeyCode::Char('n') => state.editor = Some(NoteEditor::draft(state.attach.clone())),

            KeyCode::Char('N') => state.editor = Some(NoteEditor::draft(None)),
            KeyCode::Enter | KeyCode::Char('e') => {
                if let Some(note) = selected.and_then(|id| self.notes.get(id)) {
                    state.editor = Some(NoteEditor::editing(note));
                }
            }

            KeyCode::Char('d') if !blocked => {
                if let Some(note) = selected.and_then(|id| self.notes.get(id)) {
                    let id = note.id;
                    let label = note.preview();
                    let lines = vec![
                        ("Note".to_string(), label.clone()),
                        ("Written".to_string(), notes::stamp(note.created)),
                        ("Instrument".to_string(), note.instrument_label()),
                    ];
                    self.modals.push(Modal::Notes(state));
                    self.modals
                        .push(Modal::Confirm(Box::new(ConfirmState::plain(
                            "Delete note".to_string(),
                            lines,
                            PendingAction::DeleteNote { id, label },
                        ))));
                    return;
                }
            }
            _ => {}
        }
        self.modals.push(Modal::Notes(state));
    }

    fn key_note_editor(&mut self, key: KeyEvent, mut state: NotesState) {
        let Some(editor) = state.editor.as_mut() else {
            self.modals.push(Modal::Notes(state));
            return;
        };
        match key.code {
            KeyCode::Esc => {
                let (id, body, instrument) =
                    (editor.id, editor.buffer.text(), editor.instrument.clone());
                state.editor = None;
                self.notes.upsert(id, &body, instrument, now_epoch());
                if body.trim().is_empty() {
                    self.toast(ToastLevel::Info, "empty note discarded");
                }
                self.persist_notes();
            }
            KeyCode::Enter => editor.buffer.newline(),
            KeyCode::Tab => editor.buffer.insert('\t'),
            KeyCode::Backspace => editor.buffer.backspace(),
            KeyCode::Left => editor.buffer.left(),
            KeyCode::Right => editor.buffer.right(),
            KeyCode::Up => editor.buffer.up(),
            KeyCode::Down => editor.buffer.down(),
            KeyCode::Home => editor.buffer.home(),
            KeyCode::End => editor.buffer.end(),

            KeyCode::Char(ch) if crate::tui::event::is_text_key(key) => {
                editor.buffer.insert(ch);
            }
            _ => {}
        }
        self.modals.push(Modal::Notes(state));
    }

    pub fn commit_open_editor(&mut self) {
        let mut pending = Vec::new();
        for modal in &mut self.modals {
            if let Modal::Notes(state) = modal
                && let Some(editor) = state.editor.take()
            {
                pending.push((editor.id, editor.buffer.text(), editor.instrument));
            }
        }
        let now = now_epoch();
        for (id, body, instrument) in pending {
            self.notes.upsert(id, &body, instrument, now);
        }

        self.exit_notice = self.persist_notes();
    }

    pub fn take_exit_notice(&mut self) -> Option<String> {
        self.exit_notice.take()
    }

    pub(in crate::tui::app) fn delete_note(&mut self, id: u64, label: &str) {
        if self.notes.remove(id).is_none() {
            return;
        }
        self.toast(ToastLevel::Info, format!("deleted note: {label}"));
        self.persist_notes();
    }

    fn persist_notes(&mut self) -> Option<String> {
        if self.ephemeral {
            return None;
        }
        #[cfg(test)]
        let path = self.notes_path.clone();
        #[cfg(not(test))]
        let path = crate::tui::notes::config_file_path();
        let Some(path) = path else {
            let problem = "failed to save notes: no config directory".to_string();
            self.toast_error(problem.clone());
            return Some(problem);
        };
        let outcome = self.notes.persist(&path);
        let problem = match outcome {
            Persist::Skipped | Persist::Written => return None,
            Persist::Blocked => {
                "notes.json could not be read, leaving it untouched; notes are not being saved"
                    .to_string()
            }
            Persist::Failed(err) => format!("failed to save notes: {err}"),
        };
        self.toast_error(problem.clone());
        Some(problem)
    }
}

#[cfg(test)]
pub(in crate::tui::app) mod tests {
    use super::super::super::test_support::{
        H, W, char_key, click, holding, position, region_of, render, sample_order, test_app,
    };
    use super::super::super::{App, MenuAction, Modal, ui};

    use crate::tui::marketwatch::WatchItem;
    use crate::tui::notes::NotesState;
    use crate::tui::ticket::PendingAction;
    use crate::tui::workspace::{PaneId, Preset};
    #[cfg(test)]
    use crossterm::event::KeyModifiers;
    use crossterm::event::{KeyCode, KeyEvent};

    pub(in crate::tui::app) fn notes_app() -> App {
        let mut app = test_app();
        app.ephemeral = true;
        app
    }

    fn type_note(app: &mut App, text: &str) {
        for ch in text.chars() {
            if ch == '\n' {
                app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
            } else {
                app.on_key(char_key(ch));
            }
        }
    }

    fn watch_row(app: &mut App, symbol: &str, token: u32) {
        app.watch.test_push(WatchItem {
            exchange: "NSE".into(),
            tradingsymbol: symbol.into(),
            instrument_token: token,
            segment: String::new(),
            ..WatchItem::default()
        });

        app.circuits.insert(token, None);
    }

    fn note_editor_app(text: &str) -> App {
        let mut app = notes_app();
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, text);
        assert!(
            matches!(app.modals.last(), Some(Modal::Notes(state)) if state.editor.is_some()),
            "the note editor is open"
        );
        app
    }

    fn note_cursor(app: &App) -> (usize, usize) {
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("the notes modal is open");
        };
        state
            .editor
            .as_ref()
            .expect("the editor is open")
            .buffer
            .cursor()
    }

    fn note_body_rect(app: &App) -> ratatui::layout::Rect {
        region_of(app, |action| {
            matches!(action, ui::HitAction::NoteCaret { .. })
        })
        .rect
    }

    #[test]
    fn note_editor_click_places_the_caret() {
        let mut app = note_editor_app("hello world\nsecond line");
        render(&app, W, H);
        let body = note_body_rect(&app);

        click(&mut app, W, H, body.x + 6, body.y);
        assert_eq!(note_cursor(&app), (0, 6));

        click(&mut app, W, H, body.x + 7, body.y + 1);
        assert_eq!(note_cursor(&app), (1, 7));

        app.on_key(char_key('X'));
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("the notes modal is open");
        };
        assert_eq!(
            state.editor.as_ref().expect("editor").buffer.text(),
            "hello world\nsecond Xline"
        );
    }

    #[test]
    fn note_editor_click_past_the_text_clamps_to_it() {
        let mut app = note_editor_app("short\nlonger line");
        render(&app, W, H);
        let body = note_body_rect(&app);
        click(&mut app, W, H, body.x + 40, body.y);
        assert_eq!(note_cursor(&app), (0, 5));
        click(&mut app, W, H, body.x + 3, body.bottom() - 1);
        assert_eq!(note_cursor(&app), (1, 3));
    }

    #[test]
    fn note_editor_click_counts_terminal_cells() {
        let mut app = note_editor_app("測試ab");
        render(&app, W, H);
        let body = note_body_rect(&app);
        click(&mut app, W, H, body.x + 4, body.y);
        assert_eq!(note_cursor(&app), (0, 2));
        app.on_key(char_key('X'));
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("the notes modal is open");
        };
        assert_eq!(
            state.editor.as_ref().expect("editor").buffer.text(),
            "測試Xab"
        );
    }

    #[test]
    fn a_click_outside_the_note_body_leaves_the_caret_alone() {
        let mut app = note_editor_app("hello");
        render(&app, W, H);
        let body = note_body_rect(&app);
        let before = note_cursor(&app);

        click(&mut app, W, H, body.x + 2, body.bottom());
        assert_eq!(note_cursor(&app), before);

        click(&mut app, W, H, body.x - 1, body.y);
        assert_eq!(note_cursor(&app), before);
        assert!(
            matches!(app.modals.last(), Some(Modal::Notes(_))),
            "the editor is still open"
        );
    }

    #[test]
    fn the_note_list_still_selects_rows_not_a_caret() {
        let mut app = notes_app();
        for i in 0..3 {
            app.notes
                .upsert(None, &format!("note {i}"), None, 1_000 + i);
        }
        app.open_notes();
        render(&app, W, H);
        assert!(
            app.hit
                .borrow()
                .regions
                .iter()
                .all(|region| !matches!(region.action, ui::HitAction::NoteCaret { .. })),
            "the list recorded a caret target"
        );
    }

    #[test]
    fn notes_key_opens_the_pad_from_every_pane() {
        for pane in Preset::ALL.into_iter().flat_map(Preset::panes).copied() {
            let mut app = notes_app();
            app.set_pane(pane);
            app.on_key(char_key('n'));
            assert!(
                matches!(app.modals.last(), Some(Modal::Notes(_))),
                "n did not open the notes pad in {pane:?}"
            );
        }
    }

    #[test]
    fn notes_key_attaches_the_selected_watch_row() {
        let mut app = notes_app();
        watch_row(&mut app, "INFY", 408_065);
        watch_row(&mut app, "TCS", 2_953_217);
        app.set_pane(PaneId::TradingWatch);
        app.set_selection(1);
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("expected the notes pad");
        };
        assert_eq!(
            state
                .attach
                .as_ref()
                .map(crate::tui::notes::NoteInstrument::label),
            Some("NSE:TCS".to_string())
        );
    }

    #[test]
    fn notes_key_attaches_the_selected_position_and_holding() {
        let mut app = notes_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![position("INFY", 408_065, 10, 100.0, 110.0, 105.0)];
            row
        });
        app.holdings = Some(vec![holding("RELIANCE", 738_561, 100.0, 110.0, 105.0, 5)]);

        app.set_pane(PaneId::PortfolioPositions);
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.pop() else {
            panic!("expected the notes pad");
        };
        assert_eq!(
            state
                .attach
                .as_ref()
                .map(crate::tui::notes::NoteInstrument::label),
            Some("NSE:INFY".to_string())
        );

        app.set_pane(PaneId::PortfolioHoldings);
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.pop() else {
            panic!("expected the notes pad");
        };
        assert_eq!(
            state
                .attach
                .as_ref()
                .map(crate::tui::notes::NoteInstrument::label),
            Some("NSE:RELIANCE".to_string())
        );
    }

    #[test]
    fn notes_key_attaches_the_selected_overview_row_in_either_panel() {
        let mut app = notes_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![
                position("INFY", 408_065, 10, 100.0, 110.0, 105.0),
                position("SBIN", 779_521, 5, 10.0, 800.0, 795.0),
            ];
            row
        });
        app.holdings = Some(vec![
            holding("RELIANCE", 738_561, 100.0, 110.0, 105.0, 5),
            holding("ITC", 424_961, 400.0, 410.0, 405.0, 20),
        ]);
        app.set_workspace(Preset::Overview);

        app.workspaces
            .focus(Preset::Overview, PaneId::OverviewPositions);
        app.set_selection(1);
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.pop() else {
            panic!("expected the notes pad");
        };
        assert_eq!(
            state
                .attach
                .as_ref()
                .map(crate::tui::notes::NoteInstrument::label),
            Some("NSE:SBIN".to_string())
        );

        app.workspaces
            .focus(Preset::Overview, PaneId::OverviewHoldings);
        let expected = {
            let order = app.overview_holdings_display_order();
            let holdings = app.holdings.as_ref().unwrap();
            format!("NSE:{}", holdings[order[1]].tradingsymbol)
        };
        app.set_selection(1);
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.pop() else {
            panic!("expected the notes pad");
        };
        assert_eq!(
            state
                .attach
                .as_ref()
                .map(crate::tui::notes::NoteInstrument::label),
            Some(expected)
        );
    }

    #[test]
    fn notes_key_attaches_the_focused_composite_panes_row() {
        let mut app = notes_app();
        app.positions = Some({
            let mut row = crate::tui::data::test_models::positions();
            row.net = vec![position("INFY", 408_065, 10, 100.0, 110.0, 105.0)];
            row
        });
        app.holdings = Some(vec![holding("TCS", 2_953_217, 100.0, 110.0, 105.0, 5)]);
        app.set_workspace(Preset::Overview);
        app.set_workspace(Preset::Portfolio);
        app.focus_pane_for_test(PaneId::PortfolioHoldings);

        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("expected the notes pad");
        };
        assert_eq!(
            state
                .attach
                .as_ref()
                .map(crate::tui::notes::NoteInstrument::label),
            Some("NSE:TCS".to_string())
        );
        assert!(state.editor.is_some(), "the second n starts a draft");
    }

    #[test]
    fn notes_key_with_no_row_selected_is_a_general_note() {
        let mut app = notes_app();
        app.set_pane(PaneId::TradingWatch);
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.pop() else {
            panic!("expected the notes pad");
        };
        assert!(state.attach.is_none());

        app.orders = Some(vec![sample_order("OPEN")]);
        app.set_pane(PaneId::OrdersBook);
        app.on_key(char_key('n'));
        let Some(Modal::Notes(state)) = app.modals.pop() else {
            panic!("expected the notes pad");
        };
        assert!(state.attach.is_none());
    }

    #[test]
    fn writing_a_note_saves_it_on_esc_with_the_instrument_attached() {
        let mut app = notes_app();
        watch_row(&mut app, "INFY", 408_065);
        app.set_pane(PaneId::TradingWatch);
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "hold above ₹1520\nelse cut");
        assert!(app.notes.is_empty(), "nothing is stored until Esc");

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.notes.len(), 1);
        let note = app.notes.newest_first()[0];
        assert_eq!(note.body, "hold above ₹1520\nelse cut");
        assert_eq!(note.instrument_label(), "NSE:INFY");
        assert_eq!(note.created, note.updated);

        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("Esc from the editor must return to the list");
        };
        assert!(state.editor.is_none());

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.modals.is_empty());
    }

    #[test]
    fn shift_n_declines_the_attach_offer() {
        let mut app = notes_app();
        watch_row(&mut app, "INFY", 408_065);
        app.set_pane(PaneId::TradingWatch);
        app.on_key(char_key('n'));
        app.on_key(KeyEvent::new(KeyCode::Char('N'), KeyModifiers::SHIFT));
        type_note(&mut app, "market-wide thought");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let note = app.notes.newest_first()[0];
        assert!(note.instrument.is_none());
        assert_eq!(note.instrument_label(), "General");
    }

    #[test]
    fn the_editor_swallows_command_keys_as_text() {
        let mut app = notes_app();
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "qdj?");
        assert!(!app.should_quit, "q inside the editor must not quit");
        assert!(
            matches!(app.modals.last(), Some(Modal::Notes(state)) if state.editor.is_some()),
            "the editor must still be open"
        );
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.notes.newest_first()[0].body, "qdj?");
    }

    #[test]
    fn the_editor_ignores_control_chords_instead_of_typing_their_letter() {
        let mut app = notes_app();
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "ab");
        app.on_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
        app.on_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT));
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.notes.newest_first()[0].body, "ab");
    }

    #[test]
    fn an_empty_note_is_discarded_and_says_so() {
        let mut app = notes_app();
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "  \n ");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.notes.is_empty());
        assert_eq!(
            app.visible_toast().map(|toast| toast.text.clone()),
            Some("empty note discarded".to_string())
        );
    }

    #[test]
    fn editing_loads_the_body_with_the_cursor_at_its_end_and_keeps_created() {
        let mut app = notes_app();
        app.notes.upsert(None, "first thought", None, 1_000);
        app.on_key(char_key('n'));
        app.on_key(char_key('e'));
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("expected the notes pad");
        };
        let editor = state.editor.as_ref().expect("e must open the editor");
        assert_eq!(editor.buffer.text(), "first thought");
        assert_eq!(editor.buffer.cursor(), (0, 13), "cursor lands at the end");

        type_note(&mut app, ", and more");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let note = app.notes.newest_first()[0];
        assert_eq!(note.body, "first thought, and more");
        assert_eq!(note.created, 1_000, "an edit never rewrites `created`");
        assert!(note.updated >= 1_000);
        assert_eq!(app.notes.len(), 1, "an edit must not fork a second note");
    }

    #[test]
    fn blanking_an_existing_note_deletes_it() {
        let mut app = notes_app();
        app.notes.upsert(None, "ab", None, 1_000);
        app.on_key(char_key('n'));
        app.on_key(char_key('e'));
        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.notes.is_empty());
    }

    #[test]
    fn delete_confirms_against_the_highlighted_row_by_id() {
        let mut app = notes_app();
        app.notes.upsert(None, "older", None, 100);
        app.notes.upsert(None, "newer", None, 200);
        let newer_id = app.notes.ids_newest_first()[0];
        app.on_key(char_key('n'));
        app.on_key(char_key('j'));
        app.on_key(char_key('d'));

        let Some(Modal::Confirm(confirm)) = app.modals.last() else {
            panic!("delete must go through the confirm modal");
        };
        let PendingAction::DeleteNote { id, label } = &confirm.action else {
            panic!("expected a DeleteNote confirmation");
        };
        assert_eq!(label, "older");
        assert_ne!(*id, newer_id);

        app.on_key(char_key('n'));
        assert_eq!(app.notes.len(), 2);
        assert!(matches!(app.modals.last(), Some(Modal::Notes(_))));
    }

    #[test]
    fn confirmed_delete_removes_the_note_and_keeps_the_pad_open() {
        let mut app = notes_app();
        app.notes.upsert(None, "keep me", None, 100);
        app.notes.upsert(None, "delete me", None, 200);
        app.on_key(char_key('n'));
        app.on_key(char_key('d'));
        render(&app, W, H);
        app.on_key(char_key('y'));

        assert_eq!(app.notes.len(), 1);
        assert_eq!(app.notes.newest_first()[0].body, "keep me");
        assert!(!app.pending_action);
        assert!(
            matches!(app.modals.last(), Some(Modal::Notes(_))),
            "the pad must survive a local delete"
        );
        assert_eq!(app.modals.len(), 1);
    }

    #[test]
    fn delete_on_an_empty_pad_does_nothing() {
        let mut app = notes_app();
        app.on_key(char_key('n'));
        app.on_key(char_key('d'));
        assert!(matches!(app.modals.last(), Some(Modal::Notes(_))));
        assert_eq!(app.modals.len(), 1);
    }

    #[test]
    fn list_keys_act_on_the_highlighted_row_after_a_shrink() {
        let mut app = notes_app();
        app.notes.upsert(None, "only one", None, 100);
        app.modals.push(Modal::Notes(NotesState {
            selected: 5,
            ..NotesState::default()
        }));
        app.on_key(char_key('e'));
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("expected the notes pad");
        };
        assert_eq!(
            state.editor.as_ref().map(|editor| editor.buffer.text()),
            Some("only one".to_string())
        );
    }

    #[test]
    fn the_palette_notes_action_opens_the_same_pad() {
        let mut app = notes_app();
        watch_row(&mut app, "INFY", 408_065);
        app.set_pane(PaneId::TradingWatch);
        app.exec_menu_action(MenuAction::Notes);
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("the palette action must open the notes pad");
        };
        assert_eq!(
            state
                .attach
                .as_ref()
                .map(crate::tui::notes::NoteInstrument::label),
            Some("NSE:INFY".to_string())
        );
    }

    fn saving_notes_app(name: &str) -> (App, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("kite-exit-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("notes.json");
        let mut app = test_app();
        app.ephemeral = false;
        app.notes_path = Some(path.clone());
        (app, path)
    }

    fn saved_bodies(path: &std::path::Path) -> Vec<String> {
        crate::tui::notes::load_from(path)
            .newest_first()
            .iter()
            .map(|note| note.body.clone())
            .collect()
    }

    #[test]
    fn ctrl_c_with_an_open_editor_saves_what_was_typed() {
        let (mut app, path) = saving_notes_app("ctrlc");
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "1520 is the level\nsize down below it");

        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.should_quit, "Ctrl-C must still quit");
        assert!(
            !path.exists(),
            "nothing is written mid-edit; the save happens on the way out"
        );

        app.commit_open_editor();
        assert_eq!(
            saved_bodies(&path),
            vec!["1520 is the level\nsize down below it".to_string()],
            "the open editor's text must be on disk after Ctrl-C"
        );
    }

    #[test]
    fn a_teardown_with_no_keystroke_still_saves_an_open_editor() {
        let (mut app, path) = saving_notes_app("teardown");
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "thesis for the swing");
        app.expire_session();

        app.commit_open_editor();
        assert_eq!(
            saved_bodies(&path),
            vec!["thesis for the swing".to_string()],
            "a relogin teardown must not eat the open note"
        );
    }

    #[test]
    fn quitting_out_of_an_edit_updates_the_note_it_was_editing() {
        let (mut app, path) = saving_notes_app("edit");
        app.notes.upsert(None, "first draft", None, 1_000);
        app.on_key(char_key('n'));
        app.on_key(char_key('e'));
        type_note(&mut app, ", revised");
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        app.commit_open_editor();
        assert_eq!(
            saved_bodies(&path),
            vec!["first draft, revised".to_string()]
        );
    }

    #[test]
    fn quitting_out_of_a_blank_draft_stores_nothing() {
        let (mut app, path) = saving_notes_app("blank");
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "   ");
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        app.commit_open_editor();
        assert!(
            !path.exists(),
            "an empty draft must not even create the file"
        );
    }

    #[test]
    fn an_ephemeral_session_folds_the_buffer_but_writes_nothing() {
        let (mut app, path) = saving_notes_app("ephemeral");
        app.ephemeral = true;
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "not mine to keep");
        app.commit_open_editor();
        assert_eq!(app.notes.len(), 1);
        assert!(!path.exists(), "a --token session persists nothing");
    }

    #[test]
    fn a_save_that_fails_only_at_exit_still_reaches_the_user() {
        let (mut app, _path) = saving_notes_app("exit-notice");

        app.notes = crate::tui::notes::Pad::unreadable();
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "typed into a pad that cannot be saved");
        app.commit_open_editor();

        let notice = app
            .take_exit_notice()
            .expect("a blocked exit save must leave a notice for stderr");
        assert!(
            notice.contains("notes.json"),
            "the notice must name the file: {notice}"
        );

        assert_eq!(app.take_exit_notice(), None);
    }

    #[test]
    fn a_missing_config_directory_reports_unsaved_notes() {
        let mut app = notes_app();
        app.ephemeral = false;
        app.notes_path = None;
        app.notes.upsert(None, "keep this note", None, 100);
        app.commit_open_editor();
        let problem = "failed to save notes: no config directory";
        assert_eq!(app.take_exit_notice().as_deref(), Some(problem));
        assert_eq!(
            app.visible_toast().map(|toast| toast.text.as_str()),
            Some(problem)
        );
    }

    #[test]
    fn a_clean_exit_leaves_no_notice() {
        let (mut app, _path) = saving_notes_app("exit-clean");
        app.on_key(char_key('n'));
        app.on_key(char_key('n'));
        type_note(&mut app, "saves fine");
        app.commit_open_editor();
        assert_eq!(
            app.take_exit_notice(),
            None,
            "a successful save must not print anything to stderr"
        );
    }

    #[test]
    fn a_blocked_pad_refuses_a_delete_it_cannot_persist() {
        let mut app = notes_app();
        app.notes = crate::tui::notes::Pad::unreadable();
        app.notes
            .upsert(None, "a note the pad cannot lose", None, 0);
        app.on_key(char_key('n'));
        app.on_key(char_key('d'));
        assert!(
            matches!(app.modals.last(), Some(Modal::Notes(_))),
            "a refused delete opens no confirm"
        );
        assert_eq!(app.notes.len(), 1, "nothing was removed");

        app.on_key(char_key('n'));
        assert!(
            matches!(app.modals.last(), Some(Modal::Notes(state)) if state.editor.is_some()),
            "a blocked pad still opens its editor, saying it cannot save"
        );
    }

    #[test]
    fn notes_list_click_selects_a_note() {
        let mut app = notes_app();
        for i in 0..3 {
            app.notes
                .upsert(None, &format!("note {i}"), None, 1_000 + i);
        }
        app.open_notes();
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Notes,
                    ..
                }
            )
        });
        click(&mut app, W, H, rows.rect.x + 2, rows.rect.y + 2);
        let Some(Modal::Notes(state)) = app.modals.last() else {
            panic!("the pad is still open");
        };
        assert_eq!(state.selected, 2);
    }

    #[test]
    fn a_new_note_retires_the_notes_list_row_targets() {
        use super::super::super::test_support::press_at;
        let mut app = notes_app();
        for i in 0..3 {
            app.notes
                .upsert(None, &format!("note {i}"), None, 1_000 + i);
        }
        app.open_notes();
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Notes,
                    ..
                }
            )
        });

        app.on_key(char_key('N'));
        type_note(&mut app, "fresh");
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(app.notes.ids_newest_first().len(), 4);
        let selected = |app: &App| match app.modals.last() {
            Some(Modal::Notes(state)) if state.editor.is_none() => state.selected,
            _ => panic!("the list is back on screen"),
        };
        let before = selected(&app);
        press_at(&mut app, rows.rect.x + 2, rows.rect.y + 2);
        assert_eq!(
            selected(&app),
            before,
            "the click was aimed at note 0, which has moved down a row"
        );
    }
}
