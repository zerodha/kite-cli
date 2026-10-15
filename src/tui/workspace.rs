mod registry;
mod tree;

use crate::tui::split::{Axis, Split, SplitRatios};
use ratatui::layout::Rect;
pub use registry::{Band, PaneId, Preset, Workspaces, focus_next};
use tree::{Extent, Node, resolve_split};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneRect {
    pub id: PaneId,
    pub outer: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seam {
    pub split: Split,
    pub span: Rect,
    pub first: Rect,
    pub second: Rect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    panes: Vec<PaneRect>,
    hidden: Vec<PaneId>,
    bands: Vec<(Band, Rect)>,
    seams: Vec<Seam>,
    compact: bool,
}

impl Resolved {
    pub fn panes(&self) -> &[PaneRect] {
        &self.panes
    }

    pub fn hidden(&self) -> &[PaneId] {
        &self.hidden
    }

    pub fn rect(&self, pane: PaneId) -> Option<Rect> {
        self.panes
            .iter()
            .find(|drawn| drawn.id == pane)
            .map(|drawn| drawn.outer)
    }

    pub fn band(&self, band: Band) -> Option<Rect> {
        self.bands
            .iter()
            .find(|(kind, _)| *kind == band)
            .map(|(_, rect)| *rect)
    }

    pub fn seams(&self) -> &[Seam] {
        &self.seams
    }

    pub fn compact(&self) -> bool {
        self.compact
    }
}

pub struct OverviewContent<'a> {
    pub positions: usize,
    pub index_rows: &'a dyn Fn(u16) -> usize,
}

pub struct Request<'a> {
    pub preset: Preset,
    pub focused: PaneId,
    pub maximized: Option<PaneId>,
    pub ratios: &'a SplitRatios,
    pub overview: Option<&'a OverviewContent<'a>>,
}

const COMPACT_BODY_HEIGHT: u16 = 28;

const FOCUSED_ONLY_HEIGHT: u16 = 12;

const FOCUSED_ONLY_WIDTH: u16 = 70;

const BOOKS_BPS: u16 = 5_800;

const HOLDINGS_MIN: u16 = 6;

const MOVERS_MIN: u16 = 4;

const POSITIONS_CHROME: u16 = 5;

const INDICES_CHROME: u16 = 2;

pub fn resolve(body: Rect, request: &Request<'_>) -> Resolved {
    let maximized = request
        .maximized
        .filter(|pane| request.preset.panes().contains(pane));
    if let Some(pane) = maximized {
        return finish(
            request.preset,
            vec![PaneRect {
                id: pane,
                outer: body,
            }],
            Vec::new(),
            Vec::new(),
            body,
        );
    }

    match request.preset {
        Preset::Overview => resolve_overview(body, request),
        Preset::Trading
        | Preset::Portfolio
        | Preset::Derivatives
        | Preset::Orders
        | Preset::Invest => resolve_composite(body, request),
    }
}

fn finish(
    preset: Preset,
    panes: Vec<PaneRect>,
    bands: Vec<(Band, Rect)>,
    seams: Vec<Seam>,
    body: Rect,
) -> Resolved {
    let hidden = preset
        .panes()
        .iter()
        .copied()
        .filter(|pane| !panes.iter().any(|drawn| drawn.id == *pane))
        .collect();
    Resolved {
        panes,
        hidden,
        bands,
        seams,
        compact: body.height < COMPACT_BODY_HEIGHT,
    }
}

const COMPOSITE_MIN_WIDTH: u16 = 120;

const COMPOSITE_MIN_HEIGHT: u16 = 30;

fn resolve_composite(body: Rect, request: &Request<'_>) -> Resolved {
    if body.width < COMPOSITE_MIN_WIDTH || body.height < COMPOSITE_MIN_HEIGHT {
        let leaf = if request.preset.panes().contains(&request.focused) {
            request.focused
        } else {
            request.preset.default_focus()
        };
        return finish(
            request.preset,
            vec![PaneRect {
                id: leaf,
                outer: body,
            }],
            Vec::new(),
            Vec::new(),
            body,
        );
    }
    let mut panes = Vec::new();
    let mut seams = Vec::new();
    resolve_split(
        &composite_tree(request.preset),
        body,
        request.ratios,
        &mut panes,
        &mut seams,
    );
    finish(request.preset, panes, Vec::new(), seams, body)
}

