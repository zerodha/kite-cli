use super::{ChartHit, ControlIntent, ModalField};
use crate::tui::app::{AlertFormStage, App, Modal, SortTarget};
use crate::tui::bids::Section as BidsSection;
use crate::tui::pane_state::ChartId;
use crate::tui::split::Split;
use crate::tui::workspace::{PaneId, Preset};
use crossterm::event::KeyEvent;
use ratatui::layout::{Position, Rect};

#[derive(Debug, Clone, Copy)]
pub struct HitRegion {
    pub rect: Rect,
    pub action: HitAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitAction {
    Rows {
        list: HitList,
        offset: usize,
    },
    Row {
        list: HitList,
        index: usize,
    },
    Pane(PaneId),
    ModalControl {
        field: ModalField,
        intent: ControlIntent,
    },
    Text(HitText),
    NoteCaret {
        scroll: usize,
    },
    Key(KeyEvent),
    Workspace(Preset),
    BidsSection(BidsSection),
    Header {
        owner: PaneId,
        target: SortTarget,
    },
    Capture,
    Divider {
        split: Split,
        span: Rect,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitList {
    Pane(PaneId),
    Chain { owner: PaneId, band: ChainBand },
    BidsApplications(PaneId),
    Search,
    Palette,
    ThemePicker,
    WorkspacePicker,
    EventLog,
    Notes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainBand {
    Call,
    Strike,
    Put,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitText {
    Help,
    ModalBody,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    workspace: Preset,
    focused: PaneId,
    maximized: Option<PaneId>,
    modals: Vec<(std::mem::Discriminant<Modal>, Option<AlertFormStage>)>,
    bids: (BidsSection, bool),
    chart: (Option<PaneId>, bool),
}

impl Surface {
    pub fn of(app: &App) -> Self {
        let workspace = app.workspace();
        Self {
            workspace,
            focused: app.focused_pane(),
            maximized: app.workspaces.maximized(workspace),
            modals: app
                .modals
                .iter()
                .map(|modal| {
                    let stage = match modal {
                        Modal::Alert(state) => Some(state.stage),
                        _ => None,
                    };
                    (std::mem::discriminant(modal), stage)
                })
                .collect(),
            bids: (app.bids_pane.section(), app.bids_pane.application_focus()),
            chart: (app.chart_panel_pane(), app.chart_maximized),
        }
    }
}

fn list_identity(app: &App, list: HitList) -> u64 {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    match list {
        HitList::Pane(pane) => hash_pane_rows(app, pane, &mut hasher),
        HitList::Chain { owner, .. } => hash_pane_rows(app, owner, &mut hasher),
        HitList::BidsApplications(_) => {
            let applications = app.bids.applications.as_deref().unwrap_or_default();
            applications.len().hash(&mut hasher);
            for application in applications {
                (
                    &application.symbol,
                    &application.investor_type,
                    &application.created_at,
                )
                    .hash(&mut hasher);
            }
        }
        HitList::Search | HitList::ThemePicker | HitList::EventLog | HitList::Notes => {
            hash_modal_rows(app, &mut hasher);
        }
        HitList::Palette | HitList::WorkspacePicker => {}
    }
    hasher.finish()
}

fn hash_pane_rows(app: &App, pane: PaneId, hasher: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    match pane {
        PaneId::PortfolioPositions | PaneId::OverviewPositions => {
            let net = app.positions.as_ref().map_or(&[][..], |data| &data.net);
            let order = match pane {
                PaneId::OverviewPositions => (0..net.len()).collect(),
                _ => app.positions_order_in(pane),
            };
            hash_drawn(hasher, net, &order, |position| {
                (
                    &position.exchange,
                    &position.tradingsymbol,
                    &position.product,
                )
            });
        }
        PaneId::PortfolioHoldings | PaneId::OverviewHoldings => {
            let holdings = app.holdings.as_deref().unwrap_or_default();
            let order = match pane {
                PaneId::OverviewHoldings => app.overview_holdings_display_order(),
                _ => app.holdings_order_in(pane),
            };
            hash_drawn(hasher, holdings, &order, |holding| {
                (&holding.exchange, &holding.tradingsymbol)
            });
        }
        PaneId::OrdersBook | PaneId::TradingOrders => {
            let orders = app.orders.as_deref().unwrap_or_default();
            hash_drawn(hasher, orders, &app.orders_order_in(pane), |order| {
                &order.order_id
            });
        }
        PaneId::OrdersGtt => {
            let gtts = app.gtts.as_deref().unwrap_or_default();
            hash_drawn(hasher, gtts, &app.gtts_display_order(), |gtt| gtt.id);
        }
        PaneId::PortfolioMf => {
            let funds = app.mf_holdings.as_deref().unwrap_or_default();
            hash_drawn(hasher, funds, &app.mf_display_order(), |fund| {
                &fund.tradingsymbol
            });
        }
        PaneId::OrdersAlerts => {
            let alerts = app.alert_list();
            hash_drawn(hasher, alerts, &app.alerts_display_order(), |alert| {
                &alert.uuid
            });
        }
        PaneId::TradingWatch => {
            let rows = app.watch_rows_of(pane);
            rows.len().hash(hasher);
            for item in rows {
                item.instrument_token.hash(hasher);
            }
        }
        PaneId::InvestBids => {
            for section in BidsSection::ALL {
                let issues = app.bids.instruments(section).unwrap_or_default();
                issues.len().hash(hasher);
                for issue in issues {
                    (&issue.symbol, &issue.tradingsymbol, &issue.name).hash(hasher);
                }
            }
            let auctions = app.auctions.as_deref().unwrap_or_default();
            auctions.len().hash(hasher);
            for auction in auctions {
                (
                    &auction.auction_number,
                    &auction.exchange,
                    &auction.tradingsymbol,
                )
                    .hash(hasher);
            }
        }
        PaneId::DerivativesChain => {
            let chain = &app.derivatives_chain;
            (chain.underlying(), chain.expiry()).hash(hasher);
            let strikes = chain.rows();
            strikes.len().hash(hasher);
            for row in strikes {
                (
                    row.strike.to_bits(),
                    row.call.as_ref().map(|leg| leg.instrument_token),
                    row.put.as_ref().map(|leg| leg.instrument_token),
                )
                    .hash(hasher);
            }
        }
        _ => {}
    }
}

fn hash_drawn<'a, T, K: std::hash::Hash>(
    hasher: &mut impl std::hash::Hasher,
    records: &'a [T],
    order: &[usize],
    identity: impl Fn(&'a T) -> K,
) {
    use std::hash::Hash;
    order.len().hash(hasher);
    for &index in order {
        records.get(index).map(&identity).hash(hasher);
    }
}

fn hash_modal_rows(app: &App, hasher: &mut impl std::hash::Hasher) {
    use std::hash::Hash;
    match app.modals.last() {
        Some(Modal::Search(state)) => {
            state.results.len().hash(hasher);
            for instrument in &state.results {
                instrument.instrument_token.hash(hasher);
            }
        }
        Some(Modal::ThemePicker(state)) => state.results.hash(hasher),
        Some(Modal::EventLog(_)) => {
            let toasts = app.toasts();
            toasts.len().hash(hasher);
            for toast in toasts {
                (toast.at, &toast.text).hash(hasher);
            }
        }
        Some(Modal::Notes(_)) => app.notes.ids_newest_first().hash(hasher),
        _ => {}
    }
}

#[derive(Debug, Clone, Default)]
pub struct HitMap {
    pub surface: Option<Surface>,
    pub rows_drawn_for: Vec<(HitList, u64)>,
    pub regions: Vec<HitRegion>,
    pub charts: Vec<PaneChartHit>,
    pub hidden: Vec<PaneId>,
    pub hints: Vec<HitRegion>,
}

#[derive(Debug, Clone)]
pub struct PaneChartHit {
    pub chart: ChartId,
    pub hit: ChartHit,
}

impl HitMap {
    pub fn push(&mut self, rect: Rect, action: HitAction) {
        self.regions.push(HitRegion { rect, action });
    }

    pub fn push_chart(&mut self, chart: ChartId, hit: ChartHit) {
        self.charts.push(PaneChartHit { chart, hit });
    }

    pub fn chart_under(&self, column: u16, row: u16) -> Option<&PaneChartHit> {
        let point = Position { x: column, y: row };
        self.charts
            .iter()
            .rev()
            .find(|entry| entry.hit.outer.contains(point))
    }

    pub fn invalidate(&mut self) {
        self.regions.clear();
        self.hints.clear();
        self.charts.clear();
        self.hidden.clear();
        self.rows_drawn_for.clear();
        self.surface = None;
    }

    pub fn record_row_identities(&mut self, app: &App) {
        self.rows_drawn_for.clear();
        for region in &self.regions {
            let (HitAction::Rows { list, .. } | HitAction::Row { list, .. }) = region.action else {
                continue;
            };
            if !self.rows_drawn_for.iter().any(|(drawn, _)| *drawn == list) {
                self.rows_drawn_for.push((list, list_identity(app, list)));
            }
        }
    }

    pub fn rows_are_current(&self, app: &App, list: HitList) -> bool {
        self.rows_drawn_for
            .iter()
            .any(|&(drawn, identity)| drawn == list && identity == list_identity(app, list))
    }

    pub fn at(&self, column: u16, row: u16) -> Option<HitRegion> {
        let point = Position { x: column, y: row };
        self.regions
            .iter()
            .rev()
            .chain(self.hints.iter().rev())
            .find(|region| region.rect.contains(point))
            .copied()
    }
}
