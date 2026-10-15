use kiteconnect::OrderType;
pub const ORDER_TYPES: [OrderType; 4] = [
    OrderType::Market,
    OrderType::Limit,
    OrderType::Sl,
    OrderType::SlM,
];
pub const PRODUCTS: [&str; 4] = ["CNC", "MIS", "NRML", "MTF"];

pub const VALIDITIES: [&str; 3] = ["DAY", "IOC", "TTL"];

pub const TTL_MINUTES: [u16; 11] = [1, 2, 3, 5, 10, 15, 30, 45, 60, 90, 120];

pub fn validity_params(index: usize, ttl_index: usize) -> (&'static str, Option<u16>) {
    match index {
        2 => ("TTL", Some(TTL_MINUTES[ttl_index])),
        1 => ("IOC", None),
        _ => ("DAY", None),
    }
}

pub(in crate::tui::app) fn validity_indices(validity: &str, ttl: Option<i64>) -> (usize, usize) {
    let ttl_index = ttl
        .and_then(|minutes| {
            TTL_MINUTES
                .iter()
                .position(|candidate| i64::from(*candidate) == minutes)
        })
        .unwrap_or(0);
    match validity {
        "IOC" => (1, ttl_index),
        "TTL" => (2, ttl_index),
        _ => (0, ttl_index),
    }
}

pub(in crate::tui::app) fn default_product_index(exchange: &str) -> usize {
    match exchange {
        "NFO" | "BFO" | "CDS" | "BCD" | "MCX" | "NCO" => 2,
        _ => 0,
    }
}

pub fn allowed_products(exchange: &str) -> &'static [&'static str] {
    const EQUITY: [&str; 3] = ["CNC", "MIS", "MTF"];
    const DERIVATIVE: [&str; 2] = ["NRML", "MIS"];
    match exchange {
        "NFO" | "BFO" | "CDS" | "BCD" | "MCX" | "NCO" => &DERIVATIVE,
        _ => &EQUITY,
    }
}

pub fn allowed_gtt_products(exchange: &str) -> &'static [&'static str] {
    const NSE: [&str; 2] = ["CNC", "MTF"];
    const EQUITY: [&str; 1] = ["CNC"];
    const DERIVATIVE: [&str; 1] = ["NRML"];
    match exchange {
        "NSE" => &NSE,
        "NFO" | "BFO" | "CDS" | "BCD" | "MCX" | "NCO" => &DERIVATIVE,
        _ => &EQUITY,
    }
}

pub(in crate::tui::app) fn clamp_product_index(product_idx: usize, allowed: &[&str]) -> usize {
    match PRODUCTS.get(product_idx) {
        Some(label) if allowed.contains(label) => product_idx,
        _ => product_label_index(allowed[0]),
    }
}

pub fn cycle_product(product_idx: usize, allowed: &[&str], delta: isize) -> usize {
    let label = PRODUCTS.get(product_idx).copied().unwrap_or(PRODUCTS[0]);
    let cur = allowed.iter().position(|p| *p == label).unwrap_or(0);
    let next = (cur.cast_signed() + delta).rem_euclid(allowed.len().cast_signed()) as usize;
    product_label_index(allowed[next])
}

pub fn product_label_index(label: &str) -> usize {
    PRODUCTS.iter().position(|p| *p == label).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn allowed_products_never_offer_invalid_per_exchange() {
        use super::{PRODUCTS, allowed_products};

        for exchange in ["NFO", "BFO", "CDS", "BCD", "MCX", "NCO"] {
            let allowed = allowed_products(exchange);
            assert!(allowed.contains(&"NRML"), "{exchange} should offer NRML");
            assert!(!allowed.contains(&"CNC"), "{exchange} must not offer CNC");
            assert!(!allowed.contains(&"MTF"), "{exchange} must not offer MTF");
            for product in allowed {
                assert!(PRODUCTS.contains(product));
            }
        }

        for exchange in ["BSE", "", "SOMETHING_ELSE"] {
            let allowed = allowed_products(exchange);
            assert!(allowed.contains(&"CNC"), "{exchange} should offer CNC");
            assert!(allowed.contains(&"MTF"), "{exchange} should offer MTF");
            assert!(!allowed.contains(&"NRML"), "{exchange} must not offer NRML");
        }
    }

    #[test]
    fn allowed_gtt_products_follow_the_exchange() {
        use super::allowed_gtt_products;
        assert_eq!(allowed_gtt_products("NSE"), ["CNC", "MTF"]);
        for exchange in ["BSE", "", "SOMETHING_ELSE"] {
            assert_eq!(allowed_gtt_products(exchange), ["CNC"], "{exchange}");
        }

        for exchange in ["NFO", "BFO", "CDS", "BCD", "MCX", "NCO"] {
            assert_eq!(allowed_gtt_products(exchange), ["NRML"], "{exchange}");
        }
    }

    proptest::proptest! {



        #[test]
        fn product_toggle_stays_within_allowed_set(
            start in 0usize..super::PRODUCTS.len(),
            deltas in proptest::collection::vec(proptest::sample::select(vec![-1isize, 1]), 0..24),
            derivative in proptest::bool::ANY,
        ) {
            use super::{PRODUCTS, allowed_products, clamp_product_index, cycle_product};
            let exchange = if derivative { "NFO" } else { "NSE" };
            let allowed = allowed_products(exchange);
            let mut idx = clamp_product_index(start, allowed);
            proptest::prop_assert!(allowed.contains(&PRODUCTS[idx]));
            for delta in deltas {
                idx = cycle_product(idx, allowed, delta);
                proptest::prop_assert!(
                    allowed.contains(&PRODUCTS[idx]),
                    "{} invalid for {exchange}",
                    PRODUCTS[idx]
                );
            }
        }
    }
}
