use super::{CANCEL, FIELD_HELP, TOO_SMALL, side_color};
use crate::tui::app::{
    App, ORDER_TYPES, PRODUCTS, Side, TTL_MINUTES, VALIDITIES, allowed_gtt_products,
    allowed_products,
};
use crate::tui::form::ModalField;
use crate::tui::format;
use crate::tui::ticket::{TicketField, TicketKind, TicketState, ticket_varieties};
use crate::tui::ui::forms;
use crate::tui::ui::{always, nav};
use ratatui::Frame;
use ratatui::style::{Modifier, Style};

pub(in crate::tui::ui) fn ticket_footer(state: &TicketState) -> &'static forms::FormFooter {
    if state.is_gtt() {
        return const {
            &forms::FormFooter {
                help: FIELD_HELP,
                actions: &[
                    forms::FormAction {
                        key: "Enter",
                        label: "Review GTT",
                        kind: forms::ActionKind::Primary,
                    },
                    CANCEL,
                ],
                hints: &[
                    nav(&["Tab:field", "↑↓:step", "←→/space:choose"]),
                    always(&["Enter:review GTT", "Esc:cancel"]),
                ],
            }
        };
    }
    if state.is_auction() {
        return const {
            &forms::FormFooter {
                help: FIELD_HELP,
                actions: &[
                    forms::FormAction {
                        key: "Enter",
                        label: "Review auction",
                        kind: forms::ActionKind::Primary,
                    },
                    CANCEL,
                ],
                hints: &[
                    nav(&["Tab:field", "↑↓:step", "←→/space:choose"]),
                    always(&["Enter:review auction", "Esc:cancel"]),
                ],
            }
        };
    }
    const {
        &forms::FormFooter {
            help: FIELD_HELP,
            actions: &[
                forms::FormAction {
                    key: "Enter",
                    label: "Review",
                    kind: forms::ActionKind::Primary,
                },
                CANCEL,
            ],
            hints: &[
                nav(&["Tab:field", "↑↓:step", "←→/space:choose"]),
                always(&["Enter:review", "Esc:cancel"]),
            ],
        }
    }
}

fn ticket_identity(ticket: &TicketState) -> (String, Style) {
    let warning = Style::default()
        .fg(format::warning())
        .add_modifier(Modifier::BOLD);
    match &ticket.kind {
        TicketKind::Gtt { modify } => {
            let stage = modify.map_or_else(|| "Edit".to_string(), |id| id.to_string());
            (
                format!("GTT {} {} / {stage}", ticket.side.label(), ticket.label()),
                warning,
            )
        }
        TicketKind::Auction { auction_number } => (
            format!("Auction SELL {} / {auction_number}", ticket.label()),
            warning,
        ),
        TicketKind::Regular => {
            let stage = ticket
                .modify
                .as_ref()
                .map_or_else(|| "Edit".to_string(), |target| target.order_id.clone());
            let verb = if ticket.modify.is_some() {
                "Modify "
            } else {
                ""
            };
            (
                format!("{verb}{} {} / {stage}", ticket.side.label(), ticket.label()),
                Style::default()
                    .fg(side_color(ticket.side))
                    .add_modifier(Modifier::BOLD),
            )
        }
    }
}

fn ticket_subtitle(state: &TicketState, ltp: Option<f64>) -> String {
    let effect = if state.is_gtt() {
        Some("Stored trigger")
    } else if state.is_auction() {
        Some("Auction sale")
    } else if state.modify.is_some() {
        Some("Modify order")
    } else {
        None
    };
    let facts = format!(
        "LTP {} | lot {} | tick {}",
        ltp.map_or_else(|| ".".to_string(), format::price),
        state.lot_size,
        state.tick_size
    );
    match effect {
        Some(effect) => format!("{effect} | {facts}"),
        None => facts,
    }
}

fn well(
    state: &TicketState,
    field: TicketField,
    label: &'static str,
    value: &str,
) -> forms::FormItem {
    forms::FormItem::Row(
        forms::FormRow::well(
            ModalField::Ticket(field),
            label,
            value,
            state.focused() == field && state.visible_fields().contains(&field),
        )
        .issue(forms::issue_for(state.issue.as_ref(), field)),
    )
}

