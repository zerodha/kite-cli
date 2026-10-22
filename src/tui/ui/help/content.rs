use crate::tui::workspace::PaneId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Binding {
    key: &'static str,
    desc: &'static str,
}

const fn b(key: &'static str, desc: &'static str) -> Binding {
    Binding { key, desc }
}

#[derive(Debug, Clone)]
struct HelpGroup {
    title: Option<&'static str>,
    bindings: Vec<Binding>,
}

fn group(title: &'static str, bindings: Vec<Binding>) -> HelpGroup {
    HelpGroup {
        title: Some(title),
        bindings,
    }
}

fn flat_group(bindings: Vec<Binding>) -> HelpGroup {
    HelpGroup {
        title: None,
        bindings,
    }
}

#[derive(Debug, Clone)]
struct HelpSection {
    title: String,
    groups: Vec<HelpGroup>,
}

#[expect(
    clippy::too_many_lines,
    reason = "exhaustive per-view help table keeps key documentation together"
)]
fn help_bindings(pane: PaneId) -> Vec<HelpSection> {
    let global = HelpSection {
        title: "Global".to_string(),
        groups: vec![
            group(
                "Getting around",
                vec![
                    b(
                        "1-6",
                        "the six workspaces, in strip order: Overview, Trading, Portfolio, Option Chain, Orders, IPO & Bids",
                    ),
                    b("← / →", "previous / next workspace"),
                    b(
                        "`",
                        "workspace picker: every workspace with its number key and its panes",
                    ),
                    b("Tab / Shift-Tab", "next / previous pane of the workspace"),
                    b(
                        "z",
                        "give the focused pane the whole body; press it again to put the workspace back",
                    ),
                    b("j / k  ↑ / ↓", "move the selection down / up"),
                    b(
                        "g / G",
                        "jump to the first / last row (in Watch and Holdings, g creates a GTT instead)",
                    ),
                    b(
                        "mouse",
                        "click a workspace on the strip, a pane to focus it, a row, a column header to sort that pane, or a spot in a note to put the caret there · drag a panel border to resize the split · the wheel scrolls the pane under the pointer and nothing else",
                    ),
                    b("=", "put every dragged panel divider back"),
                ],
            ),
            group(
                "Acting",
                vec![
                    b(":", "command palette, e.g. :buy INFY 10 @1520"),
                    b(
                        "Ctrl-K",
                        "commands, the browsable action menu, which also lists every workspace and every pane of the one on screen",
                    ),
                    b(
                        ":workspace NAME",
                        "switch workspace by name: Overview, Trading, Portfolio, Option Chain, Orders or IPO & Bids",
                    ),
                    b("Tab", "in the palette: complete, never run"),
                    b("r", "refresh account data"),
                    b("A", "open the Orders workspace's Alerts pane"),
                    b(
                        ":alert [SYMBOL]",
                        "new price alert on any instrument, from any pane",
                    ),
                ],
            ),
            group(
                "Notes and tools",
                vec![
                    b("n", "notes, a new note attaches to the selected row"),
                    b("N", "in the notes list: start a note attached to nothing"),
                    b("T", "theme picker"),
                    b("F", "funds / margin breakdown"),
                    b("Ctrl-L", "event log"),
                ],
            ),
            group(
                "Inside a form or a modal",
                vec![
                    b("Tab / Shift-Tab", "next / previous field in a form"),
                    b("Home / End", "in a text field: start / end of the line"),
                    b(
                        "PgUp / PgDn",
                        "in the event log: scroll the selected message's detail, not the list",
                    ),
                    b("space", "with a choice field focused: take the next option"),
                    b("e", "in the notes list: edit the selected note"),
                ],
            ),
            group(
                "Getting out",
                vec![
                    b("?", "toggle this help"),
                    b("Esc", "close the top modal / step back"),
                    b("q / Ctrl-C", "quit"),
                ],
            ),
        ],
    };
    let view_groups = match pane {
        PaneId::PortfolioPositions => vec![
            group(
                "Trading",
                vec![
                    b("b / s", "buy / sell the selected position"),
                    b("a", "set price alert (server-side, simple or ATO)"),
                    b(
                        "c",
                        "convert its DAY or OVERNIGHT quantity to Kite's target product",
                    ),
                ],
            ),
            group("Chart", positions_chart_bindings()),
            group("Table", vec![b("S", "cycle sort")]),
            this_pane_group(),
            seam_group(),
        ],
        PaneId::TradingOrders | PaneId::OrdersBook => vec![
            group(
                "Orders",
                vec![
                    b("Space", "mark / unmark selected open order"),
                    b("Ctrl-A", "mark / unmark all open orders"),
                    b("x", "cancel marked orders, or selected"),
                    b("m", "modify selected open order"),
                    b("Enter", "order detail + history"),
                ],
            ),
            group("Chart", chart_bindings()),
            group("Table", vec![b("S", "cycle sort")]),
            this_pane_group(),
            seam_group(),
        ],
        PaneId::PortfolioHoldings => vec![
            group(
                "Trading",
                vec![
                    b("b / s", "buy / sell the selected holding"),
                    b("g", "create GTT trigger"),
                    b("B", "set price alert (server-side, simple or ATO)"),
                    b("a", "open CDSL TPIN/OTP authorisation for consent accounts"),
                ],
            ),
            group("Chart", chart_bindings()),
            group("Table", vec![b("S", "cycle sort")]),
            this_pane_group(),
            seam_group(),
        ],
        PaneId::OrdersGtt => vec![
            flat_group(vec![
                b("m", "modify selected"),
                b("x", "delete selected"),
                b("S", "cycle sort"),
            ]),
            seam_group(),
        ],
        PaneId::OrdersAlerts => vec![
            group(
                "The selected alert",
                vec![
                    b(
                        "r",
                        "enable / disable, a triggered alert comes back disabled",
                    ),
                    b("e", "edit its condition"),
                    b("d", "delete it on your Kite account (asks first)"),
                ],
            ),
            group(
                "A new alert",
                vec![
                    b("a", "pick an instrument, then set the condition"),
                    b(
                        "Type row",
                        "Simple notifies; ATO also places a basket of orders",
                    ),
                ],
            ),
            group(
                "In an ATO's basket (in the form)",
                vec![
                    b("a / e", "add an order / edit the selected one"),
                    b("d / c", "remove / duplicate the selected order"),
                    b("[ / ]", "move the selected order earlier / later"),
                    b(
                        "Enter",
                        "edit the selected order; Tab to the fields to review",
                    ),
                ],
            ),
            group(
                "Its history",
                vec![
                    b("Enter", "every trigger, and the price at that time"),
                    b("Esc", "close the history panel (stays in this pane)"),
                ],
            ),
            group(
                "Table",
                vec![b("S", "cycle sort"), b("R", "refresh from Kite")],
            ),
            seam_group(),
        ],
        PaneId::DerivativesChain => vec![
            group(
                "The ladder",
                vec![
                    b("j/k", "select strike"),
                    b("[ / ]", "previous / next expiry"),
                    b("u", "cycle index underlying"),
                    b("/", "search stock underlying"),
                    b("h / l", "focus the call / put side"),
                ],
            ),
            group(
                "The focused leg",
                vec![
                    b("b / s", "buy / sell it"),
                    b("a", "set price alert (server-side, simple or ATO)"),
                ],
            ),
            group(
                "Links",
                vec![b(
                    "chart",
                    "the chart beside the ladder is of the underlying spot, so walking the strikes never turns it into an option chart",
                )],
            ),
            seam_group(),
        ],
        PaneId::PortfolioMf => vec![flat_group(vec![b("S", "cycle sort")]), seam_group()],
        PaneId::InvestBids => vec![
            group(
                "Sections",
                vec![
                    b(
                        "[ / ]",
                        "cycle IPO, govt. securities, auctions, corporate actions, SSE IPO",
                    ),
                    b("a", "switch between IPO issues and IPO applications"),
                    b(
                        "Enter",
                        "open the selected section in Kite Web for bid/application actions",
                    ),
                    b("r", "refresh the current Bids data"),
                ],
            ),
            group(
                "Auction workflow",
                vec![b(
                    "s",
                    "open a fixed SELL CNC LIMIT DAY ticket for the selected auction holding",
                )],
            ),
        ],
        PaneId::OverviewPositions | PaneId::OverviewHoldings => vec![
            group(
                "Moving around",
                vec![
                    b("[ / ]", "focus the Positions / Holdings snapshot"),
                    b(
                        "mouse",
                        "click anywhere in a book, border, title or empty message, to focus it",
                    ),
                    b("Enter", "open the focused row in its full view"),
                    b("r", "refresh the account books"),
                ],
            ),
            group(
                "Reading it",
                vec![
                    b(
                        "account row",
                        "counts as open/total: `Pos 5/12` is 5 open of 12 net positions, `?` is a source that has not loaded",
                    ),
                    b(
                        "small terminals",
                        "the KPI and account bands shrink first; below 12 main rows or 70 columns only the focused book is drawn, and Tab or `[ / ]` switches which",
                    ),
                ],
            ),
            seam_group(),
        ],
        PaneId::OverviewIndices => vec![
            flat_group(vec![
                b("j / k", "scroll the index levels"),
                b("r", "refresh"),
                b(
                    "source",
                    "the index rows are the index instruments of the Watch workspace's open watchlist, not a separate feed, changing Trading's watch list does not change them",
                ),
            ]),
            seam_group(),
        ],
        PaneId::OverviewMovers => vec![
            flat_group(vec![
                b(
                    "u",
                    "cycle movers universe (Nifty 50/100/500/F&O, then all, illiquid)",
                ),
                b("j / k", "scroll the screener"),
                b("r", "refresh"),
            ]),
            seam_group(),
        ],
        PaneId::TradingWatch => vec![
            group(
                "Instruments",
                vec![
                    b("/", "add an instrument to this list"),
                    b("d", "remove the selected row"),
                    b("w", "create or switch a watchlist on your account"),
                    b("[ / ]", "previous / next watchlist"),
                    b("J / K", "move the selected row down / up, within its group"),
                    b("r", "reload the watchlists from your account"),
                ],
            ),
            group(
                "Trading",
                vec![
                    b("b / s", "buy / sell the selected instrument"),
                    b("g", "create GTT trigger"),
                    b("a", "set price alert (server-side, simple or ATO)"),
                ],
            ),
            group(
                "Links",
                vec![b(
                    "chart and depth",
                    "the two panes beside this list follow the row selected here, and nothing else: moving a row in Orders retargets neither",
                )],
            ),
            seam_group(),
        ],
        PaneId::TradingChart | PaneId::DerivativesChart => vec![
            group("Reading it", chart_bindings_without_toggle()),
            group(
                "Instrument",
                vec![b(
                    "source",
                    "a chart leaf draws what its source pane names, Trading's watch row, or the Option Chain's underlying spot, so it has no row and no trade key of its own",
                )],
            ),
            seam_group(),
        ],
        PaneId::TradingDepth => vec![
            flat_group(vec![
                b(
                    "Enter / z",
                    "give the ladder the body, where it draws the quote block above it; press either again to put the workspace back",
                ),
                b("r", "refresh"),
                b(
                    "instrument",
                    "the ladder follows the Trading watch row; an index has no depth and says so",
                ),
            ]),
            seam_group(),
        ],
        PaneId::PortfolioFunds => vec![
            flat_group(vec![
                b("j / k", "scroll the segment tables"),
                b("F", "the same figures as a centred modal"),
                b("r", "refresh"),
            ]),
            seam_group(),
        ],
    };
    let view_section = HelpSection {
        title: format!("{} · this pane", pane.title()),
        groups: view_groups,
    };
    vec![global, view_section]
}