fn composite_tree(preset: Preset) -> Node {
    fn seam(split: Split, numerator: u16, denominator: u16, one: Node, two: Node) -> Node {
        Node::Split {
            split: Some(split),
            axis: split.axis(),
            first: Extent::Fraction {
                numerator,
                denominator,
                minimums: split.minimums(),
            },
            children: (Box::new(one), Box::new(two)),
        }
    }
    match preset {
        Preset::Trading => seam(
            Split::TradingRoot,
            2,
            5,
            Node::Leaf(PaneId::TradingWatch),
            seam(
                Split::TradingRight,
                3,
                5,
                Node::Leaf(PaneId::TradingChart),
                seam(
                    Split::TradingBottom,
                    2,
                    5,
                    Node::Leaf(PaneId::TradingDepth),
                    Node::Leaf(PaneId::TradingOrders),
                ),
            ),
        ),
        Preset::Portfolio => seam(
            Split::PortfolioRoot,
            1,
            2,
            Node::Leaf(PaneId::PortfolioPositions),
            seam(
                Split::PortfolioRight,
                3,
                5,
                Node::Leaf(PaneId::PortfolioHoldings),
                Node::Leaf(PaneId::PortfolioFunds),
            ),
        ),
        Preset::Derivatives => seam(
            Split::DerivativesRoot,
            3,
            5,
            Node::Leaf(PaneId::DerivativesChain),
            Node::Leaf(PaneId::DerivativesChart),
        ),
        Preset::Orders => seam(
            Split::OrdersRoot,
            3,
            5,
            Node::Leaf(PaneId::OrdersBook),
            seam(
                Split::OrdersRight,
                1,
                2,
                Node::Leaf(PaneId::OrdersGtt),
                Node::Leaf(PaneId::OrdersAlerts),
            ),
        ),
        Preset::Invest => seam(
            Split::InvestRoot,
            1,
            2,
            Node::Leaf(PaneId::InvestMf),
            Node::Leaf(PaneId::InvestBids),
        ),
        Preset::Overview => Node::Leaf(preset.default_focus()),
    }
}

fn resolve_overview(body: Rect, request: &Request<'_>) -> Resolved {
    let compact = body.height < COMPACT_BODY_HEIGHT;
    let (kpi_rows, account_rows) = if compact { (3, 1) } else { (5, 3) };

    let account_h = account_rows.min(body.height);
    let kpi_h = kpi_rows.min(body.height - account_h);
    let main_h = body.height - account_h - kpi_h;

    let kpi = Rect {
        height: kpi_h,
        ..body
    };
    let main = Rect {
        y: body.y.saturating_add(kpi_h),
        height: main_h,
        ..body
    };
    let account = Rect {
        y: main.y.saturating_add(main_h),
        height: account_h,
        ..body
    };

    let mut bands = Vec::new();
    if kpi_h > 0 {
        bands.push((Band::Kpi, kpi));
    }
    if account_h > 0 {
        bands.push((Band::Account, account));
    }

    let mut panes = Vec::new();
    let mut seams = Vec::new();
    if main_h < FOCUSED_ONLY_HEIGHT || body.width < FOCUSED_ONLY_WIDTH {
        let leaf = if request.preset.panes().contains(&request.focused) {
            request.focused
        } else {
            request.preset.default_focus()
        };
        if leaf == PaneId::OverviewIndices
            && let Some(content) = request.overview
        {
            (content.index_rows)(main.width);
        }
        panes.push(PaneRect {
            id: leaf,
            outer: main,
        });
    } else {
        resolve_split(
            &overview_main(main, request),
            main,
            request.ratios,
            &mut panes,
            &mut seams,
        );
    }

    finish(request.preset, panes, bands, seams, body)
}

