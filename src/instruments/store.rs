use super::IST_OFFSET_SECS;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::time::SystemTime;

/// The instrument master fields used by the CLI. The full payload stays in the cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instrument {
    pub instrument_token: u32,
    pub tradingsymbol: String,
    pub name: String,
    pub exchange: String,
    /// The instruments master spells this key `expiry_date`; the alias accepts
    /// it on input (without which serde leaves this `None`, breaking option
    /// expiry grouping and the search modal's expiry column) while keeping our
    /// serialized output field named `expiry` for CLI JSON consumers.
    #[serde(alias = "expiry_date")]
    pub expiry: Option<String>,
    pub strike: Option<f64>,
    pub tick_size: f64,
    pub lot_size: u32,
    pub instrument_type: String,
    pub segment: String,
    pub freeze_quantity: Option<u32>,
    pub max_order_quantity: Option<u32>,
}

impl Instrument {
    #[must_use]
    pub fn label(&self) -> String {
        format!("{}:{}", self.exchange, self.tradingsymbol)
    }

    /// Whether this row is a spot index rather than a tradable instrument.
    /// The master reports indices with `instrument_type: "EQ"` (NIFTY 50,
    /// SENSEX, NIFTY BANK …); the `INDICES` segment is the only thing that
    /// distinguishes them from cash equities.
    #[must_use]
    pub fn is_index(&self) -> bool {
        self.segment.eq_ignore_ascii_case("INDICES")
    }

    /// Instrument type for display. Mirrors `instrument_type` except for
    /// indices, which would otherwise be mislabelled `EQ`.
    #[must_use]
    pub fn display_type(&self) -> &str {
        if self.is_index() {
            "INDEX"
        } else {
            &self.instrument_type
        }
    }
}

/// Uppercase, whitespace-free keys and derivative aliases for one instrument.
/// Read the stored symbol and interned name through the store's key accessors.
struct SearchKey {
    /// `None` reuses the row's already-compact tradingsymbol.
    symbol_upper: Option<Box<str>>,
    /// Index into the distinct compacted names in [`InstrumentStore::name_pool`].
    name_upper: usize,
    /// Human-friendly month and day-month aliases for derivative symbols.
    /// The web interface exposes these as the optional search name for weekly
    /// contracts because their compact symbols carry numeric dates.
    month_alias: Option<Box<str>>,
    date_alias: Option<Box<str>>,
}

/// Uppercase `text` with every whitespace character removed. See [`SearchKey`].
fn compact_upper(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

const MONTH_NAMES: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

/// Build the web interface's human-friendly expiry alias once while the master is
/// loaded. The raw weekly symbol stores a numeric date such as `26908`, but
/// the web interface also searches the equivalent `8 SEP` name.
fn derivative_aliases(
    inst: &Instrument,
    compact_symbol: &str,
    compact_name: &str,
) -> Option<(Box<str>, Box<str>)> {
    if !matches!(inst.instrument_type.as_str(), "CE" | "PE" | "FUT")
        || compact_name.is_empty()
        || !compact_symbol.starts_with(compact_name)
    {
        return None;
    }
    let (month, day) = iso_month_day(inst.expiry.as_deref()?)?;
    let suffix = match inst.instrument_type.as_str() {
        "CE" | "PE" => format!("{}{typ}", inst.strike?, typ = inst.instrument_type),
        "FUT" => "FUT".to_string(),
        _ => return None,
    };
    let month = MONTH_NAMES[month as usize - 1];
    Some((
        format!("{compact_name}{month}{suffix}").into_boxed_str(),
        format!("{compact_name}{day}{month}{suffix}").into_boxed_str(),
    ))
}

fn iso_month_day(expiry: &str) -> Option<(u8, u8)> {
    if !is_iso_date(expiry) {
        return None;
    }
    let bytes = expiry.as_bytes();
    let month = bytes[5].wrapping_sub(b'0') * 10 + bytes[6].wrapping_sub(b'0');
    let day = bytes[8].wrapping_sub(b'0') * 10 + bytes[9].wrapping_sub(b'0');
    (1..=12)
        .contains(&month)
        .then_some((month, day))
        .filter(|(_, day)| (1..=31).contains(day))
}

/// Order matches by quality, then start offset, then span.
/// Comparing the start first keeps the typed underlying ahead of a substring
/// in another underlying whose expiry digits give it a shorter fuzzy span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MatchRank {
    tier: u8,
    start: usize,
    span: usize,
}