fn segments(
    state: &TicketState,
    field: TicketField,
    label: &'static str,
    options: Vec<forms::Segment>,
) -> forms::FormItem {
    forms::FormItem::Row(
        forms::FormRow::segments(
            ModalField::Ticket(field),
            label,
            options,
            state.focused() == field && state.visible_fields().contains(&field),
        )
        .issue(forms::issue_for(state.issue.as_ref(), field)),
    )
}

fn fixed(label: &'static str, value: String, reason: &str) -> forms::FormItem {
    forms::FormItem::Row(forms::FormRow::fixed(label, value, reason))
}

fn toggle(
    state: &TicketState,
    field: TicketField,
    label: &'static str,
    off: &str,
    on: &str,
    value: bool,
) -> forms::FormItem {
    segments(
        state,
        field,
        label,
        vec![
            forms::Segment::new(off, !value),
            forms::Segment::new(on, value),
        ],
    )
}

fn gtt_ticket_items(state: &TicketState, wide: bool) -> Vec<forms::FormItem> {
    let mut items = Vec::new();
    items.push(well(state, TicketField::Quantity, "Quantity", &state.qty));
    items.push(segments(
        state,
        TicketField::Side,
        "Side",
        vec![
            forms::Segment::new("BUY", state.side == Side::Buy),
            forms::Segment::new("SELL", state.side == Side::Sell),
        ],
    ));
    items.push(segments(
        state,
        TicketField::GttType,
        "Type",
        vec![
            forms::Segment::new("single", !state.gtt_oco),
            forms::Segment::new("OCO", state.gtt_oco),
        ],
    ));
    items.push(fixed(
        "Product",
        allowed_gtt_products(&state.exchange)[0].to_string(),
        "fixed",
    ));
    items.push(forms::FormItem::Section(
        if state.gtt_oco {
            "Stop loss"
        } else {
            "Trigger"
        }
        .to_string(),
    ));
    let (price_label, trigger_label) = if state.gtt_oco {
        ("SL price", "SL trigger")
    } else {
        ("Price", "Trigger")
    };
    items.push(well(state, TicketField::Price, price_label, &state.price));
    items.push(well(
        state,
        TicketField::Trigger,
        trigger_label,
        &state.trigger,
    ));
    if state.gtt_oco {
        items.push(forms::FormItem::Section("Target".to_string()));
        items.push(well(
            state,
            TicketField::TargetPrice,
            "Tgt price",
            &state.price2,
        ));
        items.push(well(
            state,
            TicketField::TargetTrigger,
            "Tgt trigger",
            &state.trigger2,
        ));
    }
    if state.side == Side::Sell {
        items.push(toggle(
            state,
            TicketField::Trailing,
            "Trailing",
            "Off",
            "On",
            state.gtt_trailing,
        ));
        if state.gtt_trailing {
            items.push(well(
                state,
                TicketField::TrailPoints,
                "Trail pts",
                &state.trailing_points,
            ));
        }
    } else {
        items.push(fixed("Trailing", "Off".to_string(), "SELL only"));
    }
    items.push(forms::FormItem::Note(
        "Creates stored LIMIT orders when triggers fire.".to_string(),
    ));
    if wide {
        items.push(forms::FormItem::Note(
            if state.gtt_oco {
                "OCO uses the lower trigger for stop loss and upper for target."
            } else {
                "The stored LIMIT order is placed when the trigger fires."
            }
            .to_string(),
        ));
    }
    items
}

fn auction_ticket_items(
    state: &TicketState,
    auction_number: &str,
    wide: bool,
) -> Vec<forms::FormItem> {
    let mut items = vec![
        well(state, TicketField::Quantity, "Quantity", &state.qty),
        well(state, TicketField::Price, "Price", &state.price),
        fixed("Side", "SELL".to_string(), "auction"),
        fixed("Order type", "LIMIT".to_string(), "auction"),
        fixed("Product", "CNC".to_string(), "auction"),
        forms::FormItem::Section("Execution".to_string()),
        fixed("Validity", "DAY".to_string(), "auction"),
        fixed("Auction", auction_number.to_string(), "fixed"),
        forms::FormItem::Note("No sale is sent until you confirm the review.".to_string()),
    ];
    if wide {
        items.push(forms::FormItem::Note(
            "Quantity steps by lot; Price steps by tick.".to_string(),
        ));
    }
    items
}

