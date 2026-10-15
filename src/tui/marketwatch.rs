use crate::tui::instruments::InstrumentStore;
use kiteconnect::KiteConnect;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

mod requests;
use requests::{apply, fetch_catalog, fetch_detail};

const WEIGHT_STEP: f64 = 0.0001;

const STATIC_REJECTED: &str = "this is a Kite-curated list, edit it in Kite Web first";

macro_rules! server_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Deserialize)]
        pub struct $name(#[serde(deserialize_with = "server_id_text")] String);

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_string())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

server_id!(WatchId, "A marketwatch's server ID.");
server_id!(GroupId, "A group's server ID, such as `GROUP:default`.");
server_id!(ItemId, "An item's server ID. Never an instrument token.");

impl WatchId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ItemId {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(transparent)]
pub struct Catalog {
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Section {
    #[serde(default)]
    pub items: Vec<Summary>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Summary {
    pub id: WatchId,
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub is_static: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Detail {
    #[serde(default, deserialize_with = "null_default")]
    pub name: String,
    #[serde(default, deserialize_with = "null_default")]
    pub is_static: bool,
    #[serde(default)]
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Group {
    #[serde(default)]
    pub id: GroupId,
    #[serde(default, deserialize_with = "null_default")]
    pub weight: f64,
    #[serde(default)]
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Item {
    #[serde(default)]
    pub id: ItemId,
    #[serde(default, deserialize_with = "null_default")]
    pub weight: f64,
    #[serde(default, deserialize_with = "null_default")]
    pub tradingsymbol: String,
    #[serde(default, deserialize_with = "null_default")]
    pub segment: String,
    #[serde(default, deserialize_with = "null_default")]
    pub exchange: String,
    #[serde(default, deserialize_with = "null_default")]
    pub instrument_token: u32,
}

impl Detail {
    pub fn rows(&self) -> Vec<WatchItem> {
        self.groups
            .iter()
            .flat_map(|group| {
                group.items.iter().map(move |item| WatchItem {
                    exchange: item.exchange.clone(),
                    tradingsymbol: item.tradingsymbol.clone(),
                    instrument_token: item.instrument_token,
                    segment: item.segment.clone(),
                    id: item.id.clone(),
                    group: group.id.clone(),
                })
            })
            .collect()
    }

    pub fn editable(&self) -> Result<(), String> {
        if self.is_static {
            Err(STATIC_REJECTED.to_string())
        } else {
            Ok(())
        }
    }

    pub fn reorder_form(&self) -> Vec<(String, String)> {
        let mut form = Vec::new();
        let mut weight = 0;
        for group in &self.groups {
            weight += 1;
            form.push((group.id.to_string(), weight.to_string()));
            for item in &group.items {
                weight += 1;
                form.push((item.id.to_string(), weight.to_string()));
            }
        }
        form
    }

    pub fn move_row(&mut self, index: usize, delta: isize) -> Option<usize> {
        let mut base = 0;
        for group in &mut self.groups {
            let len = group.items.len();
            if index < base + len {
                return move_row(&mut group.items, index - base, delta).map(|index| base + index);
            }
            base += len;
        }
        None
    }

    fn target_group(&self, index: usize) -> Option<&Group> {
        let mut base = 0;
        for group in &self.groups {
            if index < base + group.items.len() {
                return Some(group);
            }
            base += group.items.len();
        }
        self.groups.first()
    }
}

impl Group {
    fn append_weight(&self) -> f64 {
        self.items.last().map_or(self.weight, |item| item.weight) + WEIGHT_STEP
    }
}

pub fn move_row<T>(items: &mut [T], idx: usize, delta: isize) -> Option<usize> {
    let len = items.len();
    if idx >= len {
        return None;
    }
    let target = idx
        .checked_add_signed(delta)
        .filter(|target| *target < len)?;
    items.swap(idx, target);
    Some(target)
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct WatchItem {
    pub exchange: String,
    pub tradingsymbol: String,
    pub instrument_token: u32,
    pub segment: String,
    pub id: ItemId,
    pub group: GroupId,
}

impl WatchItem {
    pub fn from_token(token: u32) -> Self {
        Self {
            tradingsymbol: format!("token:{token}"),
            instrument_token: token,
            ..Self::default()
        }
    }

    pub fn label(&self) -> String {
        crate::tui::format::instrument_label(&self.exchange, &self.tradingsymbol)
    }

    pub fn is_index(&self) -> bool {
        self.segment == "INDICES"
    }
}

fn server_id_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Wire {
        Text(String),
        Number(i64),
    }
    Ok(match Wire::deserialize(deserializer)? {
        Wire::Text(text) => text,
        Wire::Number(number) => number.to_string(),
    })
}

fn null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Debug, Clone)]
pub enum Mutation {
    Add {
        watch: WatchId,
        exchange: String,
        tradingsymbol: String,
        weight: f64,
        group: GroupId,
        label: String,
    },
    Remove {
        watch: WatchId,
        item: ItemId,
        label: String,
    },
    Reorder {
        watch: WatchId,
        form: Vec<(String, String)>,
    },
    Create {
        name: String,
        weight: usize,
    },
    Rename {
        watch: WatchId,
        name: String,
    },
    Delete {
        watch: WatchId,
        name: String,
    },
}

impl Mutation {
    pub fn watch(&self) -> Option<&WatchId> {
        match self {
            Self::Add { watch, .. }
            | Self::Remove { watch, .. }
            | Self::Reorder { watch, .. }
            | Self::Rename { watch, .. }
            | Self::Delete { watch, .. } => Some(watch),
            Self::Create { .. } => None,
        }
    }

    pub fn refetches_detail(&self) -> bool {
        matches!(
            self,
            Self::Add { .. } | Self::Remove { .. } | Self::Reorder { .. } | Self::Rename { .. }
        )
    }

    pub fn changes_catalog(&self) -> bool {
        matches!(
            self,
            Self::Create { .. } | Self::Rename { .. } | Self::Delete { .. }
        )
    }

    pub fn verb(&self) -> &'static str {
        match self {
            Self::Add { .. } => "adding",
            Self::Remove { .. } => "removing",
            Self::Reorder { .. } => "reordering",
            Self::Create { .. } => "creating watchlist",
            Self::Rename { .. } => "renaming watchlist",
            Self::Delete { .. } => "deleting watchlist",
        }
    }

    pub fn done(&self) -> String {
        match self {
            Self::Add { label, .. } => format!("added {label}"),
            Self::Remove { label, .. } => format!("removed {label}"),
            Self::Reorder { .. } => "watchlist reordered".to_string(),
            Self::Create { name, .. } => format!("watchlist created: {name}"),
            Self::Rename { name, .. } => format!("watchlist renamed: {name}"),
            Self::Delete { name, .. } => format!("watchlist deleted: {name}"),
        }
    }
}

#[derive(Debug)]
pub enum Event {
    Catalog {
        generation: u64,
        result: Result<Catalog, String>,
    },
    Detail {
        watch: WatchId,
        generation: u64,
        result: Result<Detail, String>,
    },
    Mutation {
        mutation: Box<Mutation>,
        result: Result<Option<WatchId>, String>,
    },
}

async fn send(tx: mpsc::Sender<crate::tui::event::AppEvent>, event: Event) {
    crate::tui::data::send(&tx, crate::tui::data::DataEvent::Marketwatch(event)).await;
}

pub fn spawn_catalog(
    client: KiteConnect,
    tx: mpsc::Sender<crate::tui::event::AppEvent>,
    generation: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_catalog(&client).await;
        send(tx, Event::Catalog { generation, result }).await;
    })
}

pub fn spawn_detail(
    client: KiteConnect,
    tx: mpsc::Sender<crate::tui::event::AppEvent>,
    watch: WatchId,
    generation: u64,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = fetch_detail(&client, &watch).await;
        send(
            tx,
            Event::Detail {
                watch,
                generation,
                result,
            },
        )
        .await;
    })
}