impl MatchRank {
    const fn new(tier: u8, start: usize, span: usize) -> Self {
        Self { tier, start, span }
    }
}

pub struct InstrumentStore {
    pub(super) instruments: Vec<Instrument>,
    keys: Vec<SearchKey>,
    /// The distinct compacted names, indexed by [`SearchKey::name_upper`].
    name_pool: Vec<Box<str>>,
    by_token: HashMap<u32, usize>,
}

impl std::fmt::Debug for InstrumentStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InstrumentStore")
            .field("len", &self.instruments.len())
            .finish_non_exhaustive()
    }
}

impl InstrumentStore {
    #[must_use]
    pub fn new(mut instruments: Vec<Instrument>) -> Self {
        instruments.shrink_to_fit();

        let by_token = instruments
            .iter()
            .enumerate()
            .map(|(idx, inst)| (inst.instrument_token, idx))
            .collect();

        let mut name_pool: Vec<Box<str>> = Vec::new();
        let mut name_ids: HashMap<Box<str>, usize> = HashMap::new();
        let mut keys = Vec::with_capacity(instruments.len());
        for inst in &instruments {
            let compact_symbol = compact_upper(&inst.tradingsymbol);
            let compact_name = compact_upper(&inst.name);
            let (month_alias, date_alias) =
                derivative_aliases(inst, &compact_symbol, &compact_name)
                    .map_or((None, None), |(month, date)| (Some(month), Some(date)));
            let name_upper = if let Some(id) = name_ids.get(compact_name.as_str()) {
                *id
            } else {
                let id = name_pool.len();
                let interned: Box<str> = compact_name.into_boxed_str();
                name_pool.push(interned.clone());
                name_ids.insert(interned, id);
                id
            };
            keys.push(SearchKey {
                symbol_upper: (compact_symbol != inst.tradingsymbol)
                    .then(|| compact_symbol.into_boxed_str()),
                name_upper,
                month_alias,
                date_alias,
            });
        }
        name_pool.shrink_to_fit();

        Self {
            instruments,
            keys,
            name_pool,
            by_token,
        }
    }

    /// The compacted-uppercase tradingsymbol of row `idx`. See [`SearchKey`].
    fn symbol_key(&self, idx: usize) -> &str {
        self.keys[idx]
            .symbol_upper
            .as_deref()
            .unwrap_or(&self.instruments[idx].tradingsymbol)
    }

    /// The compacted-uppercase name of row `idx`. See [`SearchKey`].
    fn name_key(&self, idx: usize) -> &str {
        &self.name_pool[self.keys[idx].name_upper]
    }

