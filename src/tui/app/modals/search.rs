use super::super::panes::chain::underlying_name_of;
use super::super::{App, Modal, SearchPurpose, ToastLevel};
use crate::tui::instruments::{Instrument, InstrumentStore};
use crate::tui::workspace::PaneId;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug)]
pub struct SearchState {
    pub input: String,
    pub selected: usize,
    pub results: Vec<Instrument>,
    pub purpose: SearchPurpose,

    pub origin: PaneId,
}

impl Default for SearchState {
    fn default() -> Self {
        Self {
            input: String::new(),
            selected: 0,
            results: Vec::new(),
            purpose: SearchPurpose::default(),
            origin: PaneId::TradingWatch,
        }
    }
}

impl SearchState {
    pub(in crate::tui::app) fn from_pane(origin: PaneId) -> Self {
        Self {
            origin,
            ..Self::default()
        }
    }

    fn refresh(&mut self, store: &InstrumentStore) {
        self.results = store.search(&self.input, 50).into_iter().cloned().collect();
        self.selected = self.selected.min(self.results.len().saturating_sub(1));
    }
}

impl App {
    pub(in crate::tui::app) fn refresh_search_results(&mut self, store: Option<&InstrumentStore>) {
        let Some(store) = store.or(self.instruments.as_deref()) else {
            return;
        };
        if let Some(Modal::Search(state)) = self.modals.last_mut() {
            state.refresh(store);
        }
    }

    pub(in crate::tui::app) fn key_search(&mut self, key: KeyEvent, mut state: SearchState) {
        match key.code {
            KeyCode::Esc => return,
            KeyCode::Enter => {
                if self.instruments.is_none() {
                    self.toast(
                        ToastLevel::Info,
                        "instrument master unavailable; Ctrl-R retries the download",
                    );
                    self.modals.push(Modal::Search(state));
                    return;
                }
                if let Some(inst) = state.results.get(state.selected) {
                    match state.purpose {
                        SearchPurpose::AddToWatch => {
                            let pane = match self.watch_instance(state.origin) {
                                Some(_) => state.origin,
                                None => PaneId::TradingWatch,
                            };
                            self.add_watch_item(pane, inst);
                        }

                        SearchPurpose::ChainUnderlying => {
                            self.set_chain_underlying(state.origin, underlying_name_of(inst));
                        }
                        SearchPurpose::NewAlert => {
                            self.open_alert_form(
                                &inst.exchange,
                                &inst.tradingsymbol,
                                inst.instrument_token,
                                &inst.segment,
                            );
                        }
                        SearchPurpose::AtoBasketItem => {
                            self.open_ato_item_new(inst);
                        }
                    }
                }
                return;
            }
            KeyCode::Down => {
                state.selected = (state.selected + 1).min(state.results.len().saturating_sub(1));
            }
            KeyCode::Up => state.selected = state.selected.saturating_sub(1),
            KeyCode::Char('n') if key.modifiers == KeyModifiers::CONTROL => {
                state.selected = (state.selected + 1).min(state.results.len().saturating_sub(1));
            }
            KeyCode::Char('p') if key.modifiers == KeyModifiers::CONTROL => {
                state.selected = state.selected.saturating_sub(1);
            }
            KeyCode::Backspace => {
                state.input.pop();
                state.selected = 0;
                if let Some(store) = &self.instruments {
                    state.refresh(store);
                }
            }
            KeyCode::Char(c) if crate::tui::event::is_text_key(key) => {
                state.input.push(c);
                state.selected = 0;
                if let Some(store) = &self.instruments {
                    state.refresh(store);
                }
            }
            _ => {}
        }
        self.modals.push(Modal::Search(state));
    }

    pub(in crate::tui::app) fn open_new_alert_search(&mut self) {
        self.modals.push(Modal::Search(SearchState {
            purpose: SearchPurpose::NewAlert,
            ..SearchState::default()
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::test_support::{
        H, W, char_key, char_key_code, click, master_inst, press_at, region_of, render, test_app,
    };
    use super::super::super::{Modal, ui};
    use crate::tui::instruments::InstrumentStore;
    use crate::tui::workspace::PaneId;
    use std::sync::Arc;

    #[test]
    fn search_result_click_selects_it() {
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![
            master_inst("NSE", "NSE", "INFY", 1),
            master_inst("NSE", "NSE", "INFYA", 2),
            master_inst("NSE", "NSE", "INFYB", 3),
        ])));
        app.set_pane(PaneId::TradingWatch);
        app.on_key(char_key('/'));
        for c in "INFY".chars() {
            app.on_key(char_key(c));
        }
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Search,
                    ..
                }
            )
        });
        click(&mut app, W, H, rows.rect.x + 2, rows.rect.y + 1);
        let Some(Modal::Search(state)) = app.modals.last() else {
            panic!("the search is still open");
        };
        assert_eq!(state.selected, 1);
    }

    #[test]
    fn a_refiltered_search_retires_its_row_targets() {
        use crossterm::event::KeyCode;
        let mut app = test_app();
        app.instruments = Some(Arc::new(InstrumentStore::new(vec![
            master_inst("NSE", "NSE", "INFY", 1),
            master_inst("NSE", "NSE", "INFYA", 2),
            master_inst("NSE", "NSE", "INFYB", 3),
            master_inst("NSE", "NSE", "TCS", 4),
            master_inst("NSE", "NSE", "TCSA", 5),
            master_inst("NSE", "NSE", "TCSB", 6),
        ])));
        app.set_pane(PaneId::TradingWatch);
        app.on_key(char_key('/'));
        for c in "INFY".chars() {
            app.on_key(char_key(c));
        }
        render(&app, W, H);
        let rows = region_of(&app, |action| {
            matches!(
                action,
                ui::HitAction::Rows {
                    list: ui::HitList::Search,
                    ..
                }
            )
        });

        for _ in 0..4 {
            app.on_key(char_key_code(KeyCode::Backspace));
        }
        for c in "TCS".chars() {
            app.on_key(char_key(c));
        }
        let selected = |app: &super::super::super::App| match app.modals.last() {
            Some(Modal::Search(state)) => (state.results.len(), state.selected),
            _ => panic!("the search is still open"),
        };
        let (count, before) = selected(&app);
        assert_eq!(count, 3, "the new query has as many rows as the old");
        press_at(&mut app, rows.rect.x + 2, rows.rect.y + 2);
        assert_eq!(
            selected(&app).1,
            before,
            "the click was aimed at INFYB, which is gone"
        );
    }
}
