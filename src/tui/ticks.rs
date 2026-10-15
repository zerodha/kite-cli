use kiteconnect::ticker::Tick;

pub fn discard_zero_prices(tick: &mut Tick) {
    for price in [
        &mut tick.last_price,
        &mut tick.open,
        &mut tick.high,
        &mut tick.low,
        &mut tick.close,
    ] {
        if *price == Some(0.0) {
            *price = None;
        }
    }
}

pub fn merge(tick: &mut Tick, mut new: Tick) {
    discard_zero_prices(&mut new);
    tick.is_index = new.is_index;
    macro_rules! take {
        ($($field:ident),+) => { $( if new.$field.is_some() { tick.$field = new.$field; } )+ };
    }
    take!(
        last_price,
        last_quantity,
        average_price,
        volume,
        buy_quantity,
        sell_quantity,
        open,
        high,
        low,
        close,
        oi,
        oi_day_high,
        oi_day_low,
        last_trade_time,
        exchange_time,
        depth
    );
}

pub fn change(tick: &Tick) -> Option<f64> {
    let (last, close) = (tick.last_price?, tick.close?);
    if close == 0.0 {
        return None;
    }
    Some(last - close)
}

pub fn change_percent(tick: &Tick) -> Option<f64> {
    Some(change(tick)? / tick.close? * 100.0)
}

#[cfg(test)]
mod tests {
    use super::{Tick, change, merge};

    #[test]
    fn merge_ignores_all_zero_snapshot_prices() {
        let mut good = Tick::default();
        good.last_price = Some(100.0);
        good.open = Some(98.0);
        good.high = Some(102.0);
        good.low = Some(97.0);
        good.close = Some(99.0);
        let mut stale_snapshot = Tick::default();
        stale_snapshot.last_price = Some(0.0);
        stale_snapshot.open = Some(0.0);
        stale_snapshot.high = Some(0.0);
        stale_snapshot.low = Some(0.0);
        stale_snapshot.close = Some(0.0);
        merge(&mut good, stale_snapshot);
        assert_eq!(good.last_price, Some(100.0));
        assert_eq!(good.open, Some(98.0));
        assert_eq!(good.high, Some(102.0));
        assert_eq!(good.low, Some(97.0));
        assert_eq!(good.close, Some(99.0));

        let mut fresh = Tick::default();
        fresh.last_price = Some(105.0);
        fresh.high = Some(106.0);
        merge(&mut good, fresh);
        assert_eq!(good.last_price, Some(105.0));
        assert_eq!(good.high, Some(106.0));
    }

    #[test]
    fn merge_keeps_fields_missing_from_ltp_tick() {
        let mut base = Tick::default();
        base.instrument_token = 1;
        base.last_price = Some(10.0);
        base.close = Some(9.0);
        merge(&mut base, {
            let mut value = Tick::default();
            value.instrument_token = 1;
            value.last_price = Some(11.0);
            value
        });
        assert_eq!(base.last_price, Some(11.0));
        assert_eq!(base.close, Some(9.0));
        assert_eq!(change(&base), Some(2.0));
    }
}
