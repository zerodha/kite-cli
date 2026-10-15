use super::*;
use crate::tui::ticket::GttSeed;

#[test]
fn switching_order_type_preserves_hidden_price_and_trigger() {
    let mut state = ticket();
    state.order_type = OrderType::Sl; // SL
    state.trigger = "1500.00".to_string();
    state.market_protection = true;
    state.select(TicketField::OrderType, ControlIntent::Option(0), NO_SEED);
    assert_eq!(state.price, "1523.40");
    assert_eq!(state.trigger, "1500.00");
    assert_eq!(state.qty, "10", "quantity is untouched by an order type");
    assert!(state.market_protection_enabled());
}

#[test]
fn a_mode_change_that_hides_the_focus_leaves_it_on_the_controlling_row() {
    let mut state = ticket();
    state.focus = TicketField::Price;
    state.select(TicketField::OrderType, ControlIntent::Option(0), NO_SEED);
    assert_eq!(
        state.focused(),
        TicketField::OrderType,
        "MARKET has no price row to stay on"
    );
}

#[test]
fn a_field_issue_survives_tab_and_clears_on_a_relevant_edit() {
    let mut state = ticket();
    state.qty = "0".to_string();
    let issue = build_confirm(&state, None, 0).unwrap_err();
    assert_eq!(issue.field, Some(TicketField::Quantity));
    state.issue = Some(issue);
    state.move_focus(1);
    assert!(state.issue.is_some(), "Tab alone does not clear a failure");
    state.edit(TicketField::Price, KeyCode::Char('1'));
    assert!(
        state.issue.is_some(),
        "another field's edit does not clear it either"
    );
    state.edit(TicketField::Quantity, KeyCode::Char('5'));
    assert!(state.issue.is_none());
}

#[test]
fn a_controlling_row_clears_the_failure_it_makes_moot() {
    let mut state = ticket();
    state.order_type = OrderType::Sl; // SL, which needs a trigger
    state.trigger = String::new();
    let issue = build_confirm(&state, None, 0).unwrap_err();
    assert_eq!(issue.field, Some(TicketField::Trigger));
    state.issue = Some(issue);
    state.select(TicketField::OrderType, ControlIntent::Option(1), NO_SEED);
    assert!(state.issue.is_none());
}

#[test]
fn every_validator_names_the_row_that_failed() {
    use TicketField as F;
    let cases: Vec<(TicketState, F)> = vec![
        (
            TicketState {
                qty: "0".to_string(),
                ..ticket()
            },
            F::Quantity,
        ),
        (
            TicketState {
                price: "1523.43".to_string(),
                ..ticket()
            },
            F::Price,
        ),
        (
            TicketState {
                order_type: OrderType::Sl, // SL
                trigger: "0".to_string(),
                ..ticket()
            },
            F::Trigger,
        ),
        (
            TicketState {
                variety: 1, // iceberg
                iceberg_legs: "99".to_string(),
                ..ticket()
            },
            F::Legs,
        ),
        (
            TicketState {
                exchange: "NFO".to_string(),
                product: 3, // MTF, which F&O never accepts
                ..ticket()
            },
            F::Product,
        ),
    ];
    for (state, field) in cases {
        let issue = build_confirm(&state, Some(1500.0), 0).unwrap_err();
        assert_eq!(issue.field, Some(field), "{}", issue.message);
    }
    let mut oco = oco_ticket();
    oco.trigger2 = oco.trigger.clone();
    oco.price2 = oco.price.clone();
    let issue = build_confirm(&oco, Some(1523.40), 0).unwrap_err();
    assert_eq!(issue.field, Some(F::TargetTrigger));
}

#[test]
fn a_conversion_bucket_change_reseeds_the_quantity_but_a_reselect_does_not() {
    let mut state = PositionConversionState::new(&conversion_position(), 1)
        .expect("the fixture has convertible buckets");
    let first = state.qty.clone();
    state.select(ConversionField::Quantity, ControlIntent::Focus);
    state.qty = "3".to_string();
    state.select(ConversionField::Bucket, ControlIntent::Option(0));
    assert_eq!(state.qty, "3");
    state.select(ConversionField::Bucket, ControlIntent::Next);
    assert_ne!(state.qty, "3");
    state.select(ConversionField::Bucket, ControlIntent::Previous);
    assert_eq!(state.qty, first);
}

#[test]
fn a_conversion_issue_names_its_row_and_clears_on_that_edit() {
    let mut state = PositionConversionState::new(&conversion_position(), 1)
        .expect("the fixture has convertible buckets");
    state.qty = "0".to_string();
    let issue = build_position_conversion_confirm(&state).unwrap_err();
    assert_eq!(issue.field, Some(ConversionField::Quantity));
    state.issue = Some(issue);
    state.clear_issue(ConversionField::Bucket);
    assert!(state.issue.is_some(), "another row does not clear it");
    state.clear_issue(ConversionField::Quantity);
    assert!(state.issue.is_none());
}

#[test]
fn tab_walks_every_visible_row_in_both_directions() {
    let mut state = ticket();
    let fields = state.visible_fields();
    let mut seen = Vec::new();
    for _ in 0..fields.len() {
        seen.push(state.focused());
        state.move_focus(1);
    }
    assert_eq!(seen, fields, "Tab visits each visible row once, in order");
    assert_eq!(state.focused(), fields[0], "and then wraps");
    state.move_focus(-1);
    assert_eq!(state.focused(), *fields.last().unwrap());
}

#[test]
fn a_long_value_is_stored_whole_however_the_well_shows_it() {
    let mut state = ticket();
    state.qty = String::new();
    for digit in "1234567890".chars() {
        state.edit(TicketField::Quantity, KeyCode::Char(digit));
    }
    assert_eq!(
        state.qty, "1234567890",
        "the buffer is not clipped by a well"
    );
}

#[test]
fn buy_trailing_is_not_a_gtt_focus_target() {
    let mut state = gtt_ticket();
    state.side = Side::Sell;
    state.gtt_trailing = true;
    state.select(TicketField::Side, ControlIntent::Option(0), NO_SEED);
    assert_eq!(state.side, Side::Buy);
    assert!(!state.gtt_trailing, "a BUY trigger cannot trail");
    assert!(!state.visible_fields().contains(&TicketField::Trailing));
}

#[test]
fn oco_defaults_put_each_limit_price_on_its_trigger() {
    let mut state = gtt_ticket();
    state.side = Side::Sell;
    state.price = "100.00".into();
    state.trigger = "105.00".into();
    state.gtt_oco = false;
    state.select(
        TicketField::GttType,
        ControlIntent::Next,
        GttSeed {
            ltp: Some(100.0),
            percent: 5.0,
        },
    );
    assert_eq!(state.trigger, "95.00");
    assert_eq!(state.price, "95.00");
    assert_eq!(state.trigger2, "105.00");
    assert_eq!(state.price2, "105.00");
}

#[test]
fn a_cover_leg_modify_keeps_its_parent_id_in_the_wire_request() {
    let mut state = ticket();
    state.modify = Some(ModifyTarget {
        parent_order_id: Some("parent".into()),
        order_id: "child".into(),
        variety: "co".into(),
        quantity_editable: false,
    });
    let review = build_confirm(&state, None, 1).unwrap();
    let PendingAction::Modify { form, .. } = review.action else {
        panic!("missing modify")
    };
    assert!(form.contains(&("parent_order_id".into(), "parent".into())));
}