fn regular_ticket_items(state: &TicketState, wide: bool) -> Vec<forms::FormItem> {
    let visible = state.visible_fields();
    let mut items = Vec::new();
    let order_type = state.order_type;
    let modify = state.modify.is_some();
    let co = state.variety_is_co();
    if state.quantity_editable() {
        items.push(well(state, TicketField::Quantity, "Quantity", &state.qty));
    } else {
        items.push(fixed("Quantity", state.qty.clone(), "fixed on modify"));
    }
    if visible.contains(&TicketField::OrderType) {
        let entry_types = if co && !modify { 2 } else { ORDER_TYPES.len() };
        items.push(segments(
            state,
            TicketField::OrderType,
            "Order type",
            ORDER_TYPES
                .iter()
                .enumerate()
                .map(|(index, label)| {
                    if index < entry_types {
                        forms::Segment::new(label.to_string(), *label == state.order_type)
                    } else {
                        forms::Segment::disabled(label.to_string())
                    }
                })
                .collect(),
        ));
    } else {
        items.push(fixed(
            "Order type",
            order_type.to_string(),
            "fixed on modify",
        ));
    }
    if visible.contains(&TicketField::Price) {
        items.push(well(state, TicketField::Price, "Price", &state.price));
    }
    if visible.contains(&TicketField::Trigger) {
        items.push(well(state, TicketField::Trigger, "Trigger", &state.trigger));
    }
    if co {
        items.push(fixed("Product", "MIS".to_string(), "cover order"));
    } else if modify {
        items.push(fixed(
            "Product",
            PRODUCTS[state.product].to_string(),
            "fixed on modify",
        ));
    } else {
        let allowed = allowed_products(&state.exchange);
        let selected = allowed
            .iter()
            .position(|label| *label == PRODUCTS[state.product])
            .unwrap_or(0);
        items.push(segments(
            state,
            TicketField::Product,
            "Product",
            allowed
                .iter()
                .enumerate()
                .map(|(index, label)| forms::Segment::new(*label, index == selected))
                .collect(),
        ));
    }
    items.extend(regular_execution_items(state, wide));
    items
}

fn regular_execution_items(state: &TicketState, wide: bool) -> Vec<forms::FormItem> {
    let visible = state.visible_fields();
    let modify = state.modify.is_some();
    let co = state.variety_is_co();
    let mut items = Vec::new();
    items.push(forms::FormItem::Section("Execution".to_string()));
    let validity_label = if state.validity == 2 {
        format!("TTL {}m", TTL_MINUTES[state.validity_ttl])
    } else {
        VALIDITIES[state.validity].to_string()
    };
    if co {
        items.push(fixed("Validity", "DAY".to_string(), "cover order"));
    } else if modify {
        items.push(fixed("Validity", validity_label, "fixed on modify"));
    } else {
        items.push(segments(
            state,
            TicketField::Validity,
            "Validity",
            VALIDITIES
                .iter()
                .enumerate()
                .map(|(index, label)| forms::Segment::new(*label, index == state.validity))
                .collect(),
        ));
        if visible.contains(&TicketField::TtlMinutes) {
            items.push(forms::FormItem::Row(
                forms::FormRow::carousel(
                    ModalField::Ticket(TicketField::TtlMinutes),
                    "TTL mins",
                    &TTL_MINUTES[state.validity_ttl].to_string(),
                    state.focused() == TicketField::TtlMinutes,
                )
                .issue(forms::issue_for(
                    state.issue.as_ref(),
                    TicketField::TtlMinutes,
                )),
            ));
        }
        if state.variety_is_iceberg() {
            items.push(fixed("Autoslice", "Off".to_string(), "iceberg legs"));
        } else if state.autoslice_forced() {
            items.push(fixed("Autoslice", "On".to_string(), "over freeze limit"));
        } else {
            items.push(toggle(
                state,
                TicketField::Autoslice,
                "Autoslice",
                "Off",
                "On",
                state.autoslice,
            ));
        }
    }
    if !modify {
        if co {
            items.push(fixed("Autoslice", "Off".to_string(), "cover order"));
        }
        if visible.contains(&TicketField::Protection) {
            items.push(toggle(
                state,
                TicketField::Protection,
                "Protection",
                "Off",
                "auto",
                state.market_protection,
            ));
        }
        items.push(segments(
            state,
            TicketField::Variety,
            "Variety",
            ticket_varieties(&state.exchange)
                .iter()
                .enumerate()
                .map(|(index, label)| forms::Segment::new(*label, index == state.variety))
                .collect(),
        ));
        if visible.contains(&TicketField::Legs) {
            items.push(well(state, TicketField::Legs, "Legs", &state.iceberg_legs));
        }
    }
    items.push(forms::FormItem::Note(
        "No order is sent until you confirm the review.".to_string(),
    ));
    if wide {
        items.push(forms::FormItem::Note(
            "Quantity steps by lot; Price steps by tick.".to_string(),
        ));
    }
    items
}

