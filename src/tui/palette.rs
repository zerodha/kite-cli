use std::ops::Range;

use crate::tui::app::Side;
use crate::tui::workspace::{PaneId, Preset};

#[derive(Debug, Clone, PartialEq)]
pub enum WatchlistCmd {
    Switch(String),
    SwitchId { id: String, name: String },
    Rename(String),
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    None,
    Required,
    Optional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Jump {
    Workspace(Preset),
    Pane(PaneId),
}

impl Jump {
    pub fn syntax(self) -> String {
        match self {
            Jump::Workspace(preset) => format!("{}: switch workspace", preset.title()),
            Jump::Pane(pane) => format!("{}: open in {}", pane.title(), pane.preset().title()),
        }
    }

    pub fn config_token(self) -> &'static str {
        match self {
            Jump::Workspace(preset) => preset.config_token(),
            Jump::Pane(pane) => pane.config_token(),
        }
    }
}

pub const WORKSPACE_SYNTAX: &str =
    "workspace NAME: Overview, Trading, Portfolio, Derivatives, Orders, Invest";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keyword {
    pub word: &'static str,
    pub arg: ArgKind,
    pub jump: Option<Jump>,
}

impl Keyword {
    pub fn completion_text(self) -> String {
        match self.arg {
            ArgKind::None => self.word.to_string(),
            ArgKind::Required | ArgKind::Optional => format!("{} ", self.word),
        }
    }

    pub fn syntax(self) -> String {
        if let Some(jump) = self.jump {
            return jump.syntax();
        }
        match self.word {
            "workspace" => WORKSPACE_SYNTAX.to_string(),
            "buy" | "sell" => format!("{} SYMBOL [QTY] [@PRICE]", self.word),
            "alert" => "alert [SYMBOL]: new price alert".to_string(),
            "theme" => "theme PRESET: Enter saves".to_string(),
            "wl" => "wl NAME · wl rename NAME · wl delete".to_string(),
            "quit" => "quit: leave the terminal".to_string(),
            _ => unreachable!("keyword catalog defines syntax for {}", self.word),
        }
    }
}

pub const CATALOG: [Keyword; 21] = [
    Keyword {
        word: "buy",
        arg: ArgKind::Required,
        jump: None,
    },
    Keyword {
        word: "sell",
        arg: ArgKind::Required,
        jump: None,
    },
    Keyword {
        word: "alert",
        arg: ArgKind::Optional,
        jump: None,
    },
    Keyword {
        word: "theme",
        arg: ArgKind::Optional,
        jump: None,
    },
    Keyword {
        word: "wl",
        arg: ArgKind::Optional,
        jump: None,
    },
    Keyword {
        word: "watch",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::TradingWatch)),
    },
    Keyword {
        word: "positions",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::PortfolioPositions)),
    },
    Keyword {
        word: "orders",
        arg: ArgKind::None,
        jump: Some(Jump::Workspace(Preset::Orders)),
    },
    Keyword {
        word: "holdings",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::PortfolioHoldings)),
    },
    Keyword {
        word: "mf",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::InvestMf)),
    },
    Keyword {
        word: "gtt",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::OrdersGtt)),
    },
    Keyword {
        word: "alerts",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::OrdersAlerts)),
    },
    Keyword {
        word: "bids",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::InvestBids)),
    },
    Keyword {
        word: "auction",
        arg: ArgKind::None,
        jump: Some(Jump::Pane(PaneId::InvestBids)),
    },
    Keyword {
        word: "overview",
        arg: ArgKind::None,
        jump: Some(Jump::Workspace(Preset::Overview)),
    },
    Keyword {
        word: "trading",
        arg: ArgKind::None,
        jump: Some(Jump::Workspace(Preset::Trading)),
    },
    Keyword {
        word: "portfolio",
        arg: ArgKind::None,
        jump: Some(Jump::Workspace(Preset::Portfolio)),
    },
    Keyword {
        word: "derivatives",
        arg: ArgKind::None,
        jump: Some(Jump::Workspace(Preset::Derivatives)),
    },
    Keyword {
        word: "invest",
        arg: ArgKind::None,
        jump: Some(Jump::Workspace(Preset::Invest)),
    },
    Keyword {
        word: "workspace",
        arg: ArgKind::Required,
        jump: None,
    },
    Keyword {
        word: "quit",
        arg: ArgKind::None,
        jump: None,
    },
];

