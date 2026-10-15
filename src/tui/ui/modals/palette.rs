use super::layout::modal_block;
use super::search::instrument_row_spans;
use super::side_color;
use crate::tui::app::{App, ExecItem, PaletteAction, PaletteRow, PaletteState};
use crate::tui::format;
use crate::tui::palette::{Draft, Jump, WatchlistCmd, WatchlistDraft};
use crate::tui::settings;
use crate::tui::ui::centered;
use crate::tui::ui::{HitAction, HitList};
use crate::tui::workspace::PaneId;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Wrap};
use std::fmt::Write;

pub(super) fn draw_palette(frame: &mut Frame, state: &PaletteState, app: &App) {
    let area = centered(frame.area(), 64, 20);
    let inner = modal_block(
        frame,
        app,
        area,
        " Commands ".to_string(),
        Style::default().fg(format::accent()),
    );
    let [input_area, context_area, results_area, hint_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(inner);

    let input_line = Line::from(vec![
        Span::styled("❯ ", Style::default().fg(format::accent())),
        Span::raw(state.input.clone()),
        Span::styled("▏", Style::default().fg(format::accent())),
    ]);
    frame.render_widget(Paragraph::new(input_line), input_area);
    frame.render_widget(
        Paragraph::new(Span::styled(app.palette_context(state), format::dim())),
        context_area,
    );
    frame.render_widget(
        Paragraph::new(Line::from(palette_hint_spans(state))),
        hint_area,
    );

    if state.results.is_empty() {
        if let Some(message) = palette_empty_message(app, state) {
            frame.render_widget(
                Paragraph::new(Span::styled(message, format::dim())).wrap(Wrap { trim: true }),
                results_area,
            );
        }
        return;
    }

    let content_width = results_area.width.saturating_sub(2) as usize;
    let items: Vec<ListItem> = state
        .results
        .iter()
        .enumerate()
        .map(|(idx, row)| palette_list_item(idx, row, content_width))
        .collect();
    let mut list_state = ListState::default();
    list_state.select(state.selected.filter(|idx| *idx < state.results.len()));
    frame.render_stateful_widget(
        List::new(items)
            .highlight_style(format::selection_style())
            .highlight_symbol("▶ "),
        results_area,
        &mut list_state,
    );

    let mut y = results_area.y;
    for (idx, row) in state.results.iter().enumerate().skip(list_state.offset()) {
        let height =
            u16::try_from(palette_row_lines(idx, row, content_width).len()).unwrap_or(u16::MAX);
        if y.saturating_add(height) > results_area.bottom() {
            break;
        }
        if !matches!(row, PaletteRow::Header(_)) {
            app.hit.borrow_mut().push(
                Rect {
                    x: results_area.x,
                    y,
                    width: results_area.width,
                    height,
                },
                HitAction::Row {
                    list: HitList::Palette,
                    index: idx,
                },
            );
        }
        y += height;
    }
}

fn palette_empty_message(app: &App, state: &PaletteState) -> Option<&'static str> {
    match &state.draft {
        Draft::Order(order) if order.error.is_some() => None,
        Draft::Watchlist(WatchlistDraft::DeleteExtra | WatchlistDraft::RenameMissing { .. }) => {
            None
        }
        Draft::Order(_) | Draft::Alert { .. } | Draft::Search | Draft::Partial
            if app.instruments.is_none() =>
        {
            Some("Instrument master not available")
        }
        _ => Some("no matches"),
    }
}

fn palette_hint_spans(state: &PaletteState) -> Vec<Span<'static>> {
    let mut hints: Vec<String> = Vec::new();
    if state.can_complete() {
        hints.push("Tab complete".to_string());
    }
    if let Some(effect) = state.enter_effect() {
        hints.push(format!("Enter {effect}"));
    }
    hints.push("Esc close".to_string());
    hints.push("↑↓/wheel select".to_string());
    let mut spans = Vec::new();
    for (idx, hint) in hints.into_iter().enumerate() {
        if idx > 0 {
            spans.push(Span::styled(" · ", format::dim()));
        }
        spans.push(Span::styled(hint, format::dim()));
    }
    spans
}

fn palette_list_item(idx: usize, row: &PaletteRow, width: usize) -> ListItem<'static> {
    ListItem::new(palette_row_lines(idx, row, width))
}

fn palette_row_lines(idx: usize, row: &PaletteRow, width: usize) -> Vec<Line<'static>> {
    match row {
        PaletteRow::Header(label) => {
            let mut lines = Vec::new();
            if idx != 0 {
                lines.push(Line::default());
            }
            lines.push(Line::from(Span::styled(*label, format::title_style())));
            lines
        }
        PaletteRow::Item { action, key } => {
            let mut spans = action_content(action);
            if let Some(key) = key {
                let used: usize =
                    spans.iter().map(Span::width).sum::<usize>() + format::display_width(key);
                let pad = width.saturating_sub(used).max(1);
                spans.push(Span::raw(" ".repeat(pad)));
                spans.push(Span::styled((*key).to_string(), format::dim()));
            }
            vec![Line::from(spans)]
        }
    }
}

