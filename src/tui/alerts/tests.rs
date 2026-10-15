use super::*;
use kiteconnect::ticker::Tick;

#[test]
fn attributes_keep_kite_web_wire_labels_and_tick_values_together() {
    let mut tick = Tick::default();
    tick.last_price = Some(120.0);
    tick.high = Some(130.0);
    tick.low = Some(90.0);
    tick.open = Some(100.0);
    tick.close = Some(100.0);
    tick.last_quantity = Some(7);
    tick.average_price = Some(115.0);
    tick.volume = Some(1_000);
    tick.buy_quantity = Some(800);
    tick.sell_quantity = Some(600);
    tick.oi = Some(500);
    tick.oi_day_high = Some(550);
    tick.oi_day_low = Some(450);
    let expected = [
        (
            Attribute::LastTradedPrice,
            "LastTradedPrice",
            "Last price",
            120.0,
        ),
        (Attribute::HighPrice, "HighPrice", "High price", 130.0),
        (Attribute::LowPrice, "LowPrice", "Low price", 90.0),
        (Attribute::OpenPrice, "OpenPrice", "Open price", 100.0),
        (Attribute::ClosePrice, "ClosePrice", "Close price", 100.0),
        (Attribute::DayChange, "DayChange", "Day change", 20.0),
        (
            Attribute::DayChangePercent,
            "DayChangePercent",
            "Day change %",
            20.0,
        ),
        (
            Attribute::IntraDayChange,
            "IntraDayChange",
            "Intraday change",
            20.0,
        ),
        (
            Attribute::IntraDayChangePercent,
            "IntraDayChangePercent",
            "Intraday change %",
            20.0,
        ),
        (
            Attribute::LastTradedQuantity,
            "LastTradedQuantity",
            "Last traded qty.",
            7.0,
        ),
        (
            Attribute::AverageTradePrice,
            "AverageTradePrice",
            "Avg. traded price",
            115.0,
        ),
        (Attribute::VolumeTraded, "VolumeTraded", "Volume", 1_000.0),
        (
            Attribute::TotalBuyQuantity,
            "TotalBuyQuantity",
            "Total buy qty.",
            800.0,
        ),
        (
            Attribute::TotalSellQuantity,
            "TotalSellQuantity",
            "Total sell qty.",
            600.0,
        ),
        (Attribute::OpenInterest, "OpenInterest", "OI", 500.0),
        (
            Attribute::OpenInterestDayHigh,
            "OpenInterestDayHigh",
            "OI day high",
            550.0,
        ),
        (
            Attribute::OpenInterestDayLow,
            "OpenInterestDayLow",
            "OI day low",
            450.0,
        ),
    ];
    assert_eq!(Attribute::ALL.len(), expected.len());
    for (attribute, wire, label, value) in expected {
        assert_eq!(attribute.wire(), wire);
        assert_eq!(attribute.label(), label);
        assert_eq!(Attribute::from_wire(wire), Some(attribute));
        assert_eq!(attribute.current_value(&tick), Some(value));
    }
    assert_eq!(Attribute::label_for_wire("FutureField"), "FutureField");
}

#[test]
fn operators_round_trip_without_losing_strictness() {
    for (operator, wire) in [
        (Operator::Equal, "=="),
        (Operator::GreaterThan, ">"),
        (Operator::GreaterThanOrEqual, ">="),
        (Operator::LessThan, "<"),
        (Operator::LessThanOrEqual, "<="),
    ] {
        assert_eq!(operator.wire(), wire);
        assert_eq!(Operator::from_wire(wire), Some(operator));
    }
    assert_eq!(Operator::from_wire("!="), None);
    assert!(Operator::GreaterThan.matches(101.0, 100.0));
    assert!(!Operator::GreaterThan.matches(100.0, 100.0));
    assert!(Operator::LessThan.matches(99.0, 100.0));
    assert!(!Operator::LessThan.matches(100.0, 100.0));
}