pub(super) fn draw_ticket(frame: &mut Frame, state: &TicketState, app: &App) {
    let area = frame.area();
    let ltp = app.live_ltp(state.instrument_token);
    let (title, border) = ticket_identity(state);
    let wide = area.width >= forms::WIDE_TERMINAL;
    let mut items = Vec::new();
    if !forms::title_fits(area, &title) {
        items.push(forms::FormItem::Note(title.clone()));
    }
    items.push(forms::FormItem::Section("Order".to_string()));
    items.extend(match &state.kind {
        TicketKind::Gtt { .. } => gtt_ticket_items(state, wide),
        TicketKind::Auction { auction_number } => auction_ticket_items(state, auction_number, wide),
        TicketKind::Regular => regular_ticket_items(state, wide),
    });
    forms::general_issue(&mut items, state.issue.as_ref());

    let plan = forms::BodyPlan::new(&items, forms::content_width(area), forms::stacked(area));
    let layout = forms::ModalLayout::measure(area, plan.rows());
    let Some(layout) = forms::draw_frame(
        frame,
        app,
        layout,
        &title,
        border,
        &ticket_subtitle(state, ltp),
        TOO_SMALL,
    ) else {
        return;
    };
    forms::draw_body(frame, app, &layout, &plan, &state.viewport);
    forms::draw_footer(frame, app, &layout, ticket_footer(state));
}

#[cfg(test)]
mod render_tests {
    use super::super::render_tests::{
        assert_mockup, mockup_app, modal_ascii, modal_rect, rect_tuple, rendered_text, test_app,
    };
    use super::{draw_ticket, ticket_footer};
    use crate::tui::app::Side;
    use crate::tui::form::FormViewport;
    use crate::tui::ticket::{TicketField, TicketKind, TicketState};

    fn ticket(exchange: &str, validity: usize) -> TicketState {
        TicketState {
            side: Side::Buy,
            exchange: exchange.to_string(),
            tradingsymbol: "TEST26AUGFUT".to_string(),
            instrument_token: 1,
            modify: None,
            kind: TicketKind::Regular,
            qty: "1".to_string(),
            price: "100.00".to_string(),
            trigger: String::new(),
            gtt_oco: false,
            gtt_trailing: false,
            trailing_points: String::new(),
            trigger2: String::new(),
            price2: String::new(),
            order_type: kiteconnect::OrderType::Limit,
            product: 2,
            validity,
            validity_ttl: 0,
            focus: TicketField::Quantity,
            issue: None,
            viewport: FormViewport::default(),
            tick_size: 0.05,
            lot_size: 1,
            freeze_quantity: Some(100),
            max_order_quantity: Some(100),
            autoslice: false,
            market_protection: false,
            variety: 0,
            iceberg_legs: "2".to_string(),
        }
    }

