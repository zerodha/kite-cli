use crate::tui::app::ChartTarget;
use crate::tui::instruments::OptionChain;
use crate::tui::marketwatch::{Watch, WatchItem};
use crate::tui::pane_state::{ChartId, ChartPane};
use crate::tui::ui;
use crate::tui::workspace::PaneId;
use ratatui::layout::Rect;
use std::cell::{Cell, RefCell};

pub struct PaneHits<'a> {
    map: &'a RefCell<ui::HitMap>,
    owner: PaneId,
    list: ui::HitList,
}

impl<'a> PaneHits<'a> {
    pub fn new(map: &'a RefCell<ui::HitMap>, owner: PaneId) -> Self {
        Self {
            map,
            owner,
            list: ui::HitList::Pane(owner),
        }
    }

    pub fn under(&self, list: ui::HitList) -> PaneHits<'a> {
        PaneHits {
            map: self.map,
            owner: self.owner,
            list,
        }
    }

    pub fn owner(&self) -> PaneId {
        self.owner
    }

    pub fn push(&self, rect: Rect, action: ui::HitAction) {
        self.map.borrow_mut().push(rect, action);
    }

    pub fn push_rows(&self, rect: Rect, offset: usize) {
        self.push(
            rect,
            ui::HitAction::Rows {
                list: self.list,
                offset,
            },
        );
    }

    pub fn push_chart(&self, chart: ChartId, hit: ui::ChartHit) {
        self.map.borrow_mut().push_chart(chart, hit);
    }
}

pub struct BookList<'a, S> {
    pub selected: usize,
    pub sort: S,
    pub display: &'a [usize],
    pub viewport: &'a Cell<usize>,
    pub focus: bool,
}

impl<S> BookList<'_, S> {
    pub fn title(&self, title: &str) -> String {
        ui::focus_title(title.trim(), self.focus)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineChart {
    Closed,
    Open { maximized: bool },
}

pub struct InlineChartInput<'a> {
    pub open: InlineChart,
    pub target: Option<&'a ChartTarget>,
    pub chart: &'a ChartPane,
}

pub struct PlainList<'a> {
    pub selected: usize,
    pub focused: bool,
    pub viewport: Option<&'a Cell<usize>>,
}

pub struct WatchInput<'a> {
    pub rows: &'a [WatchItem],
    pub watch: Watch<'a, 'a>,
    pub selected: usize,
    pub viewport: &'a Cell<usize>,
    pub focused: bool,
}

pub struct ChainInput<'a> {
    pub chain: Option<&'a OptionChain>,
    pub selected: usize,
    pub call_focus: bool,
    pub spot_token: Option<u32>,
    pub atm_strike: Option<f64>,
    pub underlying: &'a str,
}

pub struct BidsInput<'a> {
    pub section: crate::tui::bids::Section,
    pub application_focus: bool,
    pub selected: usize,
    pub viewport: &'a Cell<usize>,
    pub focused: bool,
}
