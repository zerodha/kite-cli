use anyhow::Result;
use kiteconnect::KiteConnect;
use kiteconnect::Request;
use serde::Deserialize;
use serde_json::{Value, json};

/// The row field the movers panel both ranks and displays. Also the `order_by`
/// the gainers/losers presets sort on, see [`mover_presets`].
pub const CHANGE_PERCENT: &str = "change_percent";

pub const ORDER_DESC: &str = "desc";
pub const ORDER_ASC: &str = "asc";

/// `per_page` the movers fetch asks for. 25 is the page size Kite Web uses;
/// the caller truncates to the rows it draws.
pub const DEFAULT_PER_PAGE: u32 = 25;

/// The four index universes the screener's `tags` filter accepts, in the order
/// the terminal cycles them. These are the exact `values` of the `tags` filter
/// in `GET /screener/meta` (`{"type":"distinct","values":[…]}`), the strings
/// have to match byte-for-byte or the filter silently matches nothing.
///
/// They are baked in rather than fetched because the terminal cycles a fixed
/// enum of universes, and a dynamic list can't be a keybinding cycle. `meta` is
/// still the source of truth for what these strings are; if Kite ever adds an
/// index, this list (and `MoversUniverse` in `tui::data`) is the one place
/// to extend.
pub const UNIVERSE_TAGS: [&str; 4] = ["Nifty 50", "Nifty 100", "Nifty 500", "Nifty F&O Stocks"];

/// One `POST /screener/instruments` request.
///
/// `query` is a flat **string** DSL, not structured JSON: `field>=v`,
/// `field<v`, `field='str'`, `&` for AND, and OR only within a single distinct
/// field as `(a='x' OR a='y')`. `market_cap>=0` is the service's no-filter
/// sentinel. There is no column-selection parameter, the full ~35-field row
/// always comes back, and the caller picks what it needs.
///
/// An empty `tags` means the whole market, which is exactly what the movers
/// panel must not default to: see [`ScreenerQuery::tags`].
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenerQuery {
    pub query: String,
    pub order_by: String,
    /// [`ORDER_DESC`] or [`ORDER_ASC`].
    pub order: String,
    pub per_page: u32,
    /// 1-based.
    pub page: u32,
    /// Index universes to restrict to (from [`UNIVERSE_TAGS`]). Empty means the
    /// unconstrained whole market.
    ///
    /// Empty is deliberately NOT the default anywhere in this terminal.
    /// An unconstrained `top_gainers` query can rank a suspended series first
    /// (for example **+2781.36% on volume 0**), while the same query restricted
    /// to `["Nifty 500"]` returns liquid rows. So an index universe is the default
    /// and the unconstrained query is an explicitly-labelled opt-in.
    pub tags: Vec<String>,
}

/// JSON body for `POST /screener/instruments`.
///
/// `tags` is **omitted entirely** when no universe is selected rather than sent
/// as `[]`, because the service reads an absent `tags` as "whole market" and an
/// empty array is an untested third state.
#[must_use]
pub fn instruments_body(query: &ScreenerQuery) -> Value {
    let mut body = json!({
        "per_page": query.per_page,
        "page": query.page,
        "query": query.query,
        "order_by": query.order_by,
        "order": query.order,
    });
    if !query.tags.is_empty()
        && let Some(object) = body.as_object_mut()
    {
        object.insert("tags".to_string(), json!(query.tags));
    }
    body
}

/// One entry of `GET /screener/presets`' `data.presets[]`.
///
/// Deliberately does **not** model `key` or `name`. The service owns the preset
/// list, and matching on a key string (`"top_gainers"`) would break the moment
/// Kite renames or reorders it. [`mover_presets`] instead selects on the shape
/// of the preset, what it orders by, and in which direction, which is what
/// the panel actually cares about.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ScreenerPreset {
    #[serde(default)]
    pub query: String,
    #[serde(default)]
    pub order_by: String,
    #[serde(default)]
    pub order: String,
    /// Set on presets whose ranking is computed rather than read off a row
    /// field (live example: `high_volatility_stocks`). Skipped for movers ,
    /// a computed ranking is not a change-percent ranking.
    #[serde(default)]
    pub has_computation: bool,
}

