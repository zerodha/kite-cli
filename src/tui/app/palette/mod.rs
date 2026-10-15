pub(super) mod build;
pub(super) mod execute;
pub(super) mod menu;
pub(super) mod state;

use super::modals::theme::{preview_theme, restore_theme};
use super::{App, ExecItem, KeyCode, KeyEvent, KeyModifiers, Modal, PaletteAction, PaletteState};
use crate::tui::palette::{CompletionEdit, Draft, WatchlistDraft};
use build::side_word;

impl App {
    pub(in crate::tui::app) fn open_palette(&mut self) {
        self.open_palette_with_input(String::new());
    }

    fn open_palette_with_input(&mut self, input: String) {
        let mut state = PaletteState {
            input,
            ..PaletteState::from_pane(self.focused_pane())
        };
        self.rebuild_palette(&mut state, None);
        self.modals.push(Modal::Palette(state));
    }

    pub(in crate::tui::app) fn open_watchlist_manager(&mut self) {
        self.open_palette_with_input("wl ".to_string());
    }

    pub(in crate::tui::app) fn key_palette(&mut self, key: KeyEvent, mut state: PaletteState) {
        let ctrl = key.modifiers == KeyModifiers::CONTROL;
        match key.code {
            KeyCode::Esc => {
                restore_theme(&mut state.preview_base);
                return;
            }

            KeyCode::Tab => {
                self.complete_palette(&mut state);
                self.modals.push(Modal::Palette(state));
                return;
            }

            KeyCode::BackTab => {
                self.modals.push(Modal::Palette(state));
                return;
            }
            KeyCode::Enter => {
                match state.selected_action().cloned() {
                    Some(PaletteAction::Complete { edit, .. }) => {
                        self.apply_completion(&mut state, &edit);
                        self.modals.push(Modal::Palette(state));
                        return;
                    }

                    Some(PaletteAction::Execute { .. }) if state.choice_needed() => {
                        self.modals.push(Modal::Palette(state));
                        return;
                    }

                    Some(PaletteAction::Execute { item, .. })
                        if matches!(*item, ExecItem::Theme(_)) =>
                    {
                        state.preview_base = None;
                        self.exec_palette(*item);
                    }

                    Some(PaletteAction::Execute { item, .. }) => {
                        restore_theme(&mut state.preview_base);
                        self.exec_palette(*item);
                    }

                    None => {
                        self.modals.push(Modal::Palette(state));
                    }
                }
                return;
            }

            KeyCode::Down => state.step(1),
            KeyCode::Up => state.step(-1),
            KeyCode::Char('n') if ctrl => state.step(1),
            KeyCode::Char('p') if ctrl => state.step(-1),
            KeyCode::Backspace => {
                state.input.pop();
                self.rebuild_palette(&mut state, None);
            }
            KeyCode::Char(c) if crate::tui::event::is_text_key(key) => {
                state.input.push(c);
                self.rebuild_palette(&mut state, None);
            }
            _ => {}
        }

        Self::sync_preview(&mut state);
        self.modals.push(Modal::Palette(state));
    }

    fn complete_palette(&self, state: &mut PaletteState) {
        if state.choice_needed() {
            return;
        }
        let Some(edit) = state
            .selected_action()
            .and_then(PaletteAction::completion)
            .cloned()
            .filter(|edit| !edit.is_noop(&state.input))
        else {
            return;
        };
        self.apply_completion(state, &edit);
    }

    fn apply_completion(&self, state: &mut PaletteState, edit: &CompletionEdit) {
        let identity = state.selected_identity();
        state.input = edit.apply(&state.input);
        self.rebuild_palette(state, identity);
    }

    pub(in crate::tui::app) fn sync_preview(state: &mut PaletteState) {
        let name = match state.selected_item() {
            Some(ExecItem::Theme(name)) => Some(name.clone()),
            _ => None,
        };
        match name {
            Some(name) => preview_theme(&mut state.preview_base, &name),
            None => restore_theme(&mut state.preview_base),
        }
    }