pub fn spawn_mutation(
    client: KiteConnect,
    tx: mpsc::Sender<crate::tui::event::AppEvent>,
    mutation: Mutation,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let result = apply(&client, &mutation).await;
        send(
            tx,
            Event::Mutation {
                mutation: Box::new(mutation),
                result,
            },
        )
        .await;
    })
}

#[derive(Debug, Default)]
pub enum Load<T> {
    #[default]
    Loading,
    Ready {
        value: T,
        stale: bool,
    },
    Failed(String),
}

impl<T> Load<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Ready { value, .. } => Some(value),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Self::Loading)
    }

    pub fn is_stale(&self) -> bool {
        matches!(self, Self::Ready { stale: true, .. })
    }

    pub fn error(&self) -> Option<&str> {
        match self {
            Self::Failed(err) => Some(err),
            _ => None,
        }
    }

    pub fn set(&mut self, result: Result<T, String>) {
        match result {
            Ok(value) => {
                *self = Self::Ready {
                    value,
                    stale: false,
                }
            }
            Err(err) => {
                if let Self::Ready { value, .. } = std::mem::replace(self, Self::Failed(err)) {
                    *self = Self::Ready { value, stale: true };
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    NeedsLogin,
    Loading,
    Failed,
    Empty,
}

#[derive(Debug)]
pub struct State {
    ephemeral: Option<Vec<WatchItem>>,
    client: Option<KiteConnect>,
    user_id: String,
    remembered: Option<WatchId>,
    catalog: CatalogState,
    watches: HashMap<WatchId, WatchData>,
    pending: Option<Mutation>,
}

#[derive(Debug, Default)]
struct CatalogState {
    load: Load<Catalog>,
    generation: u64,
}

#[derive(Debug, Default)]
struct WatchData {
    detail: Load<Detail>,
    rows: Vec<WatchItem>,
    generation: u64,
    confirmed: Option<Detail>,
    queued_reorder: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reselect {
    Keep,
    Open(WatchId),
    Nothing,
}

#[derive(Debug)]
pub struct Moved {
    pub index: usize,
    pub mutation: Option<Mutation>,
}

#[derive(Debug, Clone)]
pub enum Follow {
    Listing {
        exchange: String,
        tradingsymbol: String,
    },
    Item(ItemId),
}

impl State {
    pub fn ephemeral(rows: Vec<WatchItem>) -> Self {
        Self {
            ephemeral: Some(rows),
            ..Self::remote(None, String::new(), None)
        }
    }

    pub fn remote(
        client: Option<KiteConnect>,
        user_id: String,
        remembered: Option<WatchId>,
    ) -> Self {
        Self {
            ephemeral: None,
            client,
            user_id,
            remembered,
            catalog: CatalogState::default(),
            watches: HashMap::new(),
            pending: None,
        }
    }

    pub fn account(&self) -> &str {
        &self.user_id
    }

    pub fn remembered(&self) -> Option<&WatchId> {
        self.remembered.as_ref()
    }

    pub fn watch<'s, 'i>(&'s self, id: Option<&'i WatchId>) -> Watch<'s, 'i> {
        Watch { store: self, id }
    }

    pub fn is_ephemeral(&self) -> bool {
        self.ephemeral.is_some()
    }

    pub fn summaries(&self) -> impl Iterator<Item = &Summary> {
        self.catalog
            .load
            .value()
            .into_iter()
            .flat_map(|catalog| &catalog.sections)
            .flat_map(|section| &section.items)
    }

    pub fn count(&self) -> usize {
        self.summaries().count()
    }

    pub fn pending(&self) -> Option<&'static str> {
        self.pending.as_ref().map(Mutation::verb)
    }

    pub fn client(&self) -> Result<KiteConnect, String> {
        match &self.client {
            Some(client) => Ok(client.clone()),
            None => Err("log in again to load your marketwatch".to_string()),
        }
    }

    pub fn writable(&self, watch: Option<&WatchId>) -> Result<&WatchId, String> {
        if let Some(pending) = self.pending() {
            return Err(format!("still {pending}, wait for it to finish"));
        }
        self.editable_watch(watch)
    }

    fn reorderable(&self, watch: Option<&WatchId>) -> Result<&WatchId, String> {
        match &self.pending {
            Some(Mutation::Reorder { .. }) | None => self.editable_watch(watch),
            Some(pending) => Err(format!("still {}, wait for it to finish", pending.verb())),
        }
    }

    fn editable_watch(&self, watch: Option<&WatchId>) -> Result<&WatchId, String> {
        if self.is_ephemeral() {
            return Err("a --token watchlist is not saved to your account".to_string());
        }
        let view = self.watch(watch);
        if view.is_stale() {
            return Err("watchlist is out of date, press r to reload".to_string());
        }
        let Some((id, data)) = watch.and_then(|watch| self.watches.get_key_value(watch)) else {
            return Err("no watchlist loaded".to_string());
        };
        let Some(detail) = data.detail.value() else {
            return Err("no watchlist loaded".to_string());
        };
        detail.editable()?;
        Ok(id)
    }

    pub fn set_catalog(&mut self, result: Result<Catalog, String>) {
        self.catalog.load.set(result);
    }

    pub fn reselect(&self, current: Option<&WatchId>) -> Reselect {
        let known = |id: &WatchId| self.summaries().any(|summary| summary.id == *id);
        match current {
            Some(id) if known(id) => Reselect::Keep,
            _ => match self.summaries().next() {
                Some(summary) => Reselect::Open(summary.id.clone()),
                None => Reselect::Nothing,
            },
        }
    }

    pub fn open(&mut self, watch: &WatchId) -> u64 {
        let data = self.watches.entry(watch.clone()).or_default();
        data.detail = Load::Loading;
        data.rows.clear();
        data.generation += 1;
        data.confirmed = None;
        data.queued_reorder = false;
        data.generation
    }

    #[cfg(test)]
    pub fn generation(&self, watch: &WatchId) -> u64 {
        self.watches.get(watch).map_or(0, |data| data.generation)
    }

    pub fn begin_catalog(&mut self) -> u64 {
        self.catalog.generation += 1;
        self.catalog.generation
    }

    pub fn accepts_catalog(&self, generation: u64) -> bool {
        generation >= self.catalog.generation
    }

    pub fn refresh(&mut self, watch: &WatchId) -> Option<u64> {
        if self.pending.is_some() {
            return None;
        }
        let data = self.watches.entry(watch.clone()).or_default();
        data.generation += 1;
        Some(data.generation)
    }

    pub fn set_detail(
        &mut self,
        watch: &WatchId,
        generation: u64,
        result: Result<Detail, String>,
    ) -> bool {
        let Some(data) = self.watches.get_mut(watch) else {
            return false;
        };
        if generation < data.generation {
            return false;
        }
        let accepted = result.is_ok();
        data.detail.set(result);
        if let Some(detail) = data.detail.value() {
            data.rows = detail.rows();
        }
        if accepted {
            data.confirmed = None;
            data.queued_reorder = false;
        }
        true
    }

    pub fn begin(&mut self, mutation: Mutation) {
        if let Some(watch) = mutation.watch().cloned() {
            self.watches.entry(watch).or_default().generation += 1;
        }
        if mutation.changes_catalog() {
            self.catalog.generation += 1;
        }
        self.pending = Some(mutation);
    }

    pub fn finish(&mut self) {
        self.pending = None;
    }

    pub fn ephemeral_rows_mut(&mut self) -> Option<&mut Vec<WatchItem>> {
        self.ephemeral.as_mut()
    }

    pub fn enrich(&mut self, store: &InstrumentStore) {
        if let Some(rows) = &mut self.ephemeral {
            for row in rows.iter_mut() {
                if let Some(inst) = store.by_token(row.instrument_token) {
                    row.exchange.clone_from(&inst.exchange);
                    row.tradingsymbol.clone_from(&inst.tradingsymbol);
                    row.segment.clone_from(&inst.segment);
                }
            }
            return;
        }
        for data in self.watches.values_mut() {
            for row in &mut data.rows {
                let Some(inst) = resolve_row(store, row) else {
                    continue;
                };
                row.instrument_token = inst.instrument_token;
                row.exchange.clone_from(&inst.exchange);
                row.segment.clone_from(&inst.segment);
            }
        }
    }

    pub fn remember(&self, path: &std::path::Path, watch: Option<&WatchId>) -> Result<(), String> {
        let Some(active) = watch else {
            return Ok(());
        };
        if self.user_id.is_empty() {
            return Ok(());
        }
        crate::tui::settings::save_selected_watch(path, &self.user_id, active.as_str())
    }

    pub fn add_mutation(
        &self,
        watch: Option<&WatchId>,
        index: usize,
        exchange: &str,
        tradingsymbol: &str,
        label: String,
    ) -> Result<Mutation, String> {
        let id = self.writable(watch)?.clone();
        let detail = self
            .watch(watch)
            .detail()
            .value()
            .ok_or("no watchlist loaded")?;
        let group = detail
            .target_group(index)
            .ok_or("this watchlist has no group to add to")?;
        Ok(Mutation::Add {
            watch: id,
            exchange: exchange.to_string(),
            tradingsymbol: tradingsymbol.to_string(),
            weight: group.append_weight(),
            group: group.id.clone(),
            label,
        })
    }

    pub fn reorder_row(
        &mut self,
        watch: Option<&WatchId>,
        index: usize,
        delta: isize,
    ) -> Result<Option<Moved>, String> {
        let id = self.reorderable(watch)?.clone();
        let sending = self.pending.is_none();
        let data = self
            .watches
            .get_mut(&id)
            .ok_or_else(|| "no watchlist loaded".to_string())?;
        let Load::Ready { value: detail, .. } = &mut data.detail else {
            return Err("no watchlist loaded".to_string());
        };
        let previous = detail.clone();
        let Some(target) = detail.move_row(index, delta) else {
            return Ok(None);
        };
        let form = detail.reorder_form();
        data.rows = rows_preserving_enrichment(detail, &data.rows);
        data.confirmed.get_or_insert(previous);
        let mutation = if sending {
            Some(Mutation::Reorder { watch: id, form })
        } else {
            data.generation += 1;
            data.queued_reorder = true;
            None
        };
        Ok(Some(Moved {
            index: target,
            mutation,
        }))
    }

    pub fn take_queued_reorder(&mut self, finished: Option<&WatchId>) -> Option<Mutation> {
        let queued = |id: &&WatchId| {
            self.watches
                .get(*id)
                .is_some_and(|data| data.queued_reorder)
        };
        let id = (finished.filter(queued))
            .or_else(|| self.watches.keys().find(queued))?
            .clone();
        let data = self.watches.get_mut(&id)?;
        data.queued_reorder = false;
        let detail = data.detail.value()?;
        Some(Mutation::Reorder {
            watch: id,
            form: detail.reorder_form(),
        })
    }

    pub fn restore_confirmed(&mut self, watch: Option<&WatchId>) -> bool {
        let Some(data) = watch.and_then(|id| self.watches.get_mut(id)) else {
            return false;
        };
        let Some(confirmed) = data.confirmed.take() else {
            return false;
        };
        data.queued_reorder = false;
        data.rows = rows_preserving_enrichment(&confirmed, &data.rows);
        data.detail = Load::Ready {
            value: confirmed,
            stale: false,
        };
        true
    }

    pub fn create_weight(&self) -> usize {
        self.catalog
            .load
            .value()
            .map_or(0, |catalog| catalog.sections.len())
    }

    pub fn catalog_available(&self) -> bool {
        self.catalog.load.value().is_some()
    }

    pub fn resolve(&self, query: &str) -> Result<Option<WatchId>, String> {
        if self.catalog.load.value().is_none() {
            return Err(
                "watchlists are not available yet; refresh them before creating a list".to_string(),
            );
        }
        let mut named = self
            .summaries()
            .filter(|summary| summary.name.eq_ignore_ascii_case(query));
        if let Some(first) = named.next() {
            let count = 1 + named.count();
            return if count == 1 {
                Ok(Some(first.id.clone()))
            } else {
                Err(format!(
                    "{count} watchlists are named {query}, switch by id"
                ))
            };
        }
        Ok(self
            .summaries()
            .find(|summary| summary.id.as_str() == query)
            .map(|summary| summary.id.clone()))
    }

    pub fn cycle(&self, current: Option<&WatchId>, delta: isize) -> Option<WatchId> {
        let count = self.count();
        if count <= 1 {
            return None;
        }
        let position = self.watch(current).position().cast_signed();
        let next = (position + delta).rem_euclid(count.cast_signed());
        self.summaries()
            .nth(usize::try_from(next).expect("remainder is nonnegative"))
            .map(|summary| summary.id.clone())
    }
}

#[derive(Clone, Copy)]
pub struct Watch<'s, 'i> {
    store: &'s State,
    id: Option<&'i WatchId>,
}

impl<'s> Watch<'s, '_> {
    pub fn rows(&self) -> &'s [WatchItem] {
        if let Some(rows) = self.store.ephemeral.as_deref() {
            return rows;
        }
        self.data()
            .map(|data| data.rows.as_slice())
            .unwrap_or_default()
    }

    pub fn detail(&self) -> &'s Load<Detail> {
        const NEVER_READ: &Load<Detail> = &Load::Loading;
        self.data().map_or(NEVER_READ, |data| &data.detail)
    }

    pub fn is_ephemeral(&self) -> bool {
        self.store.is_ephemeral()
    }

    pub fn count(&self) -> usize {
        self.store.count()
    }

    pub fn position(&self) -> usize {
        let Some(id) = self.id else {
            return 0;
        };
        self.store
            .summaries()
            .position(|summary| summary.id == *id)
            .unwrap_or(0)
    }

    pub fn title(&self) -> &'s str {
        if self.is_ephemeral() {
            return "tokens";
        }
        if let Some(detail) = self.detail().value()
            && !detail.name.is_empty()
        {
            return &detail.name;
        }
        self.summary()
            .map(|summary| summary.name.as_str())
            .unwrap_or_default()
    }

    pub fn is_static(&self) -> bool {
        if let Some(detail) = self.detail().value() {
            return detail.is_static;
        }
        self.summary().is_some_and(|summary| summary.is_static)
    }

    pub fn pending(&self) -> Option<&'static str> {
        self.store.pending()
    }

    pub fn unavailable(&self) -> Option<Unavailable> {
        if self.is_ephemeral() {
            return None;
        }
        if self.store.client.is_none() && self.detail().value().is_none() {
            return Some(Unavailable::NeedsLogin);
        }
        if self.id.is_some() {
            return None;
        }
        if self.store.catalog.load.is_loading() {
            return Some(Unavailable::Loading);
        }
        if self.store.catalog.load.error().is_some() {
            return Some(Unavailable::Failed);
        }
        Some(Unavailable::Empty)
    }

    pub fn error(&self) -> Option<&'s str> {
        self.store
            .catalog
            .load
            .error()
            .or_else(|| self.detail().error())
    }

    pub fn is_stale(&self) -> bool {
        self.store.catalog.load.is_stale() || self.detail().is_stale()
    }

    fn data(&self) -> Option<&'s WatchData> {
        self.store.watches.get(self.id?)
    }

    fn summary(&self) -> Option<&'s Summary> {
        let id = self.id?;
        self.store.summaries().find(|summary| summary.id == *id)
    }
}