/// The query/ordering half of a preset, resolved for one side of the movers
/// panel.
#[derive(Debug, Clone, PartialEq)]
pub struct MoverPreset {
    pub query: String,
    pub order_by: String,
    pub order: String,
}

impl MoverPreset {
    /// Built-in gainers ranking, used when the server's preset list has nothing
    /// usable (or could not be fetched). Mirrors the live `top_gainers` preset.
    #[must_use]
    pub fn gainers() -> Self {
        Self {
            query: "change_percent>0".to_string(),
            order_by: CHANGE_PERCENT.to_string(),
            order: ORDER_DESC.to_string(),
        }
    }

    /// Built-in losers ranking. Mirrors the live `top_losers` preset, whose
    /// `order` is `asc` so the *most* negative change comes first.
    #[must_use]
    pub fn losers() -> Self {
        Self {
            query: "change_percent<0".to_string(),
            order_by: CHANGE_PERCENT.to_string(),
            order: ORDER_ASC.to_string(),
        }
    }
}

/// Pick the gainers and losers presets out of the server's list, falling back
/// to the built-ins for either side the server doesn't offer.
///
/// Selection is by shape, never by `key`: a preset qualifies when it ranks on
/// [`CHANGE_PERCENT`], isn't a computed ranking, and sorts in the direction the
/// side needs. That keeps the server as the owner of the actual query strings
/// (so a tightened `top_gainers` filter reaches the terminal for free) without
/// the terminal hardcoding preset names it doesn't control.
pub fn mover_presets(presets: &[ScreenerPreset]) -> (MoverPreset, MoverPreset) {
    let pick = |order: &str| {
        presets
            .iter()
            .find(|preset| {
                !preset.has_computation
                    && preset.order_by == CHANGE_PERCENT
                    && preset.order == order
                    && !preset.query.is_empty()
            })
            .map(|preset| MoverPreset {
                query: preset.query.clone(),
                order_by: preset.order_by.clone(),
                order: preset.order.clone(),
            })
    };
    (
        pick(ORDER_DESC).unwrap_or_else(MoverPreset::gainers),
        pick(ORDER_ASC).unwrap_or_else(MoverPreset::losers),
    )
}

/// # Errors
/// Returns an error if the request fails or the response cannot be decoded.
pub async fn presets(client: &KiteConnect) -> Result<Value> {
    Ok(client.call(Request::get("/screener/presets")).raw().await?)
}