fn overview_main(main: Rect, request: &Request<'_>) -> Node {
    let positions = request.overview.map_or(0, |content| content.positions);
    let books = Extent::Ratio {
        bps: BOOKS_BPS,
        minimums: Split::OverviewMain.minimums(),
    };
    let left_w = request
        .ratios
        .length(Split::OverviewMain, main.width, books.length(main.width))
        .min(main.width);
    let index_rows = request
        .overview
        .map_or(0, |content| (content.index_rows)(main.width - left_w));

    Node::Split {
        split: Some(Split::OverviewMain),
        axis: Axis::Horizontal,
        first: books,
        children: (
            Box::new(Node::Split {
                split: Some(Split::OverviewBooks),
                axis: Axis::Vertical,
                first: Extent::Content {
                    length: u16::try_from(positions)
                        .unwrap_or(u16::MAX)
                        .saturating_add(POSITIONS_CHROME),
                    reserve: HOLDINGS_MIN,
                },
                children: (
                    Box::new(Node::Leaf(PaneId::OverviewPositions)),
                    Box::new(Node::Leaf(PaneId::OverviewHoldings)),
                ),
            }),
            Box::new(Node::Split {
                split: None,
                axis: Axis::Vertical,
                first: Extent::Content {
                    length: u16::try_from(index_rows)
                        .unwrap_or(u16::MAX)
                        .saturating_add(INDICES_CHROME),
                    reserve: MOVERS_MIN,
                },
                children: (
                    Box::new(Node::Leaf(PaneId::OverviewIndices)),
                    Box::new(Node::Leaf(PaneId::OverviewMovers)),
                ),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Band, Extent, OverviewContent, PaneId, Preset, Request, Resolved, Workspaces, focus_next,
        resolve,
    };
    use crate::tui::split::{Split, SplitRatios};
    use ratatui::layout::{Constraint, Layout, Rect};

    fn body(width: u16, height: u16) -> Rect {
        Rect {
            x: 0,
            y: 1,
            width,
            height: height.saturating_sub(2),
        }
    }

    fn no_rows(_: u16) -> usize {
        0
    }

    fn overview(positions: usize, index_rows: usize) -> (OverviewContent<'static>, SplitRatios) {
        let rows: &'static dyn Fn(u16) -> usize = match index_rows {
            0 => &no_rows,
            1 => &|_| 1,
            3 => &|_| 3,
            4 => &|_| 4,
            n => panic!("add a fixture arm for {n} index rows"),
        };
        (
            OverviewContent {
                positions,
                index_rows: rows,
            },
            SplitRatios::default(),
        )
    }

    fn resolve_overview_at(
        area: Rect,
        positions: usize,
        index_rows: usize,
        focused: PaneId,
    ) -> Resolved {
        let (content, ratios) = overview(positions, index_rows);
        resolve(
            area,
            &Request {
                preset: Preset::Overview,
                focused,
                maximized: None,
                ratios: &ratios,
                overview: Some(&content),
            },
        )
    }

    fn rect_of(resolved: &Resolved, id: PaneId) -> Option<Rect> {
        resolved
            .panes()
            .iter()
            .find(|pane| pane.id == id)
            .map(|pane| pane.outer)
    }

    fn hidden_in(resolved: &Resolved, preset: Preset) -> Vec<PaneId> {
        preset
            .panes()
            .iter()
            .copied()
            .filter(|pane| rect_of(resolved, *pane).is_none())
            .collect()
    }

    fn assert_tiles(resolved: &Resolved, area: Rect) {
        let mut rects: Vec<Rect> = resolved.panes().iter().map(|pane| pane.outer).collect();
        for band in [Band::Kpi, Band::Account] {
            rects.extend(resolved.band(band));
        }
        for rect in &rects {
            assert!(rect.x >= area.x, "{rect:?} starts left of {area:?}");
            assert!(rect.y >= area.y, "{rect:?} starts above {area:?}");
            assert!(rect.right() <= area.right(), "{rect:?} exceeds {area:?}");
            assert!(rect.bottom() <= area.bottom(), "{rect:?} exceeds {area:?}");
        }
        for (i, one) in rects.iter().enumerate() {
            for two in &rects[i + 1..] {
                assert!(
                    one.intersection(*two).is_empty(),
                    "{one:?} and {two:?} overlap"
                );
            }
        }
    }

    fn assert_bands_tile(resolved: &Resolved, area: Rect) {
        let kpi = resolved.band(Band::Kpi).map_or(0, |rect| rect.height);
        let account = resolved.band(Band::Account).map_or(0, |rect| rect.height);
        let main: u16 = resolved
            .panes()
            .iter()
            .map(|pane| pane.outer)
            .fold(0, |rows, rect| rows.max(rect.bottom()))
            .saturating_sub(area.y.saturating_add(kpi));
        assert!(
            kpi + main + account <= area.height,
            "bands overflow {area:?}"
        );
    }

    #[test]
    fn no_workspace_starts_maximized() {
        let workspaces = Workspaces::default();
        for preset in Preset::ALL {
            assert_eq!(workspaces.maximized(preset), None);
        }
    }

    #[test]
    fn maximize_gives_one_pane_the_body_and_hides_the_rest() {
        let area = body(120, 35);
        let (content, ratios) = overview(2, 4);
        let resolved = resolve(
            area,
            &Request {
                preset: Preset::Overview,
                focused: PaneId::OverviewHoldings,
                maximized: Some(PaneId::OverviewHoldings),
                ratios: &ratios,
                overview: Some(&content),
            },
        );
        assert_eq!(rect_of(&resolved, PaneId::OverviewHoldings), Some(area));
        assert_eq!(
            hidden_in(&resolved, Preset::Overview),
            [
                PaneId::OverviewPositions,
                PaneId::OverviewIndices,
                PaneId::OverviewMovers
            ]
        );
        assert_eq!(resolved.band(Band::Kpi), None);
        assert_eq!(resolved.band(Band::Account), None);
        assert_eq!(resolved.seams().len(), 0);
        assert_tiles(&resolved, area);
    }

    #[test]
    fn a_maximized_workspace_has_one_focus_target() {
        let area = body(120, 35);
        let (content, ratios) = overview(2, 4);
        let resolved = resolve(
            area,
            &Request {
                preset: Preset::Overview,
                focused: PaneId::OverviewPositions,
                maximized: Some(PaneId::OverviewPositions),
                ratios: &ratios,
                overview: Some(&content),
            },
        );
        assert_eq!(resolved.panes().len(), 1);
        assert_eq!(
            focus_next(&[PaneId::OverviewPositions], PaneId::OverviewPositions, 1),
            PaneId::OverviewPositions
        );
    }

    #[test]
    fn a_pane_outside_the_preset_cannot_maximize_it() {
        let area = body(120, 35);
        let (content, ratios) = overview(2, 4);
        let resolved = resolve(
            area,
            &Request {
                preset: Preset::Overview,
                focused: PaneId::OverviewPositions,
                maximized: Some(PaneId::TradingWatch),
                ratios: &ratios,
                overview: Some(&content),
            },
        );
        assert_eq!(resolved.panes().len(), 4, "the composition still resolves");
    }

    #[test]
    fn overview_bands_are_budgeted_for_each_terminal_size() {
        for (height, kpi, main, account, compact) in [
            (22u16, 3u16, 18u16, 1u16, true), // 80x24, no toast
            (19, 3, 15, 1, true),             // 80x24, three toast rows
            (33, 5, 25, 3, false),            // 120x35
            (43, 5, 35, 3, false),            // 160x45
        ] {
            let area = Rect {
                x: 0,
                y: 1,
                width: 80,
                height,
            };
            let resolved = resolve_overview_at(area, 2, 4, PaneId::OverviewPositions);
            assert_eq!(
                (
                    resolved.band(Band::Kpi).map_or(0, |rect| rect.height),
                    resolved.band(Band::Account).map_or(0, |rect| rect.height),
                    resolved.compact(),
                ),
                (kpi, account, compact),
                "body height {height}"
            );
            let top = resolved
                .panes()
                .iter()
                .map(|pane| pane.outer.y)
                .min()
                .unwrap_or(area.y + kpi);
            let bottom = resolved
                .panes()
                .iter()
                .map(|pane| pane.outer.bottom())
                .max()
                .unwrap_or(top);
            assert_eq!(bottom - top, main, "main height at body {height}");
            assert_tiles(&resolved, area);
            assert_bands_tile(&resolved, area);
        }
    }

    #[test]
    fn the_overview_compacts_immediately_below_the_threshold() {
        for (height, kpi, account, compact) in [(28u16, 5u16, 3u16, false), (27, 3, 1, true)] {
            let area = Rect {
                x: 0,
                y: 1,
                width: 120,
                height,
            };
            let resolved = resolve_overview_at(area, 2, 4, PaneId::OverviewPositions);
            assert_eq!(resolved.compact(), compact, "body height {height}");
            assert_eq!(
                (
                    resolved.band(Band::Kpi).map_or(0, |r| r.height),
                    resolved.band(Band::Account).map_or(0, |r| r.height)
                ),
                (kpi, account)
            );
        }
    }

    #[test]
    fn the_account_row_is_allocated_first_on_a_tiny_body() {
        for (height, kpi, account) in [
            (0u16, 0u16, 0u16),
            (1, 0, 1),
            (2, 1, 1),
            (4, 3, 1),
            (5, 3, 1),
        ] {
            let area = Rect {
                x: 0,
                y: 1,
                width: 80,
                height,
            };
            let resolved = resolve_overview_at(area, 2, 4, PaneId::OverviewPositions);
            assert_eq!(
                (
                    resolved.band(Band::Kpi).map_or(0, |r| r.height),
                    resolved.band(Band::Account).map_or(0, |r| r.height)
                ),
                (kpi, account),
                "body height {height}"
            );
            assert_tiles(&resolved, area);
            assert_bands_tile(&resolved, area);
        }
    }

    #[test]
    fn the_overview_falls_back_to_one_book_below_the_main_or_width_floor() {
        let collapsed = |width: u16, height: u16| {
            let area = Rect {
                x: 0,
                y: 1,
                width,
                height,
            };
            resolve_overview_at(area, 2, 4, PaneId::OverviewPositions)
                .panes()
                .len()
                == 1
        };
        assert!(!collapsed(80, 16), "main 12 is fine");
        assert!(collapsed(80, 15), "main 11 is not");
        assert!(!collapsed(70, 33));
        assert!(collapsed(69, 33));
        for height in [19, 22] {
            assert!(
                !collapsed(80, height),
                "80x24 must never need the fallback (body height {height})"
            );
        }
    }

    #[test]
    fn the_collapsed_overview_draws_only_the_focused_leaf() {
        let area = body(80, 17); // main 11, below the floor
        for (focused, hidden) in [
            (
                PaneId::OverviewPositions,
                [
                    PaneId::OverviewHoldings,
                    PaneId::OverviewIndices,
                    PaneId::OverviewMovers,
                ],
            ),
            (
                PaneId::OverviewHoldings,
                [
                    PaneId::OverviewPositions,
                    PaneId::OverviewIndices,
                    PaneId::OverviewMovers,
                ],
            ),
            (
                PaneId::OverviewIndices,
                [
                    PaneId::OverviewPositions,
                    PaneId::OverviewHoldings,
                    PaneId::OverviewMovers,
                ],
            ),
            (
                PaneId::OverviewMovers,
                [
                    PaneId::OverviewPositions,
                    PaneId::OverviewHoldings,
                    PaneId::OverviewIndices,
                ],
            ),
        ] {
            let resolved = resolve_overview_at(area, 2, 4, focused);
            assert_eq!(resolved.panes().len(), 1, "{focused:?}");
            assert!(rect_of(&resolved, focused).is_some());
            for pane in hidden {
                assert_eq!(
                    rect_of(&resolved, pane),
                    None,
                    "{pane:?} still has a rectangle"
                );
                assert!(hidden_in(&resolved, Preset::Overview).contains(&pane));
            }
            assert!(
                Preset::Overview.panes().contains(&PaneId::OverviewMovers),
                "a hidden pane is still a focus stop"
            );
            assert_eq!(resolved.seams().len(), 0);
            assert_tiles(&resolved, area);
        }
    }

    fn resolve_composite_at(area: Rect, preset: Preset, focused: PaneId) -> Resolved {
        let ratios = SplitRatios::default();
        resolve(
            area,
            &Request {
                preset,
                focused,
                maximized: None,
                ratios: &ratios,
                overview: None,
            },
        )
    }

    type CanvasFixture = (u16, u16, Preset, &'static [(PaneId, Rect)]);
    const COMPOSITE_CANVASES: &[CanvasFixture] = &[
        (
            120u16,
            35u16,
            Preset::Trading,
            &[
                (PaneId::TradingWatch, Rect::new(0, 1, 48, 33)),
                (PaneId::TradingChart, Rect::new(48, 1, 72, 19)),
                (PaneId::TradingDepth, Rect::new(48, 20, 28, 14)),
                (PaneId::TradingOrders, Rect::new(76, 20, 44, 14)),
            ],
        ),
        (
            160,
            45,
            Preset::Trading,
            &[
                (PaneId::TradingWatch, Rect::new(0, 1, 64, 43)),
                (PaneId::TradingChart, Rect::new(64, 1, 96, 25)),
                (PaneId::TradingDepth, Rect::new(64, 26, 38, 18)),
                (PaneId::TradingOrders, Rect::new(102, 26, 58, 18)),
            ],
        ),
        (
            80,
            24,
            Preset::Trading,
            &[(PaneId::TradingWatch, Rect::new(0, 1, 80, 22))],
        ),
        (
            120,
            35,
            Preset::Portfolio,
            &[
                (PaneId::PortfolioPositions, Rect::new(0, 1, 60, 33)),
                (PaneId::PortfolioHoldings, Rect::new(60, 1, 60, 19)),
                (PaneId::PortfolioFunds, Rect::new(60, 20, 60, 14)),
            ],
        ),
        (
            160,
            45,
            Preset::Portfolio,
            &[
                (PaneId::PortfolioPositions, Rect::new(0, 1, 80, 43)),
                (PaneId::PortfolioHoldings, Rect::new(80, 1, 80, 25)),
                (PaneId::PortfolioFunds, Rect::new(80, 26, 80, 18)),
            ],
        ),
        (
            80,
            24,
            Preset::Portfolio,
            &[(PaneId::PortfolioPositions, Rect::new(0, 1, 80, 22))],
        ),
        (
            120,
            35,
            Preset::Derivatives,
            &[
                (PaneId::DerivativesChain, Rect::new(0, 1, 72, 33)),
                (PaneId::DerivativesChart, Rect::new(72, 1, 48, 33)),
            ],
        ),
        (
            160,
            45,
            Preset::Derivatives,
            &[
                (PaneId::DerivativesChain, Rect::new(0, 1, 96, 43)),
                (PaneId::DerivativesChart, Rect::new(96, 1, 64, 43)),
            ],
        ),
        (
            80,
            24,
            Preset::Derivatives,
            &[(PaneId::DerivativesChain, Rect::new(0, 1, 80, 22))],
        ),
        (
            120,
            35,
            Preset::Orders,
            &[
                (PaneId::OrdersBook, Rect::new(0, 1, 72, 33)),
                (PaneId::OrdersGtt, Rect::new(72, 1, 48, 16)),
                (PaneId::OrdersAlerts, Rect::new(72, 17, 48, 17)),
            ],
        ),
        (
            160,
            45,
            Preset::Orders,
            &[
                (PaneId::OrdersBook, Rect::new(0, 1, 96, 43)),
                (PaneId::OrdersGtt, Rect::new(96, 1, 64, 21)),
                (PaneId::OrdersAlerts, Rect::new(96, 22, 64, 22)),
            ],
        ),
        (
            80,
            24,
            Preset::Orders,
            &[(PaneId::OrdersBook, Rect::new(0, 1, 80, 22))],
        ),
        (
            120,
            35,
            Preset::Invest,
            &[
                (PaneId::InvestMf, Rect::new(0, 1, 60, 33)),
                (PaneId::InvestBids, Rect::new(60, 1, 60, 33)),
            ],
        ),
        (
            160,
            45,
            Preset::Invest,
            &[
                (PaneId::InvestMf, Rect::new(0, 1, 80, 43)),
                (PaneId::InvestBids, Rect::new(80, 1, 80, 43)),
            ],
        ),
        (
            80,
            24,
            Preset::Invest,
            &[(PaneId::InvestMf, Rect::new(0, 1, 80, 22))],
        ),
    ];

    #[test]
    fn each_composite_canvas_resolves_cell_for_cell() {
        for &(width, height, preset, rects) in COMPOSITE_CANVASES {
            let area = body(width, height);
            let resolved = resolve_composite_at(area, preset, preset.default_focus());
            let drawn: Vec<(PaneId, Rect)> = resolved
                .panes()
                .iter()
                .map(|pane| (pane.id, pane.outer))
                .collect();
            assert_eq!(drawn, rects, "{preset:?} at {width}x{height}");
            assert_tiles(&resolved, area);
            let cells: u32 = rects
                .iter()
                .map(|(_, rect)| u32::from(rect.width) * u32::from(rect.height))
                .sum();
            assert_eq!(cells, u32::from(area.width) * u32::from(area.height));
            assert!(resolved.band(Band::Kpi).is_none());
        }
    }

    #[test]
    fn a_composite_collapses_at_its_breakpoint_and_not_one_cell_before_it() {
        for preset in Preset::ALL.into_iter().filter(|p| *p != Preset::Overview) {
            let full = preset.panes().len();
            for (width, height, drawn) in [
                (120u16, 30u16, full),
                (119, 30, 1),
                (121, 30, full),
                (120, 29, 1),
                (120, 31, full),
                (119, 29, 1),
            ] {
                let area = Rect::new(0, 1, width, height);
                let resolved = resolve_composite_at(area, preset, preset.default_focus());
                assert_eq!(
                    resolved.panes().len(),
                    drawn,
                    "{preset:?} at a {width}x{height} body"
                );
                assert_eq!(resolved.hidden().len(), full - drawn);
                assert_tiles(&resolved, area);
            }
        }
    }

    #[test]
    fn a_collapsed_composite_draws_the_focused_leaf_and_hides_the_others() {
        let area = body(80, 24);
        for pane in Preset::Trading.panes().iter().copied() {
            let resolved = resolve_composite_at(area, Preset::Trading, pane);
            assert_eq!(resolved.panes().len(), 1);
            assert_eq!(resolved.rect(pane), Some(area));
            for other in Preset::Trading.panes().iter().copied() {
                if other != pane {
                    assert_eq!(resolved.rect(other), None, "{other:?} still drew");
                    assert!(resolved.hidden().contains(&other));
                }
            }
            assert_eq!(Preset::Trading.panes().len(), 4);
        }
        let stale = resolve_composite_at(area, Preset::Trading, PaneId::InvestMf);
        assert_eq!(stale.rect(PaneId::TradingWatch), Some(area));
    }

    #[test]
    fn maximize_gives_one_composite_leaf_the_whole_body() {
        let area = body(160, 45);
        let ratios = SplitRatios::default();
        let resolved = resolve(
            area,
            &Request {
                preset: Preset::Trading,
                focused: PaneId::TradingWatch,
                maximized: Some(PaneId::TradingOrders),
                ratios: &ratios,
                overview: None,
            },
        );
        assert_eq!(resolved.rect(PaneId::TradingOrders), Some(area));
        assert_eq!(resolved.hidden().len(), 3);

        let mut workspaces = Workspaces::default();
        assert_eq!(
            workspaces.toggle_maximize(Preset::Trading, PaneId::TradingChart),
            Some(PaneId::TradingChart)
        );
        assert_eq!(workspaces.focused(Preset::Trading), PaneId::TradingChart);
        assert_eq!(
            workspaces.toggle_maximize(Preset::Trading, PaneId::TradingChart),
            None
        );
        assert_eq!(workspaces.maximized(Preset::Trading), None);
        assert_eq!(workspaces.focused(Preset::Trading), PaneId::TradingChart);

        workspaces.toggle_maximize(Preset::Trading, PaneId::TradingWatch);
        assert_eq!(
            workspaces.toggle_maximize(Preset::Portfolio, PaneId::TradingWatch),
            None
        );
        assert_eq!(
            workspaces.maximized(Preset::Trading),
            Some(PaneId::TradingWatch)
        );
        workspaces.clear_maximize(Preset::Trading);
        assert_eq!(workspaces.maximized(Preset::Trading), None);
    }

    #[test]
    fn a_composite_tiles_every_size_around_its_breakpoint() {
        for preset in [Preset::Trading, Preset::Portfolio, Preset::Derivatives] {
            for width in [0u16, 1, 2, 3, 4, 5, 71, 119, 120, 121, 160, 240] {
                for height in [0u16, 1, 2, 3, 4, 5, 29, 30, 31, 43, 60] {
                    for toast in 0u16..=3 {
                        let area = Rect {
                            x: 0,
                            y: 1,
                            width,
                            height: height.saturating_sub(toast),
                        };
                        let resolved = resolve_composite_at(area, preset, preset.default_focus());
                        assert_tiles(&resolved, area);
                        assert_eq!(
                            resolved.panes().len() + resolved.hidden().len(),
                            preset.panes().len(),
                            "{preset:?} lost a pane at {width}x{height}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_overview_canvases_resolve_cell_for_cell() {
        for (width, height, kpi, positions, holdings, indices, movers, account) in [
            (
                80u16,
                24u16,
                Rect::new(0, 1, 80, 3),
                Rect::new(0, 4, 46, 7),
                Rect::new(0, 11, 46, 11),
                Rect::new(46, 4, 34, 6),
                Rect::new(46, 10, 34, 12),
                Rect::new(0, 22, 80, 1),
            ),
            (
                120,
                35,
                Rect::new(0, 1, 120, 5),
                Rect::new(0, 6, 70, 7),
                Rect::new(0, 13, 70, 18),
                Rect::new(70, 6, 50, 6),
                Rect::new(70, 12, 50, 19),
                Rect::new(0, 31, 120, 3),
            ),
            (
                160,
                45,
                Rect::new(0, 1, 160, 5),
                Rect::new(0, 6, 93, 7),
                Rect::new(0, 13, 93, 28),
                Rect::new(93, 6, 67, 6),
                Rect::new(93, 12, 67, 29),
                Rect::new(0, 41, 160, 3),
            ),
        ] {
            let area = body(width, height);
            let resolved = resolve_overview_at(area, 2, 4, PaneId::OverviewPositions);
            let at = |pane| rect_of(&resolved, pane).unwrap_or(Rect::ZERO);
            assert_eq!(
                (
                    resolved.band(Band::Kpi).unwrap_or(Rect::ZERO),
                    at(PaneId::OverviewPositions),
                    at(PaneId::OverviewHoldings),
                    at(PaneId::OverviewIndices),
                    at(PaneId::OverviewMovers),
                    resolved.band(Band::Account).unwrap_or(Rect::ZERO),
                ),
                (kpi, positions, holdings, indices, movers, account),
                "{width}x{height}"
            );
            assert!(
                hidden_in(&resolved, Preset::Overview).is_empty(),
                "{width}x{height}"
            );
            assert_tiles(&resolved, area);
        }
    }

    #[test]
    fn the_resolver_tiles_every_size_around_every_breakpoint() {
        let mut sizes: Vec<(u16, u16)> = Vec::new();
        for width in [
            0u16, 1, 2, 3, 4, 5, 69, 70, 71, 79, 80, 81, 119, 120, 121, 159, 160, 161,
        ] {
            for height in [
                0u16, 1, 2, 3, 4, 5, 15, 16, 17, 23, 24, 25, 27, 28, 29, 34, 35, 36, 44, 45, 46,
            ] {
                sizes.push((width, height));
            }
        }
        for (width, height) in sizes {
            for toast in 0u16..=3 {
                let area = Rect {
                    x: 0,
                    y: 1,
                    width,
                    height: height.saturating_sub(2).saturating_sub(toast),
                };
                for positions in [0usize, 2, 40] {
                    for focused in [PaneId::OverviewPositions, PaneId::OverviewHoldings] {
                        let resolved = resolve_overview_at(area, positions, 4, focused);
                        assert_tiles(&resolved, area);
                        assert_bands_tile(&resolved, area);
                        for pane in hidden_in(&resolved, Preset::Overview) {
                            assert_eq!(rect_of(&resolved, pane), None);
                        }
                    }
                }
                for preset in Preset::ALL.into_iter().filter(|p| *p != Preset::Overview) {
                    for focused in preset.panes().iter().copied() {
                        let resolved = resolve_composite_at(area, preset, focused);
                        assert_tiles(&resolved, area);
                    }
                }
                let (content, ratios) = overview(2, 4);
                let maximized = resolve(
                    area,
                    &Request {
                        preset: Preset::Overview,
                        focused: PaneId::OverviewHoldings,
                        maximized: Some(PaneId::OverviewHoldings),
                        ratios: &ratios,
                        overview: Some(&content),
                    },
                );
                assert_tiles(&maximized, area);
            }
        }
    }

    #[test]
    fn the_books_seam_follows_the_positions_count_within_its_column() {
        let area = body(120, 35);
        for (positions, pos_height) in [(0usize, 5u16), (2, 7), (10, 15), (24, 19), (400, 19)] {
            let resolved = resolve_overview_at(area, positions, 4, PaneId::OverviewPositions);
            let books = rect_of(&resolved, PaneId::OverviewPositions).unwrap();
            let holdings = rect_of(&resolved, PaneId::OverviewHoldings).unwrap();
            assert_eq!(books.height, pos_height, "{positions} positions");
            assert_eq!(holdings.y, books.bottom());
            assert!(
                holdings.height >= 6,
                "{positions} positions starve holdings"
            );
            assert_tiles(&resolved, area);
        }
    }

    #[test]
    fn the_indices_panel_is_sized_to_its_lines_and_movers_keeps_its_floor() {
        let area = body(120, 35);
        for (rows, height) in [(1usize, 3u16), (3, 5), (4, 6)] {
            let resolved = resolve_overview_at(area, 2, rows, PaneId::OverviewPositions);
            let indices = rect_of(&resolved, PaneId::OverviewIndices).unwrap();
            let movers = rect_of(&resolved, PaneId::OverviewMovers).unwrap();
            assert_eq!(indices.height, height, "{rows} index rows");
            assert_eq!(movers.y, indices.bottom());
            assert!(movers.height >= 4);
        }
    }

    #[test]
    fn the_overview_records_both_of_its_draggable_seams() {
        let area = body(120, 35);
        let resolved = resolve_overview_at(area, 2, 4, PaneId::OverviewPositions);
        let seams: Vec<Split> = resolved.seams().iter().map(|seam| seam.split).collect();
        assert_eq!(
            seams,
            [Split::OverviewBooks, Split::OverviewMain],
            "innermost seam first"
        );
        let books = resolved
            .seams()
            .iter()
            .find(|seam| seam.split == Split::OverviewBooks)
            .unwrap();
        assert_eq!(
            books.first,
            rect_of(&resolved, PaneId::OverviewPositions).unwrap()
        );
        assert_eq!(
            books.second,
            rect_of(&resolved, PaneId::OverviewHoldings).unwrap()
        );
        let main = resolved
            .seams()
            .iter()
            .find(|seam| seam.split == Split::OverviewMain)
            .unwrap();
        assert_eq!(
            main.span,
            rect_of(&resolved, PaneId::OverviewPositions)
                .unwrap()
                .union(rect_of(&resolved, PaneId::OverviewMovers).unwrap())
        );
    }

    #[test]
    fn a_dragged_seam_replaces_the_content_and_ratio_extents() {
        let area = body(120, 35);
        let (content, mut ratios) = overview(2, 4);
        ratios.set(Split::OverviewMain, 0.25);
        ratios.set(Split::OverviewBooks, 0.5);
        let resolved = resolve(
            area,
            &Request {
                preset: Preset::Overview,
                focused: PaneId::OverviewPositions,
                maximized: None,
                ratios: &ratios,
                overview: Some(&content),
            },
        );
        let positions = rect_of(&resolved, PaneId::OverviewPositions).unwrap();
        assert_eq!(positions.width, 40);
        assert_eq!(positions.height, 13);
        assert_tiles(&resolved, area);
    }

    #[test]
    fn the_undragged_left_column_is_where_percentage_58_put_it() {
        let books = Extent::Ratio {
            bps: super::BOOKS_BPS,
            minimums: Split::OverviewMain.minimums(),
        };
        for width in [70u16, 80, 100, 120, 133, 160, 200, 240] {
            let area = Rect {
                x: 0,
                y: 0,
                width,
                height: 30,
            };
            let [left, _] =
                Layout::horizontal([Constraint::Percentage(58), Constraint::Min(0)]).areas(area);
            assert_eq!(
                left.width,
                books.length(width),
                "the left column moved at width {width}"
            );
        }
    }
}
