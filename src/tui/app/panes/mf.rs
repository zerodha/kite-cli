use super::super::{App, cmp_desc};
use crate::tui::data::{MfHolding, fund_display_name};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MfSort {
    #[default]
    Name,
    Value,
    Pnl,
    DayChange,
}

impl MfSort {
    pub const ALL: [Self; 4] = [Self::Name, Self::Value, Self::Pnl, Self::DayChange];

    pub(in crate::tui::app) fn next(self) -> Self {
        match self {
            MfSort::Name => MfSort::Value,
            MfSort::Value => MfSort::Pnl,
            MfSort::Pnl => MfSort::DayChange,
            MfSort::DayChange => MfSort::Name,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MfSort::Name => "name ▲",
            MfSort::Value => "value ▼",
            MfSort::Pnl => "P&L ▼",
            MfSort::DayChange => "day ▼",
        }
    }
}

pub(in crate::tui::app) fn mf_key(holding: &MfHolding) -> String {
    holding.tradingsymbol.clone()
}

pub fn mf_order(holdings: &[MfHolding], mode: MfSort) -> Vec<usize> {
    let mut order: Vec<usize> = (0..holdings.len()).collect();
    if mode == MfSort::Name {
        order.sort_by(|&a, &b| {
            fund_display_name(&holdings[a].tradingsymbol)
                .cmp(fund_display_name(&holdings[b].tradingsymbol))
        });
    } else {
        let key = |h: &MfHolding| -> f64 {
            match mode {
                MfSort::Value => crate::tui::data::mf_current(h),
                MfSort::Pnl => h.pnl,
                MfSort::DayChange => h.day_change.unwrap_or(0.0) * h.quantity,
                MfSort::Name => f64::NEG_INFINITY,
            }
        };
        order.sort_by(|&a, &b| cmp_desc(key(&holdings[a]), key(&holdings[b])));
    }
    order
}

impl App {
    pub fn mf_display_order(&self) -> Vec<usize> {
        let Some(mf) = self.mf_holdings.as_deref() else {
            return Vec::new();
        };
        self.books
            .mf
            .cached_order(self.mf_gen, |sort| mf_order(mf, sort))
    }

    pub fn mf_display_row(&self, display: &[usize]) -> usize {
        let mf = self.mf_holdings.as_deref().unwrap_or_default();
        self.books
            .mf
            .display_row(display, |i| mf.get(i).map(mf_key))
    }
}

#[cfg(test)]
mod tests {
    use super::super::super::MfSort;
    use super::mf_order;
    use crate::tui::data::MfHolding;

    fn mf(name: &str, qty: f64, nav: f64, pnl: f64, day_change: f64) -> MfHolding {
        {
            let mut row = crate::tui::data::test_models::mf();
            row.tradingsymbol = name.into();
            row.quantity = qty;
            row.average_price = 100.0;
            row.last_price = nav;
            row.close_price = Some(100.0);
            row.pnl = pnl;
            row.day_change = Some(day_change);
            row.day_change_percentage = Some(0.0);
            row
        }
    }

    #[test]
    fn mf_sort_name_ascends_by_display_name() {
        let ms = [
            mf("Zeta Fund - Direct Plan", 1.0, 110.0, 10.0, 1.0),
            mf("Alpha Fund - Direct Plan", 1.0, 110.0, 10.0, 1.0),
            mf("Mid Fund", 1.0, 110.0, 10.0, 1.0),
        ];
        assert_eq!(mf_order(&ms, MfSort::Name), vec![1, 2, 0]);
    }

    #[test]
    fn mf_sort_value_pnl_and_day_descend() {
        let ms = [
            mf("A", 10.0, 110.0, 100.0, 1.0),
            mf("B", 1.0, 130.0, 300.0, 5.0),
            mf("C", 5.0, 120.0, 50.0, 20.0),
        ];
        assert_eq!(mf_order(&ms, MfSort::Value), vec![0, 2, 1]);
        assert_eq!(mf_order(&ms, MfSort::Pnl), vec![1, 0, 2]);
        assert_eq!(mf_order(&ms, MfSort::DayChange), vec![2, 0, 1]);
    }

    #[test]
    fn mf_sort_then_index_map_round_trips() {
        let ms = [
            mf("A", 10.0, 110.0, 100.0, 1.0),
            mf("B", 1.0, 130.0, 300.0, 5.0),
        ];
        let order = mf_order(&ms, MfSort::Pnl);

        assert_eq!(ms[order[0]].tradingsymbol, "B");
    }
}
