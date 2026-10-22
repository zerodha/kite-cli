#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneId {
    OverviewPositions,
    OverviewHoldings,
    OverviewIndices,
    OverviewMovers,
    TradingWatch,
    TradingChart,
    TradingDepth,
    TradingOrders,
    PortfolioPositions,
    PortfolioHoldings,
    PortfolioFunds,
    PortfolioMf,
    DerivativesChain,
    DerivativesChart,
    OrdersBook,
    OrdersGtt,
    OrdersAlerts,
    InvestBids,
}

impl PaneId {
    pub fn config_token(self) -> &'static str {
        match self {
            PaneId::OverviewPositions => "overview.positions",
            PaneId::OverviewHoldings => "overview.holdings",
            PaneId::OverviewIndices => "overview.indices",
            PaneId::OverviewMovers => "overview.movers",
            PaneId::TradingWatch => "trading.watch",
            PaneId::TradingChart => "trading.chart",
            PaneId::TradingDepth => "trading.depth",
            PaneId::TradingOrders => "trading.orders",
            PaneId::PortfolioPositions => "portfolio.positions",
            PaneId::PortfolioHoldings => "portfolio.holdings",
            PaneId::PortfolioFunds => "portfolio.funds",
            PaneId::PortfolioMf => "portfolio.mf",
            PaneId::DerivativesChain => "derivatives.chain",
            PaneId::DerivativesChart => "derivatives.chart",
            PaneId::OrdersBook => "orders.orders",
            PaneId::OrdersGtt => "orders.gtt",
            PaneId::OrdersAlerts => "orders.alerts",
            PaneId::InvestBids => "invest.bids",
        }
    }

    pub fn from_config_token(preset: Preset, token: &str) -> Option<PaneId> {
        preset
            .panes()
            .iter()
            .copied()
            .find(|pane| pane.config_token() == token)
    }

    pub fn title(self) -> &'static str {
        match self {
            PaneId::OverviewPositions | PaneId::PortfolioPositions => "Positions",
            PaneId::OverviewHoldings | PaneId::PortfolioHoldings => "Holdings",
            PaneId::OverviewIndices => "Indices",
            PaneId::OverviewMovers => "Movers",
            PaneId::TradingWatch => "Watch",
            PaneId::TradingChart | PaneId::DerivativesChart => "Chart",
            PaneId::TradingDepth => "Depth",
            PaneId::TradingOrders | PaneId::OrdersBook => "Orders",
            PaneId::PortfolioFunds => "Funds",
            PaneId::DerivativesChain => "Chain",
            PaneId::OrdersGtt => "GTT",
            PaneId::OrdersAlerts => "Alerts",
            PaneId::PortfolioMf => "MF",
            PaneId::InvestBids => "Bids",
        }
    }

    pub fn preset(self) -> Preset {
        match self {
            PaneId::OverviewPositions
            | PaneId::OverviewHoldings
            | PaneId::OverviewIndices
            | PaneId::OverviewMovers => Preset::Overview,
            PaneId::TradingWatch
            | PaneId::TradingChart
            | PaneId::TradingDepth
            | PaneId::TradingOrders => Preset::Trading,
            PaneId::PortfolioPositions
            | PaneId::PortfolioMf
            | PaneId::PortfolioHoldings
            | PaneId::PortfolioFunds => Preset::Portfolio,
            PaneId::DerivativesChain | PaneId::DerivativesChart => Preset::Derivatives,
            PaneId::OrdersBook | PaneId::OrdersGtt | PaneId::OrdersAlerts => Preset::Orders,
            PaneId::InvestBids => Preset::Invest,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Band {
    Kpi,
    Account,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    Overview,
    Trading,
    Portfolio,
    Derivatives,
    Orders,
    Invest,
}

impl Preset {
    pub const ALL: [Preset; 6] = [
        Preset::Overview,
        Preset::Trading,
        Preset::Portfolio,
        Preset::Derivatives,
        Preset::Orders,
        Preset::Invest,
    ];

    pub const COUNT: usize = Self::ALL.len();

    fn index(self) -> usize {
        self as usize
    }

    pub fn title(self) -> &'static str {
        match self {
            Preset::Overview => "Overview",
            Preset::Trading => "Trading",
            Preset::Portfolio => "Portfolio",
            Preset::Derivatives => "Option Chain",
            Preset::Orders => "Orders",
            Preset::Invest => "IPO & Bids",
        }
    }

    pub fn key(self) -> char {
        let at = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0);
        char::from(b'1' + u8::try_from(at).expect("bounded TUI value fits u8"))
    }

    pub fn from_key(key: char) -> Option<Preset> {
        let at = key.to_digit(10)?.checked_sub(1)?;
        Self::ALL.get(at as usize).copied()
    }

    pub fn config_token(self) -> &'static str {
        match self {
            Preset::Overview => "overview",
            Preset::Trading => "trading",
            Preset::Portfolio => "portfolio",
            Preset::Derivatives => "derivatives",
            Preset::Orders => "orders",
            Preset::Invest => "invest",
        }
    }

    pub fn from_config_token(token: &str) -> Option<Preset> {
        Self::ALL
            .into_iter()
            .find(|preset| preset.config_token() == token)
    }

    pub fn cycle(self, delta: isize) -> Preset {
        let at = Self::ALL
            .iter()
            .position(|preset| *preset == self)
            .unwrap_or(0)
            .cast_signed();
        let len = Self::ALL.len().cast_signed();
        Self::ALL[(at + delta).rem_euclid(len) as usize]
    }

    pub fn panes(self) -> &'static [PaneId] {
        match self {
            Preset::Overview => &[
                PaneId::OverviewPositions,
                PaneId::OverviewHoldings,
                PaneId::OverviewIndices,
                PaneId::OverviewMovers,
            ],
            Preset::Trading => &[
                PaneId::TradingWatch,
                PaneId::TradingChart,
                PaneId::TradingDepth,
                PaneId::TradingOrders,
            ],
            Preset::Portfolio => &[
                PaneId::PortfolioPositions,
                PaneId::PortfolioMf,
                PaneId::PortfolioHoldings,
                PaneId::PortfolioFunds,
            ],
            Preset::Derivatives => &[PaneId::DerivativesChain, PaneId::DerivativesChart],
            Preset::Orders => &[PaneId::OrdersBook, PaneId::OrdersGtt, PaneId::OrdersAlerts],
            Preset::Invest => &[PaneId::InvestBids],
        }
    }

    pub fn default_focus(self) -> PaneId {
        match self {
            Preset::Overview => PaneId::OverviewPositions,
            Preset::Trading => PaneId::TradingWatch,
            Preset::Portfolio => PaneId::PortfolioPositions,
            Preset::Derivatives => PaneId::DerivativesChain,
            Preset::Orders => PaneId::OrdersBook,
            Preset::Invest => PaneId::InvestBids,
        }
    }
}