fn seam_group() -> HelpGroup {
    group(
        "Sizing it",
        vec![
            b(
                "drag a border",
                "the border between two panes of this workspace resizes the split; each pane keeps its own minimum, so a seam stops rather than collapsing a neighbour",
            ),
            b(
                "=",
                "put every dragged divider back where the preset had it",
            ),
        ],
    )
}

fn this_pane_group() -> HelpGroup {
    group(
        "This pane",
        vec![b(
            "own state",
            "a pane keeps its own cursor, sort and scroll: Trading's order book and the Orders workspace's are two tables, and sorting one leaves the other alone",
        )],
    )
}

fn chart_bindings() -> Vec<Binding> {
    vec![
        b(
            "c",
            "open the selected row's chart inside this pane, which takes the body while it is open; c or Esc closes it and puts the workspace back",
        ),
        b("C", "make the chart fill the pane / back to the split"),
        b("i", "cycle chart interval"),
        b("I", "cycle chart indicator (SMA/EMA/Bollinger/RSI/MACD)"),
        b("v", "show / hide chart volume"),
        b("h / l", "scan to an older / newer candle"),
        b(
            "mouse",
            "point at a candle to read it; click to keep it selected",
        ),
    ]
}

fn chart_bindings_without_toggle() -> Vec<Binding> {
    let mut bindings = chart_bindings();
    bindings.remove(0);
    bindings
}

