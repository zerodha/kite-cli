use crate::cli::StreamMode;
use crate::tui::app::{
    AlertsSort, ChartHover, ChartTarget, GttSort, HoldingsSort, MfSort, OrdersSort, PositionsSort,
};
use crate::tui::bids::Section as BidsSection;
use crate::tui::candles::{CandleStore, SeriesKey};
use crate::tui::data::ChartInterval;
use crate::tui::indicators::{self, ChartIndicator};
use crate::tui::instruments::{OptionChain, OptionLeg, OptionRow};
use crate::tui::marketwatch::{Follow, ItemId, State as WatchStore, WatchId, WatchItem};
use crate::tui::subscriptions::{Demands, Reserve};
use crate::tui::workspace::PaneId;
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PositionKey {
    pub exchange: String,
    pub symbol: String,
    pub product: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoldingKey {
    pub exchange: String,
    pub symbol: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cursor<K> {
    Unset,
    Record(K),
    Lost,
}

struct OrderMemo<S> {
    generation: u64,
    sort: S,
    order: Vec<usize>,
}

pub struct ListPane<K, S> {
    row: usize,
    cursor: Cursor<K>,
    sort: S,
    offset: Cell<usize>,
    memo: RefCell<Option<OrderMemo<S>>>,
}

impl<K, S: Default> Default for ListPane<K, S> {
    fn default() -> Self {
        Self {
            row: 0,
            cursor: Cursor::Unset,
            sort: S::default(),
            offset: Cell::new(0),
            memo: RefCell::new(None),
        }
    }
}

impl<K, S: Copy> ListPane<K, S> {
    pub fn sort(&self) -> S {
        self.sort
    }

    pub fn set_sort(&mut self, sort: S) {
        self.sort = sort;
    }
}

impl<K, S> ListPane<K, S> {
    pub fn viewport(&self) -> &Cell<usize> {
        &self.offset
    }
}

impl<K: PartialEq, S: Copy + PartialEq> ListPane<K, S> {
    pub fn cached_order(
        &self,
        generation: u64,
        compute: impl FnOnce(S) -> Vec<usize>,
    ) -> Vec<usize> {
        if let Some(memo) = self.memo.borrow().as_ref()
            && memo.generation == generation
            && memo.sort == self.sort
        {
            return memo.order.clone();
        }
        let order = compute(self.sort);
        *self.memo.borrow_mut() = Some(OrderMemo {
            generation,
            sort: self.sort,
            order: order.clone(),
        });
        order
    }
}

impl<K: PartialEq, S> ListPane<K, S> {
    pub fn display_row(&self, order: &[usize], key: impl Fn(usize) -> Option<K>) -> usize {
        let clamped = self.row.min(order.len().saturating_sub(1));
        match &self.cursor {
            Cursor::Record(chosen) => position_of(order, chosen, key).unwrap_or(clamped),
            Cursor::Unset | Cursor::Lost => clamped,
        }
    }

    pub fn target(&self, order: &[usize], key: impl Fn(usize) -> Option<K>) -> Option<usize> {
        match &self.cursor {
            Cursor::Unset => order
                .get(self.row.min(order.len().saturating_sub(1)))
                .copied(),
            Cursor::Record(chosen) => position_of(order, chosen, key).map(|row| order[row]),
            Cursor::Lost => None,
        }
    }

    pub fn select(
        &mut self,
        row: usize,
        order: &[usize],
        key: impl Fn(usize) -> Option<K>,
    ) -> bool {
        if order.is_empty() {
            return false;
        }
        let previous = (self.display_row(order, &key), self.target(order, &key));
        self.row = row.min(order.len() - 1);
        self.cursor = match key(order[self.row]) {
            Some(chosen) => Cursor::Record(chosen),
            None => Cursor::Lost,
        };
        (self.row, self.target(order, &key)) != previous
    }

    pub fn step(
        &mut self,
        delta: isize,
        order: &[usize],
        key: impl Fn(usize) -> Option<K>,
    ) -> bool {
        if order.is_empty() {
            return false;
        }
        let row = self
            .display_row(order, &key)
            .saturating_add_signed(delta)
            .min(order.len() - 1);
        self.select(row, order, key)
    }

    pub fn reconcile(&mut self, order: &[usize], key: impl Fn(usize) -> Option<K>) {
        let clamped = self.row.min(order.len().saturating_sub(1));
        match &self.cursor {
            Cursor::Unset => {
                self.row = clamped;
                if let Some(chosen) = order.get(clamped).and_then(|&natural| key(natural)) {
                    self.cursor = Cursor::Record(chosen);
                }
            }
            Cursor::Record(chosen) => {
                if let Some(row) = position_of(order, chosen, key) {
                    self.row = row;
                } else {
                    self.cursor = Cursor::Lost;
                    self.row = clamped;
                }
            }
            Cursor::Lost => self.row = clamped,
        }
    }
}

fn position_of<K: PartialEq>(
    order: &[usize],
    chosen: &K,
    key: impl Fn(usize) -> Option<K>,
) -> Option<usize> {
    order
        .iter()
        .position(|&natural| key(natural).as_ref() == Some(chosen))
}

pub type PositionsPane = ListPane<PositionKey, PositionsSort>;
pub type HoldingsPane = ListPane<HoldingKey, HoldingsSort>;
pub type MfPane = ListPane<String, MfSort>;
pub type GttPane = ListPane<i64, GttSort>;
pub type AlertsPane = ListPane<String, AlertsSort>;

pub type SnapshotPane<K> = ListPane<K, ()>;

#[derive(Default)]
pub struct OrdersPane {
    pub list: ListPane<String, OrdersSort>,
    marked: HashSet<String>,
}

impl OrdersPane {
    pub fn is_marked(&self, order_id: &str) -> bool {
        self.marked.contains(order_id)
    }

    pub fn has_marks(&self) -> bool {
        !self.marked.is_empty()
    }

    pub fn toggle_mark(&mut self, order_id: String) {
        if !self.marked.remove(&order_id) {
            self.marked.insert(order_id);
        }
    }

    pub fn toggle_marks(&mut self, order_ids: Vec<String>) {
        if order_ids.is_empty() {
            return;
        }
        if order_ids.iter().all(|id| self.marked.contains(id)) {
            for id in order_ids {
                self.marked.remove(&id);
            }
        } else {
            self.marked.extend(order_ids);
        }
    }

    pub fn retain_marks(&mut self, keep: impl Fn(&str) -> bool) {
        self.marked.retain(|order_id| keep(order_id));
    }
}

#[derive(Default)]
pub struct Books {
    pub overview_positions: SnapshotPane<PositionKey>,
    pub overview_holdings: SnapshotPane<HoldingKey>,
    pub trading_orders: OrdersPane,
    pub portfolio_positions: PositionsPane,
    pub portfolio_holdings: HoldingsPane,
    pub orders: OrdersPane,
    pub gtt: GttPane,
    pub alerts: AlertsPane,
    pub mf: MfPane,
}

fn natural_order(len: usize) -> Vec<usize> {
    (0..len).collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WatchRowId {
    Item(ItemId),
    Listing { exchange: String, token: u32 },
}

fn watch_row_id(item: &WatchItem) -> WatchRowId {
    if item.id.is_empty() {
        WatchRowId::Listing {
            exchange: item.exchange.clone(),
            token: item.instrument_token,
        }
    } else {
        WatchRowId::Item(item.id.clone())
    }
}

#[derive(Default)]
pub struct WatchPane {
    watch: Option<WatchId>,
    list: ListPane<WatchRowId, ()>,
    follow: Option<Follow>,
}

impl WatchPane {
    pub fn opened(watch: Option<WatchId>) -> Self {
        Self {
            watch,
            ..Self::default()
        }
    }

    pub fn watch(&self) -> Option<&WatchId> {
        self.watch.as_ref()
    }

    pub fn open(&mut self, watch: WatchId) {
        self.watch = Some(watch);
        self.list = ListPane::default();
        self.follow = None;
    }

    pub fn close(&mut self) {
        self.watch = None;
        self.list = ListPane::default();
        self.follow = None;
    }

    pub fn viewport(&self) -> &Cell<usize> {
        self.list.viewport()
    }

    pub fn rows<'a>(&'a self, store: &'a WatchStore) -> &'a [WatchItem] {
        store.watch(self.watch.as_ref()).rows()
    }

    pub fn display_row(&self, store: &WatchStore) -> usize {
        let rows = self.rows(store);
        self.list.display_row(&natural_order(rows.len()), |i| {
            rows.get(i).map(watch_row_id)
        })
    }

    pub fn target(&self, store: &WatchStore) -> Option<usize> {
        let rows = self.rows(store);
        self.list.target(&natural_order(rows.len()), |i| {
            rows.get(i).map(watch_row_id)
        })
    }

    pub fn selected<'a>(&'a self, store: &'a WatchStore) -> Option<&'a WatchItem> {
        self.rows(store).get(self.target(store)?)
    }

    pub fn select(&mut self, row: usize, store: &WatchStore) -> bool {
        let rows = store.watch(self.watch.as_ref()).rows();
        self.list.select(row, &natural_order(rows.len()), |i| {
            rows.get(i).map(watch_row_id)
        })
    }

    pub fn step(&mut self, delta: isize, store: &WatchStore) -> bool {
        let rows = store.watch(self.watch.as_ref()).rows();
        self.list.step(delta, &natural_order(rows.len()), |i| {
            rows.get(i).map(watch_row_id)
        })
    }

    pub fn reconcile(&mut self, store: &WatchStore) {
        let rows = store.watch(self.watch.as_ref()).rows();
        self.list.reconcile(&natural_order(rows.len()), |i| {
            rows.get(i).map(watch_row_id)
        });
    }

    pub fn expect(&mut self, follow: Option<Follow>) {
        self.follow = follow;
    }

    pub fn follow_confirmed(&mut self, store: &WatchStore) -> bool {
        let followed = self.follow.take();
        self.follow_row(followed.as_ref(), store)
    }

    pub fn follow_restored(&mut self, store: &WatchStore) -> bool {
        let followed = self.follow.clone();
        self.follow_row(followed.as_ref(), store)
    }

    fn follow_row(&mut self, follow: Option<&Follow>, store: &WatchStore) -> bool {
        let id = self.watch.clone();
        let Some(row) = follow_row(follow, store.watch(id.as_ref()).rows()) else {
            return false;
        };
        self.select(row, store);
        true
    }

    pub fn demands(&self, store: &WatchStore, detail_visible: bool) -> Demands {
        let mut demands = Demands::default();
        for item in self.rows(store) {
            demands.want(Reserve::Visible, item.instrument_token, StreamMode::Quote);
        }
        if detail_visible && let Some(item) = self.selected(store) {
            demands.want(Reserve::Target, item.instrument_token, StreamMode::Full);
        }
        demands
    }
}

fn follow_row(follow: Option<&Follow>, rows: &[WatchItem]) -> Option<usize> {
    match follow? {
        Follow::Listing {
            exchange,
            tradingsymbol,
        } => rows.iter().position(|row| {
            row.tradingsymbol == *tradingsymbol && row.exchange.eq_ignore_ascii_case(exchange)
        }),
        Follow::Item(id) => rows.iter().position(|row| row.id == *id),
    }
}

pub struct ChainPane {
    underlying: String,
    expiry: Option<String>,
    chain: Option<OptionChain>,
    spot_token: Option<u32>,
    call_focus: bool,
    recentered: bool,
    list: ListPane<f64, ()>,
}

impl Default for ChainPane {
    fn default() -> Self {
        Self::new("NIFTY")
    }
}

impl ChainPane {
    pub fn new(underlying: &str) -> Self {
        Self {
            underlying: underlying.to_string(),
            expiry: None,
            chain: None,
            spot_token: None,
            call_focus: true,
            recentered: false,
            list: ListPane::default(),
        }
    }

    pub fn underlying(&self) -> &str {
        &self.underlying
    }

    pub fn expiry(&self) -> Option<&str> {
        self.expiry.as_deref()
    }

    pub fn chain(&self) -> Option<&OptionChain> {
        self.chain.as_ref()
    }

    pub fn rows(&self) -> &[OptionRow] {
        self.chain
            .as_ref()
            .map_or(&[][..], |chain| chain.rows.as_slice())
    }

    pub fn spot_token(&self) -> Option<u32> {
        self.spot_token
    }

    pub fn call_focus(&self) -> bool {
        self.call_focus
    }

    pub fn set_call_focus(&mut self, call: bool) {
        self.call_focus = call;
    }

    pub fn set_underlying(&mut self, underlying: String) {
        self.underlying = underlying;
        self.expiry = None;
    }

    pub fn set_expiry(&mut self, expiry: Option<String>) {
        self.expiry = expiry;
    }

    pub fn install(&mut self, chain: OptionChain, spot_token: Option<u32>, spot: Option<f64>) {
        self.expiry.clone_from(&chain.expiry);
        self.spot_token = spot_token;
        self.chain = Some(chain);
        self.recentered = false;
        self.list = ListPane::default();
        if let Some(row) = self.atm_row(spot) {
            self.select(row);
        }
    }

    pub fn clear(&mut self) {
        self.chain = None;
        self.list = ListPane::default();
    }

    pub fn recenter(&mut self, spot: Option<f64>) -> bool {
        if self.recentered || spot.is_none() {
            return false;
        }
        let Some(row) = self.atm_row(spot) else {
            return false;
        };
        self.recentered = true;
        self.select(row)
    }

    pub fn atm_row(&self, spot: Option<f64>) -> Option<usize> {
        let rows = self.rows();
        if rows.is_empty() {
            return None;
        }
        match spot {
            Some(spot) => rows
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    (a.strike - spot)
                        .abs()
                        .partial_cmp(&(b.strike - spot).abs())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(idx, _)| idx),
            None => Some(rows.len() / 2),
        }
    }

    pub fn display_row(&self) -> usize {
        self.list
            .display_row(&self.order(), |i| self.rows().get(i).map(|row| row.strike))
    }

    pub fn target(&self) -> Option<usize> {
        self.list
            .target(&self.order(), |i| self.rows().get(i).map(|row| row.strike))
    }

    pub fn selected(&self) -> Option<&OptionRow> {
        self.rows().get(self.target()?)
    }

    pub fn select(&mut self, row: usize) -> bool {
        let order = self.order();
        let rows = self
            .chain
            .as_ref()
            .map_or(&[][..], |chain| chain.rows.as_slice());
        self.list
            .select(row, &order, |i| rows.get(i).map(|row| row.strike))
    }

    pub fn step(&mut self, delta: isize) -> bool {
        let order = self.order();
        let rows = self
            .chain
            .as_ref()
            .map_or(&[][..], |chain| chain.rows.as_slice());
        self.list
            .step(delta, &order, |i| rows.get(i).map(|row| row.strike))
    }

    pub fn focused_leg(&self) -> Option<Result<OptionLeg, f64>> {
        let row = self.selected()?;
        let leg = if self.call_focus {
            row.call.clone()
        } else {
            row.put.clone()
        };
        Some(leg.ok_or(row.strike))
    }

    fn order(&self) -> Vec<usize> {
        natural_order(self.rows().len())
    }

    pub fn demands(&self) -> Demands {
        let mut demands = Demands::default();
        if let Some(token) = self.spot_token {
            demands.want(Reserve::Visible, token, StreamMode::Quote);
        }
        let rows = self.rows();
        let center = self.display_row();
        let mut order: Vec<usize> = (0..rows.len()).collect();
        order.sort_by_key(|&i| i.abs_diff(center));
        for i in order {
            let row = &rows[i];
            for leg in [row.call.as_ref(), row.put.as_ref()].into_iter().flatten() {
                demands.want(Reserve::Wing, leg.instrument_token, StreamMode::Full);
            }
        }
        demands
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChartId(u32);

impl ChartId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartBinding {
    SelectionOf(PaneId),
    UnderlyingOf(PaneId),
}

impl ChartBinding {
    pub fn of_book(pane: PaneId) -> Option<Self> {
        matches!(
            pane,
            PaneId::PortfolioPositions
                | PaneId::PortfolioHoldings
                | PaneId::TradingOrders
                | PaneId::OrdersBook
        )
        .then_some(Self::SelectionOf(pane))
    }

    pub fn source(self) -> PaneId {
        match self {
            Self::SelectionOf(pane) | Self::UnderlyingOf(pane) => pane,
        }
    }
}

struct IndicatorMemo {
    key: (SeriesKey, ChartIndicator, u64),
    series: indicators::Series,
}

pub struct ChartPane {
    id: ChartId,
    binding: ChartBinding,
    resolved: Option<ChartTarget>,
    interval: ChartInterval,
    volume: bool,
    indicator: ChartIndicator,
    cursor: Option<usize>,
    hover: Option<ChartHover>,
    memo: RefCell<Option<IndicatorMemo>>,
}

impl ChartPane {
    pub fn new(
        id: ChartId,
        binding: ChartBinding,
        interval: ChartInterval,
        volume: bool,
        indicator: ChartIndicator,
    ) -> Self {
        Self {
            id,
            binding,
            resolved: None,
            interval,
            volume,
            indicator,
            cursor: None,
            hover: None,
            memo: RefCell::new(None),
        }
    }

    pub fn id(&self) -> ChartId {
        self.id
    }

    pub fn binding(&self) -> ChartBinding {
        self.binding
    }

    pub fn bind(&mut self, binding: ChartBinding) -> bool {
        if self.binding == binding {
            return false;
        }
        self.binding = binding;
        self.cursor = None;
        self.hover = None;
        true
    }

    pub fn target(&self) -> Option<&ChartTarget> {
        self.resolved.as_ref()
    }

    pub fn resolve(&mut self, target: Option<ChartTarget>) -> bool {
        if self.resolved == target {
            return false;
        }
        self.resolved = target;
        self.cursor = None;
        self.hover = None;
        true
    }

    pub fn token(&self) -> Option<u32> {
        self.resolved
            .as_ref()
            .map(|target| target.instrument_token)
            .filter(|token| *token != 0)
    }

    pub fn key(&self) -> Option<SeriesKey> {
        self.token()
            .map(|token| SeriesKey::new(token, self.interval))
    }

    pub fn key_for(&self, token: u32) -> SeriesKey {
        SeriesKey::new(token, self.interval)
    }

    pub fn interval(&self) -> ChartInterval {
        self.interval
    }

    pub fn cycle_interval(&mut self) {
        self.interval = self.interval.cycle();
        self.cursor = None;
        self.hover = None;
    }

    pub fn volume(&self) -> bool {
        self.volume
    }

    pub fn toggle_volume(&mut self) {
        self.volume = !self.volume;
    }

    pub fn indicator(&self) -> ChartIndicator {
        self.indicator
    }

    pub fn next_indicator(&mut self) {
        self.indicator = self.indicator.cycle();
    }

    #[cfg(test)]
    pub fn set_indicator(&mut self, indicator: ChartIndicator) {
        self.indicator = indicator;
    }

    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Option<usize>) {
        self.cursor = cursor;
    }

    pub fn move_cursor(&mut self, older: bool, len: usize) {
        if len == 0 {
            return;
        }
        self.cursor = match (self.cursor, older) {
            (None, true) => Some(1.min(len - 1)),
            (None | Some(0), false) => None,
            (Some(offset), true) => Some((offset + 1).min(len - 1)),
            (Some(offset), false) => Some(offset - 1),
        };
    }

    pub fn hover(&self) -> Option<&ChartHover> {
        self.hover.as_ref()
    }

    pub fn hover_for(&self, token: u32) -> Option<&ChartHover> {
        self.hover
            .as_ref()
            .filter(|hover| hover.token == token && hover.interval == self.interval)
    }

    pub fn set_hover(&mut self, hover: Option<ChartHover>) {
        self.hover = hover;
    }

    pub fn inspect(&mut self, mut hover: ChartHover, anchored: Option<(String, usize)>) {
        if let Some((window_ts, visible_n)) = anchored {
            hover.window_ts = window_ts;
            hover.visible_n = visible_n;
        }
        self.hover = Some(hover);
    }

    pub fn seed_cursor_from_hover(&mut self, hover: &ChartHover, store: &CandleStore) {
        if Some(hover.token) != self.token() || hover.interval != self.interval {
            return;
        }
        if let Some(offset) = self.candle_offset(&hover.ts, store) {
            self.cursor = Some(offset);
        }
    }

    pub fn candle_offset(&self, ts: &str, store: &CandleStore) -> Option<usize> {
        let candles = store.candles(self.key()?)?;
        let index = candles
            .iter()
            .rposition(|candle| crate::tui::data::candle_timestamp(candle) == ts)?;
        Some(candles.len() - 1 - index)
    }

    pub fn indicator_series(&self, token: u32, store: &CandleStore) -> Ref<'_, indicators::Series> {
        let key = self.key_for(token);
        let memo_key = (key, self.indicator, store.generation(key));
        let stale = self.memo.borrow().as_ref().map(|memo| memo.key) != Some(memo_key);
        if stale {
            let candles = store.candles(key).unwrap_or_default();
            let series = indicators::series(self.indicator, candles);
            *self.memo.borrow_mut() = Some(IndicatorMemo {
                key: memo_key,
                series,
            });
        }
        Ref::map(self.memo.borrow(), |memo| {
            &memo
                .as_ref()
                .expect("the memo was just populated above")
                .series
        })
    }

    pub fn demands(&self) -> Demands {
        let mut demands = Demands::default();
        if let Some(token) = self.token() {
            demands.want(Reserve::Target, token, StreamMode::Full);
        }
        demands
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BidsRowId {
    Instrument(String),
    Application { symbol: String, created_at: String },
    Auction(String),
}

#[derive(Default)]
pub struct BidsPane {
    section: BidsSection,
    application_focus: bool,
    list: ListPane<BidsRowId, ()>,
}

impl BidsPane {
    pub fn section(&self) -> BidsSection {
        self.section
    }

    pub fn application_focus(&self) -> bool {
        self.application_focus
    }

    pub fn set_section(&mut self, section: BidsSection) -> bool {
        if self.section == section {
            return false;
        }
        self.section = section;
        self.focus_instruments();
        true
    }

    pub fn focus_applications(&mut self) {
        self.application_focus = true;
        self.list = ListPane::default();
    }

    fn focus_instruments(&mut self) {
        self.application_focus = false;
        self.list = ListPane::default();
    }

    pub fn toggle_application_focus(&mut self) {
        if self.application_focus {
            self.focus_instruments();
        } else {
            self.focus_applications();
        }
    }

    pub fn viewport(&self) -> &Cell<usize> {
        self.list.viewport()
    }

    pub fn display_row(&self, keys: &[Option<BidsRowId>]) -> usize {
        self.list.display_row(&natural_order(keys.len()), |i| {
            keys.get(i).cloned().flatten()
        })
    }

    pub fn target(&self, keys: &[Option<BidsRowId>]) -> Option<usize> {
        self.list.target(&natural_order(keys.len()), |i| {
            keys.get(i).cloned().flatten()
        })
    }

    pub fn select(&mut self, row: usize, keys: &[Option<BidsRowId>]) -> bool {
        self.list.select(row, &natural_order(keys.len()), |i| {
            keys.get(i).cloned().flatten()
        })
    }

    pub fn step(&mut self, delta: isize, keys: &[Option<BidsRowId>]) -> bool {
        self.list.step(delta, &natural_order(keys.len()), |i| {
            keys.get(i).cloned().flatten()
        })
    }

    pub fn reconcile(&mut self, keys: &[Option<BidsRowId>]) {
        self.list.reconcile(&natural_order(keys.len()), |i| {
            keys.get(i).cloned().flatten()
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Cursor, ListPane, PositionKey, PositionsPane};
    use crate::tui::app::PositionsSort;
    use std::cell::Cell;

    fn key(symbol: &str) -> PositionKey {
        PositionKey {
            exchange: "NSE".to_string(),
            symbol: symbol.to_string(),
            product: "MIS".to_string(),
        }
    }

    const BOOK: [&str; 3] = ["AAA", "BBB", "CCC"];

    fn record(natural: usize) -> Option<PositionKey> {
        BOOK.get(natural).copied().map(key)
    }

    #[test]
    fn the_order_memo_is_per_instance_and_keyed_by_generation_and_sort() {
        let mut one = PositionsPane::default();
        let two = PositionsPane::default();
        let computed = Cell::new(0);
        let compute = |_sort: PositionsSort| {
            computed.set(computed.get() + 1);
            vec![0, 1, 2]
        };

        assert_eq!(one.cached_order(7, compute), vec![0, 1, 2]);
        assert_eq!(computed.get(), 1);
        one.cached_order(7, compute);
        assert_eq!(computed.get(), 1, "same generation and sort is a hit");

        one.cached_order(8, compute);
        assert_eq!(computed.get(), 2, "new data generation is a miss");

        two.cached_order(8, compute);
        assert_eq!(
            computed.get(),
            3,
            "the second instance has its own memo, so it misses at the same key"
        );

        one.set_sort(PositionsSort::Symbol);
        one.cached_order(8, compute);
        assert_eq!(computed.get(), 4, "a new sort mode is a miss");
        two.cached_order(8, compute);
        assert_eq!(
            computed.get(),
            4,
            "and re-sorting one instance did not evict the other's memo"
        );
    }

    #[test]
    fn a_reorder_moves_the_highlight_with_the_record() {
        let mut pane: ListPane<PositionKey, ()> = ListPane::default();
        let order = [0, 1, 2];
        pane.select(1, &order, record);
        assert_eq!(pane.target(&order, record), Some(1), "BBB");

        let reversed = [2, 1, 0];
        pane.reconcile(&reversed, record);
        assert_eq!(pane.display_row(&reversed, record), 1);
        assert_eq!(pane.target(&reversed, record), Some(1), "still BBB");

        let moved = [1, 0, 2];
        pane.reconcile(&moved, record);
        assert_eq!(
            pane.display_row(&moved, record),
            0,
            "BBB is now the top row"
        );
    }

    #[test]
    fn a_vanished_record_clears_the_target_rather_than_taking_a_neighbour() {
        let mut pane: ListPane<PositionKey, ()> = ListPane::default();
        let order = [0, 1, 2];
        pane.select(2, &order, record);
        assert_eq!(pane.target(&order, record), Some(2), "CCC");

        let shrunk = [0, 1];
        pane.reconcile(&shrunk, record);
        assert_eq!(pane.cursor, Cursor::Lost);
        assert_eq!(pane.target(&shrunk, record), None, "no silent neighbour");
        assert_eq!(pane.display_row(&shrunk, record), 1, "clamped, not reset");

        pane.select(0, &shrunk, record);
        assert_eq!(pane.target(&shrunk, record), Some(0), "AAA");
    }

    #[test]
    fn an_untouched_pane_adopts_the_row_it_already_points_at() {
        let mut pane: ListPane<PositionKey, ()> = ListPane::default();
        let order = [0, 1, 2];
        assert_eq!(pane.target(&order, record), Some(0));

        pane.reconcile(&order, record);
        assert_eq!(pane.cursor, Cursor::Record(key("AAA")));

        let reversed = [2, 1, 0];
        pane.reconcile(&reversed, record);
        assert_eq!(pane.target(&reversed, record), Some(0), "still AAA");
    }
}

#[cfg(test)]
mod pane_tests {
    use super::{BidsPane, BidsRowId, ChainPane};
    use crate::instruments::{OptionChain, OptionLeg, OptionRow};
    use crate::tui::bids::Section;

    fn ladder(underlying: &str, expiry: &str, strikes: &[f64]) -> OptionChain {
        OptionChain {
            underlying: underlying.to_string(),
            expiries: vec![expiry.to_string(), "2026-10-29".to_string()],
            expiry: Some(expiry.to_string()),
            rows: strikes
                .iter()
                .enumerate()
                .map(|(index, strike)| {
                    let index = u32::try_from(index).expect("fixture index fits u32");
                    OptionRow {
                        strike: *strike,
                        call: Some(leg(underlying, *strike, "CE", 1_000 + index)),
                        put: Some(leg(underlying, *strike, "PE", 2_000 + index)),
                    }
                })
                .collect(),
        }
    }

    fn leg(underlying: &str, strike: f64, side: &str, token: u32) -> OptionLeg {
        OptionLeg {
            instrument_token: token,
            tradingsymbol: format!("{underlying}{strike:.0}{side}"),
            exchange: "NFO".to_string(),
            lot_size: 50,
            tick_size: 0.05,
        }
    }

    fn focused_symbol(pane: &ChainPane) -> String {
        match pane.focused_leg().expect("a selected strike") {
            Ok(leg) => leg.tradingsymbol,
            Err(strike) => panic!("no leg at {strike}"),
        }
    }

    #[test]
    fn two_chain_panes_on_different_underlyings_keep_their_own_ladders() {
        let mut nifty = ChainPane::new("NIFTY");
        let mut bank = ChainPane::new("BANKNIFTY");
        nifty.install(
            ladder("NIFTY", "2026-09-24", &[24_900.0, 25_000.0, 25_100.0]),
            Some(256_265),
            Some(25_000.0),
        );
        bank.install(
            ladder("BANKNIFTY", "2026-09-30", &[55_000.0, 56_000.0]),
            Some(260_105),
            Some(56_000.0),
        );

        assert_eq!(nifty.display_row(), 1, "25000 is NIFTY's ATM");
        assert_eq!(bank.display_row(), 1, "56000 is BANKNIFTY's ATM");
        assert_eq!(nifty.spot_token(), Some(256_265));
        assert_eq!(bank.spot_token(), Some(260_105));
        assert_eq!(nifty.expiry(), Some("2026-09-24"));
        assert_eq!(bank.expiry(), Some("2026-09-30"));

        nifty.step(-1);
        nifty.set_call_focus(false);
        assert_eq!(focused_symbol(&nifty), "NIFTY24900PE");
        assert_eq!(bank.display_row(), 1);
        assert!(bank.call_focus());
        assert_eq!(focused_symbol(&bank), "BANKNIFTY56000CE");

        bank.install(
            ladder("BANKNIFTY", "2026-09-30", &[24_900.0, 25_000.0]),
            Some(260_105),
            None,
        );
        assert_eq!(
            focused_symbol(&nifty),
            "NIFTY24900PE",
            "the NIFTY pane still holds its own 24900 put"
        );
    }

    #[test]
    fn a_new_expiry_is_a_new_ladder_rather_than_a_reordered_one() {
        let mut pane = ChainPane::new("NIFTY");
        pane.install(
            ladder("NIFTY", "2026-09-24", &[24_900.0, 25_000.0, 25_100.0]),
            None,
            Some(25_100.0),
        );
        assert_eq!(pane.display_row(), 2);

        pane.set_expiry(Some("2026-10-29".to_string()));
        pane.install(
            ladder("NIFTY", "2026-10-29", &[24_900.0, 25_000.0, 25_100.0]),
            None,
            None,
        );
        assert_eq!(
            pane.display_row(),
            1,
            "the median strike of the new ladder, not the row the old one left"
        );
        assert_eq!(pane.expiry(), Some("2026-10-29"));
    }

    #[test]
    fn the_first_spot_recentres_the_ladder_once() {
        let mut pane = ChainPane::new("NIFTY");
        pane.install(
            ladder(
                "NIFTY",
                "2026-09-24",
                &[24_800.0, 24_900.0, 25_000.0, 25_100.0],
            ),
            Some(256_265),
            None,
        );
        assert_eq!(pane.display_row(), 2, "the median row");

        assert!(pane.recenter(Some(24_800.0)));
        assert_eq!(pane.display_row(), 0);

        pane.step(3);
        assert!(
            !pane.recenter(Some(24_800.0)),
            "the guard fires once per build, so a scroll stands"
        );
        assert_eq!(pane.display_row(), 3);
    }

    fn issues(labels: &[&str]) -> Vec<Option<BidsRowId>> {
        labels
            .iter()
            .map(|label| Some(BidsRowId::Instrument(label.to_string())))
            .collect()
    }

    fn applications(symbols: &[&str]) -> Vec<Option<BidsRowId>> {
        symbols
            .iter()
            .map(|symbol| {
                Some(BidsRowId::Application {
                    symbol: symbol.to_string(),
                    created_at: "2026-09-10".to_string(),
                })
            })
            .collect()
    }

    #[test]
    fn two_bids_panes_keep_their_own_section_and_application_focus() {
        let mut ipo = BidsPane::default();
        let mut gsec = BidsPane::default();
        assert_eq!(ipo.section(), Section::Ipo, "the page opens on IPO");
        assert!(gsec.set_section(Section::Government));
        assert!(
            !gsec.set_section(Section::Government),
            "showing the section it already shows is not a change"
        );

        ipo.focus_applications();
        assert!(ipo.application_focus());
        assert_eq!(ipo.section(), Section::Ipo);
        assert!(
            !gsec.application_focus(),
            "the other pane's focus is its own"
        );
        assert_eq!(gsec.section(), Section::Government);

        let applied = applications(&["ALPHA", "BETA", "GAMMA"]);
        let gsecs = issues(&["TBILL91", "SGB2033"]);
        ipo.select(2, &applied);
        gsec.select(1, &gsecs);
        assert_eq!(ipo.target(&applied), Some(2));
        assert_eq!(gsec.target(&gsecs), Some(1));

        let fewer = applications(&["BETA", "GAMMA"]);
        ipo.reconcile(&fewer);
        assert_eq!(ipo.target(&fewer), Some(1), "still GAMMA");
        assert_eq!(gsec.target(&gsecs), Some(1), "still SGB2033");
    }

    #[test]
    fn changing_section_starts_the_cursor_over() {
        let mut pane = BidsPane::default();
        let applied = applications(&["ALPHA", "BETA"]);
        pane.focus_applications();
        pane.select(1, &applied);
        assert_eq!(pane.target(&applied), Some(1));

        assert!(pane.set_section(Section::Sse));
        assert!(
            !pane.application_focus(),
            "only the IPO section has two lists"
        );
        let sse = issues(&["SSEONE", "SSETWO", "SSETHREE"]);
        assert_eq!(pane.target(&sse), Some(0), "the new list's first row");
    }

    #[test]
    fn an_unnamed_bids_row_highlights_without_resolving_a_record() {
        let mut pane = BidsPane::default();
        let rows = vec![
            Some(BidsRowId::Instrument("ALPHA".to_string())),
            None,
            Some(BidsRowId::Instrument("GAMMA".to_string())),
        ];
        pane.select(1, &rows);
        assert_eq!(pane.display_row(&rows), 1, "the highlight is on the row");
        assert_eq!(pane.target(&rows), None, "and it names no record");

        pane.select(2, &rows);
        assert_eq!(pane.target(&rows), Some(2), "GAMMA");
    }
}
