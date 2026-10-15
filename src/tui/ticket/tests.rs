use super::{
    ConversionField, ModifyTarget, PendingAction, PositionConversionState, TicketField, TicketKind,
    TicketState, build_confirm, build_position_conversion_confirm, conversion_target, edit_numeric,
    edit_signed_numeric, gtt_default_trigger_percent, gtt_seed_trigger,
    position_conversion_choices, step_legs, step_price, step_qty, step_signed_number,
};
use crate::tui::app::Side;
use crate::tui::data::Position;
use crate::tui::form::{ControlIntent, FormViewport};
use crossterm::event::KeyCode;
use kiteconnect::OrderType;
use proptest::prelude::*;

fn ticket() -> TicketState {
    TicketState {
        side: Side::Buy,
        exchange: "NSE".to_string(),
        tradingsymbol: "INFY".to_string(),
        instrument_token: 408_065,
        modify: None,
        kind: TicketKind::Regular,
        qty: "10".to_string(),
        price: "1523.40".to_string(),
        trigger: String::new(),
        gtt_oco: false,
        gtt_trailing: false,
        trailing_points: String::new(),
        trigger2: String::new(),
        price2: String::new(),
        order_type: OrderType::Limit,
        product: 0,  // CNC
        validity: 0, // DAY
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

fn conversion_position() -> Position {
    {
        let mut row = crate::tui::data::test_models::position();
        row.tradingsymbol = "INFY".to_string();
        row.exchange = "NSE".to_string();
        row.instrument_token = 408_065;
        row.product = "CNC".to_string();
        row.overnight_quantity = -30;
        row.day_buy_quantity = 10;
        row.day_sell_quantity = 40;
        row.quantity = -60;
        row.average_price = 1500.0;
        row.last_price = 1523.4;
        row.close_price = 1510.0;
        row.pnl = 0.0;
        row.realised = 0.0;
        row.multiplier = 1.0;
        row
    }
}

#[test]
fn conversion_uses_the_selected_bucket_sign_and_web_payload() {
    let position = conversion_position();
    let choices = position_conversion_choices(&position);
    assert_eq!(choices.len(), 2);
    assert_eq!(choices[0].signed_quantity, -30);
    assert_eq!(choices[1].signed_quantity, -30);

    let mut state = PositionConversionState::new(&position, 1).expect("convertible");
    state.selected = 1;
    state.qty = "20".to_string();
    let confirm = build_position_conversion_confirm(&state).expect("valid conversion");
    let PendingAction::ConvertPosition { form } = confirm.action else {
        panic!("expected conversion action");
    };
    assert_eq!(
        form,
        vec![
            ("exchange".to_string(), "NSE".to_string()),
            ("tradingsymbol".to_string(), "INFY".to_string()),
            ("transaction_type".to_string(), "SELL".to_string()),
            ("position_type".to_string(), "overnight".to_string()),
            ("quantity".to_string(), "20".to_string()),
            ("old_product".to_string(), "CNC".to_string()),
            ("new_product".to_string(), "MIS".to_string()),
        ]
    );
}

#[test]
fn conversion_target_matches_cash_and_derivative_products() {
    assert_eq!(conversion_target("CNC", "NSE"), "MIS");
    assert_eq!(conversion_target("MIS", "BSE"), "CNC");
    assert_eq!(conversion_target("MIS", "NFO"), "NRML");
}

#[test]
fn auction_confirm_is_an_invariant_sell_cnc_limit_day_order() {
    let mut state = ticket();
    state.side = Side::Buy;
    state.kind = TicketKind::Auction {
        auction_number: "AUC-42".to_string(),
    };
    state.qty = "7".to_string();
    state.price = "1523.40".to_string();
    let confirm = build_confirm(&state, None, 0).expect("auction confirmation");
    let PendingAction::Place { variety, form } = confirm.action else {
        panic!("expected auction placement");
    };
    assert_eq!(variety, "auction");
    assert_eq!(
        form,
        vec![
            ("exchange".to_string(), "NSE".to_string()),
            ("tradingsymbol".to_string(), "INFY".to_string()),
            ("transaction_type".to_string(), "SELL".to_string()),
            ("product".to_string(), "CNC".to_string()),
            ("order_type".to_string(), "LIMIT".to_string()),
            ("quantity".to_string(), "7".to_string()),
            ("price".to_string(), "1523.4".to_string()),
            ("validity".to_string(), "DAY".to_string()),
            ("auction_number".to_string(), "AUC-42".to_string()),
        ]
    );
}

#[test]
fn auction_confirm_rejects_quantity_above_instrument_maximum() {
    let mut state = ticket();
    state.kind = TicketKind::Auction {
        auction_number: "AUC-42".to_string(),
    };
    state.qty = "7".to_string();
    state.price = "1523.40".to_string();
    state.max_order_quantity = Some(6);

    assert_eq!(
        build_confirm(&state, None, 0).unwrap_err().message,
        "quantity 7 exceeds max order quantity 6"
    );
}

fn place_form(action: PendingAction) -> Vec<(String, String)> {
    match action {
        PendingAction::Place { form, .. } => form,
        other => panic!("expected place action, got {other:?}"),
    }
}

#[test]
fn builds_limit_order_form() {
    let confirm = build_confirm(&ticket(), None, 0).unwrap();
    let super::PendingAction::Place { form, .. } = confirm.action else {
        panic!("expected place action")
    };
    let get = |key: &str| form.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    assert_eq!(get("exchange"), Some("NSE"));
    assert_eq!(get("transaction_type"), Some("BUY"));
    assert_eq!(get("order_type"), Some("LIMIT"));
    assert_eq!(get("quantity"), Some("10"));
    assert_eq!(get("price"), Some("1523.4"));
    assert_eq!(get("trigger_price"), None);
}

#[test]
fn builds_automatic_amo_regular_and_other_varieties() {
    let confirm = build_confirm(&ticket(), None, 0).unwrap();
    assert!(matches!(
        confirm.action,
        PendingAction::Place { ref variety, .. } if variety == "regular"
    ));
    assert!(!super::ticket_varieties("NSE").contains(&"amo"));

    let mut protected = ticket();
    protected.order_type = OrderType::Market;
    protected.market_protection = true;
    let form = place_form(build_confirm(&protected, None, 0).unwrap().action);
    assert!(
        form.iter()
            .any(|(key, value)| { key == "market_protection" && value == "-1" })
    );

    let mut cover = ticket();
    cover.variety = 2;
    cover.trigger = "1500.00".to_string();
    cover.validity = 2;
    let confirm = build_confirm(&cover, None, 0).unwrap();
    let form = match confirm.action {
        PendingAction::Place { variety, form } => {
            assert_eq!(variety, "co");
            form
        }
        other => panic!("expected place action, got {other:?}"),
    };
    let get = |key: &str| form.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    assert_eq!(get("product"), Some("MIS"));
    assert_eq!(get("validity"), Some("DAY"));
    assert_eq!(get("trigger_price"), Some("1500"));
}

#[test]
fn existing_amo_order_keeps_amo_modify_endpoint() {
    let mut state = ticket();
    state.modify = Some(ModifyTarget {
        parent_order_id: None,
        order_id: "AMO-1".to_string(),
        variety: "amo".to_string(),
        quantity_editable: true,
    });
    let confirm = build_confirm(&state, None, 0).expect("AMO modify remains valid");
    let PendingAction::Modify {
        variety, order_id, ..
    } = confirm.action
    else {
        panic!("expected AMO modify action");
    };
    assert_eq!(variety, "amo");
    assert_eq!(order_id, "AMO-1");
}

#[test]
fn iceberg_modify_locks_and_omits_quantity() {
    let mut state = ticket();
    state.modify = Some(ModifyTarget {
        parent_order_id: None,
        order_id: "OID1".to_string(),
        variety: "iceberg".to_string(),
        quantity_editable: false,
    });
    assert!(!state.quantity_editable());
    assert!(!state.visible_fields().contains(&TicketField::Quantity));
    let confirm = build_confirm(&state, None, 0).unwrap();
    let PendingAction::Modify { variety, form, .. } = confirm.action else {
        panic!("expected modify action");
    };
    assert_eq!(variety, "iceberg");
    assert!(!form.iter().any(|(key, _)| key == "quantity"));
}

#[test]
fn modify_form_omits_validity_and_field_is_locked() {
    let mut state = ticket();
    state.modify = Some(ModifyTarget {
        parent_order_id: None,
        order_id: "241010000123456".to_string(),
        variety: "regular".to_string(),
        quantity_editable: true,
    });
    state.validity = 1; // IOC
    assert_eq!(
        state.visible_fields(),
        [
            TicketField::Quantity,
            TicketField::OrderType,
            TicketField::Price
        ]
    );
    let confirm = build_confirm(&state, None, 0).unwrap();
    let PendingAction::Modify { form, .. } = confirm.action else {
        panic!("expected modify action")
    };
    assert!(
        !form.iter().any(|(k, _)| k == "validity"),
        "modify form must not send validity"
    );
    assert!(
        !form.iter().any(|(k, _)| k == "product"),
        "modify form must not send product"
    );
}

#[test]
fn modify_locks_product_and_keeps_original() {
    let mut state = ticket();
    state.exchange = "NFO".to_string();
    state.product = 2; // NRML (the original order's product)
    state.modify = Some(ModifyTarget {
        parent_order_id: None,
        order_id: "241010000123456".to_string(),
        variety: "regular".to_string(),
        quantity_editable: true,
    });
    let confirm = build_confirm(&state, None, 0).unwrap();
    assert!(
        confirm
            .lines
            .iter()
            .any(|(k, v)| k == "Product" && v == "NRML"),
        "modify review must show the original product"
    );
    let form = match confirm.action {
        PendingAction::Modify { form, .. } => form,
        other => panic!("expected modify action, got {other:?}"),
    };
    assert!(!form.iter().any(|(k, _)| k == "product"));
}

#[test]
fn rejects_limit_without_price() {
    let mut state = ticket();
    state.price = String::new();
    assert!(build_confirm(&state, None, 0).is_err());
}

#[test]
fn market_order_omits_price() {
    let mut state = ticket();
    state.order_type = OrderType::Market;
    state.price = "999".to_string();
    let confirm = build_confirm(&state, None, 0).unwrap();
    let super::PendingAction::Place { form, .. } = confirm.action else {
        panic!("expected place action")
    };
    assert!(!form.iter().any(|(k, _)| k == "price"));
}

#[test]
fn market_and_sl_m_focus_skip_disabled_price_fields() {
    let mut market = ticket();
    market.order_type = OrderType::Market;
    market.focus = TicketField::OrderType;
    market.move_focus(1);
    assert_eq!(
        market.focus,
        TicketField::Product,
        "MARKET skips price and trigger"
    );

    let mut sl_m = ticket();
    sl_m.order_type = OrderType::SlM;
    sl_m.focus = TicketField::OrderType;
    sl_m.move_focus(1);
    assert_eq!(
        sl_m.focus,
        TicketField::Trigger,
        "SL-M skips price but keeps trigger"
    );
}

#[test]
fn rejects_qty_not_multiple_of_lot() {
    let mut state = ticket();
    state.lot_size = 400;
    state.qty = "500".to_string();
    let err = build_confirm(&state, None, 0).unwrap_err().message;
    assert_eq!(err, "quantity must be a multiple of lot size 400");
    state.qty = "800".to_string();
    assert!(build_confirm(&state, None, 0).is_ok());
}

#[test]
fn rejects_price_off_tick() {
    let mut state = ticket();
    state.price = "1523.43".to_string(); // not a 0.05 multiple
    let err = build_confirm(&state, None, 0).unwrap_err().message;
    assert_eq!(err, "price must be a multiple of tick size 0.05");
}

#[test]
fn seeds_qty_one_lot_when_unspecified() {
    assert_eq!(super::seed_qty(0, 1), "1");
    assert_eq!(super::seed_qty(0, 75), "75");
    assert_eq!(super::seed_qty(-5, 50), "50");
    assert_eq!(super::seed_qty(0, 0), "1");
    assert_eq!(super::seed_qty(150, 75), "150");
}

#[test]
fn rejects_exchange_invalid_product() {
    let mut state = ticket();
    state.product = 2; // NRML
    let err = build_confirm(&state, None, 0).unwrap_err().message;
    assert!(err.contains("NRML"), "unexpected error: {err}");
    state.product = 0;
    assert!(build_confirm(&state, None, 0).is_ok());
    let mut gtt = gtt_ticket();
    gtt.exchange = "NFO".to_string();
    gtt.product = 3; // MTF
    assert!(build_confirm(&gtt, Some(1523.40), 0).is_err());
}

#[test]
fn steps_qty_by_lot() {
    assert_eq!(step_qty("", 400, 1), "400"); // empty seeds one lot
    assert_eq!(step_qty("", 400, -1), "400"); // never below one lot
    assert_eq!(step_qty("400", 400, 1), "800");
    assert_eq!(step_qty("800", 400, -1), "400");
    assert_eq!(step_qty("400", 400, -1), "400"); // clamps at one lot
    assert_eq!(step_qty("10", 1, 1), "11");
}

#[test]
fn steps_price_by_tick() {
    assert_eq!(step_price("1523.40", 0.05, 1, 0.0), "1523.45");
    assert_eq!(step_price("1523.40", 0.05, -1, 0.0), "1523.35");
    assert_eq!(step_price("1523.43", 0.05, 1, 0.0), "1523.50");
    assert_eq!(step_price("", 0.05, 1, 1071.72), "1071.70");
    assert_eq!(step_price("", 0.05, 1, 0.0), "0.05");
    assert_eq!(step_price("95.0000", 0.0025, 1, 0.0), "95.0025");
    assert_eq!(step_price("95.0000", 0.0025, -1, 0.0), "94.9975");
}

#[test]
fn signed_alert_steps_cross_zero_without_clamping() {
    assert_eq!(step_signed_number("-50", 0.05, 1, 0.0), "-49.95");
    assert_eq!(step_signed_number("-50", 0.05, -1, 0.0), "-50.05");
    assert_eq!(step_signed_number("-0.05", 0.05, 1, 0.0), "0.00");
}

fn gtt_ticket() -> TicketState {
    let mut state = ticket();
    state.kind = TicketKind::Gtt { modify: None };
    state.trigger = "1500.00".to_string();
    state
}

#[test]
fn gtt_builds_wire_format_form() {
    let confirm = build_confirm(&gtt_ticket(), Some(1523.40), 0).unwrap();
    let super::PendingAction::PlaceGtt { form } = confirm.action else {
        panic!("expected place-GTT action")
    };
    let get = |key: &str| {
        form.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap()
    };
    assert_eq!(get("type"), "single");

    let condition: serde_json::Value = serde_json::from_str(get("condition")).unwrap();
    assert_eq!(condition["exchange"], "NSE");
    assert_eq!(condition["tradingsymbol"], "INFY");
    assert_eq!(condition["trigger_values"][0], 1500.0);
    assert_eq!(condition["last_price"], 1523.40);

    let orders: serde_json::Value = serde_json::from_str(get("orders")).unwrap();
    assert_eq!(orders[0]["transaction_type"], "BUY");
    assert_eq!(orders[0]["quantity"], 10);
    assert_eq!(orders[0]["order_type"], "LIMIT");
    assert_eq!(orders[0]["price"], 1523.40);
    assert!(confirm.preview_request.is_none());
}

#[test]
fn gtt_trailing_single_builds_kite_web_wire_format() {
    let mut state = gtt_ticket();
    state.side = Side::Sell;
    state.gtt_trailing = true;
    state.trailing_points = "2.00".to_string();
    let confirm = build_confirm(&state, Some(1523.40), 0).unwrap();
    let form = match confirm.action {
        PendingAction::PlaceGtt { form } => form,
        other => panic!("expected place-GTT action, got {other:?}"),
    };
    assert_eq!(form[0].1, "trailing-single");
    let condition: serde_json::Value = serde_json::from_str(&form[1].1).unwrap();
    assert_eq!(condition["trailing_points"], serde_json::json!([2.0]));
}

#[test]
fn gtt_trailing_rejects_buy_and_missing_points() {
    let mut state = gtt_ticket();
    state.gtt_trailing = true;
    state.trailing_points = "2.00".to_string();
    assert!(build_confirm(&state, Some(1523.40), 0).is_err());
    state.side = Side::Sell;
    state.trailing_points.clear();
    assert!(build_confirm(&state, Some(1523.40), 0).is_err());
}

#[test]
fn trailing_points_reject_numeric_overflow() {
    let mut state = gtt_ticket();
    state.side = Side::Sell;
    state.gtt_trailing = true;
    state.trailing_points = "9".repeat(400);
    let issue = build_confirm(&state, Some(1523.40), 0).unwrap_err();
    assert_eq!(issue.field, Some(TicketField::TrailPoints));
    assert_eq!(issue.message, "trailing points must be a positive number");
}

#[test]
fn gtt_rejects_intraday_product_and_emits_delivery() {
    let mut state = gtt_ticket();
    state.product = 1; // MIS
    let err = build_confirm(&state, Some(1523.40), 0).unwrap_err().message;
    assert!(err.contains("MIS"), "unexpected error: {err}");
    state.product = 0; // CNC
    let confirm = build_confirm(&state, Some(1523.40), 0).unwrap();
    let form = match confirm.action {
        PendingAction::PlaceGtt { form } => form,
        other => panic!("expected place-GTT action, got {other:?}"),
    };
    let orders: serde_json::Value = serde_json::from_str(
        form.iter()
            .find(|(k, _)| k == "orders")
            .map(|(_, v)| v.as_str())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(orders[0]["product"], "CNC");
}

#[test]
fn gtt_requires_trigger() {
    let mut state = gtt_ticket();
    state.trigger = String::new();
    assert!(build_confirm(&state, None, 0).is_err());
}

#[test]
fn gtt_modify_targets_trigger_id() {
    let mut state = gtt_ticket();
    state.kind = TicketKind::Gtt {
        modify: Some(123_456_789),
    };
    let confirm = build_confirm(&state, None, 0).unwrap();
    match confirm.action {
        super::PendingAction::ModifyGtt { trigger_id, .. } => {
            assert_eq!(trigger_id, 123_456_789);
        }
        _ => panic!("expected modify-GTT action"),
    }
}

#[test]
fn build_confirm_stamps_preview_seq_only_for_place() {
    let confirm = build_confirm(&ticket(), None, 42).unwrap();
    assert!(matches!(confirm.action, PendingAction::Place { .. }));
    assert_eq!(confirm.preview_seq, Some(42));
    assert!(confirm.preview_request.is_some());

    let mut modify = ticket();
    modify.modify = Some(super::ModifyTarget {
        parent_order_id: None,
        order_id: "X1".to_string(),
        variety: "regular".to_string(),
        quantity_editable: true,
    });
    let confirm = build_confirm(&modify, None, 42).unwrap();
    assert!(matches!(confirm.action, PendingAction::Modify { .. }));
    assert_eq!(confirm.preview_seq, None);

    let confirm = build_confirm(&gtt_ticket(), Some(1523.40), 42).unwrap();
    assert!(matches!(confirm.action, PendingAction::PlaceGtt { .. }));
    assert_eq!(confirm.preview_seq, None);
}

#[test]
fn gtt_confirm_flags_fallback_ref_price() {
    let confirm = build_confirm(&gtt_ticket(), None, 0).unwrap();
    assert!(
        confirm
            .lines
            .iter()
            .any(|(k, v)| k == "Ref price" && v.contains("no live tick"))
    );
    let confirm = build_confirm(&gtt_ticket(), Some(1523.40), 0).unwrap();
    assert!(confirm.lines.iter().all(|(k, _)| k != "Ref price"));
}

fn oco_ticket() -> TicketState {
    let mut state = gtt_ticket();
    state.gtt_oco = true;
    state.trigger = "1550.00".to_string();
    state.price = "1551.00".to_string();
    state.trigger2 = "1500.00".to_string();
    state.price2 = "1499.00".to_string();
    state
}

#[test]
fn gtt_oco_builds_two_leg_form() {
    let confirm = build_confirm(&oco_ticket(), Some(1523.40), 0).unwrap();
    let form = match confirm.action {
        PendingAction::PlaceGtt { form } => form,
        other => panic!("expected place-GTT action, got {other:?}"),
    };
    let get = |key: &str| {
        form.iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
            .unwrap()
    };
    assert_eq!(get("type"), "two-leg");

    let condition: serde_json::Value = serde_json::from_str(get("condition")).unwrap();
    let triggers = condition["trigger_values"].as_array().unwrap();
    assert_eq!(triggers.len(), 2);
    assert_eq!(triggers[0], 1500.0);
    assert_eq!(triggers[1], 1550.0);

    let orders: serde_json::Value = serde_json::from_str(get("orders")).unwrap();
    let legs = orders.as_array().unwrap();
    assert_eq!(legs.len(), 2);
    assert_eq!(legs[0]["price"], 1499.0);
    assert_eq!(legs[1]["price"], 1551.0);
    for leg in legs {
        assert_eq!(leg["transaction_type"], "BUY");
        assert_eq!(leg["quantity"], 10);
        assert_eq!(leg["order_type"], "LIMIT");
        assert_eq!(leg["product"], "CNC");
    }
    assert!(confirm.preview_request.is_none());
}

#[test]
fn gtt_trailing_oco_uses_trailing_two_leg_type() {
    let mut state = oco_ticket();
    state.side = Side::Sell;
    state.gtt_trailing = true;
    state.trailing_points = "2.00".to_string();
    let confirm = build_confirm(&state, Some(1523.40), 0).unwrap();
    let PendingAction::PlaceGtt { form } = confirm.action else {
        panic!("expected place GTT action");
    };
    assert_eq!(form[0].1, "trailing-two-leg");
    let condition: serde_json::Value = serde_json::from_str(&form[1].1).unwrap();
    assert_eq!(condition["trailing_points"], serde_json::json!([2.0]));
}

#[test]
fn gtt_oco_rejects_equal_or_zero_triggers() {
    let mut state = oco_ticket();
    state.trigger2 = "1550.00".to_string();
    let err = build_confirm(&state, Some(1523.40), 0).unwrap_err().message;
    assert!(err.contains("different"), "unexpected error: {err}");
    let mut state = oco_ticket();
    state.trigger2 = "0".to_string();
    assert!(build_confirm(&state, Some(1523.40), 0).is_err());
    state.trigger2 = String::new();
    assert!(build_confirm(&state, Some(1523.40), 0).is_err());
}

#[test]
fn gtt_fields_track_oco_and_trailing() {
    use TicketField as F;
    let mut state = gtt_ticket();
    state.side = Side::Buy;
    assert_eq!(
        state.visible_fields(),
        [F::Quantity, F::Side, F::GttType, F::Price, F::Trigger]
    );
    state.side = Side::Sell;
    assert_eq!(
        state.visible_fields(),
        [
            F::Quantity,
            F::Side,
            F::GttType,
            F::Price,
            F::Trigger,
            F::Trailing
        ]
    );
    state.gtt_trailing = true;
    assert!(state.visible_fields().contains(&F::TrailPoints));
    state.gtt_oco = true;
    assert_eq!(
        state.visible_fields(),
        [
            F::Quantity,
            F::Side,
            F::GttType,
            F::Price,
            F::Trigger,
            F::TargetPrice,
            F::TargetTrigger,
            F::Trailing,
            F::TrailPoints
        ]
    );
}

#[test]
fn autoslice_param_only_when_enabled() {
    let mut state = ticket();
    let form = place_form(build_confirm(&state, None, 0).unwrap().action);
    assert!(!form.iter().any(|(k, _)| k == "autoslice"));
    state.autoslice = true;
    let form = place_form(build_confirm(&state, None, 0).unwrap().action);
    assert_eq!(
        form.iter()
            .find(|(k, _)| k == "autoslice")
            .map(|(_, v)| v.as_str()),
        Some("true")
    );
}

#[test]
fn confirmation_lines_include_ttl_iceberg_and_market_protection() {
    let mut state = ticket();
    state.qty = "100".to_string();
    state.order_type = OrderType::SlM;
    state.trigger = "1520.00".to_string();
    state.validity = 2;
    state.validity_ttl = 5;
    state.variety = 1;
    state.market_protection = true;
    let confirm = build_confirm(&state, Some(1523.40), 0).unwrap();
    assert_eq!(
        confirm.lines,
        [
            ("Symbol", "NSE:INFY"),
            ("Side", "BUY"),
            ("Qty", "100"),
            ("Type", "SL-M trg 1520"),
            ("Product", "CNC"),
            ("Validity", "TTL 15m"),
            ("Iceberg", "2 legs × 50"),
            ("Protection", "automatic"),
            ("Variety", "ICEBERG"),
        ]
        .map(|(key, value)| (key.to_string(), value.to_string()))
    );
}

#[test]
fn ttl_validities_emit_supported_minutes() {
    for (ttl_index, minutes) in crate::tui::app::TTL_MINUTES.into_iter().enumerate() {
        let mut state = ticket();
        state.validity = 2;
        state.validity_ttl = ttl_index;
        let form = place_form(build_confirm(&state, None, 0).unwrap().action);
        let get = |key: &str| {
            form.iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.as_str())
        };
        assert_eq!(get("validity"), Some("TTL"));
        assert_eq!(get("validity_ttl"), Some(minutes.to_string().as_str()));
    }

    let form = place_form(build_confirm(&ticket(), None, 0).unwrap().action);
    assert_eq!(
        form.iter()
            .find(|(name, _)| name == "validity")
            .map(|(_, value)| value.as_str()),
        Some("DAY")
    );
    assert!(!form.iter().any(|(name, _)| name == "validity_ttl"));
}

#[test]
fn ticket_prefs_normalize_sl_and_sl_m_to_limit_but_keep_market() {
    let mut state = ticket();
    state.order_type = OrderType::Sl; // SL
    state.trigger = "1520.00".to_string();
    let confirm = build_confirm(&state, None, 0).unwrap();
    let (_, prefs) = confirm.ticket_prefs.expect("place ticket must store prefs");
    assert_eq!(
        prefs.order_type,
        OrderType::Limit,
        "SL must normalize to LIMIT"
    );

    let mut state = ticket();
    state.order_type = OrderType::SlM; // SL-M
    state.trigger = "1520.00".to_string();
    let confirm = build_confirm(&state, None, 0).unwrap();
    let (_, prefs) = confirm.ticket_prefs.expect("place ticket must store prefs");
    assert_eq!(
        prefs.order_type,
        OrderType::Limit,
        "SL-M must normalize to LIMIT"
    );

    let mut state = ticket();
    state.order_type = OrderType::Market; // MARKET
    let confirm = build_confirm(&state, None, 0).unwrap();
    let (_, prefs) = confirm.ticket_prefs.expect("place ticket must store prefs");
    assert_eq!(
        prefs.order_type,
        OrderType::Market,
        "MARKET must be remembered as-is"
    );

    let state = ticket();
    let confirm = build_confirm(&state, None, 0).unwrap();
    let (_, prefs) = confirm.ticket_prefs.expect("place ticket must store prefs");
    assert_eq!(
        prefs.order_type,
        OrderType::Limit,
        "LIMIT must be remembered as-is"
    );
}

#[test]
fn iceberg_variety_derives_leg_quantity() {
    let mut state = ticket();
    state.variety = 1; // iceberg
    state.iceberg_legs = "3".to_string();
    state.qty = "9".to_string();
    let confirm = build_confirm(&state, None, 0).unwrap();
    let variety = match &confirm.action {
        PendingAction::Place { variety, .. } => variety.clone(),
        other => panic!("expected place action, got {other:?}"),
    };
    assert_eq!(variety, "iceberg");
    let form = place_form(confirm.action);
    let get = |key: &str| form.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    assert_eq!(get("iceberg_legs"), Some("3"));
    assert_eq!(get("iceberg_quantity"), Some("3"));
    assert_eq!(3 * 3, 9);
    state.variety = 0;
    let form = place_form(build_confirm(&state, None, 0).unwrap().action);
    assert!(!form.iter().any(|(k, _)| k == "iceberg_legs"));
    assert!(!form.iter().any(|(k, _)| k == "iceberg_quantity"));
}

#[test]
fn iceberg_rounds_uneven_split_up_like_kite_web() {
    let mut state = ticket();
    state.variety = 1;
    state.iceberg_legs = "3".to_string();
    state.qty = "10".to_string();
    let form = place_form(build_confirm(&state, None, 0).unwrap().action);
    let per_leg = form
        .iter()
        .find(|(key, _)| key == "iceberg_quantity")
        .map(|(_, value)| value.as_str());
    assert_eq!(per_leg, Some("4"));
}

#[test]
fn iceberg_splits_are_lot_aligned() {
    let mut state = ticket();
    state.variety = 1;
    state.lot_size = 75;
    state.iceberg_legs = "2".to_string();
    state.qty = "150".to_string();
    let form = place_form(build_confirm(&state, None, 0).unwrap().action);
    let get = |key: &str| form.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str());
    assert_eq!(get("iceberg_quantity"), Some("75"));
    assert_eq!(2 * 75, 150);
    state.qty = "225".to_string();
    let form = place_form(build_confirm(&state, None, 0).unwrap().action);
    let per_leg = form
        .iter()
        .find(|(key, _)| key == "iceberg_quantity")
        .map(|(_, value)| value.as_str());
    assert_eq!(per_leg, Some("150"));
}

#[test]
fn commodity_exchanges_reject_iceberg_orders() {
    for exchange in ["MCX", "NCO", "BCD"] {
        let mut state = ticket();
        state.exchange = exchange.to_string();
        state.product = 2; // NRML
        state.variety = 1;
        state.qty = "10".to_string();
        state.iceberg_legs = "2".to_string();
        let err = build_confirm(&state, None, 0).unwrap_err().message;
        assert_eq!(
            err,
            format!("iceberg orders are not available on {exchange}")
        );
        assert_eq!(super::ticket_varieties(exchange), &["regular"]);
    }
}

#[test]
fn iceberg_rejects_bad_legs_and_short_quantity() {
    let mut state = ticket();
    state.variety = 1;
    state.iceberg_legs = "1".to_string();
    assert!(build_confirm(&state, None, 0).is_err());
    state.iceberg_legs = "51".to_string();
    assert!(build_confirm(&state, None, 0).is_err());
    state.iceberg_legs = "20".to_string();
    state.qty = "10".to_string();
    assert!(build_confirm(&state, None, 0).is_err());
}

#[test]
fn fno_iceberg_under_five_lots_quotes_kite_webs_message() {
    let mut state = ticket();
    state.exchange = "NFO".to_string();
    state.product = 2; // NRML
    state.variety = 1;
    state.lot_size = 65;
    state.qty = "65".to_string();
    state.iceberg_legs = "2".to_string();
    assert_eq!(
        build_confirm(&state, Some(39.95), 0).unwrap_err().message,
        "F&O Iceberg orders require minimum 5 lots (or 325 order qty.)"
    );
    state.qty = "325".to_string();
    assert!(build_confirm(&state, Some(39.95), 0).is_ok());

    state.lot_size = 1;
    state.qty = "4".to_string();
    assert_eq!(
        build_confirm(&state, Some(39.95), 0).unwrap_err().message,
        "F&O Iceberg orders require minimum 5 lots/qty."
    );
}

#[test]
fn equity_iceberg_requires_a_one_lakh_order_value() {
    let mut state = ticket();
    state.variety = 1;
    state.qty = "100".to_string();
    state.price = "500.00".to_string();
    assert_eq!(
        build_confirm(&state, Some(500.0), 0).unwrap_err().message,
        "Equity Iceberg orders require minimum 1,00,000 value, set order qty. to atleast 200 \
         (current value: 50000.00)"
    );
    state.qty = "200".to_string();
    assert!(build_confirm(&state, Some(500.0), 0).is_ok());
    state.qty = "100".to_string();
    assert!(build_confirm(&state, None, 0).is_ok());
}

#[test]
fn iceberg_leg_count_above_quantity_uses_kite_webs_unit() {
    let mut state = ticket();
    state.variety = 1;
    state.qty = "19".to_string();
    state.price = "6000.00".to_string();
    state.iceberg_legs = "20".to_string();
    assert_eq!(
        build_confirm(&state, Some(6000.0), 0).unwrap_err().message,
        "Order qty. should be greater than Iceberg legs"
    );

    let mut fno = ticket();
    fno.exchange = "NFO".to_string();
    fno.product = 2; // NRML
    fno.variety = 1;
    fno.lot_size = 10;
    fno.qty = "80".to_string(); // 8 lots, clears the 5-lot floor
    fno.iceberg_legs = "9".to_string();
    assert_eq!(
        build_confirm(&fno, Some(39.95), 0).unwrap_err().message,
        "Order lots should be greater than Iceberg legs"
    );
}

#[test]
fn gtt_trigger_seeds_the_way_kite_web_does() {
    assert_eq!(
        (gtt_default_trigger_percent("NFO-OPT")).to_bits(),
        10.0_f64.to_bits()
    );
    assert_eq!(
        (gtt_default_trigger_percent("MCX-OPT")).to_bits(),
        10.0_f64.to_bits()
    );
    assert_eq!(
        (gtt_default_trigger_percent("NFO-FUT")).to_bits(),
        2.0_f64.to_bits()
    );
    assert_eq!(
        (gtt_default_trigger_percent("NSE")).to_bits(),
        5.0_f64.to_bits()
    );
    assert_eq!(
        (gtt_default_trigger_percent("")).to_bits(),
        5.0_f64.to_bits()
    );

    assert_eq!(
        gtt_seed_trigger(39.95, 10.0, 0.05).as_deref(),
        Some("43.90")
    );
    assert_eq!(
        gtt_seed_trigger(1000.0, 5.0, 0.05).as_deref(),
        Some("1050.00")
    );
    assert_eq!(
        gtt_seed_trigger(1000.0, -5.0, 0.05).as_deref(),
        Some("950.00")
    );
    assert_eq!(gtt_seed_trigger(0.0, 10.0, 0.05), None);
    assert_eq!(gtt_seed_trigger(0.05, -95.0, 0.05), None);
}

#[test]
fn gtt_trigger_below_the_floor_quotes_kites_message() {
    let mut state = ticket();
    state.kind = TicketKind::Gtt { modify: None };
    state.exchange = "BFO".to_string();
    state.product = 2; // NRML
    state.price = "0.05".to_string();
    state.trigger = String::new();
    assert_eq!(
        build_confirm(&state, Some(0.05), 0).unwrap_err().message,
        "trigger price is required"
    );
    state.trigger = "0".to_string();
    assert_eq!(
        build_confirm(&state, Some(0.05), 0).unwrap_err().message,
        "Trigger cannot be created with trigger price less than 0.05"
    );
    state.trigger = "0.05".to_string();
    assert!(build_confirm(&state, Some(0.05), 0).is_ok());

    state.gtt_oco = true;
    state.price2 = "0.10".to_string();
    state.trigger2 = String::new();
    assert_eq!(
        build_confirm(&state, Some(0.05), 0).unwrap_err().message,
        "second trigger price is required"
    );
}

#[test]
fn visible_fields_track_mode_and_variety() {
    use TicketField as F;
    let mut state = ticket();
    assert_eq!(
        state.visible_fields(),
        [
            F::Quantity,
            F::OrderType,
            F::Price,
            F::Product,
            F::Validity,
            F::Autoslice,
            F::Variety
        ]
    );
    state.variety = 1;
    assert!(state.visible_fields().contains(&F::Legs));
    state.variety = 0;
    state.modify = Some(ModifyTarget {
        parent_order_id: None,
        order_id: "X1".to_string(),
        variety: "regular".to_string(),
        quantity_editable: true,
    });
    assert_eq!(
        state.visible_fields(),
        [F::Quantity, F::OrderType, F::Price]
    );
    assert!(!state.variety_is_iceberg());
}

#[test]
fn steps_iceberg_legs_within_range() {
    assert_eq!(step_legs("", 1), "3"); // empty seeds two legs, then +1
    assert_eq!(step_legs("2", -1), "2"); // clamps at the minimum
    assert_eq!(step_legs("50", 1), "50"); // clamps at the maximum
    assert_eq!(step_legs("5", 1), "6");
}

#[test]
fn autoslice_param_forced_above_freeze_and_iceberg_skips() {
    let mut state = ticket();
    state.freeze_quantity = Some(100);
    state.autoslice = false;
    state.qty = "50".to_string();
    assert!(!state.effective_autoslice());

    state.autoslice = true;
    assert!(state.effective_autoslice());

    state.autoslice = false;
    state.qty = "150".to_string();
    assert!(state.effective_autoslice());

    state.variety = 1;
    assert!(!state.effective_autoslice());

    let gtt = gtt_ticket();
    assert!(!gtt.effective_autoslice());

    let mut modify = ticket();
    modify.modify = Some(ModifyTarget {
        parent_order_id: None,
        order_id: "X1".to_string(),
        variety: "regular".to_string(),
        quantity_editable: true,
    });
    assert!(!modify.effective_autoslice());
}

#[test]
fn sliced_orders_allow_total_quantity_above_exchange_limit() {
    let mut regular = ticket();
    regular.freeze_quantity = Some(100);
    regular.max_order_quantity = Some(100);
    regular.qty = "200".to_string();
    let form = place_form(build_confirm(&regular, None, 0).unwrap().action);
    assert_eq!(
        form.iter()
            .find(|(name, _)| name == "autoslice")
            .map(|(_, value)| value.as_str()),
        Some("true")
    );

    let mut iceberg = ticket();
    iceberg.variety = 1;
    iceberg.freeze_quantity = Some(100);
    iceberg.max_order_quantity = Some(100);
    iceberg.qty = "200".to_string();
    iceberg.iceberg_legs = "2".to_string();
    let form = place_form(build_confirm(&iceberg, None, 0).unwrap().action);
    assert_eq!(
        form.iter()
            .find(|(name, _)| name == "iceberg_quantity")
            .map(|(_, value)| value.as_str()),
        Some("100")
    );
}

#[test]
fn max_order_quantity_rejected_regular_and_gtt() {
    let mut state = ticket();
    state.max_order_quantity = Some(5);
    state.qty = "10".to_string();
    let err = build_confirm(&state, None, 0).unwrap_err().message;
    assert!(err.contains("exceeds max order quantity"), "got: {err}");

    state.qty = "5".to_string();
    assert!(build_confirm(&state, None, 0).is_ok());

    let mut gtt = gtt_ticket();
    gtt.max_order_quantity = Some(5);
    gtt.qty = "10".to_string();
    let err = build_confirm(&gtt, Some(100.0), 0).unwrap_err().message;
    assert!(err.contains("exceeds max order quantity"), "got: {err}");

    let mut oco = oco_ticket();
    oco.max_order_quantity = Some(5);
    oco.qty = "10".to_string();
    let err = build_confirm(&oco, Some(100.0), 0).unwrap_err().message;
    assert!(err.contains("exceeds max order quantity"), "got: {err}");
}

#[test]
fn iceberg_per_leg_rejected_above_freeze() {
    let mut state = ticket();
    state.variety = 1;
    state.iceberg_legs = "2".to_string();
    state.qty = "200".to_string();
    state.freeze_quantity = Some(50); // per-leg qty 100 > 50 freeze
    let err = build_confirm(&state, None, 0).unwrap_err().message;
    assert!(
        err.contains("per-leg quantity") && err.contains("exceeds freeze"),
        "got: {err}"
    );

    state.freeze_quantity = Some(200);
    assert!(build_confirm(&state, None, 0).is_ok());
}

#[test]
fn edit_numeric_rejects_second_dot() {
    let mut buffer = String::new();
    for ch in "12.3.4".chars() {
        edit_numeric(&mut buffer, KeyCode::Char(ch));
    }
    assert_eq!(buffer, "12.34");
    edit_numeric(&mut buffer, KeyCode::Backspace);
    assert_eq!(buffer, "12.3");
    edit_numeric(&mut buffer, KeyCode::Backspace); // "12."
    edit_numeric(&mut buffer, KeyCode::Backspace); // "12"
    edit_numeric(&mut buffer, KeyCode::Char('.'));
    assert_eq!(buffer, "12.");
}

#[test]
fn signed_numeric_accepts_only_a_leading_minus() {
    let mut buffer = String::new();
    for code in [
        KeyCode::Char('-'),
        KeyCode::Char('3'),
        KeyCode::Char('.'),
        KeyCode::Char('5'),
        KeyCode::Char('-'),
    ] {
        edit_signed_numeric(&mut buffer, code);
    }
    assert_eq!(buffer, "-3.5");
}

fn type_str(buffer: &mut String, s: &str) {
    for ch in s.chars() {
        super::edit_quantity(buffer, KeyCode::Char(ch));
    }
}

#[test]
fn edit_quantity_rejects_dot() {
    let mut buffer = String::new();
    type_str(&mut buffer, "12");
    super::edit_quantity(&mut buffer, KeyCode::Char('.'));
    assert_eq!(buffer, "12");
}

#[test]
fn edit_quantity_collapses_leading_zeros() {
    let mut buffer = String::new();
    type_str(&mut buffer, "007");
    assert_eq!(buffer, "7");
    let mut buffer = String::new();
    type_str(&mut buffer, "0");
    assert_eq!(buffer, "0");
}

#[test]
fn edit_quantity_backspace_deletes() {
    let mut buffer = String::new();
    type_str(&mut buffer, "123");
    super::edit_quantity(&mut buffer, KeyCode::Backspace);
    assert_eq!(buffer, "12");
}

#[test]
fn edit_quantity_caps_at_ten_digits_and_u32_max() {
    let mut buffer = String::new();
    type_str(&mut buffer, "99999999999999"); // far more than 10 digits typed
    assert!(
        buffer.chars().count() <= 10,
        "buffer grew past 10 digits: {buffer}"
    );
    assert!(
        buffer.parse::<u32>().is_ok(),
        "buffer must stay a valid u32: {buffer}"
    );

    let mut buffer = "4294967295".to_string();
    super::edit_quantity(&mut buffer, KeyCode::Char('1'));
    assert_eq!(buffer, "4294967295");
}

#[test]
fn edit_legs_is_digits_only_and_capped_at_two_chars() {
    let mut buffer = String::new();
    super::edit_legs(&mut buffer, KeyCode::Char('.'));
    assert_eq!(buffer, "", "a dot must never be accepted");
    super::edit_legs(&mut buffer, KeyCode::Char('5'));
    super::edit_legs(&mut buffer, KeyCode::Char('0'));
    assert_eq!(buffer, "50");
    super::edit_legs(&mut buffer, KeyCode::Char('9'));
    assert_eq!(buffer, "50");
    super::edit_legs(&mut buffer, KeyCode::Backspace);
    assert_eq!(buffer, "5");
}

#[test]
fn step_qty_overflowed_buffer_is_clamped_not_reset() {
    let huge = "99999999999999"; // all digits, overflows u32::MAX
    assert_eq!(step_qty(huge, 10, 1), u32::MAX.to_string());
    assert_eq!(step_qty(huge, 10, -1), (u32::MAX - 10).to_string());
}

proptest! {
    #[test]
    fn edit_quantity_property_never_exceeds_bounds(keys in prop::collection::vec(prop::sample::select(vec!['0','1','2','3','4','5','6','7','8','9','.','\u{8}']), 0..40)) {
        let mut buffer = String::new();
        for ch in keys {
            let code = if ch == '\u{8}' { KeyCode::Backspace } else { KeyCode::Char(ch) };
            super::edit_quantity(&mut buffer, code);
            prop_assert!(buffer.chars().all(|c| c.is_ascii_digit()));
            prop_assert!(buffer.chars().count() <= 10);
            prop_assert!(buffer.is_empty() || buffer.parse::<u32>().is_ok());
        }
    }
}
const NO_SEED: super::GttSeed = super::GttSeed {
    ltp: None,
    percent: 5.0,
};

#[test]
fn an_exact_mouse_choice_and_a_keyboard_walk_agree() {
    let mut walked = ticket();
    walked.order_type = OrderType::Market;
    for _ in 0..3 {
        walked.select(TicketField::OrderType, ControlIntent::Next, NO_SEED);
    }
    let mut clicked = ticket();
    clicked.order_type = OrderType::Market;
    clicked.select(TicketField::OrderType, ControlIntent::Option(3), NO_SEED);
    assert_eq!(walked.order_type, clicked.order_type);
    assert_eq!(walked.price, clicked.price);
    assert_eq!(walked.trigger, clicked.trigger);
    assert_eq!(
        build_confirm(&walked, None, 0).unwrap_err().message,
        build_confirm(&clicked, None, 0).unwrap_err().message
    );
}

#[test]
fn clicking_the_selected_option_changes_nothing() {
    let mut state = ticket();
    state.order_type = OrderType::Limit; // LIMIT
    state.price = "1523.40".to_string();
    state.select(TicketField::OrderType, ControlIntent::Option(1), NO_SEED);
    assert_eq!(state.order_type, OrderType::Limit);
    assert_eq!(
        state.price, "1523.40",
        "re-selecting LIMIT must not clear its own price"
    );
    assert_eq!(state.focused(), TicketField::OrderType);
}

#[path = "regressions.rs"]
mod regressions;
