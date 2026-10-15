use super::super::{App, Modal};
use crate::tui::workspace::Preset;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspacePickerState {
    pub selected: usize,
}

impl WorkspacePickerState {
    pub fn preset(self) -> Preset {
        Preset::ALL[self.selected.min(Preset::COUNT - 1)]
    }
}

impl App {
    pub(in crate::tui::app) fn open_workspace_picker(&mut self) {
        let selected = Preset::ALL
            .iter()
            .position(|preset| *preset == self.workspace())
            .unwrap_or(0);
        self.modals
            .push(Modal::WorkspacePicker(WorkspacePickerState { selected }));
    }

    pub(in crate::tui::app) fn key_workspace_picker(
        &mut self,
        key: KeyEvent,
        mut state: WorkspacePickerState,
    ) {
        let ctrl = key.modifiers == KeyModifiers::CONTROL;
        let last = Preset::COUNT - 1;
        match key.code {
            KeyCode::Esc | KeyCode::Char('`') => return,
            KeyCode::Enter => {
                self.switch_workspace(state.preset());
                return;
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(preset) = Preset::from_key(c) {
                    self.switch_workspace(preset);
                    return;
                }
            }
            KeyCode::Char('n') if ctrl => state.selected = (state.selected + 1).min(last),
            KeyCode::Char('p') if ctrl => state.selected = state.selected.saturating_sub(1),
            KeyCode::Char('j') | KeyCode::Down => state.selected = (state.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => state.selected = state.selected.saturating_sub(1),
            KeyCode::Char('g') | KeyCode::Home => state.selected = 0,
            KeyCode::Char('G') | KeyCode::End => state.selected = last,
            _ => {}
        }
        self.modals.push(Modal::WorkspacePicker(state));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, char_key, click, render, screen, scroll, test_app,
    };
    use super::super::super::{App, KeyCode, KeyEvent, KeyModifiers, Modal, ui};
    use crate::tui::workspace::Preset;

    fn selected(app: &App) -> Option<usize> {
        match app.modals.last() {
            Some(Modal::WorkspacePicker(state)) => Some(state.selected),
            _ => None,
        }
    }

    fn press(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    #[tokio::test]
    async fn the_picker_opens_on_the_current_workspace_and_enter_switches() {
        let mut app = test_app();
        app.on_key(char_key('2'));
        app.on_key(char_key('`'));
        assert_eq!(selected(&app), Some(1), "the cursor starts on Trading");

        app.on_key(char_key('j'));
        press(&mut app, KeyCode::Down);
        assert_eq!(selected(&app), Some(3));
        app.on_key(char_key('k'));
        assert_eq!(selected(&app), Some(2));
        for _ in 0..10 {
            press(&mut app, KeyCode::Up);
        }
        assert_eq!(selected(&app), Some(0), "clamped at the first");
        for _ in 0..10 {
            app.on_key(char_key('j'));
        }
        assert_eq!(
            selected(&app),
            Some(Preset::COUNT - 1),
            "clamped at the last"
        );
        assert_eq!(app.workspace(), Preset::Trading, "moving switches nothing");

        press(&mut app, KeyCode::Enter);
        assert!(app.modals.is_empty(), "Enter closes the picker");
        assert_eq!(app.workspace(), Preset::Invest);
    }

    #[tokio::test]
    async fn esc_closes_without_switching_and_a_number_switches() {
        let mut app = test_app();
        app.on_key(char_key('`'));
        app.on_key(char_key('j'));
        press(&mut app, KeyCode::Esc);
        assert!(app.modals.is_empty());
        assert_eq!(app.workspace(), Preset::Overview);

        app.on_key(char_key('`'));
        app.on_key(char_key('9'));
        assert!(selected(&app).is_some(), "9 names no workspace");
        app.on_key(char_key('5'));
        assert!(app.modals.is_empty());
        assert_eq!(app.workspace(), Preset::Orders);
    }

    #[tokio::test]
    async fn the_mouse_selects_a_row_and_enter_switches_to_it() {
        let mut app = test_app();
        app.on_key(char_key('`'));
        render(&app, W, H);
        let rows = app
            .hit
            .borrow()
            .regions
            .iter()
            .find_map(|region| match region.action {
                ui::HitAction::Rows {
                    list: ui::HitList::WorkspacePicker,
                    ..
                } => Some(region.rect),
                _ => None,
            })
            .expect("the picker recorded its rows");
        assert_eq!(
            rows.height,
            u16::try_from(Preset::COUNT).expect("six workspaces fit u16"),
            "one row per workspace"
        );

        click(&mut app, W, H, rows.x + 2, rows.y + 3);
        assert_eq!(selected(&app), Some(3), "the click selected Derivatives");
        assert_eq!(app.workspace(), Preset::Overview, "a click only selects");
        scroll(&mut app, W, H, rows.x + 2, rows.y + 3, true);
        assert_eq!(selected(&app), Some(4), "a notch down moves one row");

        press(&mut app, KeyCode::Enter);
        assert_eq!(app.workspace(), Preset::Orders);
    }

    #[test]
    fn the_picker_lists_every_workspace_its_key_and_its_panes() {
        for (width, height) in [(80u16, 24u16), (120, 35), (160, 45)] {
            let mut app = test_app();
            app.on_key(char_key('`'));
            let text = screen(&app, width, height);
            assert!(text.contains("Workspaces"), "{width}x{height}:\n{text}");
            for preset in Preset::ALL {
                let lead = format!("{}  {}", preset.key(), preset.title());
                assert!(
                    text.contains(&lead),
                    "{width}x{height} lost {lead}:\n{text}"
                );
            }
            assert!(text.contains("Watch · Chart · Depth · Orders"), "{text}");
            assert!(text.contains("Orders · GTT · Alerts"), "{text}");
            assert!(text.contains("MF · Bids"), "{text}");
        }
    }
}