    pub fn palette_context(&self, state: &PaletteState) -> String {
        if state.choice_needed() {
            return "Choose exchange/contract with arrows or click".to_string();
        }
        if let Some(PaletteAction::Complete { syntax, .. }) = state.selected_action() {
            return syntax.clone();
        }
        match &state.draft {
            Draft::Browse => "↑↓ select · Enter runs the highlighted action".to_string(),
            Draft::Order(order) => match order.error {
                Some(error) => error.message().to_string(),
                None if state.results.is_empty() => {
                    format!("{} SYMBOL [QTY] [@PRICE]", side_word(order.side))
                }
                None => "Enter opens ticket".to_string(),
            },
            Draft::Theme { query, .. } => {
                if state.results.is_empty() {
                    format!("Unknown theme: {query}")
                } else {
                    "theme PRESET: Enter saves".to_string()
                }
            }
            Draft::Alert { query, .. } => {
                if query.is_empty() {
                    "alert SYMBOL · Enter picks an instrument".to_string()
                } else if state.results.is_empty() {
                    format!("No instrument matches {query}")
                } else {
                    "Enter opens the alert form".to_string()
                }
            }
            Draft::Watchlist(wl) => self.watchlist_context(wl),
            Draft::Jump { jump, .. } => jump.syntax(),
            Draft::Workspace { query, .. } => Self::workspace_context(query),
            Draft::Quit => "quit: leave the terminal".to_string(),
            Draft::Partial => match crate::tui::palette::keyword_matches(state.input.trim()).next()
            {
                Some(kw) => kw.syntax(),
                None => Self::search_context(state),
            },
            Draft::Search => Self::search_context(state),
        }
    }

    fn search_context(state: &PaletteState) -> String {
        if state.truncated {
            "More matches; refine query".to_string()
        } else {
            "Search instruments and menu actions".to_string()
        }
    }