    fn alias_rank(&self, idx: usize, needle: &str) -> Option<MatchRank> {
        [
            self.keys[idx].month_alias.as_deref(),
            self.keys[idx].date_alias.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter_map(|alias| {
            if let Some(start) = alias.find(needle) {
                Some(MatchRank::new(2, start, needle.len()))
            } else if needle.len() >= 2 {
                subsequence_span(needle, alias).map(|(start, span)| MatchRank::new(3, start, span))
            } else {
                None
            }
        })
        .min()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.instruments.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instruments.is_empty()
    }

    #[must_use]
    pub fn by_token(&self, token: u32) -> Option<&Instrument> {
        self.by_token.get(&token).map(|idx| &self.instruments[*idx])
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<Instrument> {
        self.instruments
    }

    #[must_use]
    pub fn search(&self, query: &str, limit: usize) -> Vec<&Instrument> {
        let (exchange, needle) = split_query(query);
        if needle.is_empty() {
            return Vec::new();
        }
        let needle = needle.as_str();

        let allow_subsequence = needle.len() >= 2;
        let needle_span = needle.len();

        let mut scored: Vec<(MatchRank, usize, &Instrument)> = self
            .instruments
            .iter()
            .enumerate()
            .filter(|(_, inst)| {
                exchange
                    .as_deref()
                    .is_none_or(|ex| inst.exchange.eq_ignore_ascii_case(ex))
            })
            .filter_map(|(idx, inst)| {
                let symbol = self.symbol_key(idx);
                if symbol == needle {
                    return Some((MatchRank::new(0, 0, needle_span), idx, inst));
                }
                if symbol.starts_with(needle) {
                    return Some((MatchRank::new(1, 0, needle_span), idx, inst));
                }
                if let Some(start) = symbol.find(needle) {
                    return Some((MatchRank::new(2, start, needle_span), idx, inst));
                }
                if allow_subsequence && let Some((start, span)) = subsequence_span(needle, symbol) {
                    return Some((MatchRank::new(3, start, span), idx, inst));
                }
                let name = self.name_key(idx);
                if let Some(start) = name.find(needle) {
                    return Some((MatchRank::new(4, start, needle_span), idx, inst));
                }
                if allow_subsequence && let Some((start, span)) = subsequence_span(needle, name) {
                    return Some((MatchRank::new(5, start, span), idx, inst));
                }
                if let Some(rank) = self.alias_rank(idx, needle) {
                    return Some((rank, idx, inst));
                }
                None
            })
            .collect();

        let today = today_ist();
        let cmp = |a: &(MatchRank, usize, &Instrument), b: &(MatchRank, usize, &Instrument)| {
            a.0.cmp(&b.0)
                .then_with(|| instrument_kind_rank(a.2).cmp(&instrument_kind_rank(b.2)))
                .then_with(|| compare_expiry(a.2, b.2, &today))
                .then_with(|| exchange_rank(&a.2.exchange).cmp(&exchange_rank(&b.2.exchange)))
                .then_with(|| a.2.tradingsymbol.len().cmp(&b.2.tradingsymbol.len()))
                .then_with(|| a.2.tradingsymbol.cmp(&b.2.tradingsymbol))
                .then_with(|| a.1.cmp(&b.1))
        };

        let count = if limit == 0 {
            scored.len()
        } else {
            limit.min(scored.len())
        };
        if count < scored.len() {
            scored.select_nth_unstable_by(count, cmp);
            scored.truncate(count);
        }
        scored.sort_unstable_by(cmp);
        scored.into_iter().map(|(_, _, inst)| inst).collect()
    }

    /// All exact symbol matches, ignoring case and whitespace, without a limit.
    /// Alerts identify instruments by exchange and symbol, so exact lookup must
    /// not depend on a row surviving the fuzzy search limit.
    #[must_use]
    pub fn by_exact_symbol(&self, symbol: &str) -> Vec<&Instrument> {
        let needle = compact_upper(symbol);
        self.instruments
            .iter()
            .enumerate()
            .filter(|(idx, _)| self.symbol_key(*idx) == needle)
            .map(|(_, inst)| inst)
            .collect()
    }

    #[must_use]
    pub fn resolve(&self, symbol: &str, exchange: Option<&str>, limit: usize) -> Vec<&Instrument> {
        let query = match exchange {
            Some(exchange) if !symbol.contains(':') => format!("{exchange}:{symbol}"),
            _ => symbol.to_string(),
        };
        let (_, needle) = split_query(&query);
        self.search(&query, limit)
            .into_iter()
            .filter(|inst| compact_upper(&inst.tradingsymbol).contains(&needle))
            .collect()
    }
}

/// Compare expiry dates for otherwise comparable contracts.
///
/// The daily master normally contains only live derivatives, but test fixtures,
/// stale caches, and a session that crosses midnight can include a past row.
/// Put the nearest expiry on or after today first, then past expiries,
/// malformed values, and absent expiries.
fn compare_expiry(a: &Instrument, b: &Instrument, today: &str) -> Ordering {
    expiry_sort_key(a, today).cmp(&expiry_sort_key(b, today))
}

/// Return one stable ordering key for an expiry. A key is required instead of
/// a pairwise fallback because valid, past, and malformed rows can be mixed.
fn expiry_sort_key<'a>(inst: &'a Instrument, today: &str) -> (u8, &'a str) {
    match inst.expiry.as_deref() {
        Some(expiry) if is_iso_date(expiry) => (u8::from(expiry < today), expiry),
        Some(expiry) => (2, expiry),
        None => (3, ""),
    }
}

fn is_iso_date(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(idx, byte)| matches!(idx, 4 | 7) || byte.is_ascii_digit())
}