#[test]
fn selected_attribute_validation_uses_available_value_without_blocking_absence() {
    assert!(
        validate_level(
            Attribute::HighPrice,
            Operator::GreaterThanOrEqual,
            131.0,
            Some(130.0)
        )
        .is_ok()
    );
    assert!(
        validate_level(
            Attribute::HighPrice,
            Operator::GreaterThanOrEqual,
            130.0,
            Some(130.0)
        )
        .is_err()
    );
    assert!(validate_level(Attribute::DayChange, Operator::LessThan, -1.0, None).is_ok());
    assert!(
        validate_level(
            Attribute::DayChangePercent,
            Operator::LessThanOrEqual,
            2.0,
            None
        )
        .is_ok()
    );
}

#[test]
fn index_alerts_match_the_masters_segment_not_its_exchange() {
    assert!(lhs_matches(
        "INDICES", "NIFTY 50", "NSE", "INDICES", "NIFTY 50"
    ));
    assert!(lhs_matches("INDICES", "SENSEX", "BSE", "INDICES", "SENSEX"));
    assert!(!lhs_matches("INDICES", "INFY", "NSE", "NSE", "INFY"));
}

#[test]
fn non_index_alerts_match_on_exchange() {
    assert!(lhs_matches("NSE", "INFY", "NSE", "NSE", "INFY"));
    assert!(!lhs_matches("NSE", "INFY", "BSE", "BSE", "INFY"));
    assert!(lhs_matches("nse", "infy", "NSE", "NSE", "INFY"));
    assert!(!lhs_matches("NSE", "INFY", "NSE", "NSE", "TCS"));
}

#[test]
fn index_shortlists_every_alert_naming_a_symbol() {
    let index = AlertIndex::build(["INFY", "TCS", "INFY", "NIFTY 50"].into_iter());
    assert_eq!(index.candidates("INFY"), [0, 2]);
    assert_eq!(index.candidates("TCS"), [1]);
    assert_eq!(index.candidates("NIFTY 50"), [3]);
    assert_eq!(index.candidates("SBIN").len(), 0);
    assert_eq!(AlertIndex::default().candidates("INFY").len(), 0);
}

#[test]
fn index_lookup_ignores_case_on_both_sides() {
    let index = AlertIndex::build(["infy", "Nifty 50"].into_iter());
    assert_eq!(index.candidates("INFY"), [0]);
    assert_eq!(index.candidates("iNfY"), [0]);
    assert_eq!(index.candidates("NIFTY 50"), [1]);
    assert_eq!(index.candidates("NIFTY50").len(), 0);
    assert!(!lhs_matches("NSE", "NIFTY50", "NSE", "NSE", "NIFTY 50"));
}

#[test]
fn index_finds_every_symbol_it_was_built_from() {
    let symbols = [
        "zeel", "A_B", "AB", "ab", "NIFTY 50", "NIFTY50", "TCS", "tcs",
    ];
    let index = AlertIndex::build(symbols.into_iter());
    for (idx, symbol) in symbols.iter().enumerate() {
        assert!(
            index.candidates(symbol).contains(&idx),
            "{symbol} (position {idx}) was not found"
        );
    }
    assert_eq!(index.candidates("TCS"), [6, 7]);
}

#[test]
fn params_build_a_typed_constant_condition() {
    let p = params(
        "INFY",
        "NSE",
        "INFY",
        Attribute::LastTradedPrice,
        Operator::GreaterThanOrEqual,
        1520.0,
        true,
    );
    assert_eq!(p.name, "INFY");
    assert_eq!(p.lhs_exchange, "NSE");
    assert_eq!(p.lhs_tradingsymbol, "INFY");
    assert_eq!(p.lhs_attribute, "LastTradedPrice");
    assert_eq!(p.operator.to_string(), ">=");
    assert_eq!(p.rhs_constant.to_bits(), 1520.0_f64.to_bits());
    assert_eq!(p.status, Some(AlertStatus::Enabled));
    assert_eq!(
        params(
            "INFY",
            "NSE",
            "INFY",
            Attribute::LowPrice,
            Operator::LessThanOrEqual,
            1500.0,
            false,
        )
        .status,
        Some(AlertStatus::Disabled)
    );
}

