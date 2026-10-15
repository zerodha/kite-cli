use super::super::{
    App, ExecItem, MenuAction, PaletteAction, PaletteRow, PaletteState, RowIdentity, Side,
};
use crate::tui::instruments::Instrument;
use crate::tui::palette::{
    CompletionEdit, CompletionTarget, Draft, Jump, OrderDraft, WatchlistCmd, WatchlistDraft,
};
use crate::tui::settings;
use crate::tui::workspace::PaneId;
use std::ops::Range;

impl App {
    fn theme_results(query: &str) -> Vec<&'static str> {
        let q = query.trim().to_ascii_lowercase();
        let norm = q.replace(['-', '_'], "");
        settings::preset_names()
            .filter(|name| {
                q.is_empty()
                    || name.starts_with(&q)
                    || name.replace(['-', '_'], "").starts_with(&norm)
            })
            .collect()
    }

    pub(in crate::tui::app) fn rebuild_palette(
        &self,
        state: &mut PaletteState,
        keep: Option<RowIdentity>,
    ) {
        let build = self.palette_build(&state.input);
        state.draft = build.draft;
        state.results = build.rows;
        state.ambiguous = build.ambiguous;
        state.truncated = build.truncated;
        state.selected =
            if let Some(index) = keep.and_then(|identity| state.find_identity(&identity)) {
                Some(index)
            } else {
                state.chosen = false;
                state.first_item()
            };

        self.hit.borrow_mut().regions.clear();
    }

    #[cfg(test)]
    pub fn rebuild_palette_for_test(&self, state: &mut PaletteState) {
        self.rebuild_palette(state, None);
    }

    pub(in crate::tui::app) fn palette_build(&self, input: &str) -> PaletteBuild {
        let draft = crate::tui::palette::analyze(input);
        if draft == Draft::Browse {
            return PaletteBuild {
                rows: Self::browse_rows(self.focused_pane(), &self.hidden_panes(), input),
                draft,
                ambiguous: false,
                truncated: false,
            };
        }
        let mut build = self.draft_rows(input, &draft);
        build.draft = draft;
        if !build.rows.is_empty() {
            build.rows.insert(0, PaletteRow::Header("Results"));
        }
        build
    }

    fn browse_rows(pane: PaneId, hidden: &[PaneId], input: &str) -> Vec<PaletteRow> {
        Self::menu_rows(pane, hidden)
            .into_iter()
            .map(|row| match row {
                PaletteRow::Item { action, key } => PaletteRow::Item {
                    action: match action {
                        PaletteAction::Execute { item, .. } => PaletteAction::Execute {
                            completion: Some(Self::exec_completion(input, &item)),
                            item,
                        },
                        complete @ PaletteAction::Complete { .. } => complete,
                    },
                    key,
                },
                header @ PaletteRow::Header(_) => header,
            })
            .collect()
    }

    fn exec_completion(input: &str, item: &ExecItem) -> CompletionEdit {
        let by_jump = |jump: Jump| {
            crate::tui::palette::CATALOG
                .into_iter()
                .find(|kw| kw.jump == Some(jump))
                .map(|kw| crate::tui::palette::complete(input, CompletionTarget::Keyword(kw)))
        };
        let fallback =
            || crate::tui::palette::complete(input, CompletionTarget::MenuLabel(&menu_label(item)));
        match item {
            ExecItem::Jump(jump) => by_jump(*jump).unwrap_or_else(fallback),
            ExecItem::Quit | ExecItem::Action(MenuAction::Quit) => {
                Self::keyword_edit(input, "quit").unwrap_or_else(fallback)
            }
            ExecItem::Action(MenuAction::ManageWatchlists) => {
                Self::keyword_edit(input, "wl").unwrap_or_else(fallback)
            }
            ExecItem::Action(MenuAction::SwitchTheme) => {
                Self::keyword_edit(input, "theme").unwrap_or_else(fallback)
            }
            ExecItem::Action(MenuAction::NewAlert) => {
                Self::keyword_edit(input, "alert").unwrap_or_else(fallback)
            }
            ExecItem::Instrument(inst) => {
                crate::tui::palette::complete(input, CompletionTarget::Instrument(&inst.label()))
            }
            _ => fallback(),
        }
    }

    fn draft_rows(&self, input: &str, draft: &Draft) -> PaletteBuild {
        let mut build = PaletteBuild::default();
        match draft {
            Draft::Browse => {}
            Draft::Order(order) => self.order_rows(input, order, &mut build),
            Draft::Watchlist(wl) => self.watchlist_rows(input, wl, &mut build),
            Draft::Alert { arg, query } => self.alert_rows(input, arg, query, &mut build),
            Draft::Theme { arg, query } => {
                for preset in Self::theme_results(query) {
                    build.rows.push(exec_row(
                        ExecItem::Theme(preset.to_string()),
                        Some(crate::tui::palette::complete(
                            input,
                            CompletionTarget::ThemePreset { arg, preset },
                        )),
                    ));
                }
            }

            Draft::Jump { jump, word } => {
                build.rows.push(exec_row(
                    ExecItem::Jump(*jump),
                    Self::keyword_edit(input, word),
                ));
                Self::keyword_rows(input, Some(word), &mut build);
            }

            Draft::Workspace { arg, query } => {
                for preset in crate::tui::palette::workspace_matches(query) {
                    build.rows.push(exec_row(
                        ExecItem::Jump(Jump::Workspace(preset)),
                        Some(crate::tui::palette::complete(
                            input,
                            CompletionTarget::WorkspaceName {
                                arg,
                                name: preset.title(),
                            },
                        )),
                    ));
                }
            }
            Draft::Quit => {
                build
                    .rows
                    .push(exec_row(ExecItem::Quit, Self::keyword_edit(input, "quit")));
                Self::keyword_rows(input, Some("quit"), &mut build);
            }

            Draft::Partial => {
                Self::keyword_rows(input, None, &mut build);
                self.search_rows(input, &mut build);
            }
            Draft::Search => self.search_rows(input, &mut build),
        }
        build
    }

    fn keyword_edit(input: &str, word: &str) -> Option<CompletionEdit> {
        crate::tui::palette::CATALOG
            .into_iter()
            .find(|kw| kw.word == word)
            .map(|kw| crate::tui::palette::complete(input, CompletionTarget::Keyword(kw)))
    }

    fn keyword_rows(input: &str, skip: Option<&str>, build: &mut PaletteBuild) {
        let token = input.split_whitespace().next().unwrap_or("");
        for kw in crate::tui::palette::keyword_matches(token).filter(|kw| Some(kw.word) != skip) {
            let edit = crate::tui::palette::complete(input, CompletionTarget::Keyword(kw));
            if edit.is_noop(input) {
                continue;
            }
            build
                .rows
                .push(complete_row(kw.completion_text(), kw.syntax(), edit));
        }
    }

    fn order_rows(&self, input: &str, order: &OrderDraft, build: &mut PaletteBuild) {
        if let Some(error) = order.error {
            if error == crate::tui::palette::OrderError::MissingSymbol {
                let word = side_word(order.side);
                if let Some(edit) = Self::keyword_edit(input, word)
                    && !edit.is_noop(input)
                {
                    build.rows.push(complete_row(
                        format!("{word} "),
                        format!("{word} SYMBOL [QTY] [@PRICE]"),
                        edit,
                    ));
                }
            }
            return;
        }
        let Some(store) = &self.instruments else {
            return;
        };
        let query = &input[order.symbol.clone()];

        let hits = store.search(query, ORDER_ROWS + 1);
        build.truncated = hits.len() > ORDER_ROWS;
        build.ambiguous = instrument_choice_needed(query, &hits);
        for inst in hits.into_iter().take(ORDER_ROWS) {
            let identity = inst.label();

            let completion = (!identity.contains(char::is_whitespace)).then(|| {
                crate::tui::palette::complete(
                    input,
                    CompletionTarget::OrderSymbol {
                        symbol: &order.symbol,
                        identity: &identity,
                    },
                )
            });
            build.rows.push(exec_row(
                ExecItem::Order {
                    side: order.side,
                    instrument: inst.clone(),
                    qty: order.qty,
                    price: order.price,
                },
                completion,
            ));
        }
    }

    fn alert_rows(&self, input: &str, arg: &Range<usize>, query: &str, build: &mut PaletteBuild) {
        if query.is_empty() {
            let item = ExecItem::Action(MenuAction::NewAlert);
            let completion = Self::exec_completion(input, &item);
            build.rows.push(exec_row(item, Some(completion)));

            Self::keyword_rows(input, Some("alert"), build);
            return;
        }
        let Some(store) = &self.instruments else {
            return;
        };
        let hits = store.search(query, ORDER_ROWS + 1);
        build.truncated = hits.len() > ORDER_ROWS;
        build.ambiguous = instrument_choice_needed(query, &hits);
        for inst in hits.into_iter().take(ORDER_ROWS) {
            let identity = inst.label();
            build.rows.push(exec_row(
                ExecItem::Alert(inst.clone()),
                Some(crate::tui::palette::complete(
                    input,
                    CompletionTarget::AlertInstrument {
                        arg,
                        identity: &identity,
                    },
                )),
            ));
        }
    }

    fn watchlist_rows(&self, input: &str, wl: &WatchlistDraft, build: &mut PaletteBuild) {
        let names: Vec<String> = self
            .watch
            .summaries()
            .map(|summary| summary.name.clone())
            .collect();
        let subcommands = |prefix: &str, build: &mut PaletteBuild| {
            for form in ["wl rename ", "wl delete"] {
                let word = form.trim().rsplit(' ').next().unwrap_or("");
                if !word.starts_with(&prefix.to_ascii_lowercase()) {
                    continue;
                }
                let edit =
                    crate::tui::palette::complete(input, CompletionTarget::WatchlistForm(form));
                if edit.is_noop(input) {
                    continue;
                }
                let syntax = match word {
                    "rename" => "wl rename NAME: rename the open list",
                    _ => "wl delete: delete the open list",
                };
                build
                    .rows
                    .push(complete_row(form.to_string(), syntax.to_string(), edit));
            }
        };
        match wl {
            WatchlistDraft::List => {
                for summary in self.watch.summaries() {
                    let duplicate = names
                        .iter()
                        .filter(|name| name.eq_ignore_ascii_case(&summary.name))
                        .count()
                        > 1;
                    let target = if duplicate {
                        summary.id.as_str()
                    } else {
                        &summary.name
                    };
                    let form = format!("wl {target}");
                    let command = if duplicate {
                        WatchlistCmd::SwitchId {
                            id: summary.id.as_str().to_string(),
                            name: summary.name.clone(),
                        }
                    } else {
                        WatchlistCmd::Switch(summary.name.clone())
                    };
                    build.rows.push(exec_row(
                        ExecItem::Watchlist(command),
                        Some(crate::tui::palette::complete(
                            input,
                            CompletionTarget::WatchlistForm(&form),
                        )),
                    ));
                }
                subcommands("", build);
            }

            WatchlistDraft::Name { range, name } => {
                build.rows.push(exec_row(
                    ExecItem::Watchlist(WatchlistCmd::Switch(name.clone())),
                    None,
                ));
                for existing in names.iter().filter(|existing| {
                    *existing != name && existing.to_lowercase().starts_with(&name.to_lowercase())
                }) {
                    let edit = crate::tui::palette::complete(
                        input,
                        CompletionTarget::WatchlistName {
                            range,
                            name: existing,
                        },
                    );
                    build.rows.push(complete_row(
                        format!("wl {existing}"),
                        format!("wl {existing}: switch to it"),
                        edit,
                    ));
                }
                subcommands(name, build);
            }

            WatchlistDraft::RenameMissing { needs_space } => {
                if *needs_space {
                    build.rows.push(complete_row(
                        " ".to_string(),
                        "wl rename NAME".to_string(),
                        crate::tui::palette::complete(input, CompletionTarget::RenameSpace),
                    ));
                }
            }
            WatchlistDraft::Rename { name } => build.rows.push(exec_row(
                ExecItem::Watchlist(WatchlistCmd::Rename(name.clone())),
                None,
            )),
            WatchlistDraft::Delete => {
                build
                    .rows
                    .push(exec_row(ExecItem::Watchlist(WatchlistCmd::Delete), None));
            }

            WatchlistDraft::DeleteExtra => {}
        }
    }

    fn search_rows(&self, input: &str, build: &mut PaletteBuild) {
        let text = input.trim();
        let needle = text.to_ascii_lowercase();
        for item in Self::menu_exec_items(self.focused_pane(), &self.hidden_panes())
            .into_iter()
            .filter(|item| menu_label(item).to_ascii_lowercase().contains(&needle))
        {
            let completion = Self::exec_completion(input, &item);
            build.rows.push(exec_row(item, Some(completion)));
        }
        if let Some(store) = &self.instruments {
            let hits = store.search(text, SEARCH_ROWS + 1);
            build.truncated = hits.len() > SEARCH_ROWS;
            build.ambiguous = instrument_choice_needed(text, &hits);
            for inst in hits.into_iter().take(SEARCH_ROWS) {
                build.rows.push(exec_row(
                    ExecItem::Instrument(inst.clone()),
                    Some(crate::tui::palette::complete(
                        input,
                        CompletionTarget::Instrument(&inst.label()),
                    )),
                ));
            }
        }
        build.rows.truncate(SEARCH_ROWS);
    }
}