fn rows_preserving_enrichment(detail: &Detail, enriched: &[WatchItem]) -> Vec<WatchItem> {
    detail
        .rows()
        .into_iter()
        .map(|row| {
            enriched
                .iter()
                .find(|previous| !previous.id.is_empty() && previous.id == row.id)
                .cloned()
                .unwrap_or(row)
        })
        .collect()
}

fn resolve_row<'a>(
    store: &'a InstrumentStore,
    row: &WatchItem,
) -> Option<&'a crate::tui::instruments::Instrument> {
    let candidates: Vec<&crate::tui::instruments::Instrument> = store
        .by_exact_symbol(&row.tradingsymbol)
        .into_iter()
        .filter(|inst| row.exchange.is_empty() || inst.exchange.eq_ignore_ascii_case(&row.exchange))
        .collect();
    candidates
        .iter()
        .find(|inst| !row.segment.is_empty() && inst.segment == row.segment)
        .or_else(|| {
            candidates.iter().find(|inst| {
                row.instrument_token != 0 && inst.instrument_token == row.instrument_token
            })
        })
        .or_else(|| candidates.first().filter(|_| candidates.len() == 1))
        .copied()
}

#[cfg(test)]
impl State {
    pub fn test_loaded(rows: Vec<WatchItem>) -> Self {
        let mut state = Self::remote(None, String::new(), Some(Self::test_watch_id()));
        state.catalog.load = Load::Ready {
            value: Catalog {
                sections: vec![Section {
                    items: vec![Summary {
                        id: WatchId::from("W1"),
                        name: "default".to_string(),
                        is_static: false,
                    }],
                }],
            },
            stale: false,
        };
        state.watches.insert(
            Self::test_watch_id(),
            WatchData {
                detail: Load::Ready {
                    value: Detail {
                        name: "default".to_string(),
                        is_static: false,
                        groups: vec![Group {
                            id: GroupId::from("GROUP:default"),
                            weight: 1.0,
                            items: Vec::new(),
                        }],
                    },
                    stale: false,
                },
                ..WatchData::default()
            },
        );
        for row in rows {
            state.test_push(row);
        }
        state
    }