fn item_params() -> BasketOrderParams {
    BasketOrderParams::new(ATO_VARIETY, "BUY", "CNC", "LIMIT", "DAY", 10, 100.0)
}

fn fetched(id: i64, symbol: &str, weight: i64) -> FetchedBasketItem {
    {
        let mut row = crate::tui::data::test_models::basket_item();
        row.id = Some(id);
        row.exchange = "NSE".to_string();
        row.tradingsymbol = symbol.to_string();
        row.instrument_token = Some(408_065);
        row.weight = weight;
        row.params = serde_json::to_value(item_params()).expect("params serialize");
        row
    }
}

fn operations(write: Option<&BasketWrite>) -> &[BasketOperation] {
    match write {
        Some(BasketWrite::Modify(modify)) => &modify.items,
        Some(BasketWrite::Create(create)) => &create.items,
        Some(other) => panic!("unexpected basket write {other:?}"),
        None => &[],
    }
}

#[test]
fn ato_is_refused_for_the_exchanges_and_indices_kite_web_excludes() {
    for exchange in ["MCX", "NCO", "CDS", "BCD", "mcx"] {
        assert!(
            ato_ineligible_reason(exchange, "GOLDM").is_some(),
            "{exchange} must not offer ATO"
        );
    }
    for symbol in ["GIFT NIFTY", "US500", "HANGSENG", "us10yryield"] {
        assert!(
            ato_ineligible_reason("INDICES", symbol).is_some(),
            "{symbol} must not offer ATO"
        );
    }
    assert_eq!(ato_ineligible_reason("NSE", "INFY"), None);
    assert_eq!(ato_ineligible_reason("NFO", "INFY25SEPFUT"), None);
    assert_eq!(ato_ineligible_reason("INDICES", "NIFTY 50"), None);
}

#[test]
fn a_new_basket_inserts_every_order_with_one_based_weights() {
    let mut draft = AtoBasketDraft::new();
    for symbol in ["INFY", "TCS", "SBIN"] {
        draft
            .push(
                "NSE".to_string(),
                symbol.to_string(),
                408_065,
                item_params(),
            )
            .expect("room for three orders");
    }
    let write = draft.write(true);
    let Some(BasketWrite::Create(create)) = &write else {
        panic!("a new ATO must send a create basket, got {write:?}");
    };
    assert_eq!(create.name, "alerts-basket");
    assert_eq!(create.basket_type, "alert");
    assert_eq!(create.items.len(), 3);
    for (index, operation) in create.items.iter().enumerate() {
        let BasketOperation::Insert {
            tradingsymbol,
            weight,
            ..
        } = operation
        else {
            panic!("a creation only inserts, got {operation:?}");
        };
        assert_eq!(
            *weight,
            u32::try_from(index).expect("fixture index fits u32") + 1,
            "{tradingsymbol}"
        );
    }
}

#[test]
fn an_untouched_draft_writes_no_basket() {
    let draft = AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 10_000)]);
    assert!(draft.is_unchanged());
    assert_eq!(draft.write(false), None);
    assert_eq!(draft.removed().len(), 0);
}

#[test]
fn editing_one_order_updates_only_that_one() {
    let mut draft = AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 1), fetched(2, "TCS", 2)]);
    draft.replace(
        0,
        BasketOrderParams::new(ATO_VARIETY, "BUY", "CNC", "LIMIT", "DAY", 25, 100.0),
    );
    let write = draft.write(false);
    let operations = operations(write.as_ref());
    assert_eq!(operations.len(), 1, "only the edited order changed");
    let BasketOperation::Update {
        id,
        weight,
        params: BasketItemParams::Authored(params),
        ..
    } = &operations[0]
    else {
        panic!("expected one authored update, got {:?}", operations[0]);
    };
    assert_eq!(*id, 1);
    assert_eq!(*weight, 1);
    assert_eq!(params.quantity, 25);
    assert_eq!(params.price.to_bits(), 100.0_f64.to_bits());
    assert_eq!(params.product, "CNC");
    assert_eq!(params.validity, "DAY");
}

