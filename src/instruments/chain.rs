use super::{Instrument, InstrumentStore};
use std::collections::{BTreeMap, BTreeSet};

/// A single tradable option leg (call or put) in a chain row. Owned so the
/// caller isn't lifetime-bound to the store's borrow.
#[derive(Debug, Clone, PartialEq)]
pub struct OptionLeg {
    pub instrument_token: u32,
    pub tradingsymbol: String,
    pub exchange: String,
    pub lot_size: u32,
    pub tick_size: f64,
}

/// One strike row of an option chain: the strike price and its call/put legs
/// (either may be absent if the master lacks that leg for the strike).
#[derive(Debug, Clone, PartialEq)]
pub struct OptionRow {
    pub strike: f64,
    pub call: Option<OptionLeg>,
    pub put: Option<OptionLeg>,
}

/// An option chain for one underlying at one expiry, plus every expiry
/// available for that underlying so callers can offer expiry cycling.
#[derive(Debug, Clone, Default)]
pub struct OptionChain {
    pub underlying: String,
    /// All expiries available for this underlying, ascending (ISO YYYY-MM-DD).
    pub expiries: Vec<String>,
    /// The expiry the `rows` belong to, if any strikes were found.
    pub expiry: Option<String>,
    /// Strike rows for `expiry`, ascending by strike.
    pub rows: Vec<OptionRow>,
}

