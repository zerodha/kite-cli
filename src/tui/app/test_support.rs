use super::{
    AlertFormFocus, AlertFormStage, AlertFormState, App, KeyCode, KeyEvent, KeyModifiers, Modal,
    MouseButton, MouseEvent, MouseEventKind, PRODUCTS, Side, TicketField, ui,
};
use crate::tui::alerts::{self, Attribute, Operator};
use crate::tui::data::Alert;
use crate::tui::data::{Holding, Order, Position};
use crate::tui::form::ControlIntent;
use crate::tui::instruments::Instrument;
use crate::tui::marketwatch::State as WatchState;
use crate::tui::notes::Pad;
use crate::tui::settings::Settings;
use crate::tui::ticket::TicketState;
use crate::tui::workspace::PaneId;
use kiteconnect::AlertStatus;
use kiteconnect::KiteConnect;
use kiteconnect::OrderType;
use kiteconnect::ticker::Tick;
use tokio::sync::mpsc;

pub(super) fn test_app() -> App {
    let client = KiteConnect::builder("test-key")
        .access_token(kiteconnect::AccessToken::new("tok").unwrap())
        .build()
        .expect("dummy client builds");
    let (events_tx, _events_rx) = mpsc::channel(8);
    App::new(
        client,
        events_tx,
        WatchState::test_loaded(Vec::new()),
        Pad::default(),
        false,
        &Settings::default(),
    )
}

pub(super) fn holding(
    symbol: &str,
    token: u32,
    avg: f64,
    last: f64,
    close: f64,
    qty: i64,
) -> Holding {
    let mut row = crate::tui::data::test_models::holding();
    row.tradingsymbol = symbol.into();
    row.exchange = "NSE".into();
    row.instrument_token = token;
    row.quantity = qty;
    row.t1_quantity = 0;
    row.collateral_quantity = 0;
    row.mtf = None;
    row.average_price = avg;
    row.last_price = last;
    row.close_price = close;
    row.day_change = None;
    row.discrepancy = false;
    row
}

pub(super) fn position(
    symbol: &str,
    token: u32,
    qty: i64,
    pnl: f64,
    last: f64,
    close: f64,
) -> Position {
    let mut row = crate::tui::data::test_models::position();
    row.tradingsymbol = symbol.into();
    row.exchange = "NSE".into();
    row.instrument_token = token;
    row.product = "MIS".into();
    row.overnight_quantity = 0;
    row.day_buy_quantity = 0;
    row.day_sell_quantity = 0;
    row.quantity = qty;
    row.average_price = 0.0;
    row.last_price = last;
    row.close_price = close;
    row.pnl = pnl;
    row.realised = 0.0;
    row.multiplier = 1.0;
    row
}

pub(super) fn sample_order(status: &str) -> Order {
    let mut row = crate::tui::data::test_models::order();
    row.order_id = "OID1".into();
    row.order_timestamp = "2024-01-01T09:15:00".parse().expect("timestamp");
    row.status = status.into();
    row.status_message = None;
    row.exchange = "NSE".into();
    row.tradingsymbol = "INFY".into();
    row.instrument_token = 408_065;
    row.transaction_type = "BUY".into();
    row.order_type = "LIMIT".into();
    row.product = "CNC".into();
    row.validity = "DAY".into();
    row.validity_ttl = None;
    row.variety = "regular".into();
    row.tags = Vec::new();
    row.price = 100.0;
    row.trigger_price = 0.0;
    row.average_price = 0.0;
    row.quantity = 1.0;
    row.filled_quantity = 0.0;
    row.pending_quantity = 1.0;
    row
}

pub(super) fn master_inst(
    exchange: &str,
    segment: &str,
    tradingsymbol: &str,
    token: u32,
) -> Instrument {
    let mut inst = chain_inst(tradingsymbol, tradingsymbol, segment);
    inst.exchange = exchange.into();
    inst.instrument_token = token;
    inst
}

pub(super) fn chain_inst(tradingsymbol: &str, name: &str, segment: &str) -> Instrument {
    Instrument {
        instrument_token: 1,
        tradingsymbol: tradingsymbol.into(),
        name: name.into(),
        exchange: "NFO".into(),
        expiry: None,
        strike: None,
        tick_size: 0.05,
        lot_size: 1,
        instrument_type: "EQ".into(),
        segment: segment.into(),
        freeze_quantity: None,
        max_order_quantity: None,
    }
}

pub(super) fn ltp_tick(token: u32, price: f64) -> Tick {
    let mut tick = Tick::default();
    tick.instrument_token = token;
    tick.last_price = Some(price);
    tick
}

pub(super) fn char_key(c: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
}

pub(super) fn render(app: &App, width: u16, height: u16) {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("test terminal");
    terminal
        .draw(|frame| ui::draw(frame, app))
        .expect("frame draws");
}

pub(super) fn click(app: &mut App, width: u16, height: u16, column: u16, row: u16) {
    render(app, width, height);
    press_at(app, column, row);
}

