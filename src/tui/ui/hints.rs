use super::{HitAction, HitRegion, bar_style, columns, forms, modals};
use crate::tui::app::{AlertFormStage, App, Modal, SearchPurpose};
use crate::tui::format;
use crate::tui::workspace::PaneId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

const HINT_SEP: &str = "  ";

const HINT_GROUP_SEP: &str = " │ ";

pub(super) fn fit_hints(budget: u16, sep: &str, hints: &[&str]) -> String {
    let widths: Vec<columns::FitCol> = hints
        .iter()
        .enumerate()
        .map(|(idx, hint)| {
            columns::col(
                u16::try_from(hint.chars().count()).unwrap_or(u16::MAX),
                u8::try_from(idx).unwrap_or(u8::MAX),
            )
        })
        .collect();
    let fit = columns::fit_columns(
        budget,
        u16::try_from(sep.chars().count()).unwrap_or(u16::MAX),
        &widths,
    );
    let kept: Vec<&str> = hints
        .iter()
        .enumerate()
        .filter(|(idx, _)| fit.shows(*idx))
        .map(|(_, hint)| *hint)
        .collect();
    let width: usize = kept.iter().map(|hint| hint.chars().count()).sum::<usize>()
        + sep.chars().count() * kept.len().saturating_sub(1);
    if width > budget as usize {
        return String::new();
    }
    kept.join(sep)
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HintGroup {
    rank: u8,
    pub(super) hints: &'static [&'static str],
}

pub(super) const fn always(hints: &'static [&'static str]) -> HintGroup {
    HintGroup { rank: 0, hints }
}

const fn trade(hints: &'static [&'static str]) -> HintGroup {
    HintGroup { rank: 1, hints }
}

pub(super) const fn act(hints: &'static [&'static str]) -> HintGroup {
    HintGroup { rank: 2, hints }
}

pub(super) const fn nav(hints: &'static [&'static str]) -> HintGroup {
    HintGroup { rank: 3, hints }
}

const fn show(hints: &'static [&'static str]) -> HintGroup {
    HintGroup { rank: 4, hints }
}

const fn app(hints: &'static [&'static str]) -> HintGroup {
    HintGroup { rank: 5, hints }
}

fn hint_line_width(groups: &[HintGroup], kept: &[usize]) -> usize {
    let mut width = 0;
    let mut shown = 0;
    for (group, count) in groups.iter().zip(kept) {
        if *count == 0 {
            continue;
        }
        if shown > 0 {
            width += HINT_GROUP_SEP.chars().count();
        }
        shown += 1;
        width += group.hints[..*count]
            .iter()
            .map(|hint| hint.chars().count())
            .sum::<usize>()
            + HINT_SEP.chars().count() * (count - 1);
    }
    width
}

fn fitted_hint_groups(budget: u16, groups: &[HintGroup]) -> Vec<&'static [&'static str]> {
    let mut kept: Vec<usize> = groups.iter().map(|group| group.hints.len()).collect();
    while hint_line_width(groups, &kept) > budget as usize {
        let victim = groups
            .iter()
            .enumerate()
            .filter(|(idx, _)| kept[*idx] > 0)
            .max_by_key(|(idx, group)| (group.rank, *idx))
            .map(|(idx, _)| idx);
        match victim {
            Some(idx) => kept[idx] -= 1,
            None => break,
        }
    }
    groups
        .iter()
        .zip(&kept)
        .map(|(group, count)| &group.hints[..*count])
        .filter(|hints| !hints.is_empty())
        .collect()
}

#[cfg(test)]
fn hint_line(budget: u16, groups: &[HintGroup]) -> String {
    fitted_hint_groups(budget, groups)
        .iter()
        .map(|hints| hints.join(HINT_SEP))
        .collect::<Vec<String>>()
        .join(HINT_GROUP_SEP)
}

pub(super) fn hint_key(hint: &str) -> Option<KeyEvent> {
    let token = hint.split([':', ' ']).next()?;
    let first = match token.split('/').next() {
        Some(alternative) if !alternative.is_empty() => alternative,
        _ => token,
    };
    let code = match first {
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Tab" => KeyCode::Tab,
        "Space" => KeyCode::Char(' '),
        "Ctrl-A" | "Ctrl-K" | "Ctrl-L" => {
            let c = first.chars().last()?.to_ascii_lowercase();
            return Some(KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL));
        }
        _ => {
            let mut chars = first.chars();
            let c = chars.next()?;
            if chars.next().is_some() || !c.is_ascii_graphic() {
                return None;
            }
            KeyCode::Char(c)
        }
    };
    Some(KeyEvent::new(code, KeyModifiers::NONE))
}