    pub const TEST_WATCH: &'static str = "W1";

    fn test_watch_id() -> WatchId {
        WatchId::from(Self::TEST_WATCH)
    }

    pub fn test_set_reachable(&mut self) {
        self.client = Some(
            KiteConnect::builder("key")
                .access_token(kiteconnect::AccessToken::new("token").expect("credentials"))
                .base_url("http://127.0.0.1:1")
                .build()
                .expect("client"),
        );
    }

    pub fn test_set_static(&mut self) {
        if let Some(Load::Ready { value, .. }) = self.test_detail() {
            value.is_static = true;
        }
    }

    fn test_detail(&mut self) -> Option<&mut Load<Detail>> {
        self.watches
            .get_mut(&Self::test_watch_id())
            .map(|data| &mut data.detail)
    }

    pub fn test_push(&mut self, row: WatchItem) {
        let Some(data) = self.watches.get_mut(&Self::test_watch_id()) else {
            return;
        };
        let Load::Ready { value: detail, .. } = &mut data.detail else {
            return;
        };
        let Some(group) = detail.groups.first_mut() else {
            return;
        };
        let weight = group.items.last().map_or(group.weight, |item| item.weight) + WEIGHT_STEP;
        group.items.push(Item {
            id: ItemId::from(format!("ITEM:{}", row.tradingsymbol).as_str()),
            weight,
            tradingsymbol: row.tradingsymbol,
            segment: row.segment,
            exchange: row.exchange,
            instrument_token: row.instrument_token,
        });
        data.rows = detail.rows();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::pane_state::WatchPane;
    use httpmock::prelude::*;

    use serde_json::json;

    pub(super) fn client(server: &MockServer) -> KiteConnect {
        KiteConnect::builder("key")
            .access_token(kiteconnect::AccessToken::new("token").expect("credentials"))
            .base_url(server.base_url())
            .build()
            .expect("client")
    }

    pub(super) fn response(data: &serde_json::Value) -> String {
        json!({"status": "success", "data": data}).to_string()
    }

    pub(super) fn catalog_body() -> serde_json::Value {
        json!([
            {"name": "Favorites", "items": [
                {"id": "W1", "name": "Core", "is_static": false},
                {"id": "W2", "name": "Curated", "is_static": true}
            ]},
            {"name": "Others", "items": [{"id": "W3", "name": "Long term", "is_static": false}]}
        ])
    }

    pub(super) fn detail_body() -> serde_json::Value {
        json!({
            "id": "W1",
            "name": "Core",
            "is_static": false,
            "groups": [
                {"id": "GROUP:default", "name": "Default", "weight": 1, "color": null, "items": [
                    {"id": "ITEM:a", "weight": 1.0001, "note": null, "tradingsymbol": "INFY",
                     "segment": "NSE", "exchange": "NSE", "instrument_token": 408_065,
                     "expiry": ""},
                    {"id": "ITEM:b", "weight": 1.0002, "note": "watch this",
                     "tradingsymbol": "TCS", "segment": "NSE", "exchange": "NSE",
                     "instrument_token": 2_953_217, "expiry": null}
                ]},
                {"id": "GROUP:b", "name": "F&O", "weight": 2, "color": "COLOR_1", "items": [
                    {"id": "ITEM:c", "weight": 2.0001, "tradingsymbol": "NIFTY 50",
                     "segment": "INDICES", "exchange": "NSE", "instrument_token": 256_265}
                ]}
            ]
        })
    }

    #[test]
    fn append_weight_steps_past_the_last_item_or_the_empty_group() {
        let detail: Detail = serde_json::from_value(detail_body()).unwrap();
        assert_eq!(
            (detail.groups[0].append_weight()).to_bits(),
            1.0003_f64.to_bits()
        );
        let empty = Group {
            id: GroupId::from("GROUP:new"),
            weight: 7.0,
            items: Vec::new(),
        };
        assert_eq!(empty.append_weight().to_bits(), 7.0001_f64.to_bits());
        assert_eq!(format!("{}", 1.0001_f64), "1.0001");
    }

    #[test]
    fn move_row_stops_at_the_group_boundary() {
        let mut detail: Detail = serde_json::from_value(detail_body()).unwrap();
        assert_eq!(detail.move_row(0, 1), Some(1));
        assert_eq!(
            detail.reorder_form(),
            vec![
                ("GROUP:default".to_string(), "1".to_string()),
                ("ITEM:b".to_string(), "2".to_string()),
                ("ITEM:a".to_string(), "3".to_string()),
                ("GROUP:b".to_string(), "4".to_string()),
                ("ITEM:c".to_string(), "5".to_string()),
            ]
        );
        assert_eq!(detail.move_row(1, 1), None);
        assert_eq!(detail.move_row(2, -1), None);
        assert_eq!(detail.move_row(2, 1), None);
        assert_eq!(detail.move_row(9, 1), None, "out of range is a no-op");
    }

    #[test]
    fn add_targets_the_selected_rows_group_then_the_first() {
        let detail: Detail = serde_json::from_value(detail_body()).unwrap();
        assert_eq!(
            detail.target_group(2).map(|group| group.id.clone()),
            Some(GroupId::from("GROUP:b"))
        );
        assert_eq!(
            detail.target_group(1).map(|group| group.id.clone()),
            Some(GroupId::from("GROUP:default"))
        );
        assert_eq!(
            detail.target_group(99).map(|group| group.id.clone()),
            Some(GroupId::from("GROUP:default")),
            "a selection past the end falls back to the first group"
        );
    }

    #[test]
    fn a_static_watch_refuses_writes() {
        let mut detail = Detail {
            is_static: true,
            ..Detail::default()
        };
        assert!(detail.editable().unwrap_err().contains("Kite-curated"));
        detail.is_static = false;
        assert!(detail.editable().is_ok());
    }

    #[tokio::test]
    async fn nullable_and_unknown_fields_do_not_drop_a_row() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/api/marketwatch/W1/items");
            then.status(200).body(response(&json!({
                "id": "W1", "name": null, "is_static": null, "future_field": 1,
                "groups": [{"id": "GROUP:default", "weight": null, "items": [
                    {"id": "ITEM:a", "weight": null, "tradingsymbol": "INFY",
                     "segment": null, "exchange": null, "instrument_token": null,
                     "note": null, "expiry": null, "another_new_field": []}
                ]}]
            })));
        });
        let detail = fetch_detail(&client(&server), &WatchId::from("W1"))
            .await
            .unwrap();
        assert_eq!(detail.name, "");
        assert!(!detail.is_static);
        let rows = detail.rows();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].tradingsymbol, "INFY");
        assert_eq!(rows[0].instrument_token, 0, "the master resolves the token");
        assert_eq!(
            rows[0].label(),
            "INFY",
            "an empty exchange drops the prefix"
        );
    }

    fn catalog() -> Catalog {
        serde_json::from_value(catalog_body()).expect("catalog")
    }

    fn detail() -> Detail {
        serde_json::from_value(detail_body()).expect("detail")
    }

    fn remote(active: Option<&str>) -> State {
        let client = KiteConnect::builder("key")
            .access_token(kiteconnect::AccessToken::new("token").expect("credentials"))
            .base_url("http://127.0.0.1:1")
            .build()
            .expect("client");
        State::remote(Some(client), String::new(), active.map(WatchId::from))
    }

    fn id(name: &str) -> WatchId {
        WatchId::from(name)
    }

    fn opened(state: &mut State, active: &WatchId) -> u64 {
        state.open(active)
    }

    #[test]
    fn a_fresh_session_opens_the_first_watch_in_catalog_order() {
        let mut state = remote(None);
        assert_eq!(state.watch(None).unavailable(), Some(Unavailable::Loading));
        state.set_catalog(Ok(catalog()));
        assert_eq!(state.reselect(None), Reselect::Open(id("W1")));
        let open = id("W1");
        opened(&mut state, &open);
        assert_eq!(state.count(), 3, "sections flatten in the server's order");
        assert_eq!(state.watch(Some(&open)).position(), 0);
        assert_eq!(state.watch(Some(&open)).unavailable(), None);
    }

    #[test]
    fn a_remembered_watch_that_still_exists_is_kept() {
        let mut state = remote(Some("W3"));
        state.set_catalog(Ok(catalog()));
        let open = state.remembered().cloned().expect("remembered");
        assert_eq!(state.reselect(Some(&open)), Reselect::Keep);
        assert_eq!(state.watch(Some(&open)).position(), 2);
    }

    #[test]
    fn a_stale_remembered_watch_resolves_to_the_first_one() {
        let mut state = remote(Some("W-GONE"));
        state.set_catalog(Ok(catalog()));
        assert_eq!(
            state.reselect(Some(&id("W-GONE"))),
            Reselect::Open(id("W1"))
        );
    }

    #[test]
    fn an_empty_account_is_its_own_state() {
        let mut state = remote(None);
        state.set_catalog(Ok(Catalog::default()));
        assert_eq!(state.reselect(None), Reselect::Nothing);
        assert_eq!(state.watch(None).unavailable(), Some(Unavailable::Empty));
        assert_eq!(state.watch(None).rows().len(), 0);
        assert!(
            state.writable(None).is_err(),
            "there is nothing to write to"
        );
    }

    #[test]
    fn a_failed_first_read_is_not_an_empty_watchlist() {
        let mut state = remote(None);
        state.set_catalog(Err("offline".to_string()));
        assert_eq!(state.watch(None).unavailable(), Some(Unavailable::Failed));
        assert_eq!(state.watch(None).error(), Some("offline"));
    }

    #[test]
    fn a_failed_refresh_keeps_the_confirmed_rows_and_blocks_writes() {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let open = id("W1");
        let generation = opened(&mut state, &open);
        assert!(state.set_detail(&open, generation, Ok(detail())));
        assert_eq!(state.watch(Some(&open)).rows().len(), 3);
        assert!(state.writable(Some(&open)).is_ok());

        let generation = state.refresh(&open).unwrap();
        state.set_detail(&open, generation, Err("offline".to_string()));
        assert_eq!(
            state.watch(Some(&open)).rows().len(),
            3,
            "the last good rows stay on screen"
        );
        assert!(state.watch(Some(&open)).is_stale());
        let err = state.writable(Some(&open)).unwrap_err();
        assert!(err.contains("out of date"), "{err}");
    }

    #[test]
    fn a_late_reply_for_a_superseded_selection_is_dropped() {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let first = state.open(&id("W3"));
        let second = state.open(&id("W3"));
        assert!(second > first);

        assert!(
            !state.set_detail(&id("W3"), first, Ok(detail())),
            "a reply tagged with an old generation is dropped"
        );
        assert_eq!(state.watch(Some(&id("W3"))).rows().len(), 0);
        let mut current = detail();
        current.name = "Long term".to_string();
        assert!(state.set_detail(&id("W3"), second, Ok(current)));
        assert_eq!(state.watch(Some(&id("W3"))).title(), "Long term");
        assert!(
            state.watch(Some(&id("W1"))).rows().is_empty(),
            "and it landed in W3's rows, not in the watch the pane left"
        );
    }

    #[test]
    fn a_static_watch_is_viewable_and_read_only() {
        let mut state = remote(Some("W2"));
        state.set_catalog(Ok(catalog()));
        let open = id("W2");
        assert!(
            state.watch(Some(&open)).is_static(),
            "the catalog says so before the rows land"
        );
        let mut curated = detail();
        curated.is_static = true;
        let generation = opened(&mut state, &open);
        state.set_detail(&open, generation, Ok(curated));
        assert_eq!(
            state.watch(Some(&open)).rows().len(),
            3,
            "a curated list still renders"
        );
        assert!(state.watch(Some(&open)).is_static());
        assert!(
            state
                .writable(Some(&open))
                .unwrap_err()
                .contains("Kite-curated")
        );
        assert!(state.reorder_row(Some(&open), 0, 1).is_err());
    }

    #[test]
    fn a_write_in_flight_refuses_the_next_one() {
        let mut state = loaded();
        let open = id("W1");
        let mutation = state
            .add_mutation(Some(&open), 0, "NSE", "WIPRO", "NSE:WIPRO".to_string())
            .unwrap();
        state.begin(mutation);
        assert_eq!(state.pending(), Some("adding"));
        let err = state.writable(Some(&open)).unwrap_err();
        assert!(err.contains("still adding"), "{err}");
        state.finish();
        assert!(state.writable(Some(&open)).is_ok());
    }

    fn loaded() -> State {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let generation = state.open(&id("W1"));
        state.set_detail(&id("W1"), generation, Ok(detail()));
        state
    }

    fn flat() -> State {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let detail: Detail = serde_json::from_value(json!({
            "id": "W1", "name": "Core", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                {"id": "ITEM:a", "weight": 1.0001, "tradingsymbol": "A"},
                {"id": "ITEM:b", "weight": 1.0002, "tradingsymbol": "B"},
                {"id": "ITEM:c", "weight": 1.0003, "tradingsymbol": "C"},
                {"id": "ITEM:d", "weight": 1.0004, "tradingsymbol": "D"}
            ]}]
        }))
        .expect("detail");
        let generation = state.open(&id("W1"));
        state.set_detail(&id("W1"), generation, Ok(detail));
        state
    }

    fn rows(state: &State) -> &[WatchItem] {
        state.watch(Some(&id("W1"))).rows()
    }

    fn symbols(state: &State) -> Vec<&str> {
        state
            .watch(Some(&id("W1")))
            .rows()
            .iter()
            .map(|row| row.tradingsymbol.as_str())
            .collect()
    }

    fn form(mutation: &Mutation) -> Vec<(String, String)> {
        match mutation {
            Mutation::Reorder { form, .. } => form.clone(),
            other => panic!("expected a reorder, got {other:?}"),
        }
    }

    fn weights(ids: &[&str]) -> Vec<(String, String)> {
        ids.iter()
            .enumerate()
            .map(|(index, id)| (id.to_string(), (index + 1).to_string()))
            .collect()
    }

    #[test]
    fn moves_behind_a_reorder_in_flight_coalesce_into_one_follow_up() {
        let mut state = flat();
        let first = state
            .reorder_row(Some(&id("W1")), 0, 1)
            .unwrap()
            .expect("the row moved");
        assert_eq!(first.index, 1);
        assert_eq!(symbols(&state), ["B", "A", "C", "D"], "moved on the press");
        let sent = first.mutation.expect("the first move is sent at once");
        assert_eq!(
            form(&sent),
            weights(&["GROUP:default", "ITEM:b", "ITEM:a", "ITEM:c", "ITEM:d"])
        );
        state.begin(sent);

        for (index, expected) in [(1usize, 2usize), (2, 3)] {
            let moved = state
                .reorder_row(Some(&id("W1")), index, 1)
                .unwrap()
                .expect("the row moved");
            assert_eq!(moved.index, expected);
            assert!(moved.mutation.is_none(), "nothing is sent behind a write");
        }
        assert_eq!(symbols(&state), ["B", "C", "D", "A"]);

        state.finish();
        let queued = state
            .take_queued_reorder(Some(&id("W1")))
            .expect("one follow-up write");
        assert_eq!(
            form(&queued),
            weights(&["GROUP:default", "ITEM:b", "ITEM:c", "ITEM:d", "ITEM:a"]),
            "the follow-up carries the final order, not each step"
        );
        assert!(
            state.take_queued_reorder(Some(&id("W1"))).is_none(),
            "one queued write, not one per press"
        );
    }

    #[test]
    fn a_failed_reorder_restores_the_confirmed_order() {
        let mut state = flat();
        let first = state
            .reorder_row(Some(&id("W1")), 0, 1)
            .unwrap()
            .expect("the row moved");
        state.begin(first.mutation.expect("sent"));
        state
            .reorder_row(Some(&id("W1")), 1, 1)
            .unwrap()
            .expect("the row moved");
        assert_eq!(symbols(&state), ["B", "C", "A", "D"]);

        state.finish();
        assert!(
            state.restore_confirmed(Some(&id("W1"))),
            "the account's own order comes back"
        );
        assert_eq!(symbols(&state), ["A", "B", "C", "D"]);
        assert!(
            state.take_queued_reorder(Some(&id("W1"))).is_none(),
            "the coalesced moves are dropped with the write they were built on"
        );
        assert!(
            !state.restore_confirmed(Some(&id("W1"))),
            "the rows are the confirmed ones now"
        );
    }

    #[test]
    fn a_refresh_cannot_drop_the_moves_queued_behind_a_write() {
        let mut state = flat();
        let already_out = state.generation(&id("W1"));
        let first = state
            .reorder_row(Some(&id("W1")), 0, 1)
            .unwrap()
            .expect("the row moved");
        state.begin(first.mutation.expect("sent"));
        state
            .reorder_row(Some(&id("W1")), 1, 1)
            .unwrap()
            .expect("the row moved");
        assert_eq!(symbols(&state), ["B", "C", "A", "D"]);

        assert_eq!(
            state.refresh(&id("W1")),
            None,
            "no read goes out behind the write"
        );
        assert!(
            !state.set_detail(&WatchId::from("W1"), already_out, Ok(detail())),
            "a read issued before the write is superseded by it"
        );
        assert_eq!(symbols(&state), ["B", "C", "A", "D"]);

        state.finish();
        let queued = state
            .take_queued_reorder(Some(&id("W1")))
            .expect("the second move still has to be sent");
        assert_eq!(
            form(&queued),
            weights(&["GROUP:default", "ITEM:b", "ITEM:c", "ITEM:a", "ITEM:d"]),
            "the follow-up carries the order on screen"
        );
        assert!(
            state.refresh(&id("W1")).is_some(),
            "a settled write frees the read"
        );
    }

    #[test]
    fn a_refetch_becomes_the_confirmed_order() {
        let mut state = flat();
        let first = state
            .reorder_row(Some(&id("W1")), 0, 1)
            .unwrap()
            .expect("the row moved");
        state.begin(first.mutation.expect("sent"));
        let generation = state.generation(&id("W1"));
        state.set_detail(&id("W1"), generation, Ok(detail()));
        state.finish();
        assert!(!state.restore_confirmed(Some(&id("W1"))));
        assert_eq!(
            state.watch(Some(&id("W1"))).rows().len(),
            3,
            "the read decides what is on screen"
        );
    }

    #[test]
    fn a_reorder_and_its_rollback_keep_the_masters_resolution() {
        let store = InstrumentStore::new(vec![
            inst(408_065, "INFY", "NSE", "NSE"),
            inst(2_953_217, "TCS", "NSE", "NSE"),
        ]);
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let detail: Detail = serde_json::from_value(json!({
            "id": "W1", "name": "Core", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                {"id": "ITEM:a", "weight": 1.0001, "tradingsymbol": "INFY",
                 "segment": null, "exchange": null, "instrument_token": null},
                {"id": "ITEM:b", "weight": 1.0002, "tradingsymbol": "TCS",
                 "segment": null, "exchange": null, "instrument_token": null}
            ]}]
        }))
        .expect("detail");
        let generation = state.open(&id("W1"));
        state.set_detail(&id("W1"), generation, Ok(detail));
        state.enrich(&store);
        assert_eq!(rows(&state)[0].instrument_token, 408_065, "enriched");

        let moved = state
            .reorder_row(Some(&id("W1")), 0, 1)
            .unwrap()
            .expect("the row moved");
        assert_eq!(moved.index, 1);
        assert_eq!(symbols(&state), ["TCS", "INFY"]);
        assert_eq!(
            rows(&state)[1].instrument_token,
            408_065,
            "the moved row keeps the token the master resolved"
        );
        assert_eq!(rows(&state)[1].exchange, "NSE");
        assert_eq!(rows(&state)[0].instrument_token, 2_953_217);

        state.begin(moved.mutation.expect("sent"));
        state.finish();
        assert!(
            state.restore_confirmed(Some(&id("W1"))),
            "back where it was"
        );
        assert_eq!(symbols(&state), ["INFY", "TCS"]);
        assert_eq!(
            rows(&state)[0].instrument_token,
            408_065,
            "the rollback restores the order, not the raw identity"
        );
        assert_eq!(rows(&state)[0].exchange, "NSE");
    }

    #[test]
    fn a_move_across_a_group_boundary_is_refused_locally() {
        let mut state = loaded();
        for (index, delta) in [(1usize, 1isize), (2, -1), (2, 1)] {
            assert!(
                state
                    .reorder_row(Some(&id("W1")), index, delta)
                    .unwrap()
                    .is_none(),
                "row {index} must not leave its group"
            );
        }
        assert_eq!(symbols(&state), ["INFY", "TCS", "NIFTY 50"]);
        assert!(state.take_queued_reorder(Some(&id("W1"))).is_none());
        assert!(
            !state.restore_confirmed(Some(&id("W1"))),
            "nothing was moved"
        );
    }

    #[test]
    fn a_reorder_waits_for_a_write_that_is_not_one() {
        let mut state = flat();
        let add = state
            .add_mutation(Some(&id("W1")), 0, "NSE", "WIPRO", "NSE:WIPRO".to_string())
            .unwrap();
        state.begin(add);
        let err = state.reorder_row(Some(&id("W1")), 0, 1).unwrap_err();
        assert!(err.contains("still adding"), "{err}");
        assert_eq!(symbols(&state), ["A", "B", "C", "D"]);
    }

    #[test]
    fn an_add_appends_to_the_selected_rows_group() {
        let state = loaded();
        let Mutation::Add {
            watch,
            exchange,
            tradingsymbol,
            weight,
            group,
            ..
        } = state
            .add_mutation(Some(&id("W1")), 2, "NSE", "WIPRO", "NSE:WIPRO".to_string())
            .unwrap()
        else {
            panic!("expected an add");
        };
        assert_eq!(watch, WatchId::from("W1"));
        assert_eq!(exchange, "NSE");
        assert_eq!(tradingsymbol, "WIPRO");
        assert_eq!(group, GroupId::from("GROUP:b"));
        assert!((weight - 2.0002).abs() < 1e-9, "{weight}");
    }

    fn pane(state: &mut State, watch: &str, rows: &serde_json::Value) -> (WatchPane, u64) {
        let mut pane = WatchPane::default();
        pane.open(id(watch));
        let generation = state.open(&id(watch));
        let detail: Detail = serde_json::from_value(json!({
            "id": watch, "name": watch, "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": rows}]
        }))
        .expect("detail");
        state.set_detail(&id(watch), generation, Ok(detail));
        pane.reconcile(state);
        (pane, generation)
    }

    fn item(item_id: &str, symbol: &str) -> serde_json::Value {
        json!({"id": item_id, "weight": 1.0, "tradingsymbol": symbol,
               "segment": "NSE", "exchange": "NSE", "instrument_token": 1})
    }

    fn symbols_of(pane: &WatchPane, state: &State) -> Vec<String> {
        pane.rows(state)
            .iter()
            .map(|row| row.tradingsymbol.clone())
            .collect()
    }

    #[test]
    fn two_watch_panes_keep_their_own_rows_when_replies_arrive_reversed() {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));

        let mut a = WatchPane::default();
        a.open(id("W1"));
        let a_generation = state.open(&id("W1"));
        let mut b = WatchPane::default();
        b.open(id("W3"));
        let b_generation = state.open(&id("W3"));

        let b_detail: Detail = serde_json::from_value(json!({
            "id": "W3", "name": "Long term", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                item("ITEM:x", "SBIN"), item("ITEM:y", "ITC")
            ]}]
        }))
        .expect("detail");
        assert!(state.set_detail(&id("W3"), b_generation, Ok(b_detail)));
        b.reconcile(&state);
        assert_eq!(symbols_of(&b, &state), ["SBIN", "ITC"]);
        assert!(
            a.rows(&state).is_empty(),
            "W3's rows are not the pane on W1's"
        );

        let a_detail: Detail = serde_json::from_value(json!({
            "id": "W1", "name": "Core", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                item("ITEM:a", "INFY"), item("ITEM:b", "TCS"), item("ITEM:c", "WIPRO")
            ]}]
        }))
        .expect("detail");
        assert!(state.set_detail(&id("W1"), a_generation, Ok(a_detail)));
        a.reconcile(&state);

        assert_eq!(symbols_of(&a, &state), ["INFY", "TCS", "WIPRO"]);
        assert_eq!(
            symbols_of(&b, &state),
            ["SBIN", "ITC"],
            "the late reply did not replace the other pane's rows"
        );
        assert_eq!(a.watch(), Some(&id("W1")));
        assert_eq!(b.watch(), Some(&id("W3")));

        a.select(2, &state);
        b.select(1, &state);
        assert_eq!(
            a.rows(&state)[a.target(&state).unwrap()].tradingsymbol,
            "WIPRO"
        );
        assert_eq!(a.display_row(&state), 2);
        assert_eq!(
            b.rows(&state)[b.target(&state).unwrap()].tradingsymbol,
            "ITC"
        );
        assert_eq!(b.display_row(&state), 1);
    }

    #[test]
    fn two_panes_on_one_watch_share_its_confirmed_writes() {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let (mut a, _) = pane(
            &mut state,
            "W1",
            &json!([item("ITEM:a", "INFY"), item("ITEM:b", "TCS")]),
        );
        let mut b = WatchPane::default();
        b.open(id("W1"));
        b.reconcile(&state);
        a.select(0, &state);
        b.select(1, &state);

        let generation = state.refresh(&id("W1")).expect("no write is in flight");
        let grown: Detail = serde_json::from_value(json!({
            "id": "W1", "name": "W1", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                item("ITEM:a", "INFY"), item("ITEM:b", "TCS"), item("ITEM:c", "WIPRO")
            ]}]
        }))
        .expect("detail");
        state.set_detail(&id("W1"), generation, Ok(grown));
        a.reconcile(&state);
        b.reconcile(&state);

        assert_eq!(symbols_of(&a, &state), ["INFY", "TCS", "WIPRO"]);
        assert_eq!(
            symbols_of(&b, &state),
            symbols_of(&a, &state),
            "one copy of the account's rows, read twice"
        );
        assert_eq!(a.display_row(&state), 0, "still INFY");
        assert_eq!(b.display_row(&state), 1, "still TCS");
    }

    #[test]
    fn a_failed_reorder_restores_the_shared_order_and_each_pane_keeps_its_row() {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let (mut mover, _) = pane(
            &mut state,
            "W1",
            &json!([
                item("ITEM:a", "A"),
                item("ITEM:b", "B"),
                item("ITEM:c", "C")
            ]),
        );
        let mut other = WatchPane::default();
        other.open(id("W1"));
        other.reconcile(&state);
        mover.select(0, &state);
        other.select(2, &state);

        let reordered = state
            .reorder_row(mover.watch().cloned().as_ref(), 0, 1)
            .unwrap()
            .expect("the row moved");
        mover.select(reordered.index, &state);
        mover.expect(Some(Follow::Item(ItemId::from("ITEM:a"))));
        state.begin(reordered.mutation.expect("sent at once"));
        other.reconcile(&state);
        assert_eq!(symbols_of(&mover, &state), ["B", "A", "C"]);
        assert_eq!(mover.display_row(&state), 1, "the mover follows A");
        assert_eq!(
            other.display_row(&state),
            2,
            "the other pane still points at C, which did not move"
        );

        state.finish();
        assert!(state.restore_confirmed(mover.watch()));
        assert_eq!(
            symbols_of(&mover, &state),
            ["A", "B", "C"],
            "the account's own order is back"
        );
        assert!(mover.follow_restored(&state), "the mover follows A back");
        other.reconcile(&state);

        assert_eq!(mover.display_row(&state), 0, "A, where the account has it");
        assert_eq!(
            other.display_row(&state),
            2,
            "and the rollback did not move the pane that made no write"
        );
        assert_eq!(
            other.rows(&state)[other.target(&state).unwrap()].tradingsymbol,
            "C"
        );
    }

    #[test]
    fn the_pane_that_asked_for_a_write_follows_the_row_it_moved() {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let (mut asked, _) = pane(
            &mut state,
            "W1",
            &json!([item("ITEM:a", "INFY"), item("ITEM:b", "TCS")]),
        );
        let mut bystander = WatchPane::default();
        bystander.open(id("W1"));
        bystander.reconcile(&state);
        bystander.select(0, &state);

        asked.expect(Some(Follow::Listing {
            exchange: "NSE".to_string(),
            tradingsymbol: "WIPRO".to_string(),
        }));
        let generation = state.refresh(&id("W1")).expect("nothing in flight");
        let grown: Detail = serde_json::from_value(json!({
            "id": "W1", "name": "W1", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                item("ITEM:a", "INFY"), item("ITEM:c", "WIPRO"), item("ITEM:b", "TCS")
            ]}]
        }))
        .expect("detail");
        state.set_detail(&id("W1"), generation, Ok(grown));

        assert!(asked.follow_confirmed(&state), "onto the added row");
        assert_eq!(asked.display_row(&state), 1, "WIPRO");
        assert!(
            !asked.follow_confirmed(&state),
            "consumed once, so a later reload does not move it again"
        );

        bystander.reconcile(&state);
        assert_eq!(
            bystander.display_row(&state),
            0,
            "the pane that asked for nothing follows nothing"
        );
    }

    #[test]
    fn a_name_resolves_only_when_it_is_unambiguous() {
        let mut state = remote(None);
        let mut catalog = catalog();
        state.set_catalog(Ok(catalog.clone()));
        assert_eq!(state.reselect(None), Reselect::Open(id("W1")));
        assert_eq!(state.resolve("core").unwrap(), Some(WatchId::from("W1")));
        assert_eq!(state.resolve("W3").unwrap(), Some(WatchId::from("W3")));
        assert_eq!(
            state.resolve("Swing").unwrap(),
            None,
            "unknown names create"
        );

        catalog.sections[1].items.push(Summary {
            id: WatchId::from("W4"),
            name: "Core".to_string(),
            is_static: false,
        });
        state.set_catalog(Ok(catalog));
        let err = state.resolve("core").unwrap_err();
        assert!(err.contains("2 watchlists are named core"), "{err}");
    }

    #[test]
    fn cycling_wraps_through_the_accounts_own_order() {
        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let open = id("W1");
        assert_eq!(state.cycle(Some(&open), 1), Some(id("W2")));
        assert_eq!(
            state.cycle(Some(&open), -1),
            Some(id("W3")),
            "wraps backwards"
        );
        assert_eq!(
            state.cycle(Some(&id("W3")), 1),
            Some(id("W1")),
            "wraps forwards"
        );

        let mut single = remote(None);
        single.set_catalog(Ok(Catalog {
            sections: vec![Section {
                items: vec![Summary {
                    id: WatchId::from("W1"),
                    name: "Only".to_string(),
                    is_static: false,
                }],
            }],
        }));
        assert_eq!(
            single.cycle(Some(&id("W1")), 1),
            None,
            "one watch has nowhere to cycle to"
        );
    }

    #[test]
    fn create_weight_matches_kite_webs() {
        let mut state = remote(None);
        assert_eq!(state.create_weight(), 0);
        state.set_catalog(Ok(catalog()));
        assert_eq!(state.create_weight(), 2);
    }

    #[test]
    fn a_token_session_makes_no_account_calls() {
        let mut state = State::ephemeral(vec![WatchItem::from_token(408_065)]);
        assert!(state.is_ephemeral());
        assert_eq!(state.watch(None).unavailable(), None);
        assert_eq!(state.watch(None).rows()[0].tradingsymbol, "token:408065");
        assert!(state.client().is_err(), "there is no session to call with");
        let err = state.writable(None).unwrap_err();
        assert!(err.contains("--token"), "{err}");
        assert!(
            state.remember(std::path::Path::new("unused"), None).is_ok(),
            "nothing to remember"
        );
        state
            .ephemeral_rows_mut()
            .expect("token rows are editable in place")
            .push(WatchItem::from_token(2_953_217));
        assert_eq!(state.watch(None).rows().len(), 2);
    }

    #[test]
    fn a_session_without_a_client_asks_for_a_login() {
        let state = State::remote(None, String::new(), Some(WatchId::from("W1")));
        assert_eq!(
            state.watch(Some(&id("W1"))).unavailable(),
            Some(Unavailable::NeedsLogin)
        );
        let err = state.client().unwrap_err();
        assert!(err.contains("log in again"), "{err}");
    }

    fn inst(
        token: u32,
        symbol: &str,
        exchange: &str,
        segment: &str,
    ) -> crate::instruments::Instrument {
        crate::instruments::Instrument {
            instrument_token: token,
            tradingsymbol: symbol.to_string(),
            name: symbol.to_string(),
            exchange: exchange.to_string(),
            segment: segment.to_string(),
            expiry: None,
            strike: None,
            tick_size: 0.05,
            lot_size: 1,
            instrument_type: "EQ".to_string(),
            freeze_quantity: None,
            max_order_quantity: None,
        }
    }

    #[test]
    fn enrichment_never_drops_a_row_the_master_does_not_know() {
        let store = InstrumentStore::new(vec![
            inst(408_065, "INFY", "NSE", "NSE"),
            inst(136_508_676, "INFY", "BSE", "BSE"),
            inst(256_265, "NIFTY 50", "NSE", "INDICES"),
        ]);

        let mut state = loaded();
        state.enrich(&store);
        let rows = rows(&state);
        assert_eq!(rows.len(), 3, "an unknown row is enriched, never removed");
        assert_eq!(
            rows[0].instrument_token, 408_065,
            "NSE segment wins over BSE"
        );
        assert_eq!(rows[1].tradingsymbol, "TCS");
        assert_eq!(
            rows[1].instrument_token, 2_953_217,
            "an unknown symbol keeps the server's token"
        );
        assert_eq!(
            rows[1].id,
            ItemId::from("ITEM:b"),
            "and keeps the ID a delete needs"
        );
        assert_eq!(rows[2].instrument_token, 256_265);

        let mut tokens = State::ephemeral(vec![WatchItem::from_token(408_065)]);
        tokens.enrich(&store);
        assert_eq!(tokens.watch(None).rows()[0].label(), "NSE:INFY");
    }

    #[test]
    fn enrichment_keeps_the_exchange_and_token_the_account_gave() {
        let store = InstrumentStore::new(vec![
            inst(408_065, "INFY", "NSE", "NSE"),
            inst(136_508_676, "INFY", "BSE", "BSE"),
        ]);

        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let detail: Detail = serde_json::from_value(json!({
            "id": "W1", "name": "Core", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                {"id": "ITEM:a", "weight": 1.0001, "tradingsymbol": "INFY",
                 "segment": null, "exchange": "BSE", "instrument_token": 136_508_676},
                {"id": "ITEM:b", "weight": 1.0002, "tradingsymbol": "INFY",
                 "segment": null, "exchange": null, "instrument_token": null}
            ]}]
        }))
        .expect("detail");
        let generation = state.open(&id("W1"));
        state.set_detail(&id("W1"), generation, Ok(detail));
        state.enrich(&store);

        let enriched = rows(&state);
        assert_eq!(
            enriched[0].exchange, "BSE",
            "the account's exchange is kept"
        );
        assert_eq!(enriched[0].instrument_token, 136_508_676);
        assert_eq!(enriched[0].segment, "BSE", "resolved from the BSE listing");
        assert_eq!(enriched[1].exchange, "");
        assert_eq!(enriched[1].instrument_token, 0);

        let mut state = remote(None);
        state.set_catalog(Ok(catalog()));
        let detail: Detail = serde_json::from_value(json!({
            "id": "W1", "name": "Core", "is_static": false,
            "groups": [{"id": "GROUP:default", "weight": 1, "items": [
                {"id": "ITEM:a", "weight": 1.0001, "tradingsymbol": "INFY",
                 "segment": null, "exchange": null, "instrument_token": 136_508_676}
            ]}]
        }))
        .expect("detail");
        let generation = state.open(&id("W1"));
        state.set_detail(&id("W1"), generation, Ok(detail));
        state.enrich(&store);
        let resolved = rows(&state);
        assert_eq!(resolved[0].exchange, "BSE");
        assert_eq!(resolved[0].instrument_token, 136_508_676);
    }

    #[test]
    fn server_ids_accept_integer_and_string_wire_shapes() {
        let catalog: Catalog = serde_json::from_value(serde_json::json!([
            {"name": "Favorites", "items": [
                {"id": 1, "name": "Indices", "weight": 1, "is_static": false, "static_id": null}
            ]},
            {"name": "Others", "items": []}
        ]))
        .expect("live catalog shape");
        assert_eq!(catalog.sections[0].items[0].id, WatchId::from("1"));
        assert_eq!(catalog.sections[0].items[0].name, "Indices");

        let detail: Detail = serde_json::from_value(serde_json::json!({
            "id": 1, "name": "Indices", "weight": 1, "is_static": false, "static_id": null,
            "groups": [{"id": "GROUP:default", "name": "Default", "weight": 1, "color": "COLOR_1",
                "items": [{"id": "ITEM:9", "weight": 1, "note": null, "tradingsymbol": "NIFTY 50",
                    "instrument_token": 256_265, "segment": "INDICES", "exchange": "NSE", "expiry": ""}]}]
        }))
        .expect("live detail shape");
        assert_eq!(detail.groups[0].id, GroupId::from("GROUP:default"));
        assert_eq!(detail.groups[0].items[0].id, ItemId::from("ITEM:9"));
        assert_eq!(detail.groups[0].items[0].instrument_token, 256_265);
    }
}
