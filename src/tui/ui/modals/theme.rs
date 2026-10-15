use super::layout::{
    SELECT_MARKER, UTILITY_BODY_ROWS, UTILITY_TOO_SMALL, draw_body_line, draw_select_list,
    section_line,
};
use crate::tui::app::{App, ThemePickerState};
use crate::tui::format;
use crate::tui::settings;
use crate::tui::ui::forms;
use crate::tui::ui::{HitList, always, nav};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;

pub(in crate::tui::ui) fn theme_picker_footer(applicable: bool) -> &'static forms::FormFooter {
    const HELP: &[&str] = &["Up/Down or Ctrl-N/P Preview", "Type Filter"];
    const RESTORE: forms::FormAction = forms::FormAction {
        key: "Esc",
        label: "Restore theme",
        kind: forms::ActionKind::Secondary,
    };
    if applicable {
        return const {
            &forms::FormFooter {
                help: HELP,
                actions: &[
                    forms::FormAction {
                        key: "Enter",
                        label: "Apply theme",
                        kind: forms::ActionKind::Primary,
                    },
                    RESTORE,
                ],
                hints: &[
                    nav(&["type to filter", "↑↓:preview"]),
                    always(&["Enter:apply", "Esc:restore"]),
                ],
            }
        };
    }
    const {
        &forms::FormFooter {
            help: HELP,
            actions: &[
                forms::FormAction {
                    key: "Enter",
                    label: "Apply theme",
                    kind: forms::ActionKind::Disabled,
                },
                RESTORE,
            ],
            hints: &[nav(&["type to filter"]), always(&["Esc:restore"])],
        }
    }
}

pub(super) fn draw_theme_picker(frame: &mut Frame, state: &ThemePickerState, app: &App) {
    let area = frame.area();
    let layout = forms::ModalLayout::measure(area, UTILITY_BODY_ROWS);
    let committed = state.preview_base.unwrap_or_else(settings::theme);
    let committed_name = settings::preset_names()
        .find(|name| settings::preset(name) == Some(committed))
        .unwrap_or("custom");
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        "Themes",
        Style::default().fg(format::accent()),
        &format!("Preview only | committed: {committed_name}"),
        UTILITY_TOO_SMALL,
    ) else {
        return;
    };
    let body = layout.body;
    draw_body_line(
        frame,
        body,
        0,
        forms::query_row("Filter", &state.input, layout.content),
    );
    draw_body_line(
        frame,
        body,
        1,
        section_line("Available themes", layout.content),
    );

    if state.results.is_empty() {
        draw_body_line(
            frame,
            body,
            2,
            Line::from(Span::styled("No themes match", format::dim())),
        );
        forms::draw_footer(frame, app, &layout, theme_picker_footer(false));
        return;
    }

    let guidance = u16::from(area.width >= forms::WIDE_TERMINAL) * 2;
    let available = body.height.saturating_sub(3).saturating_sub(guidance);
    let shown = available.min(u16::try_from(state.results.len()).unwrap_or(u16::MAX));
    let selected = state.selected.min(state.results.len() - 1);
    if shown > 0 {
        let items: Vec<ListItem> = state
            .results
            .iter()
            .map(|name| {
                let theme = settings::preset(name).unwrap_or(settings::Theme::DEFAULT);
                let marker = if theme == committed { "* " } else { "  " };
                ListItem::new(Line::from(vec![
                    Span::styled(marker, format::title_style()),
                    Span::styled("██ ", Style::default().fg(theme.accent)),
                    Span::raw(*name),
                ]))
            })
            .collect();
        let list_area = Rect {
            y: body.y + 2,
            height: shown,
            ..body
        };
        draw_select_list(
            frame,
            app,
            list_area,
            items,
            Some(selected),
            HitList::ThemePicker,
            SELECT_MARKER,
        );
    }
    draw_body_line(
        frame,
        body,
        2 + shown,
        Line::from(Span::styled(
            format!("Preview: {}", state.results[selected]),
            format::dim(),
        )),
    );
    if guidance > 0 {
        draw_body_line(
            frame,
            body,
            4 + shown,
            Line::from(Span::styled(
                "Selection previews the whole TUI immediately.",
                format::dim(),
            )),
        );
    }
    forms::draw_footer(frame, app, &layout, theme_picker_footer(true));
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{
        assert_mockup, modal_ascii, modal_rect, rect_tuple, rendered_text, test_app, theme_picker,
    };
    use super::{draw_theme_picker, forms, settings, theme_picker_footer};
    use crate::tui::app::ThemePickerState;

    #[test]
    fn theme_picker_renders_title_names_and_marker() {
        let state = ThemePickerState {
            input: String::new(),
            selected: 0,
            preview_base: settings::preset("kite"),
            results: vec!["kite", "dracula"],
        };
        let text = rendered_text(60, 20, |frame| {
            draw_theme_picker(frame, &state, &test_app());
        });
        assert!(text.contains("Themes"), "title missing:\n{text}");
        assert!(text.contains("kite"), "kite row missing:\n{text}");
        assert!(text.contains("dracula"), "dracula row missing:\n{text}");
        assert!(
            text.contains("* ██ kite"),
            "committed marker missing:\n{text}"
        );
        assert!(
            text.contains("Preview: kite"),
            "preview status missing:\n{text}"
        );
        assert!(
            text.contains("committed: kite"),
            "committed theme missing:\n{text}"
        );
    }

    #[test]
    fn theme_picker_empty_results_render_placeholder() {
        let state = ThemePickerState {
            input: "zzz".to_string(),
            selected: 0,
            results: Vec::new(),
            preview_base: None,
        };
        let text = rendered_text(60, 20, |frame| {
            draw_theme_picker(frame, &state, &test_app());
        });
        assert!(
            text.contains("No themes match"),
            "placeholder missing:\n{text}"
        );
        assert!(
            !theme_picker_footer(false)
                .actions
                .iter()
                .any(|action| action.kind == forms::ActionKind::Primary),
            "Apply must be disabled with no results"
        );
    }

    #[test]
    fn theme_picker_matches_the_spec_mockup() {
        let app = test_app();
        let state = theme_picker(8);
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_theme_picker(frame, &state, &app)),
            "
