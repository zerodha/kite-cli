use super::super::{App, Modal};
use crate::tui::form::FormViewport;
use crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, Default)]
pub struct EventLogState {
    pub selected: usize,

    pub detail: FormViewport,
}

impl App {
    pub(in crate::tui::app) fn toggle_event_log(&mut self) {
        if matches!(self.modals.last(), Some(Modal::EventLog(_))) {
            self.modals.pop();
        } else if self.modals.is_empty() {
            self.modals.push(Modal::EventLog(EventLogState::default()));
        }
    }

    pub(in crate::tui::app) fn key_event_log(&mut self, key: KeyEvent, mut state: EventLogState) {
        let last = self.toasts.len().saturating_sub(1);
        let before = state.selected;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => return,
            KeyCode::Char('j') | KeyCode::Down => state.selected = (state.selected + 1).min(last),
            KeyCode::Char('k') | KeyCode::Up => state.selected = state.selected.saturating_sub(1),
            KeyCode::Char('g') => state.selected = 0,
            KeyCode::Char('G') => state.selected = last,

            KeyCode::PageUp => state.detail.page(-1),
            KeyCode::PageDown => state.detail.page(1),
            _ => {}
        }
        if state.selected != before {
            state.detail = FormViewport::default();
        }
        self.modals.push(Modal::EventLog(state));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, char_key, char_key_code, click, press_at, region_of, render, test_app,
    };
    use super::super::super::{Modal, ToastLevel, ui};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn event_log_click_selects_a_row() {
        let mut app = test_app();
        for i in 0..4 {
            app.toast(ToastLevel::Info, format!("event {i}"));
        }
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::EventLog,
                    ..
                }
            )
        });
        click(&mut app, W, H, rows.rect.x + 2, rows.rect.y + 3);
        let Some(Modal::EventLog(state)) = app.modals.last() else {
            panic!("the log is still open");
        };
        assert_eq!(state.selected, 3);
    }

    #[test]
    fn clicking_another_event_starts_its_detail_at_the_top() {
        let mut app = test_app();
        for i in 0..4 {
            app.toast(
                ToastLevel::Info,
                format!("event {i}: {}", "detail ".repeat(120)),
            );
        }
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::EventLog,
                    ..
                }
            )
        });
        app.on_key(char_key_code(KeyCode::PageDown));
        {
            let Some(Modal::EventLog(state)) = app.modals.last() else {
                panic!("the log is still open");
            };
            assert_eq!(state.selected, 0);
            assert!(
                state.detail.resolve(40, 2, None) > 0,
                "the page key moved the detail off its top"
            );
        }

        click(&mut app, W, H, rows.rect.x + 2, rows.rect.y + 3);
        let Some(Modal::EventLog(state)) = app.modals.last() else {
            panic!("the log is still open");
        };
        assert_eq!(state.selected, 3);
        assert_eq!(
            state.detail.resolve(40, 2, None),
            0,
            "another event's message starts at its own top"
        );
    }

    #[test]
    fn event_log_page_keys_scroll_only_the_detail() {
        let mut app = test_app();
        for i in 0..4 {
            app.toast(ToastLevel::Info, format!("event {i}"));
        }
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));

        render(&app, W, H);
        app.on_key(char_key_code(KeyCode::PageDown));
        let Some(Modal::EventLog(state)) = app.modals.last() else {
            panic!("the log is still open");
        };
        assert_eq!(state.selected, 0, "a page key must not move the selection");

        app.on_key(char_key('j'));
        let Some(Modal::EventLog(state)) = app.modals.last() else {
            panic!("the log is still open");
        };
        assert_eq!(state.selected, 1);

        assert_eq!(state.detail.resolve(10, 2, None), 0);
    }

    #[test]
    fn a_new_event_retires_the_event_log_row_targets() {
        use crate::tui::data::DataEvent;
        let mut app = test_app();
        for i in 0..4 {
            app.toast(ToastLevel::Info, format!("event {i}"));
        }
        app.on_key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::EventLog,
                    ..
                }
            )
        });

        app.on_data(DataEvent::ExternalLink(Ok(
            "https://kite.zerodha.com".into()
        )));
        let selected = |app: &super::super::super::App| match app.modals.last() {
            Some(Modal::EventLog(state)) => state.selected,
            _ => panic!("the log is still open"),
        };
        let before = selected(&app);
        press_at(&mut app, rows.rect.x + 2, rows.rect.y + 2);
        assert_eq!(
            selected(&app),
            before,
            "the click was aimed at event 1, which has moved down a row"
        );
    }
}