/// Run a screener query. Read-only despite the `POST`; body is JSON.
///
/// # Errors
/// Returns an error if the query fails or the response cannot be decoded.
pub async fn instruments(client: &KiteConnect, query: &ScreenerQuery) -> Result<Value> {
    Ok(client
        .call(Request::post("/screener/instruments").json(instruments_body(query)))
        .raw()
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nifty500_gainers() -> ScreenerQuery {
        ScreenerQuery {
            query: "change_percent>0".to_string(),
            order_by: CHANGE_PERCENT.to_string(),
            order: ORDER_DESC.to_string(),
            per_page: DEFAULT_PER_PAGE,
            page: 1,
            tags: vec!["Nifty 500".to_string()],
        }
    }

    /// The exact body the default movers view sends, field by field. This is the
    /// wire contract: `query` is a flat string (not a structured filter object),
    /// `per_page`/`page` are numbers (not strings), and the whole thing is JSON.
    #[test]
    fn instruments_body_matches_the_wire_contract() {
        let body = instruments_body(&nifty500_gainers());
        assert_eq!(body["per_page"], json!(25));
        assert_eq!(body["page"], json!(1));
        assert_eq!(body["query"], json!("change_percent>0"));
        assert_eq!(body["order_by"], json!("change_percent"));
        assert_eq!(body["order"], json!("desc"));
        assert_eq!(body["tags"], json!(["Nifty 500"]));
    }

    /// The whole-market query omits `tags` rather than sending `[]`: an absent
    /// `tags` is the shape the service documents as "no universe filter".
    #[test]
    fn instruments_body_omits_tags_when_unconstrained() {
        let body = instruments_body(&ScreenerQuery {
            tags: Vec::new(),
            ..nifty500_gainers()
        });
        assert!(
            body.get("tags").is_none(),
            "unconstrained query must omit tags entirely, got {body}"
        );
        assert_eq!(body["query"], json!("change_percent>0"));
    }

    /// The `data.presets[]` shape from the live `GET /screener/presets`,
    /// including the computed preset that must not be mistaken for a movers
    /// ranking.
    fn live_presets() -> Vec<ScreenerPreset> {
        serde_json::from_value(json!([
            {
                "key": "top_gainers",
                "name": "Top gainers",
                "description": "Stocks up the most today",
                "query": "change_percent>0",
                "order_by": "change_percent",
                "order": "desc",
                "type": "preset",
                "has_computation": false
            },
            {
                "key": "top_losers",
                "name": "Top losers",
                "description": "Stocks down the most today",
                "query": "change_percent<0",
                "order_by": "change_percent",
                "order": "asc",
                "type": "preset",
                "has_computation": false
            },
            {
                "key": "high_volatility_stocks",
                "name": "High volatility",
                "description": "",
                "query": "market_cap>=0",
                "order_by": "change_percent",
                "order": "desc",
                "type": "preset",
                "has_computation": true
            }
        ]))
        .expect("presets parse")
    }

    /// The server's own query strings must win, and the sides must not be
    /// swapped: losers sort `asc` so the most negative row is first.
    #[test]
    fn mover_presets_take_the_servers_queries_by_direction() {
        let (gainers, losers) = mover_presets(&live_presets());
        assert_eq!(gainers.query, "change_percent>0");
        assert_eq!(gainers.order, "desc");
        assert_eq!(losers.query, "change_percent<0");
        assert_eq!(losers.order, "asc");
    }

    /// `high_volatility_stocks` also orders `change_percent desc`, so without
    /// the `has_computation` guard it could be picked as the gainers preset and
    /// silently rank the panel by something else entirely. Pin that it isn't ,
    /// with the real presets removed, the built-in fallback is used instead.
    #[test]
    fn mover_presets_skip_computed_rankings() {
        let computed: Vec<ScreenerPreset> = live_presets()
            .into_iter()
            .filter(|preset| preset.has_computation)
            .collect();
        assert_eq!(computed.len(), 1, "fixture should have one computed preset");
        let (gainers, _) = mover_presets(&computed);
        assert_eq!(gainers, MoverPreset::gainers());
        assert_eq!(gainers.query, "change_percent>0");
    }

    /// A preset list the terminal can't use (empty, or renamed beyond
    /// recognition) must still produce a working movers view.
    #[test]
    fn mover_presets_fall_back_to_built_ins() {
        let (gainers, losers) = mover_presets(&[]);
        assert_eq!(gainers, MoverPreset::gainers());
        assert_eq!(losers, MoverPreset::losers());

        let unusable: Vec<ScreenerPreset> = serde_json::from_value(json!([
            { "query": "pe<15", "order_by": "pe", "order": "asc", "has_computation": false }
        ]))
        .expect("presets parse");
        let (gainers, losers) = mover_presets(&unusable);
        assert_eq!(gainers, MoverPreset::gainers());
        assert_eq!(losers, MoverPreset::losers());
    }

    /// The `tags` strings are matched exactly by the service, so a stray
    /// abbreviation or casing change would silently filter to nothing. Pinned
    /// against the `values` of `GET /screener/meta`'s `tags` filter.
    #[test]
    fn universe_tags_match_screener_meta() {
        assert_eq!(
            UNIVERSE_TAGS,
            ["Nifty 50", "Nifty 100", "Nifty 500", "Nifty F&O Stocks"]
        );
    }
}
