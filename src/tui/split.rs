use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Split {
    ChartRight,
    ChartBottom,
    OverviewMain,
    OverviewBooks,
    TradingRoot,
    TradingRight,
    TradingBottom,
    PortfolioRoot,
    PortfolioLeft,
    PortfolioRight,
    DerivativesRoot,
    OrdersRoot,
    OrdersRight,
}

impl Split {
    pub const ALL: [Split; 13] = [
        Split::ChartRight,
        Split::ChartBottom,
        Split::OverviewMain,
        Split::OverviewBooks,
        Split::TradingRoot,
        Split::TradingRight,
        Split::TradingBottom,
        Split::PortfolioRoot,
        Split::PortfolioLeft,
        Split::PortfolioRight,
        Split::DerivativesRoot,
        Split::OrdersRoot,
        Split::OrdersRight,
    ];

    pub fn config_token(self) -> &'static str {
        match self {
            Split::ChartRight => "chart_right",
            Split::ChartBottom => "chart_bottom",
            Split::OverviewMain => "overview_main",
            Split::OverviewBooks => "overview_books",
            Split::TradingRoot => "trading_root",
            Split::TradingRight => "trading_right",
            Split::TradingBottom => "trading_bottom",
            Split::PortfolioRoot => "portfolio_root",
            Split::PortfolioLeft => "portfolio_left",
            Split::PortfolioRight => "portfolio_right",
            Split::DerivativesRoot => "derivatives_root",
            Split::OrdersRoot => "orders_root",
            Split::OrdersRight => "orders_right",
        }
    }

    pub fn axis(self) -> Axis {
        match self {
            Split::ChartRight
            | Split::OverviewMain
            | Split::TradingRoot
            | Split::TradingBottom
            | Split::PortfolioRoot
            | Split::DerivativesRoot
            | Split::OrdersRoot => Axis::Horizontal,
            Split::ChartBottom
            | Split::OverviewBooks
            | Split::TradingRight
            | Split::PortfolioLeft
            | Split::PortfolioRight
            | Split::OrdersRight => Axis::Vertical,
        }
    }

    pub fn minimums(self) -> (u16, u16) {
        match self {
            Split::ChartRight => (80, 52),
            Split::ChartBottom => (8, 9),
            Split::OverviewMain => (40, 24),
            Split::OverviewBooks => (6, 6),
            Split::TradingRoot => (WATCH_MIN_W, CHART_MIN_W),
            Split::TradingRight => (CHART_MIN_H, CHART_MIN_H),
            Split::TradingBottom => (DEPTH_MIN_W, ORDERS_MIN_W),
            Split::PortfolioRoot => (BOOK_MIN_W, BOOK_MIN_W),
            Split::PortfolioRight => (HOLDINGS_MIN_H, FUNDS_MIN_H),
            Split::DerivativesRoot => (CHAIN_MIN_W, CHART_MIN_W),
            Split::OrdersRoot => (ORDERS_MIN_W, BOOK_MIN_W),
            Split::PortfolioLeft | Split::OrdersRight => (BOOK_MIN_H, BOOK_MIN_H),
        }
    }

    pub fn ratio_at(self, span: Rect, column: u16, row: u16) -> Option<f32> {
        let (start, total, pos) = match self.axis() {
            Axis::Horizontal => (span.x, span.width, column),
            Axis::Vertical => (span.y, span.height, row),
        };
        if total == 0 {
            return None;
        }
        let first = pos.saturating_sub(start).saturating_add(1).min(total);
        Some(f32::from(first) / f32::from(total))
    }

    pub fn axis_pos(self, column: u16, row: u16) -> u16 {
        match self.axis() {
            Axis::Horizontal => column,
            Axis::Vertical => row,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

const WATCH_MIN_W: u16 = 32;
const CHART_MIN_W: u16 = 40;
const DEPTH_MIN_W: u16 = 28;
const ORDERS_MIN_W: u16 = 40;
const BOOK_MIN_W: u16 = 40;
const CHAIN_MIN_W: u16 = 64;
const CHART_MIN_H: u16 = 10;
const HOLDINGS_MIN_H: u16 = 6;
const FUNDS_MIN_H: u16 = 8;
const BOOK_MIN_H: u16 = 6;

const RATIO_MIN: f32 = 0.05;
const RATIO_MAX: f32 = 0.95;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SplitRatios([Option<f32>; Split::ALL.len()]);

impl SplitRatios {
    pub fn get(&self, split: Split) -> Option<f32> {
        self.0[split.index()]
    }

    pub fn set(&mut self, split: Split, ratio: f32) {
        if !ratio.is_finite() {
            return;
        }
        self.0[split.index()] = Some(ratio.clamp(RATIO_MIN, RATIO_MAX));
    }

    pub fn clear(&mut self) {
        self.0 = [None; Split::ALL.len()];
    }

    pub fn is_default(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    pub fn length(&self, split: Split, total: u16, fallback: u16) -> u16 {
        let Some(ratio) = self.get(split) else {
            return fallback;
        };
        let (min_first, min_second) = split.minimums();
        if total < min_first.saturating_add(min_second) {
            return fallback;
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "rounded panel extent is clamped to u16 range"
        )]
        let want = (f32::from(total) * ratio)
            .round()
            .clamp(0.0, f32::from(u16::MAX)) as u16;
        want.clamp(min_first, total - min_second)
    }
}

#[cfg(test)]
mod tests {
    use super::{Axis, Split, SplitRatios};
    use ratatui::layout::Rect;

    #[test]
    fn config_tokens_are_unique_per_split() {
        let mut tokens: Vec<&str> = Split::ALL.iter().map(|s| s.config_token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), Split::ALL.len());
    }

    #[test]
    fn an_undragged_split_keeps_the_layouts_own_extent() {
        let ratios = SplitRatios::default();
        assert!(ratios.is_default());
        assert_eq!(ratios.length(Split::OverviewBooks, 20, 0), 0);
        assert_eq!(ratios.length(Split::ChartRight, 200, 104), 104);
    }

    #[test]
    fn a_dragged_ratio_is_clamped_off_both_minimums() {
        let mut ratios = SplitRatios::default();
        ratios.set(Split::ChartRight, 0.5);
        assert_eq!(ratios.length(Split::ChartRight, 200, 104), 100);

        ratios.set(Split::ChartRight, 0.05);
        assert_eq!(ratios.length(Split::ChartRight, 200, 104), 80);
        ratios.set(Split::ChartRight, 0.95);
        assert_eq!(ratios.length(Split::ChartRight, 200, 104), 148);
    }

    #[test]
    fn an_area_too_small_for_both_minimums_falls_back() {
        let mut ratios = SplitRatios::default();
        ratios.set(Split::ChartRight, 0.5);
        assert_eq!(ratios.length(Split::ChartRight, 100, 61), 61);
        assert_eq!(ratios.length(Split::ChartRight, 200, 148), 100);
    }

    #[test]
    fn a_non_finite_ratio_is_not_stored() {
        let mut ratios = SplitRatios::default();
        ratios.set(Split::ChartBottom, f32::NAN);
        ratios.set(Split::ChartBottom, f32::INFINITY);
        assert_eq!(ratios.get(Split::ChartBottom), None);
        assert!(ratios.is_default());
    }

    #[test]
    fn clear_forgets_every_split() {
        let mut ratios = SplitRatios::default();
        for split in Split::ALL {
            ratios.set(split, 0.4);
        }
        assert!(!ratios.is_default());
        ratios.clear();
        assert!(ratios.is_default());
    }

    #[test]
    fn a_pointer_position_reads_as_the_first_panels_extent() {
        let span = Rect {
            x: 10,
            y: 4,
            width: 100,
            height: 40,
        };
        assert_eq!(Split::ChartRight.axis(), Axis::Horizontal);
        assert_eq!(Split::ChartRight.ratio_at(span, 59, 20), Some(0.5));
        assert_eq!(Split::ChartBottom.ratio_at(span, 60, 23), Some(0.5));
        assert_eq!(Split::ChartRight.ratio_at(span, 0, 20), Some(0.01));
        assert_eq!(Split::ChartRight.ratio_at(span, 500, 20), Some(1.0));
        assert_eq!(
            Split::ChartRight.ratio_at(Rect::new(0, 0, 0, 10), 0, 0),
            None
        );
    }
}