impl InstrumentStore {
    /// Build the option chain for `underlying` (case-insensitive `name` match).
    ///
    /// Considers only CE/PE instruments in an `-OPT` segment that carry both a
    /// strike and an expiry. Returns the full ascending expiry list plus the
    /// grouped strike rows for the chosen expiry: `expiry` when it exists in the
    /// list, otherwise the nearest expiry on or after `today`,
    /// falling back to the earliest if all have passed. ISO dates sort
    /// chronologically, so the date comparisons are plain string comparisons.
    pub fn option_chain(&self, underlying: &str, expiry: Option<&str>, today: &str) -> OptionChain {
        let options: Vec<&Instrument> = self
            .instruments
            .iter()
            .filter(|inst| {
                inst.name.eq_ignore_ascii_case(underlying)
                    && inst.segment.ends_with("-OPT")
                    && matches!(inst.instrument_type.as_str(), "CE" | "PE")
                    && inst.strike.is_some()
                    && inst.expiry.as_deref().is_some_and(|exp| !exp.is_empty())
            })
            .collect();

        let expiries: Vec<String> = options
            .iter()
            .filter_map(|inst| inst.expiry.as_deref())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(str::to_owned)
            .collect();

        let chosen = match expiry {
            Some(want) if expiries.iter().any(|exp| exp == want) => Some(want.to_string()),
            _ => expiries
                .iter()
                .find(|exp| exp.as_str() >= today)
                .or_else(|| expiries.first())
                .cloned(),
        };

        let rows = match &chosen {
            Some(exp) => {
                let mut by_strike: BTreeMap<i64, OptionRow> = BTreeMap::new();
                for inst in options
                    .iter()
                    .filter(|inst| inst.expiry.as_deref() == Some(exp.as_str()))
                {
                    let Some(strike) = inst.strike else {
                        continue;
                    };
                    #[expect(
                        clippy::cast_possible_truncation,
                        reason = "rounded, saturating paise keys are the grouping contract"
                    )]
                    let key = (strike * 100.0).round() as i64;
                    let row = by_strike.entry(key).or_insert_with(|| OptionRow {
                        strike,
                        call: None,
                        put: None,
                    });
                    let leg = OptionLeg {
                        instrument_token: inst.instrument_token,
                        tradingsymbol: inst.tradingsymbol.clone(),
                        exchange: inst.exchange.clone(),
                        lot_size: inst.lot_size,
                        tick_size: inst.tick_size,
                    };
                    match inst.instrument_type.as_str() {
                        "CE" => row.call = Some(leg),
                        "PE" => row.put = Some(leg),
                        _ => {}
                    }
                }
                by_strike.into_values().collect()
            }
            None => Vec::new(),
        };

        OptionChain {
            underlying: underlying.to_string(),
            expiries,
            expiry: chosen,
            rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{InstrumentStore, OptionChain};
    use proptest::prelude::*;
    use std::collections::{BTreeSet, HashMap};

    fn inst(token: u32, exchange: &str, symbol: &str, name: &str) -> super::Instrument {
        super::Instrument {
            instrument_token: token,
            tradingsymbol: symbol.to_string(),
            name: name.to_string(),
            exchange: exchange.to_string(),
            expiry: None,
            strike: None,
            tick_size: 0.05,
            lot_size: 1,
            instrument_type: "EQ".to_string(),
            segment: exchange.to_string(),
            freeze_quantity: None,
            max_order_quantity: None,
        }
    }
    /// Build an option instrument on top of the base equity fixture.
    fn opt(token: u32, symbol: &str, itype: &str, strike: f64, expiry: &str) -> super::Instrument {
        let mut inst = inst(token, "NFO", symbol, "NIFTY");
        inst.segment = "NFO-OPT".to_string();
        inst.instrument_type = itype.to_string();
        inst.strike = Some(strike);
        inst.expiry = Some(expiry.to_string());
        inst.lot_size = 65;
        inst
    }

    #[test]
    fn option_chain_groups_by_strike_and_lists_expiries() {
        let store = InstrumentStore::new(vec![
            opt(1, "NIFTY2671424000CE", "CE", 24000.0, "2026-07-14"),
            opt(2, "NIFTY2671424000PE", "PE", 24000.0, "2026-07-14"),
            opt(3, "NIFTY2671424100CE", "CE", 24100.0, "2026-07-14"),
            opt(4, "NIFTY2671424100PE", "PE", 24100.0, "2026-07-14"),
            opt(5, "NIFTY2672124000CE", "CE", 24000.0, "2026-07-21"),
            opt(6, "NIFTY2672124000PE", "PE", 24000.0, "2026-07-21"),
            inst(7, "NFO", "NIFTY26JULFUT", "NIFTY"),
        ]);

        let chain = store.option_chain("nifty", None, "2026-07-10");
        assert_eq!(
            chain.expiries,
            vec!["2026-07-14".to_string(), "2026-07-21".to_string()]
        );
        assert_eq!(chain.expiry.as_deref(), Some("2026-07-14"));
        assert_eq!(chain.rows.len(), 2);
        assert_eq!(chain.rows[0].strike.to_bits(), 24000.0_f64.to_bits());
        assert_eq!(chain.rows[0].call.as_ref().unwrap().instrument_token, 1);
        assert_eq!(chain.rows[0].put.as_ref().unwrap().instrument_token, 2);
        assert_eq!(chain.rows[1].strike.to_bits(), 24100.0_f64.to_bits());
        assert_eq!(chain.rows[1].call.as_ref().unwrap().instrument_token, 3);

        let far = store.option_chain("NIFTY", Some("2026-07-21"), "2026-07-10");
        assert_eq!(far.rows.len(), 1);
        assert_eq!(far.rows[0].call.as_ref().unwrap().instrument_token, 5);
        assert_eq!(far.rows[0].put.as_ref().unwrap().instrument_token, 6);

        let past = store.option_chain("NIFTY", None, "2027-01-01");
        assert_eq!(past.expiry.as_deref(), Some("2026-07-14"));

        let none = store.option_chain("BANKNIFTY", None, "2026-07-10");
        assert_eq!(none.expiries.len(), 0);
        assert_eq!(none.rows.len(), 0);
        assert_eq!(none.expiry, None);
    }

    #[test]
    fn option_chain_builds_for_stock_underlying() {
        let mut infy_ce = opt(101, "INFY26JUL1600CE", "CE", 1600.0, "2026-07-30");
        infy_ce.name = "INFY".to_string();
        let mut put = opt(102, "INFY26JUL1600PE", "PE", 1600.0, "2026-07-30");
        put.name = "INFY".to_string();
        let store =
            InstrumentStore::new(vec![inst(408_065, "NSE", "INFY", "INFOSYS"), infy_ce, put]);

        let chain = store.option_chain("infy", None, "2026-07-10");
        assert_eq!(chain.underlying, "infy");
        assert_eq!(chain.expiries, vec!["2026-07-30".to_string()]);
        assert_eq!(chain.expiry.as_deref(), Some("2026-07-30"));
        assert_eq!(chain.rows.len(), 1);
        assert_eq!(chain.rows[0].strike.to_bits(), 1600.0_f64.to_bits());
        assert_eq!(chain.rows[0].call.as_ref().unwrap().instrument_token, 101);
        assert_eq!(chain.rows[0].put.as_ref().unwrap().instrument_token, 102);
    }

    const TODAY: &str = "2026-07-10";
    const UNDERLYING: &str = "NIFTY";

    /// The varying fields of one generated instrument; the rest are filled with
    /// the `inst` fixture defaults when built into a master.
    #[derive(Debug, Clone)]
    struct InstParts {
        name: &'static str,
        itype: &'static str,
        seg: &'static str,
        strike: Option<f64>,
        expiry: Option<String>,
    }

    fn arb_parts() -> impl Strategy<Value = InstParts> {
        let name = prop_oneof![
            Just("NIFTY"),
            Just("nifty"),
            Just("BANKNIFTY"),
            Just("INFY"),
        ];
        let itype = prop_oneof![Just("CE"), Just("PE"), Just("FUT"), Just("EQ")];
        let seg = prop_oneof![Just("NFO-OPT"), Just("NFO-FUT"), Just("NSE")];
        let strike = prop_oneof![
            Just(None),
            Just(Some(24000.0)),
            Just(Some(24000.05)),
            Just(Some(24100.0)),
            Just(Some(24500.0)),
            Just(Some(25000.0)),
        ];
        let expiry = prop_oneof![
            Just(None),
            Just(Some(String::new())),
            Just(Some(TODAY.to_string())),
            Just(Some("2026-07-14".to_string())),
            Just(Some("2026-06-30".to_string())),
            Just(Some("2026-07-21".to_string())),
            Just(Some("2026-08-28".to_string())),
        ];
        (name, itype, seg, strike, expiry).prop_map(|(name, itype, seg, strike, expiry)| {
            InstParts {
                name,
                itype,
                seg,
                strike,
                expiry,
            }
        })
    }

    /// Build a master from generated parts, assigning unique tokens so each leg
    /// can be traced back to exactly one instrument.
    fn build_master(parts: &[InstParts]) -> Vec<super::Instrument> {
        parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let exch = if p.seg == "NSE" { "NSE" } else { "NFO" };
                let mut it = inst(
                    1000 + u32::try_from(i).unwrap(),
                    exch,
                    &format!("SYM{i}"),
                    p.name,
                );
                it.segment = p.seg.to_string();
                it.instrument_type = p.itype.to_string();
                it.strike = p.strike;
                it.expiry = p.expiry.clone();
                it.lot_size = 25 + u32::try_from(i).unwrap();
                it.tick_size = 0.05 * f64::from(u32::try_from(1 + i % 3).unwrap());
                it
            })
            .collect()
    }

    fn paise(strike: f64) -> i64 {
        #[expect(
            clippy::cast_possible_truncation,
            reason = "oracle preserves rounded, saturating paise keys"
        )]
        let key = (strike * 100.0).round() as i64;
        key
    }

    /// The set of instruments `option_chain` should consider. This mirrors the
    /// impl's filter, so the expiry-set equality it feeds is only a
    /// consistency check, not an independent one. The bug-catching power comes
    /// from the checks that run against the *actual output*, per-leg field
    /// tracing and the completeness assertion, which don't depend on this
    /// oracle being right.
    fn valid_legs(master: &[super::Instrument]) -> Vec<&super::Instrument> {
        master
            .iter()
            .filter(|i| {
                i.name.eq_ignore_ascii_case(UNDERLYING)
                    && i.segment.ends_with("-OPT")
                    && matches!(i.instrument_type.as_str(), "CE" | "PE")
                    && i.strike.is_some()
                    && i.expiry.as_deref().is_some_and(|e| !e.is_empty())
            })
            .collect()
    }

    /// Order-independent fingerprint of a chain's shape: expiries, chosen
    /// expiry, and per-strike (key, has-call, has-put). Independent of which
    /// duplicate token wins a tie, so it's stable under input reordering.
    type ChainSig = (Vec<String>, Option<String>, Vec<(i64, bool, bool)>);

    fn signature(chain: &OptionChain) -> ChainSig {
        (
            chain.expiries.clone(),
            chain.expiry.clone(),
            chain
                .rows
                .iter()
                .map(|r| (paise(r.strike), r.call.is_some(), r.put.is_some()))
                .collect(),
        )
    }

    proptest! {
        /// Structural invariants that must hold for any master.
        #[test]
        fn option_chain_holds_invariants(parts in prop::collection::vec(arb_parts(), 0..40)) {
            let master = build_master(&parts);
            let chain = InstrumentStore::new(master.clone()).option_chain(UNDERLYING, None, TODAY);
            let by_token: HashMap<u32, &super::Instrument> =
                master.iter().map(|i| (i.instrument_token, i)).collect();
            let legs = valid_legs(&master);

            prop_assert_eq!(chain.underlying.as_str(), UNDERLYING);

            let expected_expiries: Vec<String> = legs
                .iter()
                .filter_map(|i| i.expiry.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            prop_assert_eq!(&chain.expiries, &expected_expiries);
            for w in chain.expiries.windows(2) {
                prop_assert!(w[0] < w[1], "expiries not strictly ascending");
            }

            let expected_expiry = expected_expiries
                .iter()
                .filter(|e| e.as_str() >= TODAY)
                .min()
                .or_else(|| expected_expiries.iter().min())
                .cloned();
            prop_assert_eq!(&chain.expiry, &expected_expiry);

            match &chain.expiry {
                Some(exp) => {
                    let has = legs.iter().any(|i| i.expiry.as_deref() == Some(exp.as_str()));
                    prop_assert_eq!(chain.rows.is_empty(), !has);
                }
                None => prop_assert!(chain.rows.is_empty()),
            }

            for w in chain.rows.windows(2) {
                prop_assert!(w[0].strike < w[1].strike, "rows not strictly ascending by strike");
            }
            for row in &chain.rows {
                prop_assert!(row.call.is_some() || row.put.is_some(), "empty row");
                for (leg, want) in [(&row.call, "CE"), (&row.put, "PE")] {
                    if let Some(leg) = leg {
                        let src = by_token[&leg.instrument_token];
                        prop_assert!(src.name.eq_ignore_ascii_case(UNDERLYING));
                        prop_assert!(src.segment.ends_with("-OPT"));
                        prop_assert_eq!(src.instrument_type.as_str(), want);
                        prop_assert_eq!(src.expiry.as_deref(), chain.expiry.as_deref());
                        prop_assert_eq!(paise(src.strike.unwrap()), paise(row.strike));
                        prop_assert_eq!(&leg.tradingsymbol, &src.tradingsymbol);
                        prop_assert_eq!(&leg.exchange, &src.exchange);
                        prop_assert_eq!(leg.lot_size, src.lot_size);
                        prop_assert_eq!(leg.tick_size.to_bits(), src.tick_size.to_bits());
                    }
                }
            }

            if let Some(exp) = chain.expiry.as_deref() {
                let mut want_sides: HashMap<i64, (bool, bool)> = HashMap::new();
                for l in legs.iter().filter(|l| l.expiry.as_deref() == Some(exp)) {
                    let entry = want_sides.entry(paise(l.strike.unwrap())).or_default();
                    match l.instrument_type.as_str() {
                        "CE" => entry.0 = true,
                        "PE" => entry.1 = true,
                        _ => {}
                    }
                }
                let rows_by_strike: HashMap<i64, &super::OptionRow> =
                    chain.rows.iter().map(|r| (paise(r.strike), r)).collect();
                prop_assert_eq!(chain.rows.len(), want_sides.len(), "row count mismatch");
                for (key, (want_call, want_put)) in &want_sides {
                    let row = rows_by_strike
                        .get(key)
                        .ok_or_else(|| TestCaseError::fail(format!("strike {key} missing from rows")))?;
                    if *want_call {
                        prop_assert!(row.call.is_some(), "call dropped at strike {}", key);
                    }
                    if *want_put {
                        prop_assert!(row.put.is_some(), "put dropped at strike {}", key);
                    }
                }
            }
        }

        /// The chain must not depend on the master's ordering: reversing or
        /// rotating the input yields the same shape.
        #[test]
        fn option_chain_is_order_independent(parts in prop::collection::vec(arb_parts(), 0..40)) {
            let master = build_master(&parts);
            let base = InstrumentStore::new(master.clone()).option_chain(UNDERLYING, None, TODAY);

            let mut reversed = master.clone();
            reversed.reverse();
            let rev = InstrumentStore::new(reversed).option_chain(UNDERLYING, None, TODAY);
            prop_assert_eq!(signature(&base), signature(&rev));

            if master.len() > 1 {
                let mut rotated = master.clone();
                rotated.rotate_left(1);
                let rot = InstrumentStore::new(rotated).option_chain(UNDERLYING, None, TODAY);
                prop_assert_eq!(signature(&base), signature(&rot));
            }
        }

        /// Any listed expiry, requested explicitly, is honored; an unknown
        /// expiry falls back to the default selection.
        #[test]
        fn option_chain_explicit_expiry_is_honored(parts in prop::collection::vec(arb_parts(), 0..40)) {
            let store = InstrumentStore::new(build_master(&parts));
            let base = store.option_chain(UNDERLYING, None, TODAY);

            for exp in &base.expiries {
                let chosen = store.option_chain(UNDERLYING, Some(exp), TODAY);
                prop_assert_eq!(chosen.expiry.as_deref(), Some(exp.as_str()));
                prop_assert!(!chosen.rows.is_empty());
            }

            let bogus = store.option_chain(UNDERLYING, Some("1999-01-01"), TODAY);
            prop_assert_eq!(bogus.expiry, base.expiry);
            prop_assert_eq!(bogus.rows.len(), base.rows.len());
        }
    }
}