    fn mockup_ticket() -> TicketState {
        TicketState {
            side: Side::Buy,
            exchange: "NSE".to_string(),
            tradingsymbol: "INFY".to_string(),
            instrument_token: 408_065,
            modify: None,
            kind: TicketKind::Regular,
            qty: "10".to_string(),
            price: "1500.00".to_string(),
            trigger: String::new(),
            gtt_oco: false,
            gtt_trailing: false,
            trailing_points: String::new(),
            trigger2: String::new(),
            price2: String::new(),
            order_type: kiteconnect::OrderType::Limit, // LIMIT
            product: 0,                                // CNC
            validity: 0,                               // DAY
            validity_ttl: 0,
            focus: TicketField::Quantity,
            issue: None,
            viewport: FormViewport::default(),
            tick_size: 0.05,
            lot_size: 1,
            freeze_quantity: None,
            max_order_quantity: None,
            autoslice: false,
            market_protection: false,
            variety: 0,
            iceberg_legs: "2".to_string(),
        }
    }

    fn mockup_gtt() -> TicketState {
        TicketState {
            kind: TicketKind::Gtt { modify: None },
            side: Side::Sell,
            gtt_oco: true,
            gtt_trailing: true,
            trailing_points: "5.00".to_string(),
            price: "1449.95".to_string(),
            trigger: "1450.00".to_string(),
            price2: "1600.00".to_string(),
            trigger2: "1599.95".to_string(),
            focus: TicketField::Trigger,
            ..mockup_ticket()
        }
    }