pub(super) fn screen(app: &App, width: u16, height: u16) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(width, height))
        .expect("test terminal");
    terminal
        .draw(|frame| ui::draw(frame, app))
        .expect("frame draws");
    let buffer = terminal.backend().buffer().clone();
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| buffer[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<String>>()
        .join("\n")
}

pub(in crate::tui::app) const W: u16 = 160;
pub(in crate::tui::app) const H: u16 = 40;

pub(in crate::tui::app) const HEADER_ROW: u16 = 2;

pub(in crate::tui::app) fn scroll(
    app: &mut App,
    width: u16,
    height: u16,
    column: u16,
    row: u16,
    down: bool,
) {
    render(app, width, height);
    app.on_mouse(MouseEvent {
        kind: if down {
            MouseEventKind::ScrollDown
        } else {
            MouseEventKind::ScrollUp
        },
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
}

pub(in crate::tui::app) fn text_at(
    app: &App,
    width: u16,
    height: u16,
    row: u16,
    needle: &str,
) -> u16 {
    let text = screen(app, width, height);
    let line = text.lines().nth(usize::from(row)).expect("row is in frame");
    let byte = line
        .find(needle)
        .unwrap_or_else(|| panic!("row {row} has no {needle:?}: {line:?}"));
    u16::try_from(line[..byte].chars().count()).expect("fixture column fits u16")
}

pub(in crate::tui::app) fn region_of(
    app: &App,
    want: impl Fn(ui::HitAction) -> bool,
) -> ui::HitRegion {
    let hit = app.hit.borrow();
    *hit.regions
        .iter()
        .find(|region| want(region.action))
        .expect("the surface recorded this region")
}

pub(in crate::tui::app) fn hint_of(
    app: &App,
    want: impl Fn(ui::HitAction) -> bool,
) -> ui::HitRegion {
    let hit = app.hit.borrow();
    *hit.hints
        .iter()
        .find(|region| want(region.action))
        .expect("the hint line recorded this key")
}

pub(in crate::tui::app) fn press_at(app: &mut App, column: u16, row: u16) {
    app.on_mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    });
}

pub(in crate::tui::app) fn row_offset(region: ui::HitRegion) -> usize {
    match region.action {
        ui::HitAction::Rows { offset, .. } => offset,
        other => panic!("not a row region: {other:?}"),
    }
}

pub(in crate::tui::app) fn ticket_row(app: &App, field: TicketField) -> ui::HitRegion {
    region_of(app, |action| {
        action
            == ui::HitAction::ModalControl {
                field: ui::ModalField::Ticket(field),
                intent: ControlIntent::Focus,
            }
    })
}

pub(in crate::tui::app) fn ticket_option(
    app: &App,
    field: TicketField,
    index: usize,
) -> ui::HitRegion {
    region_of(app, |action| {
        action
            == ui::HitAction::ModalControl {
                field: ui::ModalField::Ticket(field),
                intent: ControlIntent::Option(index),
            }
    })
}

pub(in crate::tui::app) fn open_ticket_state(app: &App) -> &TicketState {
    match app.modals.last() {
        Some(Modal::Ticket(state)) => state,
        other => panic!("expected a ticket, got {other:?}"),
    }
}

pub(in crate::tui::app) fn ticket_app() -> App {
    let mut app = test_app();
    app.open_ticket(Side::Buy, "NSE".into(), "INFY".into(), 1, 0, None);
    app
}

pub(in crate::tui::app) fn holdings_app() -> App {
    let mut app = test_app();
    app.set_pane(PaneId::PortfolioHoldings);
    app.holdings = Some(
        (0u32..5)
            .map(|i| {
                holding(
                    &format!("H{i}"),
                    100 + i,
                    100.0,
                    100.0 * f64::from(i + 1),
                    100.0,
                    1,
                )
            })
            .collect(),
    );
    app
}

pub(in crate::tui::app) fn edit_ticket(app: &mut App, mutate: impl FnOnce(&mut TicketState)) {
    let Some(Modal::Ticket(state)) = app.modals.last_mut() else {
        panic!("expected a ticket modal on top");
    };
    mutate(state);
}

pub(in crate::tui::app) fn open_and_review(
    app: &mut App,
    exchange: &str,
    symbol: &str,
    token: u32,
    mutate: impl FnOnce(&mut TicketState),
) {
    app.open_ticket(
        Side::Buy,
        exchange.to_string(),
        symbol.to_string(),
        token,
        0,
        None,
    );
    edit_ticket(app, mutate);
    app.on_key(char_key_code(KeyCode::Enter));
    render(app, W, H);
}