/// Today's date in IST as an ISO string.
fn today_ist() -> String {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let days = (now + IST_OFFSET_SECS) / 86_400;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{day:02}")
}

/// Split a query into its optional `EXCHANGE:` prefix and the compacted needle.
/// Compacting both halves ignores whitespace around the exchange separator.
fn split_query(query: &str) -> (Option<String>, String) {
    let query = query.trim();
    match query.split_once(':') {
        Some((exchange, rest)) if !rest.trim().is_empty() => {
            (Some(compact_upper(exchange)), compact_upper(rest))
        }
        _ => (None, compact_upper(query)),
    }
}

/// Rank instruments by kind so a broad underlying search surfaces the spot
/// index, cash, and futures before the option strikes. Indices are spelled
/// `instrument_type: "EQ"` in the master and are only told apart by their
/// `INDICES` segment, so `NSE:NIFTY 50` outranks the NIFTY ETFs for `nifty`.
fn instrument_kind_rank(inst: &Instrument) -> u8 {
    if inst.is_index() {
        return 0;
    }
    match inst.instrument_type.as_str() {
        "EQ" | "FUT" => 1,
        "CE" | "PE" => 2,
        _ => 3,
    }
}

/// Locate `needle` as a subsequence of `haystack`, returning the tightest
/// `(start, span)` window that contains it: scan forward for the earliest end,
/// then walk back from there for the latest start.
fn subsequence_span(needle: &str, haystack: &str) -> Option<(usize, usize)> {
    if needle.is_empty() {
        return None;
    }
    let needle = needle.as_bytes();
    let hay = haystack.as_bytes();

    let mut matched = 0;
    let mut end = None;
    for (i, &byte) in hay.iter().enumerate() {
        if byte == needle[matched] {
            matched += 1;
            if matched == needle.len() {
                end = Some(i);
                break;
            }
        }
    }
    let end = end?;

    let mut remaining = needle.len();
    let mut start = end;
    for i in (0..=end).rev() {
        if hay[i] == needle[remaining - 1] {
            remaining -= 1;
            start = i;
            if remaining == 0 {
                break;
            }
        }
    }
    Some((start, end - start + 1))
}

fn exchange_rank(exchange: &str) -> u8 {
    match exchange {
        "NSE" => 0,
        "BSE" => 1,
        "NFO" => 2,
        "BFO" => 3,
        "MCX" => 4,
        "NCO" => 5,
        "CDS" => 6,
        "BCD" => 7,
        _ => 8,
    }
}

#[cfg(test)]
mod tests {
    use super::{InstrumentStore, subsequence_span};

    fn inst(token: u32, exchange: &str, symbol: &str, name: &str) -> super::Instrument {
        super::Instrument {
            instrument_token: token,
            tradingsymbol: symbol.to_string(),
            name: name.to_string(),
            exchange: exchange.to_string(),
            expiry: None,
            strike: None,
            tick_size: 0.05,
            lot_size: 1,
            instrument_type: "EQ".to_string(),
            segment: exchange.to_string(),
            freeze_quantity: None,
            max_order_quantity: None,
        }
    }