    fn watchlist_context(&self, wl: &WatchlistDraft) -> String {
        let exists = |name: &str| {
            self.watch
                .summaries()
                .any(|summary| summary.name.eq_ignore_ascii_case(name))
        };
        match wl {
            WatchlistDraft::List => "wl NAME: create or switch".to_string(),
            WatchlistDraft::Name { name, .. } => {
                if !self.watch.catalog_available() {
                    "watchlists are not available; refresh before creating a list".to_string()
                } else if self
                    .watch
                    .summaries()
                    .filter(|summary| summary.name.eq_ignore_ascii_case(name))
                    .count()
                    > 1
                {
                    format!("wl {name}: multiple lists have this name; select a list by id")
                } else if exists(name) {
                    format!("wl {name}: switch to it")
                } else {
                    format!("wl {name}: create it on your account")
                }
            }
            WatchlistDraft::RenameMissing { .. } => "wl rename NAME".to_string(),
            WatchlistDraft::Rename { name } => {
                let active = self.watch_view().title();
                if exists(name) {
                    format!("Rename {active} → {name} (a list already has that name)")
                } else {
                    format!("Rename {active} → {name}")
                }
            }
            WatchlistDraft::Delete => format!(
                "Delete {} and its {} instruments",
                self.watch_view().title(),
                self.watch_items().len()
            ),
            WatchlistDraft::DeleteExtra => "wl delete takes no name".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::modals::theme::theme_filter;
    use super::super::{
        App, ExecItem, KeyCode, KeyEvent, KeyModifiers, MenuAction, PaletteRow, PaletteState,
    };
    use crate::tui::marketwatch::{State as WatchState, WatchItem};
    use crate::tui::notes::Pad;
    use crate::tui::settings::{self, Settings};
    use crate::tui::workspace::PaneId;
    use kiteconnect::KiteConnect;

    use std::sync::Arc;

    #[test]
    fn w_opens_watchlist_manager_with_the_existing_lists() {
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("token").unwrap())
            .build()
            .expect("dummy client builds");
        let mut app = App::new(
            client,
            events_tx,
            WatchState::test_loaded(Vec::new()),
            Pad::default(),
            true,
            &Settings::default(),
        );
        app.set_pane(PaneId::TradingWatch);

        app.on_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::NONE));
        let Some(super::Modal::Palette(state)) = app.modals.last() else {
            panic!("w must open the watchlist manager");
        };
        assert_eq!(state.input, "wl ");
        assert!(matches!(
            state.selected_item(),
            Some(ExecItem::Watchlist(crate::tui::palette::WatchlistCmd::Switch(name)))
                if name == "default"
        ));
    }

    use super::super::{Modal, PaletteAction};
    use crate::instruments::{Instrument, InstrumentStore};

    pub(in crate::tui::app) fn inst(
        exchange: &str,
        tradingsymbol: &str,
        token: u32,
        segment: &str,
    ) -> Instrument {
        Instrument {
            instrument_token: token,
            tradingsymbol: tradingsymbol.to_string(),
            name: tradingsymbol.to_string(),
            exchange: exchange.to_string(),
            expiry: None,
            strike: None,
            tick_size: 0.05,
            lot_size: 1,
            instrument_type: "EQ".to_string(),
            segment: segment.to_string(),
            freeze_quantity: None,
            max_order_quantity: None,
        }
    }

    pub(in crate::tui::app) fn app_with_master() -> App {
        let mut app = fresh_app(Vec::new());
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![
            inst("NSE", "INFY", 408_065, "NSE"),
            inst("BSE", "INFY", 136_248, "BSE"),
            inst("NSE", "TCS", 2_953_217, "NSE"),
            inst("NSE", "NIFTY 50", 256_265, "INDICES"),
        ])));
        app
    }

    pub(in crate::tui::app) fn fresh_app(rows: Vec<WatchItem>) -> App {
        let (events_tx, _events_rx) = tokio::sync::mpsc::channel(8);
        let client = KiteConnect::builder("test-key")
            .access_token(kiteconnect::AccessToken::new("token").unwrap())
            .build()
            .expect("dummy client builds");
        App::new(
            client,
            events_tx,
            WatchState::test_loaded(rows),
            Pad::default(),
            true,
            &Settings::default(),
        )
    }

    pub(in crate::tui::app) fn open(app: &mut App, input: &str) {
        app.on_key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::CONTROL));
        for c in input.chars() {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
    }

    pub(in crate::tui::app) fn state(app: &App) -> &PaletteState {
        match app.modals.last() {
            Some(Modal::Palette(state)) => state,
            _ => panic!("the palette is open"),
        }
    }

    pub(in crate::tui::app) fn press(app: &mut App, code: KeyCode) {
        app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    pub(in crate::tui::app) fn tabbed(app: &mut App, input: &str) -> String {
        open(app, input);
        press(app, KeyCode::Tab);
        state(app).input.clone()
    }

    #[test]
    fn the_alert_command_completes_and_opens_the_form_for_a_unique_instrument() {
        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "alert tc"), "alert NSE:TCS");
        assert_eq!(
            app.palette_context(state(&app)),
            "Enter opens the alert form"
        );
        assert_eq!(state(&app).enter_effect(), Some("opens alert form"));
        press(&mut app, KeyCode::Enter);
        let Some(Modal::Alert(form)) = app.modals.last() else {
            panic!("a unique alert row opens the form: {:?}", app.modals.last());
        };
        assert_eq!(form.tradingsymbol, "TCS");
        assert_eq!(form.instrument_token, 2_953_217);

        let mut app = app_with_master();
        assert_eq!(tabbed(&mut app, "alert nifty"), "alert NSE:NIFTY 50");
        press(&mut app, KeyCode::Enter);
        let Some(Modal::Alert(form)) = app.modals.last() else {
            panic!("an index alert opens the form: {:?}", app.modals.last());
        };
        assert_eq!(form.tradingsymbol, "NIFTY 50");
        assert_eq!(form.exchange, "INDICES");

        let mut app = app_with_master();
        open(&mut app, "alert");
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
        assert_eq!(completions, vec!["alerts".to_string()]);

        for input in ["a", "al", "ale", "aler", "alerts"] {
            let mut app = app_with_master();
            open(&mut app, input);
            assert!(
                matches!(
                    state(&app).selected_item(),
                    Some(ExecItem::Jump(crate::tui::palette::Jump::Pane(
                        PaneId::OrdersAlerts
                    )))
                ),
                "{input:?} must stay the Alerts pane jump"
            );
        }
    }

    #[test]
    fn a_query_edit_clears_an_earlier_choice() {
        let mut app = app_with_master();
        open(&mut app, "buy inf");
        press(&mut app, KeyCode::Down);
        assert!(state(&app).chosen);
        press(&mut app, KeyCode::Char('y'));
        assert!(!state(&app).chosen, "typing restates the query");
        press(&mut app, KeyCode::Backspace);
        assert!(!state(&app).chosen);
    }

    #[test]
    fn tab_never_runs_an_action() {
        let _guard = settings::theme_guard(settings::theme());
        let inputs = [
            "quit",
            "exit",
            "orders",
            "Logout",
            "Refresh",
            "theme dra",
            "wl default",
            "wl delete",
            "buy tc",
            "tc",
            "Help",
            "Notes",
            "Settings file path",
            "Toggle chart",
        ];
        for input in inputs {
            let mut app = app_with_master();
            let before_view = app.workspace();
            let before_watch = app.watch_view().title().to_owned();
            open(&mut app, input);

            let before_theme = settings::theme();
            let before_preview = state(&app).preview_base;
            let before_input = state(&app).input.clone();
            press(&mut app, KeyCode::Tab);
            assert!(
                matches!(app.modals.last(), Some(Modal::Palette(_))),
                "{input:?}: Tab opened or closed a modal"
            );
            assert_eq!(app.modals.len(), 1, "{input:?}: Tab stacked a modal");
            assert!(!app.should_quit, "{input:?}: Tab quit the app");
            assert!(!app.should_logout, "{input:?}: Tab logged out");
            assert_eq!(app.workspace(), before_view, "{input:?}: Tab switched view");
            assert_eq!(
                settings::theme(),
                before_theme,
                "{input:?}: Tab changed the theme"
            );
            assert_eq!(
                app.watch_view().title(),
                before_watch,
                "{input:?}: Tab changed the watchlist"
            );
            assert_eq!(
                state(&app).preview_base,
                before_preview,
                "{input:?}: Tab started or ended a theme preview"
            );

            let after = state(&app).input.clone();
            assert!(
                after == before_input || !after.is_empty(),
                "{input:?}: Tab emptied the input"
            );
        }
    }

    #[test]
    fn enter_on_a_completion_row_edits_and_stays_open() {
        let mut app = app_with_master();
        open(&mut app, "bu");

        let row = state(&app)
            .results
            .iter()
            .find(|row| {
                matches!(
                    row,
                    PaletteRow::Item {
                        action: PaletteAction::Complete { .. },
                        ..
                    }
                )
            })
            .expect("`bu` offers a keyword candidate");
        assert!(row.item().is_none(), "a completion row carries no command");
        assert_eq!(state(&app).enter_effect(), Some("complete"));

        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.modals.last(), Some(Modal::Palette(_))));
        assert_eq!(state(&app).input, "buy ");
    }

    #[test]
    fn enter_stays_open_on_empty_incomplete_and_invalid_input() {
        for input in ["zzzznotathing", "buy tc 0", "wl rename ", "wl delete extra"] {
            let mut app = app_with_master();
            open(&mut app, input);
            press(&mut app, KeyCode::Enter);
            assert!(
                matches!(app.modals.last(), Some(Modal::Palette(_))),
                "{input:?}: Enter must not leave the palette with nothing to run"
            );
        }
    }

    #[test]
    fn enter_on_a_chosen_order_reaches_the_ticket() {
        let mut app = app_with_master();
        open(&mut app, "buy tc 5");
        assert_eq!(state(&app).enter_effect(), Some("opens ticket"));
        press(&mut app, KeyCode::Enter);
        let Some(Modal::Ticket(ticket)) = app.modals.last() else {
            panic!("a unique order row opens a ticket: {:?}", app.modals.last());
        };
        assert_eq!(ticket.tradingsymbol, "TCS");
    }

    #[test]
    fn wl_delete_hints_the_active_list_and_its_size() {
        let mut app = fresh_app(vec![]);
        open(&mut app, "wl delete");
        assert_eq!(
            app.palette_context(state(&app)),
            "Delete default and its 0 instruments"
        );
        assert_eq!(state(&app).enter_effect(), Some("confirms delete"));

        assert!(!state(&app).can_complete());

        let mut app = fresh_app(vec![]);
        open(&mut app, "wl delete extra");
        assert_eq!(app.palette_context(state(&app)), "wl delete takes no name");
        assert!(state(&app).results.is_empty());
    }

    #[test]
    fn a_separator_insensitive_theme_prefix_completes_to_a_real_preset() {
        let _guard = settings::theme_guard(settings::theme());
        let original = settings::theme();
        let mut app = fresh_app(Vec::new());
        let preset = theme_filter("")
            .into_iter()
            .find(|name| name.contains('-') || name.contains('_'))
            .expect("some preset carries a separator");
        let typed: String = preset.replace(['-', '_'], "");
        open(&mut app, &format!("theme {typed}"));

        let previewed = settings::theme();
        press(&mut app, KeyCode::Tab);
        assert_eq!(state(&app).input, format!("theme {preset}"));
        assert_eq!(settings::theme(), previewed);

        press(&mut app, KeyCode::Esc);
        assert_eq!(settings::theme(), original);
        settings::set_theme(original);
    }

    #[test]
    fn escape_restores_a_preview_a_tab_did_not_disturb() {
        let _guard = settings::theme_guard(settings::theme());
        let original = settings::theme();
        let mut app = fresh_app(Vec::new());
        open(&mut app, "theme ");

        press(&mut app, KeyCode::Down);
        let previewed = settings::theme();
        assert!(state(&app).preview_base.is_some());
        press(&mut app, KeyCode::Tab);
        assert_eq!(
            settings::theme(),
            previewed,
            "Tab leaves an active preview alone"
        );
        press(&mut app, KeyCode::Esc);
        assert_eq!(settings::theme(), original, "Esc restores the base");
        settings::set_theme(original);
    }

    #[test]
    fn a_completed_menu_row_keeps_its_selection() {
        let mut app = app_with_master();
        open(&mut app, "refr");
        assert!(matches!(
            state(&app).selected_item(),
            Some(ExecItem::Action(MenuAction::RefreshAll))
        ));
        press(&mut app, KeyCode::Tab);
        assert_eq!(state(&app).input, "Refresh");
        assert!(
            matches!(
                state(&app).selected_item(),
                Some(ExecItem::Action(MenuAction::RefreshAll))
            ),
            "the completed row stays selected instead of handing over to a \
             coincident instrument"
        );
    }

    #[test]
    fn backtab_is_consumed_as_a_noop() {
        let mut app = app_with_master();
        let before_view = app.workspace();
        open(&mut app, "ord");
        let before = state(&app).input.clone();
        let selected = state(&app).selected;
        press(&mut app, KeyCode::BackTab);
        assert_eq!(state(&app).input, before);
        assert_eq!(state(&app).selected, selected);
        assert_eq!(
            app.workspace(),
            before_view,
            "BackTab must not cycle the view"
        );
    }

    #[test]
    fn printable_keys_stay_query_text_while_the_palette_owns_input() {
        let mut app = app_with_master();
        let before_view = app.workspace();
        open(&mut app, "");
        for c in ['j', 'k', 'q', '1', '?', ':', '/'] {
            app.on_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(state(&app).input, "jkq1?:/");
        assert_eq!(app.modals.len(), 1, "no shortcut opened another modal");
        assert_eq!(app.workspace(), before_view);
        assert!(!app.should_quit);
    }

    #[test]
    fn an_exec_row_with_no_useful_completion_omits_tab() {
        let mut app = fresh_app(Vec::new());
        open(&mut app, "quit");

        assert!(!state(&app).can_complete());
        assert_eq!(state(&app).enter_effect(), Some("quits"));
        let mut app = fresh_app(Vec::new());
        open(&mut app, "q");
        assert!(state(&app).can_complete());
    }
}