    #[test]
    fn a_field_failure_renders_under_its_own_control() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.issue = Some(crate::tui::form::FormIssue::at(
            TicketField::Quantity,
            "quantity must be a multiple of lot size 400",
        ));
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        let rows: Vec<&str> = text.lines().collect();
        let quantity = rows
            .iter()
            .position(|row| row.starts_with("| >     Quantity  [10            ]"))
            .unwrap_or_else(|| panic!("the quantity row is drawn:\n{text}"));
        assert!(
            rows[quantity + 1]
                .starts_with("|                 ! quantity must be a multiple of lot size 400"),
            "the message belongs on the row under its own control:\n{text}"
        );
    }

    #[test]
    fn a_request_failure_with_no_field_renders_above_the_footer() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.issue = Some(crate::tui::form::FormIssue::general(
            "Kite rejected the order: insufficient margin",
        ));
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(
            text.contains("| Kite rejected the order: insufficient margin"),
            "a general failure takes the full content width:\n{text}"
        );
        assert!(!text.contains('!'), "and carries no field marker:\n{text}");
    }

    #[test]
    fn a_quantity_over_the_freeze_limit_states_autoslice_instead_of_offering_it() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.freeze_quantity = Some(5);
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(
            text.contains("|      Autoslice  On (over freeze limit)"),
            "{text}"
        );
        assert!(
            !state.visible_fields().contains(&TicketField::Autoslice),
            "a forced value is not a focus target"
        );
        state.qty = "1".to_string();
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(text.contains("|      Autoslice  [*Off | On ]"), "{text}");
    }

    #[test]
    fn a_cover_order_states_its_forced_values_as_fixed_rows() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.variety = 2; // co
        state.product = 1; // MIS
        state.trigger = "1490.00".to_string();
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        for row in [
            "|        Product  MIS (cover order)",
            "|       Validity  DAY (cover order)",
            "|      Autoslice  Off (cover order)",
        ] {
            assert!(text.contains(row), "{row} missing:\n{text}");
        }
        assert!(text.contains("[ MARKET |*LIMIT |-SL |-SL-M ]"), "{text}");
    }

    #[test]
    fn an_iceberg_states_autoslice_off_whatever_the_stored_preference_is() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.autoslice = true;
        state.variety = 1; // iceberg
        state.iceberg_legs = "2".to_string();
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(
            text.contains("|      Autoslice  Off (iceberg legs)"),
            "{text}"
        );
        assert!(
            !state.visible_fields().contains(&TicketField::Autoslice),
            "a fixed row is not a focus target"
        );
        assert!(
            !state.effective_autoslice(),
            "and the payload agrees with the row"
        );
        state.variety = 0;
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(text.contains("|      Autoslice  [ Off |*On ]"), "{text}");
    }

    #[test]
    fn a_modify_keeps_its_immutable_rows_visible() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.modify = Some(crate::tui::ticket::ModifyTarget {
            parent_order_id: None,
            order_id: "241010000123456".to_string(),
            variety: "regular".to_string(),
            quantity_editable: true,
        });
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(
            text.contains("+- Modify BUY NSE:INFY / 241010000123456"),
            "{text}"
        );
        assert!(
            text.contains("|        Product  CNC (fixed on modify)"),
            "{text}"
        );
        assert!(
            text.contains("|       Validity  DAY (fixed on modify)"),
            "{text}"
        );
        for absent in ["Autoslice", "Variety", "Protection"] {
            assert!(!text.contains(absent), "{absent} must be absent:\n{text}");
        }
    }

    #[test]
    fn an_auction_shows_editable_quantity_and_price_over_fixed_terms() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.kind = TicketKind::Auction {
            auction_number: "17".to_string(),
        };
        state.side = Side::Sell;
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(text.contains("+- Auction SELL NSE:INFY / 17"), "{text}");
        for row in [
            "|           Side  SELL (auction)",
            "|     Order type  LIMIT (auction)",
            "|        Product  CNC (auction)",
            "|       Validity  DAY (auction)",
            "|        Auction  17 (fixed)",
        ] {
            assert!(text.contains(row), "{row} missing:\n{text}");
        }
        assert!(text.contains("[Enter Review auction]"), "{text}");
    }

    #[test]
    fn eleven_ttl_presets_render_as_a_carousel_not_a_chip_row() {
        let app = mockup_app();
        let mut state = mockup_ticket();
        state.validity = 2; // TTL
        state.validity_ttl = 3; // five minutes
        state.focus = TicketField::TtlMinutes;
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(text.contains("| >     TTL mins  [< 5 >]"), "{text}");
        state.validity = 0;
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(!text.contains("TTL mins"), "{text}");
    }

    #[test]
    fn a_body_taller_than_its_frame_scrolls_the_focused_row_into_view() {
        let app = mockup_app();
        let mut state = mockup_gtt();
        state.focus = TicketField::TrailPoints;
        state.viewport.request_reveal();
        let text = modal_ascii(80, 16, |frame| draw_ticket(frame, &state, &app));
        assert!(
            text.contains("| >    Trail pts  [5.00          ]"),
            "{text}"
        );
        assert!(text.contains("+- GTT SELL NSE:INFY / Edit"), "{text}");
        assert!(
            text.contains("| [Enter Review GTT]  [Esc Cancel]"),
            "{text}"
        );
        assert!(!text.contains("Quantity"), "{text}");
    }

    #[test]
    fn a_title_the_border_would_clip_is_repeated_in_the_body() {
        let app = mockup_app();
        let mut state = mockup_gtt();
        state.tradingsymbol = "NIFTY26JAN24000CE".to_string();
        state.exchange = "NFO".to_string();
        let text = modal_ascii(36, 24, |frame| draw_ticket(frame, &state, &app));
        let title = "GTT SELL NFO:NIFTY26JAN24000CE / Edit";
        assert!(
            !text.lines().next().unwrap().contains(title),
            "the border cannot hold it:\n{text}"
        );
        let body: String = text.replace(['|', '\n'], " ");
        for word in title.split(' ') {
            assert!(body.contains(word), "{word} missing:\n{text}");
        }
        let text = modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app));
        assert_eq!(
            text.matches("GTT SELL NFO:NIFTY26JAN24000CE").count(),
            1,
            "{text}"
        );
    }

    #[test]
    fn a_terminal_under_forty_eight_columns_stacks_each_label_on_its_own_row() {
        let app = mockup_app();
        let state = mockup_ticket();
        let text = modal_ascii(44, 24, |frame| draw_ticket(frame, &state, &app));
        assert!(text.contains("| > Quantity"), "{text}");
        assert!(text.contains("|   [10            ]"), "{text}");
        assert!(!text.contains("Quantity  ["), "{text}");
        assert!(text.contains("| [Enter Review]  [Esc Cancel]"), "{text}");
    }

    #[test]
    fn the_footer_and_the_root_hints_advertise_the_same_actions() {
        let state = mockup_ticket();
        let footer = ticket_footer(&state);
        let hints: String = footer
            .hints
            .iter()
            .flat_map(|group| group.hints.iter())
            .copied()
            .collect::<Vec<&str>>()
            .join(" ");
        for action in footer.actions {
            assert!(
                hints.contains(&format!("{}:", action.key)),
                "{} is on a button but not in the hints: {hints}",
                action.key
            );
        }
        let mut gtt = mockup_gtt();
        gtt.gtt_oco = false;
        assert!(
            ticket_footer(&gtt).actions[0].label.contains("GTT"),
            "a GTT names what it reviews"
        );
    }

    #[test]
    fn order_ticket_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_ticket();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app)),
            "
