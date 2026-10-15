use kiteconnect::OrderType;
pub(super) mod alert;
pub(super) mod alert_submit;
pub(super) mod ato;
pub(super) mod cancellation;
pub(super) mod confirm;
pub(super) mod conversion;
pub(super) mod events;
pub(super) mod notes;
pub(super) mod order_detail;
pub(super) mod picker;
pub(super) mod products;
pub(super) mod search;
pub(super) mod theme;
pub(super) mod ticket;
pub(super) mod ticket_seed;

use super::{
    AlertFormState, AtoItemFormState, EventLogState, OrderDetailState, PaletteState, SearchState,
    ThemePickerState, WorkspacePickerState,
};
use crate::tui::data::AlertTrigger;
use crate::tui::notes::NotesState;
use crate::tui::ticket::{ConfirmState, PositionConversionState, TicketState};

#[derive(Debug, Clone, Copy)]
pub struct TicketPrefs {
    pub product: usize,
    pub autoslice: bool,
    pub order_type: OrderType,
    pub validity: usize,

    pub validity_ttl: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchPurpose {
    #[default]
    AddToWatch,

    ChainUnderlying,

    NewAlert,

    AtoBasketItem,
}

#[derive(Debug)]
pub struct AlertHistoryState {
    pub uuid: String,

    pub request: crate::tui::data::AlertRequestId,

    pub label: String,

    pub triggers: Option<Result<Vec<AlertTrigger>, String>>,

    pub basket_len: Option<usize>,
}

#[derive(Debug)]
pub enum Modal {
    Help(u16),
    Search(SearchState),
    Palette(PaletteState),
    ThemePicker(ThemePickerState),

    WorkspacePicker(WorkspacePickerState),
    Ticket(Box<TicketState>),
    Confirm(Box<ConfirmState>),
    OrderDetail(Box<OrderDetailState>),

    PositionConvert(Box<PositionConversionState>),

    Alert(Box<AlertFormState>),

    AtoItem(Box<AtoItemFormState>),
    EventLog(EventLogState),

    Notes(NotesState),

    Funds,
}