    #[test]
    fn search_and_resolve() {
        let store = InstrumentStore::new(vec![
            inst(408_065, "NSE", "INFY", "INFOSYS"),
            inst(9_604_354, "NFO", "INFY24AUGFUT", "INFY"),
            inst(256_265, "NSE", "NIFTY 50", "NIFTY 50"),
        ]);
        assert_eq!(store.search("infy", 10)[0].tradingsymbol, "INFY");
        assert_eq!(store.search("nfo:infy", 10)[0].exchange, "NFO");
        assert_eq!(store.resolve("NSE:INFY", None, 10)[0].tradingsymbol, "INFY");
    }

    /// Callers that already hold an exact symbol (the alerts API names
    /// instruments rather than tokening them) must not go through the ranked,
    /// capped search: whether the row they want survives a result limit is an
    /// implementation detail of `search`'s ranking, not something they should
    /// depend on. Whitespace and case are insignificant, and the caller still
    /// disambiguates between exchanges itself.
    #[test]
    fn by_exact_symbol_finds_the_row_regardless_of_search_ranking() {
        let mut rows = vec![
            inst(256_265, "NSE", "NIFTY 50", "NIFTY 50"),
            inst(260_105, "NSE", "NIFTY BANK", "NIFTY BANK"),
        ];
        for i in 0..40u32 {
            rows.push(inst(
                900_000 + i,
                "NFO",
                &format!("NIFTY24AUG{}CE", 20_000 + i * 50),
                "NIFTY",
            ));
        }
        let store = InstrumentStore::new(rows);

        let hits = store.by_exact_symbol("NIFTY 50");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].instrument_token, 256_265);
        assert_eq!(
            store.by_exact_symbol("nifty50")[0].instrument_token,
            256_265
        );
        assert!(store.by_exact_symbol("NIFTY").is_empty());
    }

    #[test]
    fn search_orders_same_underlying_by_expiry_ascending() {
        let mut aug = inst(1, "NFO", "INFY24AUGFUT", "INFY");
        aug.expiry = Some("2024-08-29".to_string());
        let mut sep = inst(2, "NFO", "INFY24SEPFUT", "INFY");
        sep.expiry = Some("2024-09-26".to_string());
        let mut jul = inst(3, "NFO", "INFY24JULFUT", "INFY");
        jul.expiry = Some("2024-07-25".to_string());

        let store = InstrumentStore::new(vec![aug, sep, jul]);
        let expiries: Vec<_> = store
            .search("infy", 10)
            .into_iter()
            .map(|i| i.expiry.clone())
            .collect();
        assert_eq!(
            expiries,
            vec![
                Some("2024-07-25".to_string()),
                Some("2024-08-29".to_string()),
                Some("2024-09-26".to_string()),
            ]
        );
    }

    #[test]
    fn search_prefers_nearest_live_expiry_before_result_limit() {
        let mut past = inst(1, "NFO", "INFY24PASTFUT", "INFY");
        past.instrument_type = "FUT".to_string();
        past.expiry = Some("2000-01-01".to_string());
        let mut far = inst(2, "NFO", "INFY99FARFUT", "INFY");
        far.instrument_type = "FUT".to_string();
        far.expiry = Some("2999-12-31".to_string());
        let mut near = inst(3, "NFO", "INFY99NEARFUT", "INFY");
        near.instrument_type = "FUT".to_string();
        near.expiry = Some("2099-01-01".to_string());

        let store = InstrumentStore::new(vec![past, far, near]);
        let matches = store.search("INFY", 1);
        assert_eq!(matches[0].tradingsymbol, "INFY99NEARFUT");
    }

    #[test]
    fn cross_exchange_contracts_are_ordered_by_expiry_before_exchange() {
        let mut mcx_nov = inst(1, "MCX", "CRUDEOIL26NOVFUT", "CRUDEOIL");
        mcx_nov.instrument_type = "FUT".to_string();
        mcx_nov.expiry = Some("2099-11-19".to_string());
        let mut nco_nov = inst(2, "NCO", "CRUDEOIL26NOVFUT", "CRUDEOIL");
        nco_nov.instrument_type = "FUT".to_string();
        nco_nov.expiry = Some("2099-11-19".to_string());
        let mut mcx_aug = inst(3, "MCX", "CRUDEOIL26AUGFUT", "CRUDEOIL");
        mcx_aug.instrument_type = "FUT".to_string();
        mcx_aug.expiry = Some("2099-08-19".to_string());
        let mut nco_aug = inst(4, "NCO", "CRUDEOIL26AUGFUT", "CRUDEOIL");
        nco_aug.instrument_type = "FUT".to_string();
        nco_aug.expiry = Some("2099-08-19".to_string());

        let store = InstrumentStore::new(vec![mcx_nov, nco_nov, mcx_aug, nco_aug]);
        let labels: Vec<_> = store
            .search("CRUDEOIL", 10)
            .into_iter()
            .map(|inst| (inst.exchange.as_str(), inst.expiry.as_deref().unwrap()))
            .collect();
        assert_eq!(
            labels,
            vec![
                ("MCX", "2099-08-19"),
                ("NCO", "2099-08-19"),
                ("MCX", "2099-11-19"),
                ("NCO", "2099-11-19"),
            ]
        );
    }

    #[test]
    fn underlying_search_does_not_bury_nco_futures_behind_option_strikes() {
        let mut instruments = Vec::new();
        for strike in 0..60 {
            let mut option = inst(
                strike + 1,
                "MCX",
                &format!("CRUDEOIL26AUG{}CE", 4000 + strike * 50),
                "CRUDEOIL",
            );
            option.instrument_type = "CE".to_string();
            option.segment = "MCX-OPT".to_string();
            option.expiry = Some("2026-08-17".to_string());
            instruments.push(option);
        }
        let mut nco_future = inst(1000, "NCO", "CRUDEOIL26AUGFUT", "CRUDEOIL");
        nco_future.instrument_type = "FUT".to_string();
        nco_future.segment = "NCO-FUT".to_string();
        nco_future.expiry = Some("2026-08-19".to_string());
        instruments.push(nco_future);

        let store = InstrumentStore::new(instruments);
        let matches = store.search("CRUDEOIL", 50);
        assert_eq!(matches[0].tradingsymbol, "CRUDEOIL26AUGFUT");
        assert!(matches.iter().any(|inst| inst.exchange == "NCO"));
    }

    /// Build an option instrument on top of the base equity fixture.
    fn opt(token: u32, symbol: &str, itype: &str, strike: f64, expiry: &str) -> super::Instrument {
        let mut inst = inst(token, "NFO", symbol, "NIFTY");
        inst.segment = "NFO-OPT".to_string();
        inst.instrument_type = itype.to_string();
        inst.strike = Some(strike);
        inst.expiry = Some(expiry.to_string());
        inst.lot_size = 65;
        inst
    }

    #[test]
    fn expiry_date_field_populates() {
        let json = r#"{"instrument_token":13155586,"tradingsymbol":"NIFTY2671424400CE",
            "name":"NIFTY","exchange":"NFO","last_price":0,"strike":24400,"tick_size":0.05,
            "lot_size":65,"instrument_type":"CE","segment":"NFO-OPT","expiry_date":"2026-07-14"}"#;
        let inst: super::Instrument = serde_json::from_str(json).unwrap();
        assert_eq!(inst.expiry.as_deref(), Some("2026-07-14"));
        assert_eq!(inst.strike, Some(24400.0));
    }

    #[test]
    fn subsequence_cases() {
        assert_eq!(subsequence_span("ABC", "ABC"), Some((0, 3)));
        assert_eq!(subsequence_span("ABC", "AXBXC"), Some((0, 5)));
        assert_eq!(subsequence_span("ABC", "ZZABC"), Some((2, 3)));
        assert_eq!(subsequence_span("ABC", "AB"), None);
        assert_eq!(subsequence_span("", "ABC"), None);
    }

    /// Build the option legs a `nifty24500`-style query has to rank, using the
    /// real symbols from the bug report's screenshot.
    fn strike_master() -> Vec<super::Instrument> {
        let mut out = Vec::new();
        let rows: [(&str, &str, &str, f64, &str); 6] = [
            (
                "BANKNIFTY26SEP45000CE",
                "BANKNIFTY",
                "CE",
                45000.0,
                "2026-09-29",
            ),
            (
                "BANKNIFTY26SEP45000PE",
                "BANKNIFTY",
                "PE",
                45000.0,
                "2026-09-29",
            ),
            ("NIFTY2681824500CE", "NIFTY", "CE", 24500.0, "2026-08-18"),
            ("NIFTY2681824500PE", "NIFTY", "PE", 24500.0, "2026-08-18"),
            ("NIFTY26AUG24500CE", "NIFTY", "CE", 24500.0, "2026-08-25"),
            (
                "FINNIFTY26AUG24500CE",
                "FINNIFTY",
                "CE",
                24500.0,
                "2026-08-25",
            ),
        ];
        for (i, (symbol, name, itype, strike, expiry)) in rows.into_iter().enumerate() {
            let mut it = inst(u32::try_from(i).unwrap() + 1, "NFO", symbol, name);
            it.segment = "NFO-OPT".to_string();
            it.instrument_type = itype.to_string();
            it.strike = Some(strike);
            it.expiry = Some(expiry.to_string());
            out.push(it);
        }
        out
    }

    #[test]
    fn strike_search_ranks_the_typed_underlying_first() {
        let store = InstrumentStore::new(strike_master());
        let symbols: Vec<&str> = store
            .search("nifty24500", 10)
            .into_iter()
            .map(|i| i.tradingsymbol.as_str())
            .collect();
        assert_eq!(
            symbols,
            vec![
                "NIFTY2681824500CE",
                "NIFTY2681824500PE",
                "NIFTY26AUG24500CE",
                "FINNIFTY26AUG24500CE",
                "BANKNIFTY26SEP45000CE",
                "BANKNIFTY26SEP45000PE",
            ]
        );
    }

    #[test]
    fn weekly_expiry_aliases_match_month_date_strike_and_type_queries() {
        let rows = [
            ("NIFTY2690824000CE", "2099-09-08"),
            ("NIFTY2691524000CE", "2099-09-15"),
            ("NIFTY2692224000CE", "2099-09-22"),
            ("NIFTY26SEP24000CE", "2099-09-29"),
        ];
        let store = InstrumentStore::new(
            rows.into_iter()
                .enumerate()
                .map(|(idx, (symbol, expiry))| {
                    opt(
                        u32::try_from(idx).unwrap() + 1,
                        symbol,
                        "CE",
                        24000.0,
                        expiry,
                    )
                })
                .collect(),
        );

        let month_matches: Vec<_> = store
            .search("nifty sep 24000 ce", 10)
            .into_iter()
            .map(|inst| inst.tradingsymbol.as_str())
            .collect();
        assert_eq!(
            month_matches,
            vec![
                "NIFTY2690824000CE",
                "NIFTY2691524000CE",
                "NIFTY2692224000CE",
                "NIFTY26SEP24000CE",
            ]
        );

        let date_matches = store.search("nifty 8 sep 24000 ce", 10);
        assert_eq!(date_matches.len(), 1);
        assert_eq!(date_matches[0].tradingsymbol, "NIFTY2690824000CE");
    }

    #[test]
    fn expiry_aliases_rank_before_truncation_but_raw_symbol_wins() {
        let rows = [
            ("NIFTY2690824000CE", "2099-09-08"),
            ("NIFTY2691524000CE", "2099-09-15"),
            ("NIFTY2692224000CE", "2099-09-22"),
            ("NIFTY26SEP24000CE", "2099-09-29"),
        ];
        let store = InstrumentStore::new(
            rows.into_iter()
                .enumerate()
                .map(|(idx, (symbol, expiry))| {
                    opt(
                        u32::try_from(idx).unwrap() + 1,
                        symbol,
                        "CE",
                        24000.0,
                        expiry,
                    )
                })
                .collect(),
        );

        assert_eq!(
            store.search("nifty sep 24000 ce", 1)[0].tradingsymbol,
            "NIFTY2690824000CE"
        );
        assert_eq!(
            store.search("NIFTY2692224000CE", 1)[0].tradingsymbol,
            "NIFTY2692224000CE"
        );
    }

    #[test]
    fn whitespace_in_the_query_is_ignored() {
        let store = InstrumentStore::new(strike_master());
        assert_eq!(
            store.search("nifty 24500", 10)[0].tradingsymbol,
            "NIFTY2681824500CE"
        );
        assert_eq!(store.search("NFO: nifty", 10).len(), 6);
        assert_eq!(store.search("  nfo : nifty 24500 ", 10).len(), 6);
        assert!(store.search("NFO:   ", 10).is_empty());
    }

    #[test]
    fn spaced_symbols_match_either_spelling() {
        let mut index = inst(256_265, "NSE", "NIFTY 50", "NIFTY 50");
        index.segment = "INDICES".to_string();
        let store = InstrumentStore::new(vec![
            inst(4_634_113, "NSE", "NIFTY1", "KOTAK NIFTY 50 ETF"),
            inst(
                11_719_170,
                "NSE",
                "NIFTYBEES",
                "NIPPON INDIA ETF NIFTY 50 BEES",
            ),
            index,
        ]);
        assert_eq!(store.search("nifty50", 10)[0].tradingsymbol, "NIFTY 50");
        assert_eq!(store.search("NIFTY 50", 10)[0].tradingsymbol, "NIFTY 50");
        assert_eq!(store.search("nifty", 10)[0].tradingsymbol, "NIFTY 50");
        assert_eq!(store.search("nifty", 10)[0].display_type(), "INDEX");
        assert_eq!(store.search("niftybees", 10)[0].display_type(), "EQ");
        assert_eq!(
            store.resolve("NIFTY50", Some("NSE"), 10)[0].tradingsymbol,
            "NIFTY 50"
        );
    }

    /// `SearchKey` stores neither key as a plain `String`, an already-compact
    /// symbol is read back off the instrument, and names are interned, so the
    /// encoding has to be invisible: every key the store reports must be
    /// byte-identical to the naive `compact_upper` it replaced.
    #[test]
    fn the_deduped_search_keys_are_byte_identical_to_the_naive_ones() {
        let rows = vec![
            inst(256_265, "NSE", "NIFTY 50", "NIFTY 50"), // a symbol with a space
            inst(408_065, "NSE", "infy", "INFOSYS"),      // a lowercase symbol
            inst(1, "NFO", "NIFTY2671424400CE", "NIFTY"), // a repeated name
            inst(2, "NFO", "NIFTY2671424500CE", "NIFTY"),
        ];
        let store = InstrumentStore::new(rows.clone());
        for (idx, row) in rows.iter().enumerate() {
            assert_eq!(
                store.symbol_key(idx),
                super::compact_upper(&row.tradingsymbol),
                "symbol key {idx}"
            );
            assert_eq!(
                store.name_key(idx),
                super::compact_upper(&row.name),
                "name key {idx}"
            );
        }

        assert_eq!(store.name_pool.len(), 3);
        assert!(store.keys[0].symbol_upper.is_some());
        assert!(store.keys[1].symbol_upper.is_some());
        assert!(store.keys[2].symbol_upper.is_none());
        assert!(store.keys[3].symbol_upper.is_none());

        assert_eq!(store.search("nifty50", 10)[0].tradingsymbol, "NIFTY 50");
        assert_eq!(store.search("INFY", 10)[0].tradingsymbol, "infy");
        assert_eq!(store.by_exact_symbol("Infy")[0].instrument_token, 408_065);
    }
}
