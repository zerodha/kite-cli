use super::super::{MenuAction, Side};
use crate::tui::instruments::Instrument;
use crate::tui::palette::{CompletionEdit, Draft, Jump, WatchlistCmd};
use crate::tui::settings::Theme;
use crate::tui::workspace::PaneId;

#[derive(Debug, Clone)]
pub enum ExecItem {
    Order {
        side: Side,
        instrument: Instrument,
        qty: Option<u32>,
        price: Option<f64>,
    },

    Alert(Instrument),

    Jump(Jump),

    FocusPane(PaneId),

    Watchlist(WatchlistCmd),

    Quit,

    Theme(String),

    Action(MenuAction),

    Instrument(Instrument),
}

#[derive(Debug, Clone)]
pub enum PaletteAction {
    Execute {
        item: Box<ExecItem>,
        completion: Option<CompletionEdit>,
    },

    Complete {
        label: String,

        syntax: String,
        edit: CompletionEdit,
    },
}

impl PaletteAction {
    pub(in crate::tui::app) fn completion(&self) -> Option<&CompletionEdit> {
        match self {
            Self::Execute { completion, .. } => completion.as_ref(),
            Self::Complete { edit, .. } => Some(edit),
        }
    }

    fn item(&self) -> Option<&ExecItem> {
        match self {
            Self::Execute { item, .. } => Some(item.as_ref()),
            Self::Complete { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PaletteRow {
    Header(&'static str),

    Item {
        action: PaletteAction,
        key: Option<&'static str>,
    },
}

impl PaletteRow {
    pub(in crate::tui::app) fn item(&self) -> Option<&ExecItem> {
        match self {
            Self::Item { action, .. } => action.item(),
            Self::Header(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RowIdentity {
    Action(MenuAction),

    Jump(&'static str),

    Pane(&'static str),
    Theme(String),

    Watchlist(String),
    Quit,

    Alert(String),

    Instrument { label: String, side: Option<Side> },

    Complete(String),
}

#[derive(Debug)]
pub struct PaletteState {
    pub input: String,

    pub origin: PaneId,

    pub selected: Option<usize>,
    pub results: Vec<PaletteRow>,

    pub draft: Draft,

    pub chosen: bool,

    pub ambiguous: bool,

    pub truncated: bool,

    pub preview_base: Option<Theme>,
}

impl PaletteState {
    pub(super) fn choice_needed(&self) -> bool {
        self.ambiguous
            && !self.chosen
            && matches!(
                self.selected_item(),
                Some(ExecItem::Instrument(_) | ExecItem::Order { .. } | ExecItem::Alert(_))
            )
    }

    pub fn can_complete(&self) -> bool {
        !self.choice_needed()
            && self
                .selected_action()
                .and_then(PaletteAction::completion)
                .is_some_and(|edit| !edit.is_noop(&self.input))
    }

    pub fn enter_effect(&self) -> Option<&'static str> {
        Some(match self.selected_action()? {
            PaletteAction::Complete { .. } => "complete",
            PaletteAction::Execute { item, .. } => match item.as_ref() {
                ExecItem::Order { .. } => "opens ticket",
                ExecItem::Alert(_) => "opens alert form",
                ExecItem::Theme(_) => "saves theme",
                ExecItem::Quit => "quits",
                ExecItem::Jump(Jump::Workspace(_)) => "switches workspace",
                ExecItem::Jump(Jump::Pane(_)) => "opens the pane",
                ExecItem::FocusPane(_) => "focuses the pane",
                ExecItem::Instrument(_) => "adds to watch",
                ExecItem::Watchlist(WatchlistCmd::Switch(_) | WatchlistCmd::SwitchId { .. }) => {
                    "switch or create"
                }
                ExecItem::Watchlist(WatchlistCmd::Rename(_)) => "renames the list",
                ExecItem::Watchlist(WatchlistCmd::Delete) => "confirms delete",
                ExecItem::Action(_) => "runs",
            },
        })
    }

    #[cfg(test)]
    pub fn for_test() -> Self {
        Self::from_pane(PaneId::TradingWatch)
    }

    pub(in crate::tui::app) fn from_pane(origin: PaneId) -> Self {
        Self {
            input: String::new(),
            origin,
            selected: None,
            results: Vec::new(),
            draft: Draft::default(),
            chosen: false,
            ambiguous: false,
            truncated: false,
            preview_base: None,
        }
    }

    pub(in crate::tui::app) fn selected_action(&self) -> Option<&PaletteAction> {
        match self.results.get(self.selected?) {
            Some(PaletteRow::Item { action, .. }) => Some(action),
            _ => None,
        }
    }

    pub(in crate::tui::app) fn selected_item(&self) -> Option<&ExecItem> {
        self.selected_action()?.item()
    }

    pub(in crate::tui::app) fn first_item(&self) -> Option<usize> {
        self.results
            .iter()
            .position(|row| matches!(row, PaletteRow::Item { .. }))
    }

    pub(in crate::tui::app) fn find_identity(&self, identity: &RowIdentity) -> Option<usize> {
        self.results
            .iter()
            .position(|row| row_identity(row).as_ref() == Some(identity))
    }

    pub(in crate::tui::app) fn selected_identity(&self) -> Option<RowIdentity> {
        row_identity(self.results.get(self.selected?)?)
    }

    pub(in crate::tui::app) fn step(&mut self, delta: isize) {
        self.chosen = true;
        let len = self.results.len();
        let Some(selected) = self.selected else {
            self.selected = self.first_item();
            return;
        };
        let mut index = selected;
        loop {
            let Some(next) = index.checked_add_signed(delta).filter(|&next| next < len) else {
                return;
            };
            index = next;
            if matches!(self.results[index], PaletteRow::Item { .. }) {
                self.selected = Some(index);
                return;
            }
        }
    }
}

pub(in crate::tui::app) fn row_identity(row: &PaletteRow) -> Option<RowIdentity> {
    let Some(item) = row.item() else {
        return match row {
            PaletteRow::Item {
                action: PaletteAction::Complete { edit, .. },
                ..
            } => Some(RowIdentity::Complete(edit.text.clone())),
            _ => None,
        };
    };
    Some(match item {
        ExecItem::Action(action) => RowIdentity::Action(*action),
        ExecItem::Alert(inst) => RowIdentity::Alert(inst.label()),
        ExecItem::Jump(jump) => RowIdentity::Jump(jump.config_token()),
        ExecItem::FocusPane(pane) => RowIdentity::Pane(pane.title()),
        ExecItem::Theme(name) => RowIdentity::Theme(name.clone()),
        ExecItem::Quit => RowIdentity::Quit,
        ExecItem::Watchlist(cmd) => RowIdentity::Watchlist(match cmd {
            WatchlistCmd::Switch(name) => format!("switch {name}"),
            WatchlistCmd::SwitchId { id, .. } => format!("switch-id {id}"),
            WatchlistCmd::Rename(name) => format!("rename {name}"),
            WatchlistCmd::Delete => "delete".to_string(),
        }),
        ExecItem::Instrument(inst) => RowIdentity::Instrument {
            label: inst.label(),
            side: None,
        },
        ExecItem::Order {
            side, instrument, ..
        } => RowIdentity::Instrument {
            label: instrument.label(),
            side: Some(*side),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::{ExecItem, MenuAction, PaletteRow, PaletteState, Side, exec_row};
    use super::row_identity;
    use crate::tui::instruments::Instrument;
    use crate::tui::workspace::PaneId;

    fn grouped_state() -> PaletteState {
        let item = |a| exec_row(ExecItem::Action(a), None);
        PaletteState {
            selected: Some(1),
            results: vec![
                PaletteRow::Header("Suggested"),
                item(MenuAction::RefreshAll),
                item(MenuAction::Help),
                PaletteRow::Header("App"),
                item(MenuAction::Quit),
            ],
            ..PaletteState::from_pane(PaneId::TradingWatch)
        }
    }

    #[test]
    fn first_item_skips_leading_header() {
        let state = grouped_state();

        assert_eq!(state.first_item(), Some(1));
    }

    #[test]
    fn step_skips_headers_and_clamps() {
        let mut state = grouped_state();
        assert_eq!(state.selected, Some(1));

        state.step(1);
        assert_eq!(state.selected, Some(2));
        state.step(1);
        assert_eq!(state.selected, Some(4));

        state.step(1);
        assert_eq!(state.selected, Some(4));

        state.step(-1);
        assert_eq!(state.selected, Some(2));

        state.step(-1);
        assert_eq!(state.selected, Some(1));
        state.step(-1);
        assert_eq!(state.selected, Some(1));
        assert!(matches!(
            state.results[state.selected.expect("a selection")],
            PaletteRow::Item { .. }
        ));
    }

    #[test]
    fn selected_item_ignores_headers() {
        let mut state = grouped_state();

        state.selected = Some(0);
        assert!(state.selected_item().is_none());
        state.selected = Some(1);
        assert!(matches!(
            state.selected_item(),
            Some(ExecItem::Action(MenuAction::RefreshAll))
        ));
    }

    fn inst(exchange: &str, tradingsymbol: &str, token: u32, segment: &str) -> Instrument {
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

    #[test]
    fn row_identity_distinguishes_the_two_sides_of_one_instrument() {
        let buy = exec_row(
            ExecItem::Order {
                side: Side::Buy,
                instrument: inst("NSE", "INFY", 408_065, "NSE"),
                qty: None,
                price: None,
            },
            None,
        );
        let sell = exec_row(
            ExecItem::Order {
                side: Side::Sell,
                instrument: inst("NSE", "INFY", 408_065, "NSE"),
                qty: None,
                price: None,
            },
            None,
        );
        let watch = exec_row(
            ExecItem::Instrument(inst("NSE", "INFY", 408_065, "NSE")),
            None,
        );
        assert_ne!(row_identity(&buy), row_identity(&sell));
        assert_ne!(row_identity(&buy), row_identity(&watch));
        assert_eq!(row_identity(&buy), row_identity(&buy.clone()));
    }
}