#[derive(Debug, Default)]
pub(in crate::tui::app) struct PaletteBuild {
    draft: Draft,
    pub(in crate::tui::app) rows: Vec<PaletteRow>,

    ambiguous: bool,

    truncated: bool,
}

const ORDER_ROWS: usize = 5;
const SEARCH_ROWS: usize = 12;

pub(in crate::tui::app) fn exec_row(
    item: ExecItem,
    completion: Option<CompletionEdit>,
) -> PaletteRow {
    PaletteRow::Item {
        action: PaletteAction::Execute {
            item: Box::new(item),
            completion,
        },
        key: None,
    }
}

fn complete_row(label: String, syntax: String, edit: CompletionEdit) -> PaletteRow {
    PaletteRow::Item {
        action: PaletteAction::Complete {
            label,
            syntax,
            edit,
        },
        key: None,
    }
}

pub(in crate::tui::app) fn side_word(side: Side) -> &'static str {
    match side {
        Side::Buy => "buy",
        Side::Sell => "sell",
    }
}

fn instrument_choice_needed(query: &str, hits: &[&Instrument]) -> bool {
    if hits.len() <= 1 {
        return false;
    }
    let needle = query.trim();
    hits.iter()
        .filter(|inst| inst.label().eq_ignore_ascii_case(needle))
        .count()
        != 1
}