#[test]
fn removing_an_order_deletes_it_by_server_id() {
    let mut draft = AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 1), fetched(2, "TCS", 2)]);
    draft.remove(0);
    let write = draft.write(false);
    let operations = operations(write.as_ref());
    assert!(
        operations.contains(&BasketOperation::Delete { id: 1 }),
        "the removed order must be deleted: {operations:?}"
    );
    assert_eq!(draft.removed().len(), 1);
    assert_eq!(draft.removed()[0].label(), "NSE:INFY");
}

#[test]
fn adding_then_removing_an_order_writes_nothing_for_it() {
    let mut draft = AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 1)]);
    draft
        .push("NSE".to_string(), "TCS".to_string(), 408_065, item_params())
        .expect("room");
    draft.remove(1);
    assert!(draft.is_unchanged());
    assert_eq!(draft.write(false), None);
}

#[test]
fn reordering_renumbers_every_surviving_order() {
    let mut draft =
        AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 10_000), fetched(2, "TCS", 20_000)]);
    assert_eq!(draft.move_item(1, -1), 0, "TCS moves to the front");
    let write = draft.write(false);
    let operations = operations(write.as_ref());
    assert_eq!(
        operations.len(),
        2,
        "both positions changed: {operations:?}"
    );
    let weight_of = |symbol: &str| {
        operations.iter().find_map(|operation| match operation {
            BasketOperation::Update {
                tradingsymbol,
                weight,
                ..
            } if tradingsymbol == symbol => Some(*weight),
            _ => None,
        })
    };
    assert_eq!(weight_of("TCS"), Some(1));
    assert_eq!(weight_of("INFY"), Some(2));
}

#[test]
fn an_insert_cannot_be_sorted_ahead_of_an_untouched_order() {
    let mut draft = AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 10_000)]);
    draft
        .push("NSE".to_string(), "TCS".to_string(), 408_065, item_params())
        .expect("room");
    let write = draft.write(false);
    let operations = operations(write.as_ref());
    let mut weights: Vec<(String, u32)> = operations
        .iter()
        .map(|operation| match operation {
            BasketOperation::Insert {
                tradingsymbol,
                weight,
                ..
            }
            | BasketOperation::Update {
                tradingsymbol,
                weight,
                ..
            } => (tradingsymbol.clone(), *weight),
            other => panic!("unexpected {other:?}"),
        })
        .collect();
    weights.sort_by_key(|(_, weight)| *weight);
    assert_eq!(
        weights,
        vec![("INFY".to_string(), 1), ("TCS".to_string(), 2)],
        "the kept order must be renumbered so the new one lands after it"
    );
}

#[test]
fn an_already_consecutive_basket_does_not_renumber_untouched_orders() {
    let mut draft = AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 1), fetched(2, "TCS", 2)]);
    draft
        .push(
            "NSE".to_string(),
            "SBIN".to_string(),
            408_065,
            item_params(),
        )
        .expect("room");
    let write = draft.write(false);
    let operations = operations(write.as_ref());
    assert_eq!(
        operations.len(),
        1,
        "only the new order needs an operation: {operations:?}"
    );
    assert!(matches!(
        &operations[0],
        BasketOperation::Insert { weight: 3, .. }
    ));
}

#[test]
fn duplicating_an_order_inserts_a_copy_without_the_server_id() {
    let mut draft = AtoBasketDraft::from_fetched(&[fetched(1, "INFY", 1)]);
    draft.duplicate(0).expect("room");
    assert_eq!(draft.len(), 2);
    assert_eq!(draft.items()[0].key, DraftKey::Server(1));
    assert!(matches!(draft.items()[1].key, DraftKey::Local(_)));
    let write = draft.write(false);
    let operations = operations(write.as_ref());
    assert_eq!(operations.len(), 1, "only the copy is new: {operations:?}");
    assert!(matches!(operations[0], BasketOperation::Insert { .. }));
}

