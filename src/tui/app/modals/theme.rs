use super::super::{App, Modal};
use crate::tui::settings::{self, Theme};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Default)]
pub struct ThemePickerState {
    pub input: String,
    pub selected: usize,

    pub results: Vec<&'static str>,

    pub preview_base: Option<Theme>,
}

impl App {
    pub(in crate::tui::app) fn open_theme_picker(&mut self) {
        let results = theme_filter("");

        let active = settings::theme();
        let selected = results
            .iter()
            .position(|name| settings::preset(name) == Some(active))
            .unwrap_or(0);
        let mut state = ThemePickerState {
            input: String::new(),
            selected,
            results,
            preview_base: None,
        };
        Self::sync_theme_preview(&mut state);
        self.modals.push(Modal::ThemePicker(state));
    }

    pub(in crate::tui::app) fn key_theme_picker(
        &mut self,
        key: KeyEvent,
        mut state: ThemePickerState,
    ) {
        let ctrl = key.modifiers == KeyModifiers::CONTROL;
        match key.code {
            KeyCode::Esc => {
                restore_theme(&mut state.preview_base);
                return;
            }
            KeyCode::Enter => {
                match state.results.get(state.selected).copied() {
                    Some(name) => {
                        state.preview_base = None;
                        self.apply_theme(name);
                    }
                    None => restore_theme(&mut state.preview_base),
                }
                return;
            }
            KeyCode::Down => {
                state.selected = (state.selected + 1).min(state.results.len().saturating_sub(1));
            }
            KeyCode::Up => state.selected = state.selected.saturating_sub(1),
            KeyCode::Char('n') if ctrl => {
                state.selected = (state.selected + 1).min(state.results.len().saturating_sub(1));
            }
            KeyCode::Char('p') if ctrl => state.selected = state.selected.saturating_sub(1),
            KeyCode::Backspace => {
                state.input.pop();
                state.results = theme_filter(&state.input);
                state.selected = state.selected.min(state.results.len().saturating_sub(1));
            }
            KeyCode::Char(c) if crate::tui::event::is_text_key(key) => {
                state.input.push(c);
                state.results = theme_filter(&state.input);
                state.selected = 0;
            }
            _ => {}
        }

        Self::sync_theme_preview(&mut state);
        self.modals.push(Modal::ThemePicker(state));
    }

    pub(in crate::tui::app) fn sync_theme_preview(state: &mut ThemePickerState) {
        match state.results.get(state.selected).copied() {
            Some(name) => preview_theme(&mut state.preview_base, name),
            None => restore_theme(&mut state.preview_base),
        }
    }
}

pub(in crate::tui::app) fn theme_filter(input: &str) -> Vec<&'static str> {
    let norm = input.to_ascii_lowercase().replace(['-', '_'], "");
    settings::preset_names()
        .filter(|name| {
            norm.is_empty()
                || name
                    .to_ascii_lowercase()
                    .replace(['-', '_'], "")
                    .contains(&norm)
        })
        .collect()
}

pub(in crate::tui::app) fn preview_theme(base: &mut Option<Theme>, name: &str) {
    match settings::preset(name) {
        Some(theme) => {
            if base.is_none() {
                *base = Some(settings::theme());
            }
            settings::set_theme(theme);
        }
        None => restore_theme(base),
    }
}

pub(in crate::tui::app) fn restore_theme(base: &mut Option<Theme>) {
    if let Some(prev) = base.take() {
        settings::set_theme(prev);
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, char_key, char_key_code, click, press_at, region_of, render, row_offset, test_app,
    };
    use super::super::super::{Modal, ui};
    use super::theme_filter;
    use crate::tui::settings;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn theme_picker_click_selects_and_previews() {
        let _guard = settings::theme_guard(settings::theme());
        let mut app = test_app();
        app.open_theme_picker();
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::ThemePicker,
                    ..
                }
            )
        });

        let want = row_offset(rows) + 2;
        click(&mut app, W, H, rows.rect.x + 4, rows.rect.y + 2);
        {
            let Some(Modal::ThemePicker(state)) = app.modals.last() else {
                panic!("the picker is still open");
            };
            assert_eq!(state.selected, want);
            assert!(
                state.preview_base.is_some(),
                "selecting by click previews, exactly as ↑/↓ do"
            );
        }

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    }

    #[test]
    fn a_refiltered_theme_picker_retires_its_row_targets() {
        let _guard = settings::theme_guard(settings::theme());
        let mut app = test_app();
        app.open_theme_picker();
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::ThemePicker,
                    ..
                }
            )
        });
        let query = "o";
        assert!(
            theme_filter(query).len() > 2,
            "the filter keeps several rows"
        );
        for ch in query.chars() {
            app.on_key(char_key(ch));
        }
        let selected = |app: &super::super::super::App| match app.modals.last() {
            Some(Modal::ThemePicker(state)) => state.selected,
            _ => panic!("the picker is still open"),
        };
        let before = selected(&app);
        let target = if before == 1 { 2 } else { 1 };
        press_at(&mut app, rows.rect.x + 4, rows.rect.y + target);
        assert_eq!(
            selected(&app),
            before,
            "the click was aimed at a row of the unfiltered catalog"
        );
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    }

    #[test]
    fn the_theme_picker_previews_and_rolls_back() {
        let _guard = settings::theme_guard(settings::theme());
        let mut app = test_app();
        let original = settings::theme();
        app.open_theme_picker();
        app.on_key(char_key_code(KeyCode::Down));
        let previewed = settings::theme();
        assert_ne!(previewed, original, "the selection previews immediately");

        for ch in "zzz".chars() {
            app.on_key(char_key(ch));
        }
        {
            let Some(Modal::ThemePicker(state)) = app.modals.last() else {
                panic!("the picker is still open");
            };
            assert_eq!(state.results.len(), 0);
        }
        assert_eq!(settings::theme(), original, "no match restores the base");

        for _ in 0..3 {
            app.on_key(char_key_code(KeyCode::Backspace));
        }
        app.on_key(char_key_code(KeyCode::Down));
        assert_ne!(settings::theme(), original);
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.modals.is_empty(), "Esc closes the picker");
        assert_eq!(settings::theme(), original, "Esc restores the base");
    }

    #[test]
    fn theme_filter_matches_substring_case_and_separator_insensitively() {
        assert_eq!(theme_filter("").len(), settings::preset_names().count());

        assert!(theme_filter("DRAC").contains(&"dracula"));

        assert!(theme_filter("rosepine").contains(&"rose-pine"));
        assert!(theme_filter("onedark").contains(&"one-dark"));

        assert!(theme_filter("night").iter().any(|n| n.contains("night")));

        assert_eq!(theme_filter("definitely-not-a-theme").len(), 0);
    }
}