fn positions_chart_bindings() -> Vec<Binding> {
    let mut bindings = chart_bindings_without_toggle();
    bindings.insert(
        0,
        b(
            "Ctrl-K",
            "Toggle chart opens the selected row's chart inside this pane; Esc closes it",
        ),
    );
    bindings
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum HelpRow {
    Section(String),
    Group(&'static str),
    Entry { key: String, desc: &'static str },
    Note(String),
    Blank,
}

pub(super) fn help_rows(pane: PaneId) -> Vec<HelpRow> {
    let mut rows = vec![
        HelpRow::Note(format!(
            "These are the keys for the {} pane, each pane of a workspace has its own.",
            pane.title()
        )),
        HelpRow::Note(
            "Case matters: Shift-I below means capital I, a different key from i.".to_string(),
        ),
    ];
    for section in help_bindings(pane) {
        rows.push(HelpRow::Blank);
        rows.push(HelpRow::Section(section.title));
        for group in section.groups {
            if let Some(title) = group.title {
                rows.push(HelpRow::Group(title));
            }
            for binding in group.bindings {
                rows.push(HelpRow::Entry {
                    key: shift_label(binding.key),
                    desc: binding.desc,
                });
            }
        }
    }
    rows.push(HelpRow::Blank);
    rows.push(HelpRow::Note(
        "? or Esc close · j/k or ↑/↓ scroll · g/G top/bottom".to_string(),
    ));
    rows
}

fn is_key_sep(c: char) -> bool {
    c == ' ' || c == '/'
}

pub(super) fn shift_label(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    let mut out = String::with_capacity(key.len());
    for (i, ch) in chars.iter().enumerate() {
        let alone = ch.is_ascii_uppercase()
            && (i == 0 || is_key_sep(chars[i - 1]))
            && (i + 1 == chars.len() || is_key_sep(chars[i + 1]));
        if alone {
            out.push_str("Shift-");
        }
        out.push(*ch);
    }
    out
}

#[cfg(test)]
pub(super) fn every_pane() -> Vec<PaneId> {
    use crate::tui::workspace::Preset;
    Preset::ALL
        .into_iter()
        .flat_map(Preset::panes)
        .copied()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{HelpRow, every_pane, help_bindings, help_rows, shift_label};
    use crate::tui::app::{Modal, PaletteState, SearchPurpose, SearchState, ThemePickerState};
    use crate::tui::notes::{NoteEditor, NotesState};
    use crate::tui::workspace::PaneId;

    fn flat(pane: PaneId) -> Vec<(&'static str, &'static str)> {
        help_bindings(pane)
            .into_iter()
            .flat_map(|section| section.groups)
            .flat_map(|group| group.bindings)
            .map(|binding| (binding.key, binding.desc))
            .collect()
    }

    fn help_key_tokens(key: &str) -> Vec<String> {
        if key == "/" {
            return vec!["/".to_string()];
        }
        key.split(['/', ' '])
            .filter(|token| !token.is_empty())
            .flat_map(|token| {
                if token.chars().count() > 1 && token.chars().all(|c| "←→↑↓".contains(c)) {
                    token
                        .chars()
                        .map(|c| c.to_string())
                        .collect::<Vec<String>>()
                } else {
                    vec![token.to_string()]
                }
            })
            .collect()
    }

    fn hint_key_tokens(hint: &'static str) -> Vec<String> {
        let Some((keys, _)) = hint.split_once(':') else {
            return Vec::new();
        };
        help_key_tokens(keys)
            .into_iter()
            .filter(|token| token != "type" && token != "sending")
            .collect()
    }

    fn modal_fixtures() -> Vec<Modal> {
        let editing = NotesState {
            editor: Some(NoteEditor::draft(None)),
            ..NotesState::default()
        };
        let search = |purpose: SearchPurpose| {
            Modal::Search(SearchState {
                purpose,
                ..SearchState::default()
            })
        };
        vec![
            Modal::Help(0),
            search(SearchPurpose::AddToWatch),
            search(SearchPurpose::ChainUnderlying),
            search(SearchPurpose::NewAlert),
            search(SearchPurpose::AtoBasketItem),
            Modal::Palette(PaletteState::for_test()),
            Modal::ThemePicker(ThemePickerState::default()),
            Modal::WorkspacePicker(crate::tui::app::WorkspacePickerState { selected: 0 }),
            Modal::EventLog(crate::tui::app::EventLogState::default()),
            Modal::Notes(NotesState::default()),
            Modal::Notes(editing),
            Modal::Funds,
        ]
    }

    fn form_footer_hints() -> Vec<&'static str> {
        use crate::tui::ui::modals;
        let footers = [
            modals::theme_picker_footer(false),
            modals::theme_picker_footer(true),
            modals::conversion_footer(),
            modals::event_log_footer(),
            modals::notes_footer(false, false),
            modals::notes_footer(false, true),
            modals::notes_footer(true, false),
            modals::note_editor_footer(false),
            modals::note_editor_footer(true),
        ];
        footers
            .into_iter()
            .flat_map(|footer| footer.hints)
            .flat_map(|group| group.hints.iter().copied())
            .collect()
    }

    #[test]
    fn help_documents_every_key_the_footer_advertises() {
        for pane in every_pane() {
            let documented: Vec<String> = flat(pane)
                .into_iter()
                .flat_map(|(key, _)| help_key_tokens(key))
                .collect();
            let chart_panel = crate::tui::pane_state::ChartBinding::of_book(pane).is_some();
            let hints: Vec<&'static str> = [false, chart_panel]
                .into_iter()
                .flat_map(|chart| crate::tui::ui::hints::hints_for(None, chart, pane, false))
                .flat_map(|group| group.hints.iter().copied())
                .collect();
            for hint in hints {
                for token in hint_key_tokens(hint) {
                    assert!(
                        documented.contains(&token),
                        "{pane:?} footer names {hint:?} but help documents no {token:?}: {documented:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn help_documents_every_key_a_modal_footer_advertises() {
        let documented: Vec<String> = every_pane()
            .into_iter()
            .flat_map(flat)
            .flat_map(|(key, _)| help_key_tokens(key))
            .collect();
        let hints: Vec<&'static str> = modal_fixtures()
            .iter()
            .flat_map(|modal| {
                crate::tui::ui::hints::hints_for(
                    Some(modal),
                    false,
                    crate::tui::workspace::PaneId::TradingWatch,
                    false,
                )
            })
            .flat_map(|group| group.hints.iter().copied())
            .chain(form_footer_hints())
            .collect();
        for hint in hints {
            for token in hint_key_tokens(hint) {
                assert!(
                    documented.contains(&token),
                    "a modal footer names {hint:?} but help documents no {token:?}"
                );
            }
        }
    }

    #[test]
    fn the_single_pane_bids_section_offers_no_divider_bindings() {
        let keys: Vec<&str> = help_bindings(PaneId::InvestBids)[1]
            .groups
            .iter()
            .flat_map(|group| group.bindings.iter().map(|binding| binding.key))
            .collect();
        assert!(!keys.contains(&"="), "{keys:?}");
        assert!(!keys.contains(&"drag a border"), "{keys:?}");
    }

    #[test]
    fn every_pane_has_its_own_section_with_bindings() {
        for pane in every_pane() {
            let sections = help_bindings(pane);
            assert_eq!(sections.len(), 2, "{pane:?} must have Global + its own");
            assert_eq!(sections[0].title, "Global", "{pane:?}: Global comes first");
            assert!(
                sections[1].title.starts_with(pane.title()),
                "{pane:?}: second section must name the pane, got {:?}",
                sections[1].title
            );
            assert!(
                sections[1].groups.iter().any(|g| !g.bindings.is_empty()),
                "{pane:?}: its section must document at least one key"
            );
        }
    }

    #[test]
    fn the_body_says_which_pane_it_is_for_and_that_case_matters() {
        for pane in every_pane() {
            let notes: Vec<String> = help_rows(pane)
                .into_iter()
                .filter_map(|row| match row {
                    HelpRow::Note(text) => Some(text),
                    _ => None,
                })
                .collect();
            assert!(
                notes
                    .iter()
                    .any(|n| n.contains(pane.title()) && n.contains("each pane of a workspace")),
                "{pane:?} must say the help is contextual: {notes:?}"
            );
            assert!(
                notes.iter().any(|n| n.contains("Case matters")),
                "{pane:?} must warn that case is significant: {notes:?}"
            );
        }
    }

    #[test]
    fn a_long_section_is_grouped_under_sub_headings() {
        for pane in every_pane() {
            for section in help_bindings(pane) {
                let entries: usize = section.groups.iter().map(|g| g.bindings.len()).sum();
                if entries > 6 {
                    assert!(
                        section.groups.iter().all(|g| g.title.is_some()),
                        "{pane:?}/{}: {entries} entries need sub-headings",
                        section.title
                    );
                    assert!(
                        section.groups.len() > 1,
                        "{pane:?}/{}: one group is not a grouping",
                        section.title
                    );
                }
            }
        }
    }

    #[test]
    fn a_standalone_capital_is_spelled_out_but_a_named_key_is_not() {
        assert_eq!(shift_label("I"), "Shift-I");
        assert_eq!(shift_label("J / K"), "Shift-J / Shift-K");
        assert_eq!(shift_label("g / G"), "g / Shift-G");
        assert_eq!(shift_label("j/k"), "j/k");
        assert_eq!(shift_label("Ctrl-K"), "Ctrl-K");
        assert_eq!(shift_label("Tab"), "Tab");
        assert_eq!(shift_label("Enter"), "Enter");
        assert_eq!(shift_label("1-9 / Tab / ←→"), "1-9 / Tab / ←→");
    }

    #[test]
    fn every_capital_binding_is_decorated_in_the_rows() {
        for pane in every_pane() {
            for row in help_rows(pane) {
                if let HelpRow::Entry { key, .. } = row {
                    for part in key.split([' ', '/']).filter(|p| !p.is_empty()) {
                        assert!(
                            part.len() > 1 || !part.chars().next().unwrap().is_ascii_uppercase(),
                            "{pane:?}: bare capital {part:?} in key {key:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn global_includes_core_keys() {
        let keys: Vec<&str> = flat(PaneId::OverviewPositions)
            .iter()
            .map(|(k, _)| *k)
            .collect();
        for req in [
            "1-6",
            "← / →",
            "Tab / Shift-Tab",
            "z",
            ":workspace NAME",
            "j / k  ↑ / ↓",
            "g / G",
            "r",
            ":",
            "Ctrl-K",
            "T",
            "F",
            "n",
            "N",
            "A",
            "q / Ctrl-C",
            "Esc",
            "?",
        ] {
            assert!(keys.contains(&req), "Global must contain key '{req}'");
        }
    }

    #[test]
    fn chart_capable_panes_document_the_indicator_key() {
        for pane in [
            PaneId::TradingOrders,
            PaneId::OrdersBook,
            PaneId::PortfolioPositions,
            PaneId::PortfolioHoldings,
        ] {
            let bindings = flat(pane);
            assert!(
                bindings
                    .iter()
                    .any(|(k, a)| k == &"I" && a.contains("indicator")),
                "{pane:?} help must document I:indicator"
            );
            assert!(bindings.iter().any(|(k, _)| k == &"i"), "{pane:?} lost i");
            assert!(bindings.iter().any(|(k, _)| k == &"v"), "{pane:?} lost v");
        }
        for pane in [
            PaneId::TradingOrders,
            PaneId::OrdersBook,
            PaneId::PortfolioHoldings,
        ] {
            assert!(
                flat(pane)
                    .iter()
                    .any(|(k, a)| k == &"c" && a.contains("chart")),
                "{pane:?} lost c"
            );
        }
        for pane in [
            PaneId::OverviewPositions,
            PaneId::TradingWatch,
            PaneId::PortfolioMf,
            PaneId::OrdersGtt,
            PaneId::DerivativesChain,
        ] {
            assert!(
                !flat(pane).iter().any(|(k, _)| k == &"I"),
                "{pane:?} has no chart, so it must not advertise I"
            );
        }
        for pane in [PaneId::TradingChart, PaneId::DerivativesChart] {
            let bindings = flat(pane);
            assert!(bindings.iter().any(|(k, _)| k == &"I"), "{pane:?} lost I");
            assert!(
                !bindings.iter().any(|(k, _)| k == &"c"),
                "{pane:?} has nothing to open"
            );
        }
    }

    #[test]
    fn depth_documents_enter_and_z_as_one_zoom() {
        assert!(
            flat(PaneId::TradingDepth)
                .iter()
                .any(|(k, a)| k == &"Enter / z" && a.contains("quote block")),
            "Depth help must document Enter / z"
        );
    }

    #[test]
    fn overview_includes_pane_switch_and_enter_open() {
        let bindings = flat(PaneId::OverviewPositions);
        assert!(
            bindings.iter().any(|(k, _)| k == &"[ / ]"),
            "Overview help must document [ / ] pane focus"
        );
        assert!(
            bindings.iter().any(|(k, _)| k == &"Enter"),
            "Overview help must document Enter to open the focused row"
        );
    }

    #[test]
    fn every_pane_documents_the_alert_command() {
        for pane in every_pane() {
            assert!(
                flat(pane).iter().any(|(k, _)| k == &":alert [SYMBOL]"),
                "{pane:?} help must name the alert command"
            );
        }
    }

    #[test]
    fn chain_includes_slash_search() {
        assert!(
            flat(PaneId::DerivativesChain)
                .iter()
                .any(|(k, _)| k == &"/"),
            "Chain help must include / for stock-underlying search"
        );
    }

    #[test]
    fn positions_missing_watch_keys() {
        let bindings = flat(PaneId::PortfolioPositions)
            .into_iter()
            .filter(|(key, _)| *key != "z")
            .collect::<Vec<_>>();
        for key in ["g", "[", "]", "J", "K", "d", "[ / ]", "J / K"] {
            assert!(
                !bindings.iter().any(|(k, _)| k == &key),
                "Positions must not contain Watch-only key '{key}'"
            );
        }
    }

    #[test]
    fn holdings_includes_chart_and_gtt() {
        let bindings = flat(PaneId::PortfolioHoldings);
        assert!(
            bindings.iter().any(|(k, _)| k == &"c"),
            "Holdings must include chart (c)"
        );
        assert!(
            bindings.iter().any(|(k, _)| k == &"g"),
            "Holdings must include GTT (g)"
        );
    }

    #[test]
    fn gtt_includes_modify_and_delete() {
        let bindings = flat(PaneId::OrdersGtt);
        assert!(
            bindings.iter().any(|(k, _)| k == &"m"),
            "GTT must include modify (m)"
        );
        assert!(
            bindings.iter().any(|(k, _)| k == &"x"),
            "GTT must include delete (x)"
        );
    }

    #[test]
    fn orders_document_marking_and_bulk_cancel() {
        let bindings = flat(PaneId::OrdersBook);
        for (key, description) in [("Space", "mark"), ("Ctrl-A", "all open"), ("x", "marked")] {
            assert!(
                bindings
                    .iter()
                    .any(|(binding, text)| binding == &key && text.contains(description)),
                "Orders help must document {key}: {description}"
            );
        }
    }
}