#[test]
fn a_basket_holds_at_most_twenty_orders() {
    let mut draft = AtoBasketDraft::new();
    for index in 0..MAX_BASKET_ITEMS {
        draft
            .push(
                "NSE".to_string(),
                format!("SYM{index}"),
                408_065,
                item_params(),
            )
            .unwrap_or_else(|err| panic!("order {index} must fit: {err}"));
    }
    assert_eq!(draft.len(), 20);
    assert!(draft.is_full());
    assert!(
        draft
            .push("NSE".to_string(), "X".to_string(), 1, item_params())
            .is_err()
    );
    assert!(draft.duplicate(0).is_err());
    assert_eq!(draft.len(), 20, "a refused add must not change the basket");
}

#[test]
fn an_unauthorable_item_is_preserved_rather_than_normalized() {
    let advanced = serde_json::json!({
        "order_type": "SL-M",
        "quantity": 10,
        "price": 0,
        "disclosed_quantity": 4
    });
    let mut items = vec![fetched(1, "INFY", 1), fetched(2, "GOLDBEES", 2)];
    items[1].params = advanced.clone();
    let mut draft = AtoBasketDraft::from_fetched(&items);
    assert!(draft.items()[0].editable().is_some());
    assert!(
        draft.items()[1].editable().is_none(),
        "an unauthorable item must not be offered for editing"
    );
    assert_eq!(draft.items()[1].summary(), "Kept as saved");

    draft.move_item(1, -1);
    let write = draft.write(false);
    let operations = operations(write.as_ref());
    let preserved = operations
        .iter()
        .find_map(|operation| match operation {
            BasketOperation::Update {
                id: 2,
                params: BasketItemParams::Preserved(raw),
                weight,
                ..
            } => Some((raw.clone(), *weight)),
            _ => None,
        })
        .expect("the preserved item is updated by its move");
    assert_eq!(preserved.0, advanced, "params must go back byte-for-byte");
    assert_eq!(preserved.1, 1);
}

#[test]
fn a_valid_item_produces_complete_params() {
    let params = validate_item(
        "NSE", 408_065, "BUY", "CNC", "LIMIT", "DAY", "", "10", "1520.05", "", 1, 0.05, None,
    )
    .expect("a plain limit order is valid");
    assert_eq!(params.variety, "regular");
    assert_eq!(params.quantity, 10);
    assert_eq!(params.price.to_bits(), 1520.05_f64.to_bits());
    assert_eq!(
        (params.trigger_price).to_bits(),
        0.0_f64.to_bits(),
        "a non-SL order sends trigger 0"
    );
    assert_eq!(params.validity_ttl, 0);
    assert_eq!(params.disclosed_quantity, 0);
    assert_eq!(params.tags, Vec::<String>::new());

    let market = validate_item(
        "NSE", 408_065, "SELL", "MIS", "MARKET", "IOC", "", "10", "", "", 1, 0.05, None,
    )
    .expect("a market order needs no price");
    assert_eq!(
        (market.price).to_bits(),
        0.0_f64.to_bits(),
        "MARKET sends price 0"
    );
    assert_eq!(market.trigger_price.to_bits(), 0.0_f64.to_bits());
    assert_eq!(market.transaction_type, "SELL");
    assert_eq!(market.validity, "IOC");
}

#[test]
fn item_quantities_prices_and_triggers_are_validated() {
    let item = |quantity: &str, price: &str, lot: u32, freeze: Option<u32>| {
        validate_item(
            "NSE", 408_065, "BUY", "CNC", "LIMIT", "DAY", "", quantity, price, "", lot, 0.05,
            freeze,
        )
    };
    assert!(item("0", "100.00", 1, None).is_err(), "zero quantity");
    assert!(item("-5", "100.00", 1, None).is_err(), "negative quantity");
    assert!(
        item("1.5", "100.00", 1, None).is_err(),
        "fractional quantity"
    );
    assert!(item("", "100.00", 1, None).is_err(), "blank quantity");
    assert!(item("25", "100.00", 15, None).is_err(), "off-lot quantity");
    assert!(item("30", "100.00", 15, None).is_ok(), "two lots");
    assert!(
        item("2000", "100.00", 1, Some(1800)).is_err(),
        "past freeze"
    );
    assert!(item("1800", "100.00", 1, Some(1800)).is_ok(), "at freeze");

    assert!(item("10", "0", 1, None).is_err(), "zero price");
    assert!(item("10", "-1", 1, None).is_err(), "negative price");
    assert!(item("10", "", 1, None).is_err(), "blank price");
    assert!(item("10", "inf", 1, None).is_err(), "infinite price");
    assert!(item("10", "100.03", 1, None).is_err(), "off-tick price");
    assert!(item("10", "100.05", 1, None).is_ok(), "on-tick price");
}