fn action_content(action: &PaletteAction) -> Vec<Span<'static>> {
    match action {
        PaletteAction::Execute { item, .. } => row_content(item),
        PaletteAction::Complete { label, .. } => vec![
            Span::styled(label.trim_end().to_string(), format::bold()),
            Span::styled(" , complete", format::dim()),
        ],
    }
}

fn row_content(item: &ExecItem) -> Vec<Span<'static>> {
    match item {
        ExecItem::Order {
            side,
            instrument,
            qty,
            price,
        } => {
            let mut text = format!("{} ", side.label());
            if let Some(qty) = qty {
                write!(text, "{qty} ").expect("writing to a String");
            }
            text.push_str(&instrument.label());
            if let Some(price) = price {
                write!(text, " @ {}", format::price(*price)).expect("writing to a String");
            }
            vec![
                Span::styled(
                    text,
                    Style::default()
                        .fg(side_color(*side))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" , open ticket", format::dim()),
            ]
        }
        ExecItem::Alert(inst) => vec![
            Span::styled(format!("alert {}", inst.label()), format::bold()),
            Span::styled(" , open the alert form", format::dim()),
        ],
        ExecItem::Jump(Jump::Workspace(preset)) => {
            let panes: Vec<&str> = preset.panes().iter().copied().map(PaneId::title).collect();
            vec![
                Span::styled(format!("→ {}", preset.title()), format::bold()),
                Span::styled(format!(" , {}", panes.join(" · ")), format::dim()),
            ]
        }
        ExecItem::Jump(Jump::Pane(pane)) => vec![
            Span::styled(format!("→ {}", pane.title()), format::bold()),
            Span::styled(
                format!(" , {} workspace", pane.preset().title()),
                format::dim(),
            ),
        ],
        ExecItem::FocusPane(pane) => vec![
            Span::styled(format!("→ {}", pane.title()), format::bold()),
            Span::styled(" , focus this pane", format::dim()),
        ],
        ExecItem::Quit => vec![Span::styled("Quit", format::bold())],
        ExecItem::Watchlist(cmd) => {
            let (label, hint) = match cmd {
                WatchlistCmd::Switch(name) => (
                    format!("watchlist {name}"),
                    " , switch / create on your account",
                ),
                WatchlistCmd::SwitchId { id, name } => {
                    (format!("watchlist {name} ({id})"), " , switch to this list")
                }
                WatchlistCmd::Rename(name) => {
                    (format!("rename → {name}"), " , rename the open list")
                }
                WatchlistCmd::Delete => ("delete list".to_string(), " , delete the open list"),
            };
            vec![
                Span::styled(label, format::bold()),
                Span::styled(hint, format::dim()),
            ]
        }
        ExecItem::Theme(name) => {
            let theme = settings::preset(name).unwrap_or(settings::Theme::DEFAULT);
            let marker = if theme == settings::theme() {
                "● "
            } else {
                "  "
            };
            vec![
                Span::styled(marker, Style::default().fg(format::accent())),
                Span::styled("██ ", Style::default().fg(theme.accent)),
                Span::styled(format!("theme · {name}"), format::bold()),
            ]
        }
        ExecItem::Action(action) => vec![Span::styled(action.label(), format::bold())],
        ExecItem::Instrument(inst) => instrument_row_spans(inst, false),
    }
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{rendered_text, test_app};
    use super::{palette_row_lines, row_content};
    use crate::tui::app::{App, ExecItem, MenuAction, PaletteAction, PaletteRow, PaletteState};
    use crate::tui::instruments::Instrument;
    use crate::tui::workspace::PaneId;

    #[test]
    fn row_content_covers_every_variant() {
        for item in [
            ExecItem::Action(MenuAction::Quit),
            ExecItem::Jump(crate::tui::palette::Jump::Workspace(
                crate::tui::workspace::Preset::Trading,
            )),
            ExecItem::Theme("dracula".to_string()),
            ExecItem::Quit,
        ] {
            assert_ne!(row_content(&item).len(), 0);
        }
        let spans = row_content(&ExecItem::Action(MenuAction::RefreshAll));
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "Refresh");
        let spans = row_content(&ExecItem::Alert(alert_instrument()));
        let text: String = spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(text, "alert NSE:TCS , open the alert form");
    }

    fn alert_instrument() -> Instrument {
        Instrument {
            instrument_token: 2_953_217,
            tradingsymbol: "TCS".to_string(),
            name: "TCS".to_string(),
            exchange: "NSE".to_string(),
            expiry: None,
            strike: None,
            tick_size: 0.05,
            lot_size: 1,
            instrument_type: "EQ".to_string(),
            segment: "NSE".to_string(),
            freeze_quantity: None,
            max_order_quantity: None,
        }
    }

    #[test]
    fn palette_list_item_right_aligns_key_within_width() {
        let row = PaletteRow::Item {
            action: PaletteAction::Execute {
                item: Box::new(ExecItem::Action(MenuAction::RefreshAll)),
                completion: None,
            },
            key: Some("r"),
        };
        let lines = palette_row_lines(1, &row, 40);
        assert_eq!(lines.len(), 1);
        let line = &lines[0];
        let width: usize = line.spans.iter().map(ratatui::text::Span::width).sum();
        assert_eq!(width, 40, "item should fill the width budget");
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(text.starts_with("Refresh"), "label first: {text:?}");
        assert!(text.ends_with('r'), "key right-aligned last: {text:?}");
    }

    fn palette_state(app: &App, input: &str) -> PaletteState {
        let mut state = PaletteState {
            input: input.to_string(),
            ..PaletteState::for_test()
        };
        app.rebuild_palette_for_test(&mut state);
        state
    }

    #[test]
    fn the_palette_draws_the_contextual_syntax_on_its_second_row() {
        let app = test_app();
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "buy"), &app);
        });
        assert!(
            text.contains("buy SYMBOL [QTY] [@PRICE]"),
            "the side syntax is missing: {text}"
        );
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "buy infy 0"), &app);
        });
        assert!(
            text.contains("Quantity must be 1..4294967295"),
            "the quantity rule is missing: {text}"
        );
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "buy infy @-1"), &app);
        });
        assert!(
            text.contains("Price must be finite and > 0"),
            "the price rule is missing: {text}"
        );
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "wl delete"), &app);
        });
        assert!(
            text.contains("Delete default"),
            "the delete effect is missing: {text}"
        );
    }

    #[test]
    fn the_palette_draws_the_new_alert_row_off_the_alerts_view() {
        let mut app = test_app();
        app.set_pane(PaneId::PortfolioPositions);
        let text = rendered_text(100, 40, |frame| {
            super::draw_palette(frame, &palette_state(&app, ""), &app);
        });
        assert!(
            text.contains("New alert…"),
            "the menu row is missing off the Alerts view: {text}"
        );

        let text = rendered_text(100, 40, |frame| {
            super::draw_palette(frame, &palette_state(&app, "alert"), &app);
        });
        assert!(
            text.contains("alert SYMBOL · Enter picks an instrument"),
            "the alert syntax is missing: {text}"
        );
        assert!(
            text.contains("New alert…"),
            "a bare alert still offers the picker: {text}"
        );
    }

    #[test]
    fn the_palette_footer_offers_tab_only_when_a_row_completes() {
        let app = test_app();
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "bu"), &app);
        });
        assert!(text.contains("Tab complete"), "Tab is missing: {text}");
        assert!(
            text.contains("Enter complete"),
            "a completion row's Enter says complete, not run: {text}"
        );
        assert!(text.contains("Esc close"), "Esc is missing: {text}");
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "quit"), &app);
        });
        assert!(
            !text.contains("Tab complete"),
            "Tab must be omitted with no useful completion: {text}"
        );
        assert!(
            text.contains("Enter quits"),
            "the effect is missing: {text}"
        );
    }

    #[test]
    fn an_absent_instrument_master_says_so_rather_than_no_matches() {
        let app = test_app();
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "zzz"), &app);
        });
        assert!(
            text.contains("Instrument master not available"),
            "the neutral message is missing: {text}"
        );
        let text = rendered_text(80, 24, |frame| {
            super::draw_palette(frame, &palette_state(&app, "theme notatheme"), &app);
        });
        assert!(text.contains("no matches"), "expected no matches: {text}");
        assert!(
            text.contains("Unknown theme: notatheme"),
            "the unknown-theme message is missing: {text}"
        );
    }

    #[test]
    fn the_palette_keeps_the_selected_row_visible_and_draws_at_tiny_sizes() {
        let app = test_app();
        let state = palette_state(&app, "");
        let mut state = PaletteState {
            selected: state
                .results
                .iter()
                .enumerate()
                .filter(|(_, row)| matches!(row, PaletteRow::Item { .. }))
                .map(|(idx, _)| idx)
                .next_back(),
            ..state
        };
        let text = rendered_text(80, 24, |frame| super::draw_palette(frame, &state, &app));
        assert!(text.contains("Quit"), "the selected last row is off screen");
        for (w, h) in [(120, 35), (200, 60), (40, 10), (20, 6), (8, 3), (2, 1)] {
            state.selected = Some(1);
            let _ = rendered_text(w, h, |frame| super::draw_palette(frame, &state, &app));
        }
    }

    #[test]
    fn palette_header_has_blank_line_except_first() {
        assert_eq!(
            palette_row_lines(0, &PaletteRow::Header("Suggested"), 40).len(),
            1
        );
        assert_eq!(
            palette_row_lines(5, &PaletteRow::Header("View"), 40).len(),
            2
        );
    }
}