pub fn workspace_matches(query: &str) -> impl Iterator<Item = Preset> + '_ {
    let lower = query.to_ascii_lowercase();
    Preset::ALL
        .into_iter()
        .filter(move |preset| preset.title().to_ascii_lowercase().starts_with(&lower))
}

pub fn keyword_matches(token: &str) -> impl Iterator<Item = Keyword> {
    let lower = token.to_ascii_lowercase();
    CATALOG
        .into_iter()
        .filter(move |kw| !lower.is_empty() && kw.word.starts_with(&lower))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderError {
    MissingSymbol,
    Quantity,
    Price,
    ExtraTokens,
}

impl OrderError {
    pub fn message(self) -> &'static str {
        match self {
            Self::MissingSymbol => "Type a symbol after the side",
            Self::Quantity => "Quantity must be 1..4294967295",
            Self::Price => "Price must be finite and > 0",
            Self::ExtraTokens => "Expected SYMBOL [QTY] [@PRICE]",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct OrderDraft {
    pub side: Side,
    pub symbol: Range<usize>,
    pub qty: Option<u32>,
    pub price: Option<f64>,
    pub error: Option<OrderError>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatchlistDraft {
    List,
    Name { range: Range<usize>, name: String },
    RenameMissing { needs_space: bool },
    Rename { name: String },
    Delete,
    DeleteExtra,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum Draft {
    #[default]
    Browse,
    Partial,
    Search,
    Order(OrderDraft),
    Watchlist(WatchlistDraft),
    Theme {
        arg: Range<usize>,
        query: String,
    },
    Alert {
        arg: Range<usize>,
        query: String,
    },
    Jump {
        jump: Jump,
        word: &'static str,
    },
    Workspace {
        arg: Range<usize>,
        query: String,
    },
    Quit,
}

pub fn analyze(input: &str) -> Draft {
    let lead = input.len() - input.trim_start().len();
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Draft::Browse;
    }
    let first = trimmed.split_whitespace().next().unwrap_or("");
    let lower_first = first.to_ascii_lowercase();

    if matches!(trimmed.to_ascii_lowercase().as_str(), "q" | "quit" | "exit") {
        return Draft::Quit;
    }

    let side = match lower_first.as_str() {
        "buy" | "b" => Some(Side::Buy),
        "sell" | "s" => Some(Side::Sell),
        _ => None,
    };
    if let Some(side) = side
        && !input.trim_start()[first.len()..].is_empty()
    {
        return Draft::Order(analyze_order(side, input, lead + first.len()));
    }

    if lower_first == "wl" {
        return Draft::Watchlist(analyze_watchlist(input, lead + first.len()));
    }

    let argument = |make: fn(Range<usize>, String) -> Draft| {
        let arg = (lead + first.len())..input.len();
        let query = input[arg.clone()].trim().to_string();
        make(arg, query)
    };

    if lower_first == "theme" {
        return argument(|arg, query| Draft::Theme { arg, query });
    }

    if lower_first == "alert" {
        return argument(|arg, query| Draft::Alert { arg, query });
    }

    if lower_first == "workspace" {
        return argument(|arg, query| Draft::Workspace { arg, query });
    }

    if trimmed.split_whitespace().count() == 1
        && let Some((jump, word)) = jump_keyword(&lower_first)
    {
        return Draft::Jump { jump, word };
    }

    if keyword_matches(first).next().is_some() {
        return Draft::Partial;
    }
    Draft::Search
}

fn jump_keyword(token: &str) -> Option<(Jump, &'static str)> {
    CATALOG.into_iter().find_map(|kw| {
        kw.jump
            .filter(|_| kw.word.starts_with(token))
            .map(|jump| (jump, kw.word))
    })
}

fn analyze_order(side: Side, input: &str, after_side: usize) -> OrderDraft {
    let rest = &input[after_side..];
    let symbol_start = after_side + (rest.len() - rest.trim_start().len());
    let body = input[symbol_start..].trim_end();
    let draft = |symbol: Range<usize>, error| OrderDraft {
        side,
        symbol,
        qty: None,
        price: None,
        error,
    };
    if body.is_empty() {
        return draft(symbol_start..symbol_start, Some(OrderError::MissingSymbol));
    }

    let (head, price) = match body.split_once('@') {
        Some((head, tail)) => {
            let price: Option<f64> = tail.trim().parse().ok();
            let price = price.filter(|price| price.is_finite() && *price > 0.0);
            (head.trim_end(), Some(price))
        }
        None => (body, None),
    };

    let Some(symbol) = head.split_whitespace().next() else {
        return draft(symbol_start..symbol_start, Some(OrderError::MissingSymbol));
    };
    let symbol_range = symbol_start..(symbol_start + symbol.len());

    let mut rest_of_head = head[symbol.len()..].split_whitespace();
    let qty = match rest_of_head.next() {
        Some(raw) => match raw.parse::<u32>() {
            Ok(qty) if qty > 0 => Some(qty),
            _ => return draft(symbol_range, Some(OrderError::Quantity)),
        },
        None => None,
    };
    if rest_of_head.next().is_some() {
        return draft(symbol_range, Some(OrderError::ExtraTokens));
    }

    let price = match price {
        Some(Some(price)) => Some(price),
        Some(None) => return draft(symbol_range, Some(OrderError::Price)),
        None => None,
    };
    OrderDraft {
        side,
        symbol: symbol_range,
        qty,
        price,
        error: None,
    }
}

fn analyze_watchlist(input: &str, after_wl: usize) -> WatchlistDraft {
    let rest = &input[after_wl..];
    let arg_start = after_wl + (rest.len() - rest.trim_start().len());
    let body = input[arg_start..].trim_end();
    if body.is_empty() {
        return WatchlistDraft::List;
    }
    let (keyword, tail) = match body.split_once(char::is_whitespace) {
        Some((keyword, tail)) => (keyword, tail.trim()),
        None => (body, ""),
    };
    match keyword.to_ascii_lowercase().as_str() {
        "rename" if tail.is_empty() => WatchlistDraft::RenameMissing {
            needs_space: input.ends_with("rename"),
        },
        "rename" => WatchlistDraft::Rename {
            name: tail.to_string(),
        },
        "delete" if tail.is_empty() => WatchlistDraft::Delete,
        "delete" => WatchlistDraft::DeleteExtra,
        _ => WatchlistDraft::Name {
            range: arg_start..input.len(),
            name: body.to_string(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionEdit {
    pub range: Range<usize>,
    pub text: String,
}

impl CompletionEdit {
    pub fn whole(input: &str, text: impl Into<String>) -> Self {
        Self {
            range: 0..input.len(),
            text: text.into(),
        }
    }

    pub fn apply(&self, input: &str) -> String {
        let mut out = String::with_capacity(input.len() + self.text.len());
        out.push_str(&input[..self.range.start]);
        out.push_str(&self.text);
        out.push_str(&input[self.range.end..]);
        out
    }

    pub fn is_noop(&self, input: &str) -> bool {
        input.get(self.range.clone()) == Some(self.text.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionTarget<'a> {
    Keyword(Keyword),
    OrderSymbol {
        symbol: &'a Range<usize>,
        identity: &'a str,
    },
    Instrument(&'a str),
    ThemePreset {
        arg: &'a Range<usize>,
        preset: &'a str,
    },
    AlertInstrument {
        arg: &'a Range<usize>,
        identity: &'a str,
    },
    WatchlistName {
        range: &'a Range<usize>,
        name: &'a str,
    },
    WatchlistForm(&'a str),
    MenuLabel(&'a str),
    WorkspaceName {
        arg: &'a Range<usize>,
        name: &'a str,
    },
    RenameSpace,
}

pub fn complete(input: &str, target: CompletionTarget<'_>) -> CompletionEdit {
    match target {
        CompletionTarget::Keyword(kw) => CompletionEdit::whole(input, kw.completion_text()),
        CompletionTarget::OrderSymbol { symbol, identity } => CompletionEdit {
            range: symbol.clone(),
            text: if symbol.end == input.len() {
                format!("{identity} ")
            } else {
                identity.to_string()
            },
        },
        CompletionTarget::Instrument(identity) | CompletionTarget::MenuLabel(identity) => {
            CompletionEdit::whole(input, identity)
        }
        CompletionTarget::ThemePreset { arg, preset: value }
        | CompletionTarget::WorkspaceName { arg, name: value }
        | CompletionTarget::AlertInstrument {
            arg,
            identity: value,
        } => CompletionEdit {
            range: arg.clone(),
            text: format!(" {value}"),
        },
        CompletionTarget::WatchlistName { range, name } => CompletionEdit {
            range: range.clone(),
            text: name.to_string(),
        },
        CompletionTarget::WatchlistForm(form) => CompletionEdit::whole(input, form),
        CompletionTarget::RenameSpace => CompletionEdit {
            range: input.len()..input.len(),
            text: " ".to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_catalog_keyword_has_syntax() {
        for keyword in super::CATALOG {
            assert!(!keyword.syntax().is_empty(), "{}", keyword.word);
        }
    }

    use super::{
        ArgKind, CATALOG, CompletionTarget, Draft, Jump, Keyword, OrderDraft, OrderError, Range,
        WatchlistDraft, analyze, complete, jump_keyword, keyword_matches, workspace_matches,
    };
    use crate::tui::app::Side;
    use crate::tui::workspace::{PaneId, Preset};

    fn keyword(word: &str) -> Keyword {
        CATALOG
            .into_iter()
            .find(|kw| kw.word == word)
            .unwrap_or_else(|| panic!("{word} is not in the catalog"))
    }

    fn complete_keyword(input: &str, word: &str) -> String {
        complete(input, CompletionTarget::Keyword(keyword(word))).apply(input)
    }

    fn order_draft(input: &str) -> OrderDraft {
        match analyze(input) {
            Draft::Order(draft) => draft,
            other => panic!("expected an order draft for {input:?}, got {other:?}"),
        }
    }

    fn wl_draft(input: &str) -> WatchlistDraft {
        match analyze(input) {
            Draft::Watchlist(draft) => draft,
            other => panic!("expected a watchlist draft for {input:?}, got {other:?}"),
        }
    }

    #[test]
    fn catalog_order_agrees_with_the_destination_lookup() {
        for kw in CATALOG.into_iter().filter(|kw| kw.jump.is_some()) {
            assert_eq!(
                jump_keyword(kw.word).map(|(jump, _)| jump),
                kw.jump,
                "{:?} must parse back to its own destination",
                kw.word
            );
            for len in 1..=kw.word.len() {
                assert!(
                    jump_keyword(&kw.word[..len]).is_some(),
                    "{:?} must reach a destination",
                    &kw.word[..len]
                );
            }
        }
        assert_eq!(
            jump_keyword("a").map(|(jump, _)| jump),
            Some(Jump::Pane(PaneId::OrdersAlerts))
        );
        assert_eq!(
            jump_keyword("b").map(|(jump, _)| jump),
            Some(Jump::Pane(PaneId::InvestBids))
        );
    }

    #[test]
    fn workspace_names_are_reachable_both_ways() {
        for (word, preset) in [
            ("trading", Preset::Trading),
            ("portfolio", Preset::Portfolio),
            ("derivatives", Preset::Derivatives),
            ("overview", Preset::Overview),
            ("invest", Preset::Invest),
        ] {
            assert_eq!(
                analyze(word),
                Draft::Jump {
                    jump: Jump::Workspace(preset),
                    word
                },
                "bare {word} must be a destination"
            );
            let query = &word[..3];
            let Draft::Workspace { arg, query: typed } = analyze(&format!("workspace {query}"))
            else {
                panic!("workspace {query} must be a workspace draft");
            };
            assert_eq!(typed, query);
            assert!(workspace_matches(&typed).any(|found| found == preset));
            assert_eq!(
                complete(
                    &format!("workspace {query}"),
                    CompletionTarget::WorkspaceName {
                        arg: &arg,
                        name: preset.title(),
                    },
                )
                .apply(&format!("workspace {query}")),
                format!("workspace {}", preset.title())
            );
        }
        let Draft::Workspace { query, .. } = analyze("workspace") else {
            panic!("bare workspace must be a workspace draft");
        };
        assert_eq!(query.len(), 0);
        assert_eq!(workspace_matches(&query).count(), 6);
        assert_eq!(
            workspace_matches("ord").collect::<Vec<_>>(),
            [Preset::Orders]
        );
        assert_eq!(workspace_matches("zzz").count(), 0);
    }

    #[test]
    fn browse_and_search_drafts() {
        assert_eq!(analyze(""), Draft::Browse);
        assert_eq!(analyze("   "), Draft::Browse);
        assert_eq!(analyze("reliance industries"), Draft::Search);
        assert_eq!(analyze("xyz"), Draft::Search);
        assert_eq!(analyze("bu"), Draft::Partial);
        assert_eq!(analyze("the"), Draft::Partial);
        assert_eq!(analyze("qu"), Draft::Partial);
    }

    #[test]
    fn partial_first_keyword_completes_with_one_trailing_space() {
        assert_eq!(complete_keyword("bu", "buy"), "buy ");
        assert_eq!(complete_keyword("the", "theme"), "theme ");
        assert_eq!(complete_keyword("w", "wl"), "wl ");
        assert_eq!(complete_keyword("ord", "orders"), "orders");
        assert_eq!(complete_keyword("q", "quit"), "quit");
        assert_eq!(complete_keyword("exit", "quit"), "quit");
        assert_eq!(complete_keyword("au", "auction"), "auction");
    }

    #[test]
    fn keyword_candidates_are_offered_in_catalog_order() {
        let words = |token: &str| -> Vec<&'static str> {
            keyword_matches(token).map(|kw| kw.word).collect()
        };
        assert_eq!(words("a"), vec!["alert", "alerts", "auction"]);
        assert_eq!(words("b"), vec!["buy", "bids"]);
        assert_eq!(words("W"), vec!["wl", "watch", "workspace"]);
        assert_eq!(words("s"), vec!["sell"]);
        assert_eq!(words("t"), vec!["theme", "trading"]);
        assert_eq!(words("p"), vec!["positions", "portfolio"]);
        assert_eq!(words("d"), vec!["derivatives"]);
        assert_eq!(words("o"), vec!["orders", "overview"]);
        assert_eq!(words(""), Vec::<&str>::new());
        assert_eq!(words("exit"), Vec::<&str>::new());
    }

    #[test]
    fn a_digit_and_a_lone_side_word_are_not_view_drafts() {
        for token in ["1", "2", "5", "9"] {
            assert_eq!(analyze(token), Draft::Search, ":{token} is not a view");
        }
        assert_eq!(
            analyze("b"),
            Draft::Jump {
                jump: Jump::Pane(PaneId::InvestBids),
                word: "bids"
            }
        );
        assert_eq!(analyze("buy"), Draft::Partial);
        assert_eq!(analyze("sell"), Draft::Partial);
        assert_eq!(
            analyze("a"),
            Draft::Jump {
                jump: Jump::Pane(PaneId::OrdersAlerts),
                word: "alerts"
            }
        );
        assert_eq!(
            analyze("au"),
            Draft::Jump {
                jump: Jump::Pane(PaneId::InvestBids),
                word: "auction"
            }
        );
    }

    #[test]
    fn only_the_exact_singular_alert_is_the_alert_command() {
        assert_eq!(
            analyze("alert"),
            Draft::Alert {
                arg: 5..5,
                query: String::new()
            }
        );
        assert_eq!(
            analyze("ALERT nse:nifty 50"),
            Draft::Alert {
                arg: 5..18,
                query: "nse:nifty 50".to_string()
            },
            "the argument runs to the end of the input, spaces included"
        );
        assert_eq!(
            analyze("  alert inf "),
            Draft::Alert {
                arg: 7..12,
                query: "inf".to_string()
            }
        );
        for token in ["a", "al", "ale", "aler", "alerts"] {
            assert_eq!(
                analyze(token),
                Draft::Jump {
                    jump: Jump::Pane(PaneId::OrdersAlerts),
                    word: "alerts"
                },
                "{token:?} must stay the Alerts view jump"
            );
        }
    }

    #[test]
    fn completing_an_alert_instrument_replaces_the_whole_argument() {
        let complete_alert = |input: &str, arg: Range<usize>| {
            complete(
                input,
                CompletionTarget::AlertInstrument {
                    arg: &arg,
                    identity: "NSE:INFY",
                },
            )
            .apply(input)
        };
        assert_eq!(complete_alert("alert", 5..5), "alert NSE:INFY");
        assert_eq!(complete_alert("alert inf", 5..9), "alert NSE:INFY");
        assert!(!complete_alert("alert inf", 5..9).ends_with(' '));
    }

    #[test]
    fn quit_aliases_all_draft_as_quit() {
        for input in ["q", "quit", "exit", "QUIT", " exit "] {
            assert_eq!(analyze(input), Draft::Quit, "{input:?} is a quit draft");
        }
    }

    #[test]
    fn a_trailing_space_after_a_side_word_opens_order_context() {
        let draft = order_draft("b ");
        assert_eq!(draft.side, Side::Buy);
        assert_eq!(draft.error, Some(OrderError::MissingSymbol));
        assert_eq!(
            draft.symbol,
            2..2,
            "a completion knows where a symbol lands"
        );
        assert_eq!(order_draft("sell ").side, Side::Sell);
        assert_eq!(order_draft("s ").side, Side::Sell);
        assert_eq!(complete_keyword("b ", "buy"), "buy ");
        assert_eq!(complete_keyword("s ", "sell"), "sell ");
    }

    #[test]
    fn order_draft_carries_the_symbol_range_and_arguments() {
        let draft = order_draft("buy inf");
        assert_eq!(&"buy inf"[draft.symbol.clone()], "inf");
        assert_eq!(draft.qty, None);
        assert_eq!(draft.price, None);
        assert_eq!(draft.error, None);

        let input = "SELL nse:inf 10 @ 1520";
        let draft = order_draft(input);
        assert_eq!(&input[draft.symbol.clone()], "nse:inf");
        assert_eq!(draft.side, Side::Sell);
        assert_eq!(draft.qty, Some(10));
        assert_eq!(draft.price, Some(1520.0));
        assert_eq!(draft.error, None);
    }

    #[test]
    fn symbol_completion_preserves_the_side_quantity_and_price() {
        let identity = "NSE:INFY";
        let replace = |input: &str| {
            let draft = order_draft(input);
            complete(
                input,
                CompletionTarget::OrderSymbol {
                    symbol: &draft.symbol,
                    identity,
                },
            )
            .apply(input)
        };
        assert_eq!(replace("buy inf"), "buy NSE:INFY ");
        assert_eq!(replace("b inf"), "b NSE:INFY ");
        assert_eq!(replace("SELL nse:inf 10 @ 1520"), "SELL NSE:INFY 10 @ 1520");
        assert_eq!(replace("buy inf 5"), "buy NSE:INFY 5");
        assert_eq!(replace("buy inf@1520"), "buy NSE:INFY@1520");
        assert_eq!(replace("buy inf "), "buy NSE:INFY ");
    }

    #[test]
    fn invalid_order_arguments_become_draft_errors() {
        let cases = [
            ("buy infy 0", OrderError::Quantity),
            ("buy infy -1", OrderError::Quantity),
            ("buy infy 10.5", OrderError::Quantity),
            ("buy infy 4294967296", OrderError::Quantity),
            ("buy infy 10 20", OrderError::ExtraTokens),
            ("buy infy 10 @1520 xyz", OrderError::Price),
            ("buy infy @", OrderError::Price),
            ("buy infy @-5", OrderError::Price),
            ("buy infy @0", OrderError::Price),
            ("buy infy @nan", OrderError::Price),
            ("buy infy @inf", OrderError::Price),
            ("buy ", OrderError::MissingSymbol),
            ("buy infy@10 5", OrderError::Price),
        ];
        for (input, expected) in cases {
            assert_eq!(
                order_draft(input).error,
                Some(expected),
                "{input:?} is a recognised order with an invalid argument"
            );
        }
        for (input, _) in cases {
            assert!(
                !matches!(analyze(input), Draft::Search | Draft::Partial),
                "{input:?} must not degrade to a search"
            );
        }
    }

    #[test]
    fn scientific_price_notation_stays_accepted() {
        assert_eq!(order_draft("buy infy @1.52e3").price, Some(1520.0));
        assert_eq!(order_draft("buy infy @1.52e3").error, None);
        assert_eq!(order_draft("buy infy 1 @ 5e-1").price, Some(0.5));
    }

    #[test]
    fn theme_draft_and_preset_completion() {
        let theme = |input: &str| match analyze(input) {
            Draft::Theme { arg, query } => (arg, query),
            other => panic!("expected a theme draft for {input:?}, got {other:?}"),
        };
        assert_eq!(theme("theme").1, "");
        assert_eq!(theme("theme dra").1, "dra");
        assert_eq!(theme("THEME solarized_").1, "solarized_");

        let insert = |input: &str, preset: &str| {
            let (arg, _) = theme(input);
            complete(input, CompletionTarget::ThemePreset { arg: &arg, preset }).apply(input)
        };
        assert_eq!(insert("theme", "dracula"), "theme dracula");
        assert_eq!(insert("theme dra", "dracula"), "theme dracula");
        assert_eq!(insert("theme  dra  ", "dracula"), "theme dracula");
        assert_eq!(
            insert("THEME sol", "solarized-light"),
            "THEME solarized-light"
        );
    }

    #[test]
    fn watchlist_drafts_reserve_the_subcommand_words() {
        assert_eq!(wl_draft("wl"), WatchlistDraft::List);
        assert_eq!(wl_draft("wl "), WatchlistDraft::List);
        assert_eq!(wl_draft("WL   "), WatchlistDraft::List);
        assert_eq!(
            wl_draft("wl rename"),
            WatchlistDraft::RenameMissing { needs_space: true }
        );
        assert_eq!(
            wl_draft("wl rename "),
            WatchlistDraft::RenameMissing { needs_space: false }
        );
        assert_eq!(
            wl_draft("wl rename core"),
            WatchlistDraft::Rename {
                name: "core".to_string()
            }
        );
        assert_eq!(wl_draft("wl delete"), WatchlistDraft::Delete);
        assert_eq!(wl_draft("wl delete extra"), WatchlistDraft::DeleteExtra);
        assert_eq!(
            wl_draft("wl My List 2"),
            WatchlistDraft::Name {
                range: 3..12,
                name: "My List 2".to_string()
            }
        );
        assert_eq!(
            wl_draft("wl ren"),
            WatchlistDraft::Name {
                range: 3..6,
                name: "ren".to_string()
            }
        );
    }

    #[test]
    fn watchlist_completions_replace_the_name_or_the_whole_form() {
        let name_edit = |input: &str, name: &str| match wl_draft(input) {
            WatchlistDraft::Name { range, .. } => complete(
                input,
                CompletionTarget::WatchlistName {
                    range: &range,
                    name,
                },
            )
            .apply(input),
            other => panic!("expected a name draft, got {other:?}"),
        };
        assert_eq!(name_edit("wl my", "MyList"), "wl MyList");
        assert_eq!(name_edit("wl my li", "My List"), "wl My List");
        assert_eq!(name_edit("wl my  ", "MyList"), "wl MyList");

        let sub = |input: &str, form: &'static str| {
            complete(input, CompletionTarget::WatchlistForm(form)).apply(input)
        };
        assert_eq!(sub("wl", "wl rename "), "wl rename ");
        assert_eq!(sub("wl re", "wl rename "), "wl rename ");
        assert_eq!(sub("wl", "wl delete"), "wl delete");

        let input = "wl rename";
        assert_eq!(
            complete(input, CompletionTarget::RenameSpace).apply(input),
            "wl rename "
        );
    }

    #[test]
    fn free_text_completions_replace_the_whole_query() {
        let input = "NSE:inf";
        assert_eq!(
            complete(input, CompletionTarget::Instrument("NSE:INFY")).apply(input),
            "NSE:INFY"
        );
        let long = "midcp";
        assert_eq!(
            complete(
                long,
                CompletionTarget::Instrument("NFO:MIDCPNIFTY26AUG13400CE")
            )
            .apply(long),
            "NFO:MIDCPNIFTY26AUG13400CE"
        );
        let menu = "over";
        assert_eq!(
            complete(menu, CompletionTarget::MenuLabel("Overview")).apply(menu),
            "Overview"
        );
    }

    #[test]
    fn a_noop_edit_is_recognised_so_tab_can_be_omitted() {
        let input = "wl rename core";
        let edit = complete(input, CompletionTarget::MenuLabel("wl rename core"));
        assert!(edit.is_noop(input));
        assert!(!complete(input, CompletionTarget::MenuLabel("other")).is_noop(input));
    }

    #[test]
    fn completion_edits_land_on_utf8_boundaries() {
        let input = "buy NIFTY–50 10";
        let draft = order_draft(input);
        assert_eq!(&input[draft.symbol.clone()], "NIFTY–50");
        let completed = complete(
            input,
            CompletionTarget::OrderSymbol {
                symbol: &draft.symbol,
                identity: "NSE:NIFTY50",
            },
        )
        .apply(input);
        assert_eq!(completed, "buy NSE:NIFTY50 10");
    }

    #[test]
    fn argument_kinds_match_the_grammar() {
        assert_eq!(keyword("buy").arg, ArgKind::Required);
        assert_eq!(keyword("sell").arg, ArgKind::Required);
        assert_eq!(keyword("theme").arg, ArgKind::Optional);
        assert_eq!(keyword("wl").arg, ArgKind::Optional);
        assert_eq!(keyword("quit").arg, ArgKind::None);
        assert_eq!(keyword("workspace").arg, ArgKind::Required);
        for kw in CATALOG.into_iter().filter(|kw| kw.jump.is_some()) {
            assert_eq!(kw.arg, ArgKind::None, "{} takes no argument", kw.word);
        }
    }
}