#[test]
fn a_stop_loss_needs_a_trigger_on_the_right_side_of_its_price() {
    let stop = |side: &str, price: &str, trigger: &str| {
        validate_item(
            "NSE", 408_065, side, "MIS", "SL", "DAY", "", "10", price, trigger, 1, 0.05, None,
        )
    };
    assert!(stop("BUY", "101.00", "100.00").is_ok());
    assert!(stop("BUY", "100.00", "100.00").is_ok(), "equal is allowed");
    assert!(stop("BUY", "99.00", "100.00").is_err(), "BUY below trigger");
    assert!(stop("SELL", "99.00", "100.00").is_ok());
    assert!(stop("SELL", "101.00", "100.00").is_err(), "SELL above");
    assert!(stop("BUY", "101.00", "").is_err(), "SL needs a trigger");
    assert!(stop("BUY", "101.00", "100.03").is_err(), "off-tick trigger");

    let limit = validate_item(
        "NSE", 408_065, "BUY", "CNC", "LIMIT", "DAY", "", "10", "101.00", "100.00", 1, 0.05, None,
    )
    .expect("a limit order is valid whatever is in the trigger field");
    assert_eq!(limit.trigger_price.to_bits(), 0.0_f64.to_bits());
}

#[test]
fn ttl_validity_needs_minutes_within_the_services_bounds() {
    let ttl = |validity: &str, minutes: &str| {
        validate_item(
            "NSE", 408_065, "BUY", "CNC", "LIMIT", validity, minutes, "10", "100.00", "", 1, 0.05,
            None,
        )
    };
    assert_eq!(ttl("TTL", "1").expect("lower bound").validity_ttl, 1);
    assert_eq!(ttl("TTL", "180").expect("upper bound").validity_ttl, 180);
    assert!(ttl("TTL", "0").is_err(), "zero minutes");
    assert!(ttl("TTL", "181").is_err(), "past the upper bound");
    assert!(ttl("TTL", "").is_err(), "blank minutes");
    assert_eq!(ttl("DAY", "90").expect("day order").validity_ttl, 0);
}

#[test]
fn item_instruments_and_vocabulary_are_restricted_to_what_this_build_authors() {
    let with = |exchange: &str, token: u32, product: &str, order_type: &str| {
        validate_item(
            "NSE", token, "BUY", product, order_type, "DAY", "", "10", "100.00", "", 1, 0.05, None,
        )
        .map(|_| ())
        .or_else(|err| {
            validate_item(
                exchange, token, "BUY", product, order_type, "DAY", "", "10", "100.00", "", 1,
                0.05, None,
            )
            .map(|_| ())
            .map_err(|_| err)
        })
    };
    assert!(with("NSE", 408_065, "CNC", "LIMIT").is_ok());
    assert!(with("NSE", 0, "CNC", "LIMIT").is_err(), "unresolved symbol");
    assert!(with("NSE", 408_065, "MTF", "LIMIT").is_err(), "MTF");
    assert!(with("NSE", 408_065, "CO", "LIMIT").is_err(), "CO product");
    assert!(with("NSE", 408_065, "CNC", "SL-M").is_err(), "SL-M");

    for exchange in ["MCX", "CDS", "NCO", "BCD"] {
        assert!(
            validate_item(
                exchange, 408_065, "BUY", "CNC", "LIMIT", "DAY", "", "10", "100.00", "", 1, 0.05,
                None,
            )
            .is_err(),
            "{exchange} orders are outside this build's basket editor"
        );
    }
    for (exchange, valid, invalid) in [
        ("NSE", "CNC", "NRML"),
        ("BSE", "CNC", "NRML"),
        ("NFO", "NRML", "CNC"),
        ("BFO", "NRML", "CNC"),
    ] {
        assert_eq!(
            ato_products(exchange)[0],
            valid,
            "{exchange} defaults to its delivery product"
        );
        assert!(
            validate_item(
                exchange, 408_065, "BUY", valid, "LIMIT", "DAY", "", "10", "100.00", "", 1, 0.05,
                None,
            )
            .is_ok(),
            "{exchange} orders must accept {valid}"
        );
        let refused = validate_item(
            exchange, 408_065, "BUY", invalid, "LIMIT", "DAY", "", "10", "100.00", "", 1, 0.05,
            None,
        )
        .expect_err("{exchange} must refuse {invalid}");
        assert_eq!(refused.field, Some(AtoItemFocus::Product));
        assert!(
            validate_item(
                exchange, 408_065, "BUY", "MIS", "LIMIT", "DAY", "", "10", "100.00", "", 1, 0.05,
                None,
            )
            .is_ok(),
            "{exchange} orders must accept MIS"
        );
    }
}

