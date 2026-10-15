use crate::tui::app::App;
use crate::tui::format;
use crate::tui::ui::centered;
use ratatui::Frame;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;

mod content;
mod layout;

use content::help_rows;
use layout::{box_width, help_lines};

pub fn draw_help(frame: &mut Frame, app: &App, scroll: u16) {
    let rows = help_rows(app.focused_pane());
    let box_w = box_width(&rows, frame.area().width);
    let lines = help_lines(&rows, box_w.saturating_sub(2));
    let box_h = u16::try_from(lines.len() + 2)
        .unwrap_or(u16::MAX)
        .min(frame.area().height.saturating_sub(2));
    let area = centered(frame.area(), box_w, box_h);

    let inner = super::modals::modal_block(
        frame,
        app,
        area,
        format!(" Help · {} ", app.focused_pane().title()),
        Style::default().fg(format::accent()),
    );

    let max_scroll = u16::try_from(lines.len())
        .unwrap_or(u16::MAX)
        .saturating_sub(inner.height);
    app.help_max_scroll.set(max_scroll);
    if max_scroll > 0 {
        app.hit
            .borrow_mut()
            .push(inner, super::HitAction::Text(super::HitText::Help));
    }
    frame.render_widget(
        Paragraph::new(lines).scroll((scroll.min(max_scroll), 0)),
        inner,
    );
}

#[cfg(test)]
mod tests {
    use super::content::{HelpRow, every_pane, help_rows, shift_label};
    use super::draw_help;
    use super::layout::{KEY_INDENT, desc_col, key_width, max_key_width, wrap_words};
    use crate::tui::app::App;
    use crate::tui::notes::Pad;
    use crate::tui::settings::Settings;
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::collections::HashSet;

    fn test_app(pane: PaneId) -> App {
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("tok").unwrap())
            .build()
            .expect("dummy client builds");
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let mut app = App::new(
            client,
            events_tx,
            crate::tui::marketwatch::State::test_loaded(Vec::new()),
            Pad::default(),
            false,
            &Settings::default(),
        );
        let preset = crate::tui::workspace::Preset::ALL
            .into_iter()
            .find(|preset| preset.panes().contains(&pane))
            .unwrap_or(crate::tui::workspace::Preset::Overview);
        app.set_workspace(preset);
        app.focus_pane_for_test(pane);
        app
    }

    #[test]
    fn a_tiny_terminal_clamps_instead_of_breaking() {
        for pane in every_pane() {
            for (w, h) in [(24u16, 12u16), (30, 20), (40, 8)] {
                let rows = modal_rows(pane, w, h);
                assert!(
                    rows.iter().all(|row| row.chars().count() <= w as usize - 2),
                    "{pane:?} at {w}x{h} drew past the border: {rows:?}"
                );
                assert!(rows.len() <= h as usize, "{pane:?} at {w}x{h}: {rows:?}");
            }
        }
    }

    const TALL: u16 = 96;

    fn modal_rows(pane: PaneId, w: u16, h: u16) -> Vec<String> {
        let app = test_app(pane);
        let mut terminal = Terminal::new(TestBackend::new(w, h)).unwrap();
        terminal
            .draw(|frame| draw_help(frame, &app, 0))
            .expect("help renders");
        let buffer = terminal.backend().buffer().clone();
        let mut rows = Vec::new();
        for y in 0..h {
            let line: Vec<String> = (0..w)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect();
            let first = line.iter().position(|c| c == "│");
            let last = line.iter().rposition(|c| c == "│");
            if let (Some(first), Some(last)) = (first, last)
                && last > first
            {
                rows.push(line[first + 1..last].concat());
            }
        }
        assert!(!rows.is_empty(), "{pane:?} at {w}x{h} drew no body");
        rows
    }

    #[test]
    fn no_row_starts_description_text_in_the_key_column() {
        for pane in every_pane() {
            for width in [60u16, 80, 100, 120] {
                let rows = modal_rows(pane, width, TALL);
                let inner_w = rows[0].chars().count();
                let logical = help_rows(pane);
                let key_w = key_width(max_key_width(&logical), inner_w);
                let col = desc_col(key_w);

                let mut keys: HashSet<String> = HashSet::new();
                let mut prose: HashSet<String> = HashSet::new();
                for row in &logical {
                    match row {
                        HelpRow::Entry { key, .. } => {
                            keys.insert(key.clone());
                        }
                        HelpRow::Section(title) => {
                            prose.insert(title.clone());
                        }
                        HelpRow::Group(title) => {
                            prose.insert((*title).to_string());
                        }
                        HelpRow::Note(text) => {
                            prose.extend(wrap_words(text, inner_w.saturating_sub(1)));
                        }
                        HelpRow::Blank => {}
                    }
                }

                for (y, row) in rows.iter().enumerate() {
                    if row.trim().is_empty() {
                        continue;
                    }
                    let first = row.chars().position(|c| c != ' ').unwrap();
                    if first >= col {
                        continue; // a description or its continuation
                    }
                    let head: String = row.chars().take(col).collect();
                    let head = head.trim().to_string();
                    assert!(
                        keys.contains(&head) || prose.contains(row.trim()),
                        "{pane:?} at {width}, row {y}: {:?} begins at column {first} \
                         with something that is neither a key nor a heading",
                        row.trim_end()
                    );
                    if keys.contains(&head) {
                        assert!(
                            first >= KEY_INDENT,
                            "{pane:?} at {width}: a key must sit in its column: {row:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_wrapped_entry_keeps_its_key_at_every_tested_width() {
        for width in [60u16, 80, 100, 120] {
            let rows = modal_rows(PaneId::OrdersBook, width, TALL);
            let indicator = rows
                .iter()
                .position(|row| row.contains("cycle chart indicator"))
                .unwrap_or_else(|| panic!("the I entry must render at {width}:\n{rows:#?}"));
            assert!(
                rows[indicator].contains(&shift_label("I")),
                "at {width} the description lost its key: {:?}",
                rows[indicator]
            );
        }
    }

    #[test]
    fn the_top_of_the_modal_states_the_context_and_the_case_rule() {
        let rows = modal_rows(PaneId::TradingWatch, 100, 45);
        let head = rows[..4.min(rows.len())].concat();
        assert!(
            head.contains("keys for the Watch pane"),
            "the help must say which pane it is for: {head:?}"
        );
        assert!(
            head.contains("Case matters"),
            "the case rule belongs at the top: {head:?}"
        );
    }

    #[test]
    fn the_watch_section_is_broken_into_named_groups() {
        let text = modal_rows(PaneId::TradingWatch, 100, TALL).concat();
        for heading in ["Instruments", "Trading", "Links", "Watch · this"] {
            assert!(text.contains(heading), "missing heading {heading:?}");
        }
    }
}