+- Themes -------------------------------------------------------------+
| Preview only | committed: kite                                       |
+----------------------------------------------------------------------+
| >       Filter  [                                                  ] |
| Available themes                                                     |
| > * ## kite                                                          |
|     ## ayu                                                           |
|     ## catppuccin                                                    |
|     ## catppuccin-frappe                                             |
|     ## catppuccin-macchiato                                          |
|     ## dracula                                                       |
|     ## gruvbox                                                       |
|     ## nord                                                          |
| Preview: kite                                                        |
|                                                                      |
|                                                                      |
|                                                                      |
|                                                                      |
+----------------------------------------------------------------------+
| Up/Down or Ctrl-N/P Preview  Type Filter                             |
| [Enter Apply theme]  [Esc Restore theme]                             |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 1, 72, 22));

        let state = theme_picker(10);
        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_theme_picker(frame, &state, &app)),
            "
+- Themes -----------------------------------------------------------------------------+
| Preview only | committed: kite                                                       |
+--------------------------------------------------------------------------------------+
| >       Filter  [                                                                  ] |
| Available themes                                                                     |
| > * ## kite                                                                          |
|     ## ayu                                                                           |
|     ## catppuccin                                                                    |
|     ## catppuccin-frappe                                                             |
|     ## catppuccin-macchiato                                                          |
|     ## dracula                                                                       |
|     ## gruvbox                                                                       |
|     ## nord                                                                          |
|     ## solarized-light                                                               |
|     ## system                                                                        |
| Preview: kite                                                                        |
|                                                                                      |
| Selection previews the whole TUI immediately.                                        |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
|                                                                                      |
+--------------------------------------------------------------------------------------+
| Up/Down or Ctrl-N/P Preview  Type Filter                                             |
| [Enter Apply theme]  [Esc Restore theme]                                             |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 3, 88, 29));
    }

    #[test]
    fn the_theme_list_scrolls_to_the_selection_and_keeps_the_status_line() {
        let app = test_app();
        let results: Vec<&str> = settings::preset_names().collect();
        let last = results.len() - 1;
        let name = results[last];
        let state = ThemePickerState {
            input: String::new(),
            selected: last,
            results,
            preview_base: settings::preset("kite"),
        };
        let text = rendered_text(80, 24, |frame| draw_theme_picker(frame, &state, &app));
        assert!(
            text.contains(&format!(">   ██ {name}")),
            "the selection must be in view:\n{text}"
        );
        assert!(
            text.contains(&format!("Preview: {name}")),
            "the status line must survive a full list:\n{text}"
        );
        assert!(!text.contains("* ██ kite"), "{text}");
    }

    #[test]
    fn the_theme_filter_keeps_its_caret_inside_the_well() {
        let app = test_app();
        let state = ThemePickerState {
            input: "cat".to_string(),
            selected: 0,
            results: vec!["catppuccin"],
            preview_base: settings::preset("kite"),
        };
        let text = rendered_text(80, 24, |frame| draw_theme_picker(frame, &state, &app));
        assert!(text.contains("Filter  [cat"), "{text}");
        let long = "x".repeat(200);
        let state = ThemePickerState {
            input: long,
            selected: 0,
            results: Vec::new(),
            preview_base: settings::preset("kite"),
        };
        let ascii = modal_ascii(80, 24, |frame| draw_theme_picker(frame, &state, &app));
        for row in ascii.lines() {
            assert_eq!(row.chars().count(), 72, "{row:?}");
        }
    }
}