+- BUY NSE:INFY / Edit ------------------------------------------------+
| LTP 1,500.00 | lot 1 | tick 0.05                                     |
+----------------------------------------------------------------------+
| Order                                                                |
| >     Quantity  [10_           ]                                     |
|     Order type  [ MARKET |*LIMIT | SL | SL-M ]                       |
|          Price  [1500.00       ]                                     |
|        Product  [*CNC | MIS | MTF ]                                  |
| Execution                                                            |
|       Validity  [*DAY | IOC | TTL ]                                  |
|      Autoslice  [*Off | On ]                                         |
|        Variety  [*regular | iceberg | co ]                           |
| No order is sent until you confirm the review.                       |
+----------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                 |
| [Enter Review]  [Esc Cancel]                                         |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 3, 72, 17));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_ticket(frame, &state, &app)),
            "
+- BUY NSE:INFY / Edit ----------------------------------------------------------------+
| LTP 1,500.00 | lot 1 | tick 0.05                                                     |
+--------------------------------------------------------------------------------------+
| Order                                                                                |
| >     Quantity  [10_           ]                                                     |
|     Order type  [ MARKET |*LIMIT | SL | SL-M ]                                       |
|          Price  [1500.00       ]                                                     |
|        Product  [*CNC | MIS | MTF ]                                                  |
| Execution                                                                            |
|       Validity  [*DAY | IOC | TTL ]                                                  |
|      Autoslice  [*Off | On ]                                                         |
|        Variety  [*regular | iceberg | co ]                                           |
| No order is sent until you confirm the review.                                       |
| Quantity steps by lot; Price steps by tick.                                          |
+--------------------------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                                 |
| [Enter Review]  [Esc Cancel]                                                         |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 8, 88, 18));
    }

    #[test]
    fn gtt_ticket_matches_the_spec_mockup() {
        let app = mockup_app();
        let state = mockup_gtt();
        assert_mockup(
            &modal_ascii(80, 24, |frame| draw_ticket(frame, &state, &app)),
            "
+- GTT SELL NSE:INFY / Edit -------------------------------------------+
| Stored trigger | LTP 1,500.00 | lot 1 | tick 0.05                    |
+----------------------------------------------------------------------+
| Order                                                                |
|       Quantity  [10            ]                                     |
|           Side  [ BUY |*SELL ]                                       |
|           Type  [ single |*OCO ]                                     |
|        Product  CNC (fixed)                                          |
| Stop loss                                                            |
|       SL price  [1449.95       ]                                     |
| >   SL trigger  [1450.00_      ]                                     |
| Target                                                               |
|      Tgt price  [1600.00       ]                                     |
|    Tgt trigger  [1599.95       ]                                     |
|       Trailing  [ Off |*On ]                                         |
|      Trail pts  [5.00          ]                                     |
| Creates stored LIMIT orders when triggers fire.                      |
+----------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                 |
| [Enter Review GTT]  [Esc Cancel]                                     |
+----------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (4, 1, 72, 21));

        assert_mockup(
            &modal_ascii(120, 35, |frame| draw_ticket(frame, &state, &app)),
            "
+- GTT SELL NSE:INFY / Edit -----------------------------------------------------------+
| Stored trigger | LTP 1,500.00 | lot 1 | tick 0.05                                    |
+--------------------------------------------------------------------------------------+
| Order                                                                                |
|       Quantity  [10            ]                                                     |
|           Side  [ BUY |*SELL ]                                                       |
|           Type  [ single |*OCO ]                                                     |
|        Product  CNC (fixed)                                                          |
| Stop loss                                                                            |
|       SL price  [1449.95       ]                                                     |
| >   SL trigger  [1450.00_      ]                                                     |
| Target                                                                               |
|      Tgt price  [1600.00       ]                                                     |
|    Tgt trigger  [1599.95       ]                                                     |
|       Trailing  [ Off |*On ]                                                         |
|      Trail pts  [5.00          ]                                                     |
| Creates stored LIMIT orders when triggers fire.                                      |
| OCO uses the lower trigger for stop loss and upper for target.                       |
+--------------------------------------------------------------------------------------+
| Tab/Shift-Tab Field  Up/Down Step  Left/Right Choice                                 |
| [Enter Review GTT]  [Esc Cancel]                                                     |
+--------------------------------------------------------------------------------------+
",
        );
        assert_eq!(rect_tuple(modal_rect(&app)), (16, 6, 88, 22));
    }

    #[test]
    fn a_form_survives_a_terminal_too_small_to_hold_it() {
        let app = mockup_app();
        let state = mockup_gtt();
        let text = modal_ascii(48, 18, |frame| draw_ticket(frame, &state, &app));
        assert_eq!(rect_tuple(modal_rect(&app)), (1, 1, 46, 16));
        assert!(text.contains("[Enter Review GTT]"), "{text}");

        let text = modal_ascii(32, 10, |frame| draw_ticket(frame, &state, &app));
        assert_eq!(rect_tuple(modal_rect(&app)), (1, 1, 30, 8));
        assert!(text.contains("Esc Cancel"), "{text}");

        for (w, h) in [(1, 1), (0, 0), (12, 4)] {
            let text = rendered_text(w.max(1), h.max(1), |frame| draw_ticket(frame, &state, &app));
            assert!(!text.contains("Enter Review"), "{w}x{h}:\n{text}");
        }
    }

    #[test]
    fn commodity_ticket_renders_ttl_choices_without_iceberg() {
        let app = test_app();
        let state = ticket("MCX", 2);
        let text = rendered_text(100, 30, |frame| draw_ticket(frame, &state, &app));
        assert!(
            text.contains("TTL mins"),
            "TTL minutes field missing:\n{text}"
        );
        assert!(
            text.contains("[ DAY | IOC |*TTL ]"),
            "TTL choice missing:\n{text}"
        );
        assert!(text.contains("regular"), "regular variety missing:\n{text}");
        assert!(
            !text.contains("iceberg"),
            "commodity ticket must not offer iceberg:\n{text}"
        );
    }

    #[test]
    fn nse_ticket_renders_automatic_amo_cover_and_market_protection() {
        let app = test_app();
        let mut state = ticket("NSE", 0);
        state.order_type = kiteconnect::OrderType::Market;
        let text = rendered_text(100, 32, |frame| draw_ticket(frame, &state, &app));
        for label in ["regular", "iceberg", "co", "Protection", "auto"] {
            assert!(text.contains(label), "{label} missing from ticket:\n{text}");
        }
        assert!(
            !text.contains(" amo "),
            "manual AMO must be absent:\n{text}"
        );
        assert!(
            text.contains("No order is sent until you confirm the review."),
            "{text}"
        );
        assert!(text.contains("[Enter Review]"), "{text}");
    }

    #[test]
    fn iceberg_ticket_keeps_all_valid_products_available() {
        let app = test_app();
        let mut state = ticket("NSE", 0);
        state.product = 3;
        state.variety = 1;
        let text = rendered_text(100, 32, |frame| draw_ticket(frame, &state, &app));
        assert!(text.contains("CNC"), "{text}");
        assert!(text.contains("MIS"), "{text}");
        assert!(text.contains("MTF"), "{text}");
    }
}