fn menu_label(item: &ExecItem) -> String {
    match item {
        ExecItem::Jump(Jump::Workspace(preset)) => preset.title().to_string(),
        ExecItem::Jump(Jump::Pane(pane)) => format!("{} ({})", pane.title(), pane.preset().title()),
        ExecItem::FocusPane(pane) => format!("Focus {}", pane.title()),
        ExecItem::Theme(name) => format!("theme · {name}"),
        ExecItem::Action(action) => action.label().to_string(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        ExecItem, KeyCode, MenuAction, Modal, PaletteAction, PaletteRow, PaletteState, RowIdentity,
    };
    use super::super::state::row_identity;
    use super::super::tests::{app_with_master, fresh_app, inst, open, press, state, tabbed};
    use super::instrument_choice_needed;
    use crate::instruments::{Instrument, InstrumentStore};
    use crate::tui::data::DataEvent;
    use crate::tui::palette::{Draft, WatchlistCmd};
    use crate::tui::settings;
    use crate::tui::workspace::{PaneId, Preset};
    use std::sync::Arc;

    #[test]
    fn tab_completes_a_partial_keyword_with_one_trailing_space() {
        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "bu"), "buy ");
        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "the"), "theme ");

        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "ord"), "orders");
        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "au"), "auction");
        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "exit"), "quit");
    }

    #[test]
    fn tab_canonicalises_a_side_word_without_supplying_a_symbol() {
        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "b "), "buy ");
        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "s "), "sell ");

        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "b"), "bids");
    }

    #[test]
    fn tab_replaces_only_the_order_symbol() {
        let mut app = app_with_master();

        assert_eq!(tabbed(&mut app, "buy tc"), "buy NSE:TCS ");
        let mut app = app_with_master();
        assert_eq!(
            tabbed(&mut app, "SELL tc 10 @ 1520"),
            "SELL NSE:TCS 10 @ 1520"
        );
    }

    #[test]
    fn tab_is_a_noop_until_a_numeric_argument_is_valid() {
        for input in ["buy tc 0", "buy tc 10.5", "buy tc @-5", "buy tc 1 2"] {
            let mut app = app_with_master();
            assert_eq!(
                tabbed(&mut app, input),
                input,
                "{input:?} must not complete"
            );
            assert!(
                state(&app).results.is_empty(),
                "{input:?} offers no row to run or complete"
            );
        }
    }

    #[test]
    fn an_ambiguous_instrument_holds_tab_and_enter_until_a_row_is_chosen() {
        let mut app = app_with_master();
        open(&mut app, "buy inf");
        assert!(state(&app).ambiguous, "INFY is on NSE and BSE");
        assert_eq!(
            app.palette_context(state(&app)),
            "Choose exchange/contract with arrows or click"
        );

        press(&mut app, KeyCode::Tab);
        assert_eq!(state(&app).input, "buy inf");
        press(&mut app, KeyCode::Enter);
        assert!(
            matches!(app.modals.last(), Some(Modal::Palette(_))),
            "Enter stays in the palette while the choice is open"
        );

        press(&mut app, KeyCode::Down);
        assert!(state(&app).chosen);
        press(&mut app, KeyCode::Tab);
        assert!(
            state(&app).input.starts_with("buy BSE:INFY")
                || state(&app).input.starts_with("buy NSE:INFY"),
            "a chosen row completes: {:?}",
            state(&app).input
        );
    }

    #[test]
    fn the_new_alert_row_is_offered_from_every_pane() {
        for view in Preset::ALL.into_iter().flat_map(Preset::panes).copied() {
            let rows = super::App::menu_rows(view, &[]);
            let key = rows
                .iter()
                .find_map(|row| match row {
                    PaletteRow::Item { key, .. }
                        if matches!(row.item(), Some(ExecItem::Action(MenuAction::NewAlert))) =>
                    {
                        Some(*key)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("{view:?} must offer a new-alert row"));
            let expected = (view == PaneId::OrdersAlerts).then_some("a");
            assert_eq!(key, expected, "{view:?} advertises the wrong key");
        }
    }

    #[test]
    fn an_ambiguous_alert_instrument_holds_enter_until_a_row_is_chosen() {
        let mut app = app_with_master();
        open(&mut app, "alert inf");
        assert!(state(&app).ambiguous, "INFY is on NSE and BSE");
        assert_eq!(
            app.palette_context(state(&app)),
            "Choose exchange/contract with arrows or click"
        );
        press(&mut app, KeyCode::Tab);
        assert_eq!(state(&app).input, "alert inf", "Tab wrote nothing");
        press(&mut app, KeyCode::Enter);
        assert!(
            matches!(app.modals.last(), Some(Modal::Palette(_))),
            "Enter holds while the choice is open: {:?}",
            app.modals.last()
        );

        press(&mut app, KeyCode::Down);
        assert!(state(&app).chosen);
        press(&mut app, KeyCode::Tab);
        assert!(
            state(&app).input == "alert BSE:INFY" || state(&app).input == "alert NSE:INFY",
            "a chosen row completes: {:?}",
            state(&app).input
        );
        press(&mut app, KeyCode::Enter);
        let Some(Modal::Alert(form)) = app.modals.last() else {
            panic!("the chosen row opens the form: {:?}", app.modals.last());
        };
        assert_eq!(form.tradingsymbol, "INFY");

        let mut app = app_with_master();
        open(&mut app, "alert nse:infy");
        assert!(!state(&app).ambiguous);
        assert!(!state(&app).choice_needed());
    }

    #[test]
    fn an_exact_qualified_identity_bypasses_the_choice_guard() {
        let hits = [
            inst("NSE", "INFY", 408_065, "NSE"),
            inst("BSE", "INFY", 136_248, "BSE"),
        ];
        let refs: Vec<&Instrument> = hits.iter().collect();

        assert!(instrument_choice_needed("INFY", &refs));

        assert!(!instrument_choice_needed("NSE:INFY", &refs));
        assert!(!instrument_choice_needed("nse:infy", &refs));

        assert!(!instrument_choice_needed("INFY", &refs[..1]));

        let mut app = app_with_master();
        open(&mut app, "buy nse:infy");
        assert!(!state(&app).ambiguous);
        assert!(!state(&app).choice_needed());
    }

    #[test]
    fn bare_wl_lists_the_existing_lists_then_the_subcommands() {
        let mut app = fresh_app(Vec::new());
        open(&mut app, "wl");
        let items: Vec<&PaletteRow> = state(&app)
            .results
            .iter()
            .filter(|row| matches!(row, PaletteRow::Item { .. }))
            .collect();
        assert!(matches!(
            items[0].item(),
            Some(ExecItem::Watchlist(WatchlistCmd::Switch(name))) if name == "default"
        ));

        let completions: Vec<String> = state(&app)
            .results
            .iter()
            .filter_map(|row| match row {
                PaletteRow::Item {
                    action: PaletteAction::Complete { edit, .. },
                    ..
                } => Some(edit.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(completions, vec!["wl rename ", "wl delete"]);
    }

    #[test]
    fn tab_on_a_list_writes_its_name_and_does_not_switch() {
        let mut app = fresh_app(Vec::new());
        let before = app.watch_view().title().to_owned();
        assert_eq!(tabbed(&mut app, "wl"), "wl default");
        assert_eq!(
            app.watch_view().title(),
            before,
            "Tab must not switch lists"
        );
    }

    #[test]
    fn a_partial_list_name_keeps_the_literal_action_first() {
        let mut app = fresh_app(Vec::new());
        open(&mut app, "wl def");
        let items: Vec<&PaletteRow> = state(&app)
            .results
            .iter()
            .filter(|row| matches!(row, PaletteRow::Item { .. }))
            .collect();

        assert!(matches!(
            items[0].item(),
            Some(ExecItem::Watchlist(WatchlistCmd::Switch(name))) if name == "def"
        ));
        assert!(
            !state(&app).can_complete(),
            "the literal row has no useful completion"
        );
        assert_eq!(
            app.palette_context(state(&app)),
            "wl def: create it on your account"
        );

        press(&mut app, KeyCode::Down);
        press(&mut app, KeyCode::Tab);
        assert_eq!(state(&app).input, "wl default");
    }

    #[test]
    fn a_partial_name_is_never_reinterpreted_as_a_subcommand() {
        let mut app = fresh_app(Vec::new());
        open(&mut app, "wl ren");
        assert!(matches!(
            state(&app).draft,
            Draft::Watchlist(super::WatchlistDraft::Name { .. })
        ));

        assert!(matches!(
            state(&app).selected_item(),
            Some(ExecItem::Watchlist(WatchlistCmd::Switch(name))) if name == "ren"
        ));
    }

    #[test]
    fn wl_rename_without_a_name_offers_only_the_space() {
        let mut app = fresh_app(Vec::new());
        assert_eq!(tabbed(&mut app, "wl rename"), "wl rename ");
        assert_eq!(app.palette_context(state(&app)), "wl rename NAME");

        press(&mut app, KeyCode::Tab);
        assert_eq!(state(&app).input, "wl rename ");
        assert_eq!(state(&app).selected, None);
        assert!(state(&app).results.is_empty());
    }

    #[test]
    fn an_unknown_theme_is_not_actionable() {
        let _guard = settings::theme_guard(settings::theme());
        let mut app = fresh_app(Vec::new());
        open(&mut app, "theme notatheme");
        assert!(state(&app).results.is_empty());
        assert_eq!(state(&app).selected, None);
        assert_eq!(app.palette_context(state(&app)), "Unknown theme: notatheme");
    }

    #[test]
    fn the_master_arriving_rebuilds_an_open_palette() {
        let mut app = fresh_app(Vec::new());
        open(&mut app, "buy tc");
        assert!(state(&app).results.is_empty(), "no master, no order rows");
        assert!(matches!(state(&app).draft, Draft::Order(_)));

        app.on_data(DataEvent::Instruments(Ok(Arc::new(InstrumentStore::new(
            vec![inst("NSE", "TCS", 2_953_217, "NSE")],
        )))));
        assert!(
            matches!(state(&app).selected_item(), Some(ExecItem::Order { .. })),
            "the open palette picks the master up: {:?}",
            state(&app).results
        );
    }

    #[test]
    fn a_chosen_token_survives_the_master_refresh_and_a_lost_one_clears_it() {
        let mut app = app_with_master();
        open(&mut app, "buy inf");
        press(&mut app, KeyCode::Down);
        let chosen = state(&app).selected_identity();
        assert!(matches!(chosen, Some(RowIdentity::Instrument { .. })));

        app.on_data(DataEvent::Instruments(Ok(Arc::new(InstrumentStore::new(
            vec![
                inst("NSE", "INFY", 408_065, "NSE"),
                inst("BSE", "INFY", 136_248, "BSE"),
            ],
        )))));
        assert_eq!(state(&app).selected_identity(), chosen);
        assert!(state(&app).chosen);

        app.on_data(DataEvent::Instruments(Ok(Arc::new(InstrumentStore::new(
            vec![inst("NSE", "INFOSYS", 1, "NSE")],
        )))));
        assert_ne!(state(&app).selected_identity(), chosen);
        assert!(!state(&app).chosen);
    }

    #[test]
    fn a_truncated_result_set_says_so() {
        let rows: Vec<Instrument> = (0..30)
            .map(|i| inst("NSE", &format!("INFY{i}"), 100 + i, "NSE"))
            .collect();
        let mut app = fresh_app(Vec::new());
        app.instruments = Some(Arc::new(InstrumentStore::new(rows)));
        open(&mut app, "infy");
        assert!(state(&app).truncated);

        assert_eq!(
            app.palette_context(state(&app)),
            "Choose exchange/contract with arrows or click"
        );
        press(&mut app, KeyCode::Down);
        assert_eq!(
            app.palette_context(state(&app)),
            "More matches; refine query"
        );
    }

    #[test]
    fn browsing_the_menu_completes_to_text_that_finds_the_same_row() {
        let mut app = app_with_master();
        app.set_pane(PaneId::TradingWatch);
        open(&mut app, "");

        let command_forms = [
            (MenuAction::SwitchTheme, "theme "),
            (MenuAction::ManageWatchlists, "wl "),
            (MenuAction::Quit, "quit"),
        ];
        let rows = state(&app).results.clone();
        for (index, row) in rows.iter().enumerate() {
            let PaletteRow::Item { action, .. } = row else {
                continue;
            };
            let edit = action.completion().expect("a browse row completes");
            let identity = row_identity(row).expect("an item has an identity");
            let completed = edit.apply("");
            if let Some((_, form)) = command_forms
                .iter()
                .find(|(a, _)| identity == RowIdentity::Action(*a))
            {
                assert_eq!(&completed, form, "row {index} writes its command form");
                continue;
            }
            let build = app.palette_build(&completed);
            let rebuilt = PaletteState {
                results: build.rows,
                ..PaletteState::from_pane(PaneId::TradingWatch)
            };
            assert!(
                rebuilt.find_identity(&identity).is_some(),
                "row {index} completed to {completed:?}, which no longer finds it"
            );
        }
    }
}