#[expect(
    clippy::too_many_lines,
    reason = "exhaustive modal hint table keeps precedence visible"
)]
pub(super) fn hints_for(
    modal: Option<&Modal>,
    chart_panel: bool,
    focused: PaneId,
    notes_blocked: bool,
) -> &'static [HintGroup] {
    match modal {
        Some(Modal::Help(_)) => {
            const {
                &[
                    nav(&["j/k:scroll", "g/G:top/bottom"]),
                    always(&["q/Esc:close"]),
                ]
            }
        }
        Some(Modal::Search(state)) => match state.purpose {
            SearchPurpose::AddToWatch => {
                const {
                    &[
                        nav(&["type to search", "↑↓:select"]),
                        always(&["Enter:add", "Esc:close"]),
                    ]
                }
            }
            SearchPurpose::ChainUnderlying => {
                const {
                    &[
                        nav(&["type to search", "↑↓:select"]),
                        always(&["Enter:open chain", "Esc:close"]),
                    ]
                }
            }
            SearchPurpose::NewAlert => {
                const {
                    &[
                        nav(&["type to search", "↑↓:select"]),
                        always(&["Enter:set alert", "Esc:close"]),
                    ]
                }
            }
            SearchPurpose::AtoBasketItem => {
                const {
                    &[
                        nav(&["type to search", "↑↓:select"]),
                        always(&["Enter:build order", "Esc:back"]),
                    ]
                }
            }
        },
        Some(Modal::Palette(_)) => {
            const {
                &[
                    nav(&["type to filter", "Tab:complete", "↑↓:select"]),
                    always(&["Esc:close"]),
                ]
            }
        }
        Some(Modal::WorkspacePicker(_)) => modals::workspace_picker_footer().hints,
        Some(Modal::ThemePicker(state)) => {
            modals::theme_picker_footer(!state.results.is_empty()).hints
        }
        Some(Modal::Ticket(state)) => modals::ticket_footer(state).hints,
        Some(Modal::PositionConvert(_)) => modals::conversion_footer().hints,
        Some(Modal::Confirm(state)) => modals::confirm_footer(state).hints,
        Some(Modal::OrderDetail(_)) => const { &[always(&["Enter/Esc:close"])] },
        Some(Modal::Alert(state)) => modals::alert_footer(state).hints,
        Some(Modal::AtoItem(state)) => modals::ato_item_footer(state).hints,
        Some(Modal::EventLog(_)) => modals::event_log_footer().hints,
        Some(Modal::Funds) => const { &[always(&["Esc:close"])] },
        Some(Modal::Notes(state)) => {
            if state.editor.is_some() {
                modals::note_editor_footer(notes_blocked).hints
            } else {
                modals::notes_footer(notes_blocked, state.attach.is_some()).hints
            }
        }
        None if chart_panel => {
            const {
                &[
                    always(&["Ctrl-K:commands", "Esc:close chart", "?:help"]),
                    nav(&["j/k:move", "h/l:scan"]),
                    show(&["C:grow", "i:interval", "v:volume", "I:indicator"]),
                    app(&["n:notes", "T:theme", "q:quit"]),
                ]
            }
        }
        None => match focused {
            PaneId::OverviewHoldings => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "?:help"]),
                        act(&["Enter:Holdings"]),
                        nav(&["Tab:pane", "[ ]:books", "j/k:move", "←/→:workspaces"]),
                        show(&["F:funds", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::OverviewPositions => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "?:help"]),
                        act(&["Enter:Positions"]),
                        nav(&["Tab:pane", "[ ]:books", "j/k:move", "←/→:workspaces"]),
                        show(&["F:funds", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::OverviewIndices => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "?:help"]),
                        nav(&["Tab:pane", "j/k:scroll", "←/→:workspaces"]),
                        show(&["r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::OverviewMovers => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "?:help"]),
                        nav(&["Tab:pane", "j/k:scroll", "←/→:workspaces"]),
                        show(&["u:universe", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::TradingWatch => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        trade(&["b:buy", "s:sell", "g:gtt", "a:alert"]),
                        act(&["/:add", "d:remove", "J/K:reorder"]),
                        nav(&["j/k:move", "[ ]:lists", "w:watchlists"]),
                        show(&["r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::TradingChart | PaneId::DerivativesChart => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        nav(&["h/l:scan"]),
                        show(&[
                            "i:interval",
                            "v:volume",
                            "I:indicator",
                            "z:zoom",
                            "=:dividers",
                        ]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::TradingDepth => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        act(&["Enter:quote"]),
                        show(&["r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::TradingOrders | PaneId::OrdersBook => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        act(&[
                            "Space:mark",
                            "Ctrl-A:mark all",
                            "x:cancel",
                            "m:modify",
                            "Enter:detail",
                        ]),
                        nav(&["j/k:move"]),
                        show(&["c:chart", "S:sort", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::PortfolioPositions => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        trade(&["b:buy", "s:sell", "a:alert"]),
                        act(&["c:convert"]),
                        nav(&["j/k:move"]),
                        show(&["S:sort", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::PortfolioHoldings => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        trade(&["b:buy", "s:sell", "g:gtt", "B:alert"]),
                        act(&["a:authorise"]),
                        nav(&["j/k:move"]),
                        show(&["c:chart", "S:sort", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::PortfolioFunds => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        nav(&["j/k:scroll"]),
                        show(&["F:detail", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::DerivativesChain => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        trade(&["b:buy", "s:sell", "a:alert"]),
                        nav(&["j/k:strike", "[ ]:expiry", "h/l:side", "u:underlying"]),
                        show(&["r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::OrdersGtt => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        act(&["m:modify", "x:delete"]),
                        nav(&["j/k:move"]),
                        show(&["S:sort", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::OrdersAlerts => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        act(&[
                            "r:on/off",
                            "a:new",
                            "e:edit",
                            "d:delete",
                            "Enter:history",
                            "Esc:close history",
                        ]),
                        nav(&["j/k:move"]),
                        show(&["S:sort", "R:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::InvestMf => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        nav(&["j/k:move"]),
                        show(&["S:sort", "r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
            PaneId::InvestBids => {
                const {
                    &[
                        always(&["Ctrl-K:commands", "Tab:pane", "?:help"]),
                        trade(&["s:sell auction"]),
                        act(&["Enter:Kite Web"]),
                        nav(&["[ ]:section", "a:applications", "j/k:move"]),
                        show(&["r:refresh", "z:zoom", "=:dividers"]),
                        app(&["n:notes", "T:theme", "q:quit"]),
                    ]
                }
            }
        },
    }
}

fn hint_group_sep_style() -> Style {
    format::dim().add_modifier(Modifier::DIM)
}

pub(super) fn draw_hints(frame: &mut Frame, area: Rect, app: &App) {
    let review = match app.modals.last() {
        Some(Modal::Confirm(_)) => true,
        Some(Modal::Alert(state)) => state.stage == AlertFormStage::Review,
        _ => false,
    };
    let hints = if review && !forms::contract_fits(frame.area()) {
        const { &[always(&["Esc:back"])] }
    } else {
        hints_for(
            app.modals.last(),
            app.chart_panel_pane() == Some(app.focused_pane()),
            app.focused_pane(),
            app.notes.blocked(),
        )
    };
    let temporary_groups;
    let hints = if app.watch.is_ephemeral() {
        temporary_groups = hints
            .iter()
            .map(|group| {
                if group.hints.contains(&"[ ]:lists") {
                    HintGroup {
                        hints: &["j/k:move"],
                        ..*group
                    }
                } else {
                    *group
                }
            })
            .collect::<Vec<_>>();
        temporary_groups.as_slice()
    } else {
        hints
    };
    let groups = fitted_hint_groups(area.width.saturating_sub(1), hints);

    let mut spans = vec![Span::raw(" ")];
    let mut x = area.x.saturating_add(1);
    for (group_idx, hints) in groups.iter().enumerate() {
        if group_idx > 0 {
            spans.push(Span::styled(HINT_GROUP_SEP, hint_group_sep_style()));
            x = x.saturating_add(u16::try_from(HINT_GROUP_SEP.chars().count()).unwrap_or(u16::MAX));
        }
        for (idx, hint) in hints.iter().enumerate() {
            if idx > 0 {
                spans.push(Span::styled(HINT_SEP, format::dim()));
                x = x.saturating_add(u16::try_from(HINT_SEP.chars().count()).unwrap_or(u16::MAX));
            }
            let width = u16::try_from(hint.chars().count()).unwrap_or(u16::MAX);
            if let Some(key) = hint_key(hint) {
                app.hit.borrow_mut().hints.push(HitRegion {
                    rect: Rect {
                        x,
                        y: area.y,
                        width,
                        height: 1,
                    },
                    action: HitAction::Key(key),
                });
            }
            spans.push(Span::styled(*hint, format::dim()));
            x = x.saturating_add(width);
        }
    }

    frame.render_widget(Paragraph::new(Line::from(spans)).style(bar_style()), area);
}

#[cfg(test)]
mod tests {
    use super::{HINT_GROUP_SEP, HINT_SEP, HintGroup, fit_hints, hint_key, hint_line, hints_for};
    use crate::tui::settings;
    use crate::tui::ui::chrome_tests::{root_buffer, row, test_app};
    use crate::tui::workspace::{PaneId, Preset};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use std::fmt::Write;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn hint_key_presses_the_first_key_a_hint_names() {
        assert_eq!(hint_key("q:quit"), Some(key(KeyCode::Char('q'))));
        assert_eq!(hint_key("S:sort"), Some(key(KeyCode::Char('S'))));
        assert_eq!(hint_key("?:help"), Some(key(KeyCode::Char('?'))));
        assert_eq!(hint_key("/:add"), Some(key(KeyCode::Char('/'))));
        assert_eq!(hint_key("Enter:open"), Some(key(KeyCode::Enter)));
        assert_eq!(hint_key("Esc:close"), Some(key(KeyCode::Esc)));
        assert_eq!(hint_key("Tab:field"), Some(key(KeyCode::Tab)));
        assert_eq!(hint_key("Space:mark"), Some(key(KeyCode::Char(' '))));
        assert_eq!(hint_key("a:authorise"), Some(key(KeyCode::Char('a'))));
        assert_eq!(hint_key("q/Esc:close"), Some(key(KeyCode::Char('q'))));
        assert_eq!(hint_key("e/Enter:edit"), Some(key(KeyCode::Char('e'))));
        assert_eq!(hint_key("[ ]:section"), Some(key(KeyCode::Char('['))));
        assert_eq!(
            hint_key("Ctrl-K:menu"),
            Some(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL))
        );
        assert_eq!(
            hint_key("Ctrl-A:mark all"),
            Some(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL))
        );
    }

    #[test]
    fn hint_key_declines_hints_that_name_no_key() {
        for hint in [
            "type to search",
            "type to write",
            "type qty",
            "type/↑↓:value",
            "↑↓:select",
            "↑↓:step",
            "←/→:views",
            "←→/space:choose",
            "←→↑↓/Home/End:move",
            "1-9/0:views",
            "sending to Kite…",
        ] {
            assert_eq!(hint_key(hint), None, "{hint:?} should not be clickable");
        }
    }

    fn all_hints(groups: &[HintGroup]) -> Vec<&'static str> {
        groups
            .iter()
            .flat_map(|group| group.hints.iter().copied())
            .collect()
    }

    fn every_pane() -> Vec<PaneId> {
        Preset::ALL
            .into_iter()
            .flat_map(Preset::panes)
            .copied()
            .collect()
    }

    fn every_view_line() -> Vec<(String, &'static [HintGroup])> {
        let mut lines: Vec<(String, &'static [HintGroup])> = every_pane()
            .into_iter()
            .map(|pane| (format!("{pane:?}"), hints_for(None, false, pane, false)))
            .collect();
        lines.push((
            "chart panel".to_string(),
            hints_for(None, true, PaneId::OrdersBook, false),
        ));
        lines
    }

    #[test]
    fn every_rendered_view_hint_either_names_its_key_or_is_inert() {
        for (view, groups) in every_view_line() {
            for hint in all_hints(groups) {
                let Some(event) = hint_key(hint) else {
                    continue;
                };
                let named = match event.code {
                    KeyCode::Char(c) if event.modifiers.contains(KeyModifiers::CONTROL) => {
                        format!("Ctrl-{}", c.to_ascii_uppercase())
                    }
                    KeyCode::Char(' ') => "Space".to_string(),
                    KeyCode::Char(c) => c.to_string(),
                    KeyCode::Enter => "Enter".to_string(),
                    KeyCode::Esc => "Esc".to_string(),
                    KeyCode::Tab => "Tab".to_string(),
                    other => panic!("{view:?} hint {hint:?} produced {other:?}"),
                };
                assert!(
                    hint.starts_with(&named),
                    "{view:?} hint {hint:?} does not name {named:?}"
                );
            }
        }
    }

    #[test]
    fn the_fitted_groups_are_exactly_what_the_line_says() {
        for (name, groups) in every_view_line() {
            for budget in 0..=200u16 {
                let kept = super::fitted_hint_groups(budget, groups);
                let rebuilt: Vec<String> = kept.iter().map(|hints| hints.join(HINT_SEP)).collect();
                assert_eq!(
                    rebuilt.join(HINT_GROUP_SEP),
                    hint_line(budget, groups),
                    "{name} at {budget}"
                );
                assert!(
                    kept.iter().all(|hints| !hints.is_empty()),
                    "{name} at {budget} kept an empty group"
                );
            }
        }
    }

    #[test]
    fn fit_hints_drops_from_the_least_important_end() {
        let hints = ["?:help", "b:buy", "s:sell", "T:theme", "q:quit"];
        assert_eq!(
            fit_hints(38, "  ", &hints),
            "?:help  b:buy  s:sell  T:theme  q:quit"
        );
        assert_eq!(
            fit_hints(37, "  ", &hints),
            "?:help  b:buy  s:sell  T:theme"
        );
        assert_eq!(fit_hints(20, "  ", &hints), "?:help  b:buy");
        assert_eq!(fit_hints(6, "  ", &hints), "?:help");
        assert_eq!(fit_hints(5, "  ", &hints), "");
        assert_eq!(fit_hints(0, "  ", &[]), "");
    }

    #[test]
    fn no_hint_line_overflows_its_terminal_width() {
        for (name, groups) in every_view_line() {
            for width in 1..=200u16 {
                let line = hint_line(width - 1, groups);
                let rendered = line.chars().count() + 1; // the leading space
                assert!(
                    rendered <= width as usize,
                    "{name} at width {width} renders {rendered} columns: {line:?}"
                );
            }
        }
    }

    #[test]
    fn the_fitter_never_exceeds_its_budget() {
        let groups = [
            super::nav(&[
                "a very long hint that no terminal is this wide for",
                "j/k:move",
            ]),
            super::always(&["Enter:go", "Esc:close"]),
            super::app(&[]),
            super::trade(&["b:buy", "s:sell"]),
        ];
        for budget in 0..=120u16 {
            let line = hint_line(budget, &groups);
            assert!(
                line.chars().count() <= budget as usize,
                "budget {budget} rendered {} columns: {line:?}",
                line.chars().count()
            );
        }
    }

    #[test]
    fn the_confirm_hint_line_matches_what_the_action_does() {
        use crate::tui::ticket::{ConfirmState, PendingAction};
        let hints_for_action = |action: PendingAction| {
            let modal = crate::tui::app::Modal::Confirm(Box::new(ConfirmState::plain(
                "t".to_string(),
                Vec::new(),
                action,
            )));
            all_hints(super::hints_for(
                Some(&modal),
                false,
                PaneId::OverviewPositions,
                false,
            ))
            .join(" ")
        };
        let local = hints_for_action(PendingAction::DeleteNote {
            id: 1,
            label: "x".to_string(),
        });
        assert!(local.contains("delete locally"), "{local:?}");
        assert!(!local.contains("send"), "{local:?}");
        assert!(
            hints_for_action(PendingAction::DeleteGtt { trigger_id: 1 }).contains("delete GTT")
        );
        assert!(
            hints_for_action(PendingAction::Logout).contains("clear session and exit"),
            "a logout names what it clears"
        );
        assert!(
            hints_for_action(PendingAction::Place {
                variety: "regular".to_string(),
                form: Vec::new(),
            })
            .contains("place order")
        );
    }

    #[test]
    fn orders_hints_lead_with_mark_and_bulk_cancel() {
        let hints = all_hints(super::hints_for(None, false, PaneId::OrdersBook, false));
        for hint in ["Space:mark", "Ctrl-A:mark all", "x:cancel"] {
            assert!(
                hints.contains(&hint),
                "Orders hint line must document {hint}: {hints:?}"
            );
        }
    }

    #[test]
    fn bids_hints_document_section_and_application_navigation() {
        let hints = all_hints(super::hints_for(None, false, PaneId::InvestBids, false));
        assert!(hints.contains(&"[ ]:section"), "{hints:?}");
        assert!(hints.contains(&"a:applications"), "{hints:?}");
        assert!(hints.contains(&"Enter:Kite Web"), "{hints:?}");
    }

    #[test]
    fn the_watch_hint_line_keeps_its_trade_keys_at_eighty_columns() {
        let groups = super::hints_for(None, false, PaneId::TradingWatch, false);
        let line = hint_line(79, groups);
        assert!(line.chars().count() <= 79, "{line:?}");
        for hint in [
            "Ctrl-K:commands",
            "?:help",
            "b:buy",
            "s:sell",
            "g:gtt",
            "a:alert",
        ] {
            assert!(
                line.contains(hint),
                "{hint} dropped at 80 columns: {line:?}"
            );
        }
        for hint in ["q:quit", "T:theme", "n:notes", "z:zoom", "=:dividers"] {
            assert!(
                !line.contains(hint),
                "{hint} should not survive 80 columns: {line:?}"
            );
        }
        let wide = hint_line(400, groups);
        assert!(wide.contains("T:theme"), "{wide:?}");
        assert!(wide.contains("=:dividers"), "{wide:?}");
    }

    #[test]
    fn a_narrow_line_drops_whole_categories_least_important_first() {
        for (name, groups) in every_view_line() {
            for budget in 0..=200u16 {
                let line = hint_line(budget, groups);
                let mut clipped = false;
                for group in groups {
                    let kept = group
                        .hints
                        .iter()
                        .filter(|hint| line.contains(**hint))
                        .count();
                    if clipped {
                        assert_eq!(
                            kept, 0,
                            "{name} at {budget}: a more important group was clipped \
                             before {group:?} was emptied: {line:?}"
                        );
                    }
                    if kept < group.hints.len() {
                        clipped = true;
                    }
                }
            }
        }
    }

    #[test]
    fn every_view_line_renders_at_eighty_one_twenty_and_one_sixty_columns() {
        let mut report = String::new();
        for (name, groups) in every_view_line() {
            for width in [80u16, 120, 160] {
                let line = hint_line(width - 1, groups);
                assert!(
                    line.chars().count() < width as usize,
                    "{name} at {width}: {line:?}"
                );
                assert!(
                    line.starts_with("Ctrl-K:commands") && line.contains("?:help"),
                    "{name} at {width} lost the doors: {line:?}"
                );
                writeln!(report, "{name} @{width}: {line}").unwrap();
            }
            let line = hint_line(159, groups);
            for hint in groups
                .iter()
                .filter(|group| group.rank <= 3)
                .flat_map(|group| group.hints.iter())
            {
                assert!(
                    line.contains(hint),
                    "{name} at 160 dropped {hint}: {line:?}"
                );
            }
            let full = hint_line(400, groups);
            for hint in all_hints(groups) {
                assert!(full.contains(hint), "{name} dropped {hint}: {full:?}");
            }
        }
        let mut lines: Vec<String> = every_view_line()
            .iter()
            .filter(|(name, _)| name != "OrdersBook" && name != "DerivativesChart")
            .map(|(_, groups)| hint_line(400, groups))
            .collect();
        lines.sort();
        let count = lines.len();
        lines.dedup();
        assert_eq!(
            lines.len(),
            count,
            "two views render the same footer\n{report}"
        );
    }

    #[test]
    fn every_hint_label_follows_one_vocabulary() {
        for (name, groups) in every_view_line() {
            for hint in all_hints(groups) {
                let (keys, label) = hint
                    .split_once(':')
                    .unwrap_or_else(|| panic!("{name}: {hint:?} is not `key:label`"));
                assert!(!keys.is_empty(), "{name}: {hint:?} names no key");
                assert!(!label.is_empty(), "{name}: {hint:?} has no label");
                let proper = ["Positions", "Holdings", "Kite Web"];
                if !proper.contains(&label) {
                    assert!(
                        label.chars().next().is_some_and(|c| !c.is_uppercase()),
                        "{name}: {hint:?} label is not lowercase"
                    );
                }
                assert!(
                    !hint.contains("CDSL") && !hint.contains("older/newer"),
                    "{name}: {hint:?} kept the old wording"
                );
            }
        }
    }

    #[test]
    fn every_view_advertises_the_command_entry_and_help_at_eighty_columns() {
        for (name, groups) in every_view_line() {
            let first = groups.first().map_or(&[][..], |group| group.hints);
            assert!(
                first.first() == Some(&"Ctrl-K:commands") && first.contains(&"?:help"),
                "{name} must lead with the command entry and help: {first:?}"
            );
            let line = hint_line(79, groups);
            assert!(
                line.contains("Ctrl-K:commands"),
                "{name} dropped the command entry at 80 columns: {line:?}"
            );
            assert!(
                line.contains("?:help"),
                "{name} dropped help at 80 columns: {line:?}"
            );
        }
    }

    #[test]
    fn a_modal_hint_line_does_not_advertise_the_command_entry() {
        use crate::tui::app::Modal;
        use crate::tui::ticket::{ConfirmState, PendingAction};

        let confirm = Modal::Confirm(Box::new(ConfirmState::plain(
            "t".to_string(),
            Vec::new(),
            PendingAction::Logout,
        )));
        for modal in [Modal::Funds, Modal::Help(0), confirm] {
            let hints = all_hints(super::hints_for(
                Some(&modal),
                false,
                PaneId::OverviewPositions,
                false,
            ));
            assert!(
                !hints.iter().any(|hint| hint.contains("Ctrl-K")),
                "{hints:?}"
            );
            let escape = super::hints_for(Some(&modal), false, PaneId::OverviewPositions, false)
                .iter()
                .filter(|group| group.rank == 0)
                .flat_map(|group| group.hints.iter());
            assert!(
                escape.clone().any(|hint| hint.contains("Esc")),
                "a modal must never be able to drop its Esc hint: {hints:?}"
            );
        }
    }

    #[test]
    fn the_overview_enter_hint_names_the_book_it_would_open() {
        for (pane, expected, other) in [
            (
                PaneId::OverviewPositions,
                "Enter:Positions",
                "Enter:Holdings",
            ),
            (
                PaneId::OverviewHoldings,
                "Enter:Holdings",
                "Enter:Positions",
            ),
        ] {
            let groups = super::hints_for(None, false, pane, false);
            let hints = all_hints(groups);
            assert!(hints.contains(&expected), "{pane:?}: {hints:?}");
            assert!(!hints.contains(&other), "{pane:?}: {hints:?}");
            let line = hint_line(79, groups);
            for hint in [
                "Ctrl-K:commands",
                "Tab:pane",
                "[ ]:books",
                "j/k:move",
                expected,
                "?:help",
            ] {
                assert!(line.contains(hint), "{pane:?} dropped {hint}: {line:?}");
            }
        }
    }

    #[test]
    fn the_hint_bar_keeps_its_labels_and_click_targets() {
        let _guard = settings::theme_guard(settings::preset("kite").expect("preset exists"));
        let mut app = test_app();
        app.set_pane(PaneId::TradingWatch);
        let buffer = root_buffer(&app, 120, 24);
        let footer = row(&buffer, 23);
        assert!(footer.starts_with(" Ctrl-K:commands  "), "{footer:?}");
        assert!(footer.contains("?:help"), "{footer:?}");
        assert!(
            footer.contains(" │ "),
            "the group divider is gone: {footer:?}"
        );
        let hits = app.hit.borrow();
        assert!(
            !hits.hints.is_empty(),
            "the footer recorded no click targets"
        );
        let commands = hits.hints[0];
        assert_eq!(commands.rect.y, 23);
        assert_eq!(commands.rect.x, 1);
        assert_eq!(
            commands.action,
            super::HitAction::Key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('k'),
                crossterm::event::KeyModifiers::CONTROL,
            ))
        );
        assert!(
            !hits.regions.iter().any(|region| region.rect.y >= 22),
            "the toast band recorded a click target"
        );
    }
}