pub fn focus_next(order: &[PaneId], current: PaneId, delta: isize) -> PaneId {
    let Some(at) = order.iter().position(|pane| *pane == current) else {
        return current;
    };
    let len = order.len().cast_signed();
    let next = (at.cast_signed() + delta).rem_euclid(len);
    order[usize::try_from(next).expect("remainder is nonnegative")]
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Workspaces {
    focused: [Option<PaneId>; Preset::COUNT],
    maximized: [Option<PaneId>; Preset::COUNT],
}

impl Workspaces {
    pub fn focused(&self, preset: Preset) -> PaneId {
        self.focused[preset.index()].unwrap_or_else(|| preset.default_focus())
    }

    pub fn focus(&mut self, preset: Preset, pane: PaneId) -> bool {
        if !preset.panes().contains(&pane) || self.focused[preset.index()] == Some(pane) {
            return false;
        }
        self.focused[preset.index()] = Some(pane);
        true
    }

    pub fn restore(&mut self, preset: Preset, focused: Option<PaneId>, maximized: Option<PaneId>) {
        let defined = |pane: &PaneId| preset.panes().contains(pane);
        self.focused[preset.index()] = focused.filter(defined);
        self.maximized[preset.index()] = maximized.filter(defined);
    }

    pub fn maximized(&self, preset: Preset) -> Option<PaneId> {
        self.maximized[preset.index()]
    }

    pub fn toggle_maximize(&mut self, preset: Preset, pane: PaneId) -> Option<PaneId> {
        if !preset.panes().contains(&pane) {
            return self.maximized[preset.index()];
        }
        let next = if self.maximized[preset.index()] == Some(pane) {
            None
        } else {
            self.focused[preset.index()] = Some(pane);
            Some(pane)
        };
        self.maximized[preset.index()] = next;
        next
    }

    pub fn clear_maximize(&mut self, preset: Preset) -> bool {
        self.maximized[preset.index()].take().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::{PaneId, Preset, Workspaces, focus_next};

    #[test]
    fn every_registry_entry_has_a_distinct_slot_name_and_token() {
        let mut tokens: Vec<&str> = Preset::ALL.into_iter().map(Preset::config_token).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(
            tokens.len(),
            Preset::COUNT,
            "two entries share a config key"
        );

        assert_eq!(
            Preset::ALL,
            [
                Preset::Overview,
                Preset::Trading,
                Preset::Portfolio,
                Preset::Derivatives,
                Preset::Orders,
                Preset::Invest,
            ]
        );
    }

    #[test]
    fn the_number_keys_follow_the_strip() {
        for (at, preset) in Preset::ALL.into_iter().enumerate() {
            let key = char::from_digit(
                u32::try_from(at).expect("bounded TUI value fits u32") + 1,
                10,
            )
            .unwrap();
            assert_eq!(preset.key(), key);
            assert_eq!(Preset::from_key(key), Some(preset));
        }
        for key in ['0', '7', '8', '9', 'a'] {
            assert_eq!(Preset::from_key(key), None, "{key} selects nothing");
        }
    }

    #[test]
    fn every_pane_has_one_workspace_and_a_distinct_token() {
        let panes: Vec<PaneId> = Preset::ALL
            .into_iter()
            .flat_map(Preset::panes)
            .copied()
            .collect();
        let mut tokens: Vec<&str> = panes.iter().map(|pane| pane.config_token()).collect();
        tokens.sort_unstable();
        tokens.dedup();
        assert_eq!(tokens.len(), panes.len(), "two panes share a config key");
        for pane in panes {
            let preset = pane.preset();
            assert!(preset.panes().contains(&pane), "{pane:?}");
            assert_eq!(
                PaneId::from_config_token(preset, pane.config_token()),
                Some(pane)
            );
            for other in Preset::ALL.into_iter().filter(|other| *other != preset) {
                assert_eq!(PaneId::from_config_token(other, pane.config_token()), None);
            }
        }
    }

    #[test]
    fn the_registry_cycles_in_both_directions_and_wraps() {
        assert_eq!(Preset::Overview.cycle(1), Preset::Trading);
        assert_eq!(Preset::Overview.cycle(-1), Preset::Invest);
        assert_eq!(Preset::Invest.cycle(1), Preset::Overview);
        assert_eq!(Preset::Derivatives.cycle(1), Preset::Orders);
        assert_eq!(
            Preset::Trading.cycle(Preset::COUNT.cast_signed()),
            Preset::Trading
        );
    }

    #[test]
    fn every_pane_a_preset_draws_can_take_its_focus() {
        for preset in Preset::ALL {
            let panes = preset.panes();
            assert!(!panes.is_empty(), "{preset:?} defines no pane");
            assert!(panes.contains(&preset.default_focus()));
        }
    }

    #[test]
    fn the_overview_cycles_all_four_of_its_panes() {
        assert_eq!(
            Preset::Overview.panes(),
            [
                PaneId::OverviewPositions,
                PaneId::OverviewHoldings,
                PaneId::OverviewIndices,
                PaneId::OverviewMovers,
            ]
        );
    }

    #[test]
    fn focus_walks_the_cycle_order_in_both_directions_and_wraps() {
        let order = Preset::Overview.panes();
        assert_eq!(
            focus_next(order, PaneId::OverviewPositions, 1),
            PaneId::OverviewHoldings
        );
        assert_eq!(
            focus_next(order, PaneId::OverviewMovers, 1),
            PaneId::OverviewPositions
        );
        assert_eq!(
            focus_next(order, PaneId::OverviewPositions, -1),
            PaneId::OverviewMovers
        );
        assert_eq!(
            focus_next(order, PaneId::TradingWatch, 1),
            PaneId::TradingWatch
        );
        assert_eq!(
            focus_next(&[], PaneId::OverviewPositions, 1),
            PaneId::OverviewPositions
        );
    }

    #[test]
    fn each_workspace_remembers_its_own_focus() {
        let mut workspaces = Workspaces::default();
        assert_eq!(
            workspaces.focused(Preset::Overview),
            PaneId::OverviewPositions
        );
        workspaces.focus(
            Preset::Overview,
            focus_next(
                Preset::Overview.panes(),
                workspaces.focused(Preset::Overview),
                1,
            ),
        );
        assert_eq!(
            workspaces.focused(Preset::Overview),
            PaneId::OverviewHoldings
        );
        assert_eq!(workspaces.focused(Preset::Trading), PaneId::TradingWatch);
        assert_eq!(
            workspaces.focused(Preset::Overview),
            PaneId::OverviewHoldings
        );
    }

    #[test]
    fn a_pane_a_preset_does_not_define_cannot_take_its_focus() {
        let mut workspaces = Workspaces::default();
        workspaces.focus(Preset::Overview, PaneId::TradingWatch);
        assert_eq!(
            workspaces.focused(Preset::Overview),
            PaneId::OverviewPositions
        );
        workspaces.focus(Preset::Overview, PaneId::OverviewMovers);
        assert_eq!(workspaces.focused(Preset::Overview), PaneId::OverviewMovers);
    }
}