pub(in crate::tui::app) fn char_key_code(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

pub(in crate::tui::app) fn set_non_default_fields(state: &mut TicketState) {
    state.order_type = OrderType::Market;
    state.autoslice = true;
    state.product = PRODUCTS.iter().position(|p| *p == "MIS").unwrap();
    state.validity = 1;
}

pub(in crate::tui::app) fn issue_message(state: &AlertFormState) -> Option<&str> {
    state.issue.as_ref().map(|issue| issue.message.as_str())
}

pub(in crate::tui::app) fn alert_form(
    token: u32,
    attribute: Attribute,
    operator: Operator,
) -> Box<AlertFormState> {
    Box::new(AlertFormState {
        exchange: "NSE".into(),
        tradingsymbol: "INFY".into(),
        name: "INFY".into(),
        instrument_token: token,
        attribute,
        level: String::new(),
        level_edited: false,
        operator,
        focus: AlertFormFocus::Attribute,
        tick_size: 0.05,
        issue: None,
        uuid: None,
        status: AlertStatus::Enabled,
        ato: false,
        was_ato: false,
        draft: alerts::AtoBasketDraft::new(),
        basket_selected: 0,
        viewport: crate::tui::form::FormViewport::default(),
        stage: AlertFormStage::Edit,
    })
}

pub(in crate::tui::app) fn server_alert(
    symbol: &str,
    exchange: &str,
    operator: &str,
    level: f64,
) -> Alert {
    let mut row = crate::tui::data::test_models::alert();
    row.alert_type = kiteconnect::AlertType::Simple;
    row.uuid = format!("uuid-{symbol}-{operator}-{level}");
    row.name = symbol.into();
    row.status = AlertStatus::Enabled;
    row.lhs_attribute = "LastTradedPrice".into();
    row.lhs_exchange = exchange.into();
    row.lhs_tradingsymbol = symbol.into();
    row.operator = operator.into();
    row.rhs_type = "constant".into();
    row.rhs_constant = level;
    row
}

pub(in crate::tui::app) fn ato_alert() -> Alert {
    let mut row = server_alert("GOLDBEES", "NSE", ">=", 110.0);
    row.alert_type = kiteconnect::AlertType::Ato;
    row.basket = Some({
        let mut row = crate::tui::data::test_models::basket();
        row.items = vec![basket_item(275_218_517, "GOLDBEES", 10_000)];
        row
    });
    row
}

pub(in crate::tui::app) fn basket_item(
    id: i64,
    symbol: &str,
    weight: i64,
) -> crate::tui::data::FetchedBasketItem {
    let mut row = crate::tui::data::test_models::basket_item();
    row.id = Some(id);
    row.exchange = "NSE".into();
    row.tradingsymbol = symbol.into();
    row.instrument_token = Some(3_693_569);
    row.weight = weight;
    row.params = serde_json::json!({
        "variety": "regular",
        "transaction_type": "BUY",
        "product": "CNC",
        "order_type": "LIMIT",
        "validity": "DAY",
        "validity_ttl": 0,
        "quantity": 10,
        "disclosed_quantity": 0,
        "price": 72.25,
        "trigger_price": 0,
        "squareoff": 0,
        "stoploss": 0,
        "trailing_stoploss": 0,
        "gtt": {"target": 0, "stoploss": 0},
        "tags": []
    });
    row
}

impl App {
    pub fn assert_fixture_renders(
        client: kiteconnect::KiteConnect,
        event: crate::tui::data::DataEvent,
        preset: crate::tui::workspace::Preset,
        expected: &str,
    ) {
        use crate::tui::app::{AlertHistoryState, App};
        use crate::tui::data::DataEvent;
        use crate::tui::event::AppEvent;
        use crate::tui::settings::{self, Settings, Theme};
        let _guard = settings::theme_guard(Theme::DEFAULT);
        let (tx, _rx) = tokio::sync::mpsc::channel(64);
        let mut app = App::new(
            client,
            tx,
            crate::tui::marketwatch::State::remote(None, String::new(), None),
            crate::tui::notes::Pad::default(),
            false,
            &Settings::default(),
        );
        app.set_workspace(preset);
        if matches!(event, DataEvent::Auctions(_)) {
            app.bids_pane
                .set_section(crate::tui::bids::Section::Auctions);
        }
        if let DataEvent::AlertHistory { uuid, request, .. } = &event {
            let mut alert = crate::tui::data::test_models::alert();
            alert.uuid = uuid.clone();
            app.on_event(AppEvent::Data(DataEvent::Alerts {
                sequence: crate::tui::data::AlertListSequence(app.alerts_applied.0 + 1),
                result: Ok(vec![alert]),
            }));
            app.alert_history = Some(AlertHistoryState {
                uuid: uuid.clone(),
                request: *request,
                label: "NIFTY upper".to_string(),
                triggers: None,
                basket_len: None,
            });
        }
        app.on_event(AppEvent::Data(event));
        let mut terminal =
            ratatui::Terminal::new(ratatui::backend::TestBackend::new(300, 90)).unwrap();
        terminal
            .draw(|frame| crate::tui::ui::draw(frame, &app))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let screen = (0..90)
            .map(|y| {
                (0..300)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            screen.contains(expected),
            "screen lacks {expected:?}: {screen}"
        );
    }
}