#[test]
fn modelled_keys_are_the_keys_basket_order_params_serializes() {
    let value = serde_json::to_value(BasketOrderParams::default()).expect("params serialize");
    let mut serialized: Vec<&str> = value
        .as_object()
        .expect("params object")
        .keys()
        .map(String::as_str)
        .collect();
    serialized.sort_unstable();
    let mut modelled = MODELLED_KEYS.to_vec();
    modelled.sort_unstable();
    assert_eq!(serialized, modelled);
}

#[test]
fn only_fully_modelled_params_are_treated_as_editable() {
    let complete = serde_json::to_value(item_params()).expect("params serialize");
    assert_eq!(authorable_params(&complete), Some(item_params()));

    let sparse = serde_json::json!({"order_type": "LIMIT", "quantity": 10, "price": 72.22});
    assert_eq!(authorable_params(&sparse), None);

    let mut unknown = complete.clone();
    unknown["market_protection"] = serde_json::json!(1);
    assert_eq!(authorable_params(&unknown), None);

    for (key, value) in [
        ("variety", serde_json::json!("co")),
        ("variety", serde_json::json!("iceberg")),
        ("product", serde_json::json!("MTF")),
        ("order_type", serde_json::json!("SL-M")),
        ("validity", serde_json::json!("GTT")),
        ("transaction_type", serde_json::json!("")),
        ("disclosed_quantity", serde_json::json!(5)),
        ("squareoff", serde_json::json!(1.5)),
        ("stoploss", serde_json::json!(1.5)),
        ("trailing_stoploss", serde_json::json!(1.5)),
        ("gtt", serde_json::json!({"target": 2.0, "stoploss": 0.0})),
        ("tags", serde_json::json!(["t:mine:1"])),
    ] {
        let mut raw = complete.clone();
        raw[key] = value.clone();
        assert_eq!(
            authorable_params(&raw),
            None,
            "{key} = {value} must keep the item read-only"
        );
    }
}

#[test]
fn ato_products_are_the_exchange_products_this_editor_authors() {
    for exchange in ATO_ORDER_EXCHANGES {
        let expected: Vec<&str> = crate::tui::app::allowed_products(exchange)
            .iter()
            .copied()
            .filter(|product| ATO_PRODUCTS.contains(product))
            .collect();
        assert_eq!(ato_products(exchange), expected, "{exchange}");
    }
}

#[test]
fn the_terminal_never_builds_an_ato_alert() {
    let call = kiteconnect::KiteConnect::builder("test-key")
        .build()
        .unwrap()
        .create_alert(params(
            "INFY",
            "NSE",
            "INFY",
            Attribute::LastTradedPrice,
            Operator::GreaterThanOrEqual,
            1520.0,
            true,
        ));
    let kiteconnect::Body::Form(form) = call.request().body() else {
        panic!("an alert is a form");
    };
    assert!(form.iter().any(|(k, v)| k == "type" && v == "simple"));
}
