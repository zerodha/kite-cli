use clap::builder::TypedValueParser;
use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const EXCHANGES: [&str; 8] = ["NSE", "BSE", "NFO", "BFO", "CDS", "BCD", "MCX", "NCO"];
const PRODUCTS: [&str; 6] = ["CNC", "NRML", "MIS", "MTF", "CO", "BO"];
const VARIETIES: [&str; 5] = ["regular", "amo", "co", "iceberg", "auction"];
const ORDER_TYPES: [&str; 4] = ["MARKET", "LIMIT", "SL", "SL-M"];
const VALIDITIES: [&str; 3] = ["DAY", "IOC", "TTL"];
const TRANSACTION_TYPES: [&str; 2] = ["BUY", "SELL"];

#[derive(Parser, Debug)]
#[command(
    name = "kite",
    version,
    about = "Trading CLI and terminal UI for Zerodha Kite",
    long_about = "A composable CLI for Zerodha Kite: auth, orders, portfolio, market data, instruments, historical candles, margins, GTTs, alerts, mutual funds, WebSocket ticks, and raw API requests. `kite auth login` signs in with your user ID, password and TOTP or SMS OTP, and saves the access token it returns. KITE_API_KEY and KITE_ACCESS_TOKEN override the saved session. Use --json for stable machine-readable output.",
    after_long_help = "Quick start:\n  kite doctor\n  kite auth login\n  kite user profile\n\nCommon reads:\n  kite market ltp NSE:INFY NSE:RELIANCE\n  kite market resolve NSE:INFY\n  kite portfolio holdings\n  kite orders list\n\nSafety:\n  Order, GTT, alert, portfolio-convert, and raw write commands dry-run by default.\n  Add --confirm only when you intend to send a live write to Kite."
)]
pub struct Cli {
    /// Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted.
    #[arg(long, global = true, help_heading = "Global options")]
    pub json: bool,

    #[expect(
        clippy::doc_markdown,
        reason = "clap uses this doc verbatim as existing CLI help"
    )]
    /// Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`).
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help_heading = "Global options"
    )]
    pub config: Option<PathBuf>,

    /// Subcommand to run. When omitted, `kite` launches the smart default:
    /// login (if needed) then the TUI.
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Verify install, config, auth sources, and read-only API reachability.
    #[command(
        after_long_help = "Examples:\n  kite doctor\n  kite doctor --no-live\n\n`doctor` never prints full credentials. Without auth it reports setup steps instead of failing early."
    )]
    Doctor {
        /// Skip the live /user/margins auth check even if a session is present.
        #[arg(long)]
        no_live: bool,
    },
    /// Manage the kite config file (see `kite config path`).
    #[command(
        after_long_help = "Examples:\n  kite config path\n  kite config show\n  kite config show --reveal"
    )]
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Interactive token login and logout.
    #[command(
        after_long_help = "Examples:\n  kite auth login\n  kite auth login AB1234\n  kite auth logout\n\n`kite auth login` signs in with a user ID, password, and verification code. Saves the access token and user ID to config."
    )]
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
    /// User profile and available funds/margins.
    User {
        #[command(subcommand)]
        command: UserCommands,
    },
    /// Order book, trades, order history, and safe order writes.
    #[command(alias = "order")]
    Orders {
        #[command(subcommand)]
        command: OrdersCommands,
    },
    /// Long-term holdings, intraday/overnight positions, auctions, and conversion.
    #[command(alias = "pf")]
    Portfolio {
        #[command(subcommand)]
        command: PortfolioCommands,
    },
    /// Instruments, quotes, search/resolve helpers, and market data snapshots.
    #[command(alias = "m")]
    Market {
        #[command(subcommand)]
        command: MarketCommands,
    },
    /// Backwards-compatible quote commands. Prefer `kite market ...`.
    Quotes {
        #[command(subcommand)]
        command: LegacyQuotesCommands,
    },
    /// Historical candle data.
    Historical {
        #[command(subcommand)]
        command: HistoricalCommands,
    },
    /// Order margin, basket margin, and charges calculators.
    Margins {
        #[command(subcommand)]
        command: MarginsCommands,
    },
    /// Good Till Triggered (GTT) triggers.
    #[command(alias = "gtts")]
    Gtt {
        #[command(subcommand)]
        command: GttCommands,
    },
    /// Server-side price alerts. ATO basket creation and editing stay in the TUI.
    #[command(
        after_long_help = "Examples:\n  kite alerts list --status enabled\n  kite alerts get UUID\n  kite alerts history UUID\n  kite alerts create --name 'INFY target' --lhs NSE:INFY --operator '>=' --value 1500\n  kite alerts create --name 'Index comparison' --lhs 'NSE:NIFTY 50' --operator '>' --rhs 'NSE:NIFTY NEXT 50'\n  kite alerts modify UUID --value 1600\n  kite alerts disable UUID\n  kite alerts enable UUID --confirm\n  kite alerts delete UUID --confirm\n\nWrites dry-run by default. Add --confirm to send a live write. Modify, enable and disable read the stored alert before showing the exact request. Use the TUI to create or edit ATO basket alerts."
    )]
    Alerts {
        #[command(subcommand)]
        command: AlertsCommands,
    },
    /// Mutual fund orders, SIPs, holdings, and instrument master.
    #[command(alias = "mutual-funds")]
    Mf {
        #[command(subcommand)]
        command: MfCommands,
    },
    /// WebSocket live tick stream helpers.
    #[command(alias = "ws")]
    Stream {
        #[command(subcommand)]
        command: StreamCommands,
    },
    /// Interactive terminal trading UI: watchlist, positions, orders, holdings.
    Tui(TuiArgs),
    /// Authenticated raw Kite API escape hatch.
    #[command(alias = "api")]
    Request {
        #[command(subcommand)]
        command: RequestCommands,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommands {
    /// Print the config path.
    Path,
    /// Show the saved session, redacted by default.
    Show {
        /// Reveal secrets. Avoid in shared shells/logs.
        #[arg(long)]
        reveal: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCommands {
    /// Sign in with a user ID, password, and verification code.
    Login {
        #[expect(
            clippy::doc_markdown,
            reason = "clap uses this doc verbatim as existing CLI help"
        )]
        /// Kite user_id (Zerodha client ID). Prompted if omitted.
        #[arg(value_name = "USER_ID")]
        user_id: Option<String>,
    },
    /// Revoke and clear the saved session.
    Logout {
        #[arg(long, hide = true)]
        confirm: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum UserCommands {
    /// Retrieve the logged-in user's profile.
    Profile,
    /// Retrieve funds and margins for all segments or one segment.
    Margins {
        /// equity or commodity.
        #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(["equity", "commodity"]).map(|value| value.to_ascii_lowercase()))]
        segment: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum OrdersCommands {
    /// Retrieve today's order book.
    #[command(alias = "ls")]
    List {
        /// Filter by status, ignoring case. OPEN includes working and trigger-pending orders.
        #[arg(long)]
        status: Option<String>,
        /// Client-side filter by tradingsymbol.
        #[arg(long)]
        tradingsymbol: Option<String>,
    },
    #[expect(
        clippy::doc_markdown,
        reason = "clap uses this doc verbatim as existing CLI help"
    )]
    /// Retrieve history for one order_id.
    #[command(alias = "history")]
    Get {
        /// Kite order ID.
        order_id: String,
    },
    #[expect(
        clippy::doc_markdown,
        reason = "clap uses this doc verbatim as existing CLI help"
    )]
    /// Retrieve all trades for the day or trades for one order_id.
    Trades {
        /// Limit trades to this order ID.
        #[arg(long)]
        order_id: Option<String>,
    },
    /// Place an order. Dry-runs unless --confirm is provided.
    #[command(
        after_long_help = "Examples:\n  kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY --order-type MARKET --quantity 1 --product CNC --validity DAY\n  kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY --order-type LIMIT --quantity 1 --product CNC --price 1500 --validity DAY\n\nThe command prints a dry-run request by default. Add --confirm only to place a live order."
    )]
    Place(OrderPlaceArgs),
    /// Modify an open/pending order. Dry-runs unless --confirm is provided.
    #[command(
        after_long_help = "Example:\n  kite orders modify --order-id 151220000000000 --quantity 2 --price 1500\n\nThe command prints a dry-run request by default. Add --confirm only to modify a live order."
    )]
    Modify(OrderModifyArgs),
    /// Cancel an open/pending order. Dry-runs unless --confirm is provided.
    #[command(
        after_long_help = "Example:\n  kite orders cancel --order-id 151220000000000\n\nThe command prints a dry-run request by default. Add --confirm only to cancel a live order."
    )]
    Cancel(OrderCancelArgs),
}

#[derive(Subcommand, Debug)]
pub enum PortfolioCommands {
    /// Long-term equity holdings.
    #[command(alias = "h")]
    Holdings,
    /// Short-term positions, returned as net/day groups.
    #[command(alias = "pos")]
    Positions,
    /// Holdings available in current auctions.
    Auctions,
    /// Convert the margin product of an open position. Dry-runs unless --confirm is provided.
    #[command(
        after_long_help = "Example:\n  kite portfolio convert --exchange NSE --tradingsymbol INFY --transaction-type BUY --position-type day --quantity 1 --old-product MIS --new-product CNC\n\nThe command prints a dry-run request by default. Add --confirm only to send the live conversion."
    )]
    Convert(PositionConvertArgs),
}

#[derive(Subcommand, Debug)]
pub enum MarketCommands {
    /// Show instruments, or download JSON to --out.
    #[command(alias = "instrument")]
    Instruments(InstrumentDumpArgs),
    /// Inspect and manage the local instrument cache.
    Cache {
        #[command(subcommand)]
        command: MarketCacheCommands,
    },
    /// Search the instrument master by symbol or name.
    Search(InstrumentSearchArgs),
    /// Resolve a symbol into stable instrument metadata/token(s).
    Resolve(InstrumentResolveArgs),
    /// Full market quotes; limit: 500 instruments.
    #[command(alias = "full")]
    Quote(QuoteArgs),
    /// OHLC + LTP quotes; limit: 1000 instruments.
    Ohlc(QuoteArgs),
    /// LTP quotes; limit: 1000 instruments.
    Ltp(QuoteArgs),
}

#[derive(Subcommand, Debug)]
pub enum MarketCacheCommands {
    /// Print the local instrument cache path.
    Path,
    /// Show cache metadata and freshness.
    Info,
    /// Refresh the local instrument cache from instruments.json.
    Refresh,
    /// Delete the local instrument cache file.
    Clear,
}

#[derive(Subcommand, Debug)]
pub enum LegacyQuotesCommands {
    /// LTP quotes. Prefer `kite market ltp`.
    Ltp(QuoteArgs),
    /// OHLC quotes. Prefer `kite market ohlc`.
    Ohlc(QuoteArgs),
    /// Full quotes. Prefer `kite market quote`.
    Full(QuoteArgs),
}

#[derive(Subcommand, Debug)]
pub enum HistoricalCommands {
    /// Retrieve historical candle records for an instrument token.
    Candles(HistoricalArgs),
}

#[derive(Subcommand, Debug)]
pub enum MarginsCommands {
    /// Calculate margins for each order in a JSON array.
    Orders(MarginOrdersArgs),
    /// Calculate basket margins for spread orders.
    Basket(MarginBasketArgs),
    /// Calculate virtual contract note / order charges.
    Charges(MarginChargesArgs),
}

#[derive(Subcommand, Debug)]
pub enum GttCommands {
    /// List all visible GTT triggers.
    #[command(alias = "ls")]
    List,
    /// Retrieve one GTT trigger.
    Get {
        /// GTT trigger ID.
        trigger_id: i64,
    },
    /// Place a GTT trigger. Dry-runs unless --confirm is provided.
    #[command(
        after_long_help = "Example:\n  kite gtt place --type single --condition-json '{\"exchange\":\"NSE\",\"tradingsymbol\":\"INFY\",\"trigger_values\":[702.0],\"last_price\":798.0}' --orders-json '[{\"exchange\":\"NSE\",\"tradingsymbol\":\"INFY\",\"transaction_type\":\"BUY\",\"quantity\":1,\"order_type\":\"LIMIT\",\"product\":\"CNC\",\"price\":702.5}]'\n\nThe command prints a dry-run request by default. Add --confirm only to place the live GTT."
    )]
    Place(GttWriteArgs),
    /// Modify a GTT trigger. Dry-runs unless --confirm is provided.
    Modify(GttModifyArgs),
    /// Delete a GTT trigger. Dry-runs unless --confirm is provided.
    Delete(GttDeleteArgs),
}

#[derive(Subcommand, Debug)]
pub enum AlertsCommands {
    /// List alerts, optionally filtered by status.
    List {
        #[arg(long, value_parser = ["enabled", "disabled", "deleted"])]
        status: Option<String>,
    },
    /// Retrieve one alert.
    Get { uuid: String },
    /// Retrieve an alert's trigger history.
    History { uuid: String },
    /// Create a simple alert. Dry-runs unless --confirm is provided.
    #[command(
        mut_arg("name", |arg| arg.required(true)),
        mut_arg("lhs", |arg| arg.required(true)),
        mut_arg("operator", |arg| arg.required(true)),
        mut_arg("rhs_attribute", |arg| arg.requires("rhs")),
        group(clap::ArgGroup::new("rhs_condition").args(["value", "rhs"]).required(true)),
        after_long_help = "Example:\n  kite alerts create --name 'INFY target' --lhs NSE:INFY --operator '>=' --value 1500\n\nThe attribute defaults to LastTradedPrice. ATO basket alerts can only be created in the TUI. Add --confirm to create the live alert."
    )]
    Create(AlertWriteArgs),
    /// Modify a simple alert, preserving omitted fields and status. Dry-runs by default.
    #[command(
        group = clap::ArgGroup::new("changes").required(true).multiple(true).args(["name", "lhs", "attribute", "operator", "value", "rhs", "rhs_attribute"]),
        after_long_help = "Example:\n  kite alerts modify UUID --value 1600\n\nReads the stored alert to build the full condition. Use the TUI to edit ATO basket alerts. Add --confirm to modify the live alert."
    )]
    Modify {
        uuid: String,
        #[command(flatten)]
        write: AlertWriteArgs,
    },
    /// Delete one or more alerts. Dry-runs unless --confirm is provided.
    Delete {
        #[arg(required = true, num_args = 1..)]
        uuid: Vec<String>,
        #[command(flatten)]
        confirm: ConfirmArgs,
    },
    /// Enable an alert, preserving its type and basket. Dry-runs by default.
    Enable {
        uuid: String,
        #[command(flatten)]
        confirm: ConfirmArgs,
    },
    /// Disable an alert, preserving its type and basket. Dry-runs by default.
    Disable {
        uuid: String,
        #[command(flatten)]
        confirm: ConfirmArgs,
    },
}

#[derive(Args, Debug)]
pub struct AlertWriteArgs {
    /// Alert name.
    #[arg(long)]
    pub name: Option<String>,
    /// Left-hand instrument in EXCHANGE:SYMBOL form.
    #[arg(long, value_name = "EXCHANGE:SYMBOL")]
    pub lhs: Option<String>,
    /// Left-hand attribute. Defaults to `LastTradedPrice` on create.
    #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(crate::alert_attributes::Attribute::ALL.map(crate::alert_attributes::Attribute::wire)))]
    pub attribute: Option<String>,
    /// Comparison operator. Quote it to prevent shell redirection.
    #[arg(long, value_parser = ["<=", ">=", "<", ">", "=="])]
    pub operator: Option<String>,
    /// Right-hand constant, instead of --rhs.
    #[arg(long, conflicts_with = "rhs", value_parser = parse_alert_value, allow_hyphen_values = true)]
    pub value: Option<f64>,
    /// Right-hand instrument in EXCHANGE:SYMBOL form, instead of --value.
    #[arg(long, conflicts_with = "value", value_name = "EXCHANGE:SYMBOL")]
    pub rhs: Option<String>,
    /// Right-hand attribute. Defaults to `LastTradedPrice` for a new instrument condition.
    #[arg(long, conflicts_with = "value", value_parser = clap::builder::PossibleValuesParser::new(crate::alert_attributes::Attribute::ALL.map(crate::alert_attributes::Attribute::wire)))]
    pub rhs_attribute: Option<String>,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

fn parse_order_price(raw: &str) -> Result<f64, String> {
    raw.parse::<f64>()
        .ok()
        .filter(|price| price.is_finite() && *price >= 0.0)
        .ok_or_else(|| "price must be a finite, non-negative number".to_string())
}

fn parse_order_tag(raw: &str) -> Result<String, String> {
    if raw.is_empty() || raw.len() > 20 || !raw.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("order tag must contain 1 to 20 ASCII letters or digits".to_string());
    }
    Ok(raw.to_string())
}

fn parse_quote_instrument(raw: &str) -> Result<String, String> {
    if !raw
        .split_once(':')
        .is_some_and(|(exchange, symbol)| !exchange.trim().is_empty() && !symbol.trim().is_empty())
    {
        return Err("instrument must use EXCHANGE:SYMBOL form, for example NSE:INFY".to_string());
    }
    Ok(raw.to_string())
}

fn parse_alert_value(raw: &str) -> Result<f64, String> {
    raw.parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
        .ok_or_else(|| "alert value must be a finite number".into())
}

#[derive(Subcommand, Debug)]
pub enum MfCommands {
    /// Mutual fund orders from the last 7 days.
    #[command(alias = "orderbook")]
    Orders,
    /// Retrieve one mutual fund order by id.
    Order {
        /// Mutual fund order ID.
        order_id: String,
    },
    /// Active and paused SIPs.
    Sips,
    /// Mutual fund holdings in DEMAT.
    #[command(alias = "h")]
    Holdings,
    /// Download mutual fund instrument CSV, write to --out, or emit bounded JSON with --json.
    Instruments(MfInstrumentDumpArgs),
    /// Search mutual fund instruments by fund name or ISIN.
    Search(MfInstrumentSearchArgs),
}

#[derive(Subcommand, Debug)]
pub enum StreamCommands {
    /// Display live WebSocket ticks; use --json for JSON lines/arrays.
    Ticks(StreamTicksArgs),
    /// Terminal UI for live ticks.
    Tui(TuiArgs),
}

#[derive(Subcommand, Debug)]
pub enum RequestCommands {
    /// Authenticated raw GET. Keeps the full API response shape.
    Get(RawGetArgs),
    /// Authenticated raw POST. Requires --confirm for live request.
    Post(RawWriteArgs),
    /// Authenticated raw PUT. Requires --confirm for live request.
    Put(RawWriteArgs),
    /// Authenticated raw DELETE. Requires --confirm for live request.
    Delete(RawDeleteArgs),
}

#[derive(Args, Debug, Clone)]
pub struct ConfirmArgs {
    /// Show the request without sending it.
    #[arg(long)]
    pub dry_run: bool,
    /// Required to execute a live write/delete.
    #[arg(long)]
    pub confirm: bool,
}

impl ConfirmArgs {
    #[must_use]
    pub fn is_dry_run(&self) -> bool {
        !self.confirm || self.dry_run
    }
}

#[derive(Args, Debug)]
pub struct OrderPlaceArgs {
    /// Order variety.
    #[arg(long, default_value = "regular", ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(VARIETIES).map(|value| value.to_ascii_lowercase()))]
    pub variety: String,
    /// Exchange to send the order to.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(EXCHANGES).map(|value| value.to_ascii_uppercase()))]
    pub exchange: String,
    /// Exchange trading symbol, for example INFY, RELIANCE, NIFTY24JANFUT.
    #[arg(long)]
    pub tradingsymbol: String,
    /// BUY or SELL.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(TRANSACTION_TYPES).map(|value| value.to_ascii_uppercase()))]
    pub transaction_type: String,
    /// MARKET, LIMIT, SL, SL-M.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(ORDER_TYPES).map(|value| value.to_ascii_uppercase()))]
    pub order_type: String,
    /// Number of shares/contracts/units to transact.
    #[arg(long)]
    pub quantity: i64,
    /// CNC, NRML, MIS, MTF, etc.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(PRODUCTS).map(|value| value.to_ascii_uppercase()))]
    pub product: String,
    /// Limit price for LIMIT/SL orders. Omit or use 0 for MARKET orders.
    #[arg(long, value_parser = parse_order_price)]
    pub price: Option<f64>,
    /// Trigger price for SL/SL-M orders.
    #[arg(long, value_parser = parse_order_price)]
    pub trigger_price: Option<f64>,
    /// Quantity disclosed to the market for equity orders.
    #[arg(long)]
    pub disclosed_quantity: Option<i64>,
    /// DAY, IOC, TTL.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(VALIDITIES).map(|value| value.to_ascii_uppercase()))]
    pub validity: Option<String>,
    /// TTL validity in minutes. Used only when --validity TTL.
    #[arg(long)]
    pub validity_ttl: Option<i64>,
    /// Number of iceberg legs, between 2 and 50.
    #[arg(long, value_parser = clap::value_parser!(i64).range(2..=50))]
    pub iceberg_legs: Option<i64>,
    /// Quantity per iceberg leg.
    #[arg(long)]
    pub iceberg_quantity: Option<i64>,
    /// Auction number for auction variety orders.
    #[arg(long)]
    pub auction_number: Option<String>,
    /// -1 for auto protection, or 0-100 percent.
    #[arg(long, value_parser = clap::value_parser!(i64).range(-1..=100), allow_hyphen_values = true)]
    pub market_protection: Option<i64>,
    /// Enable autoslice for freeze-quantity orders.
    #[arg(long)]
    pub autoslice: bool,
    /// Optional alphanumeric order tag, max 20 characters.
    #[arg(long, value_parser = parse_order_tag)]
    pub tag: Option<String>,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Args, Debug)]
pub struct OrderModifyArgs {
    /// Order variety.
    #[arg(long, default_value = "regular", ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(VARIETIES).map(|value| value.to_ascii_lowercase()))]
    pub variety: String,
    #[expect(
        clippy::doc_markdown,
        reason = "clap uses this doc verbatim as existing CLI help"
    )]
    /// Kite order_id to modify.
    #[arg(long)]
    pub order_id: String,
    /// New order type.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(ORDER_TYPES).map(|value| value.to_ascii_uppercase()))]
    pub order_type: Option<String>,
    /// New quantity.
    #[arg(long)]
    pub quantity: Option<i64>,
    /// New limit price.
    #[arg(long, value_parser = parse_order_price)]
    pub price: Option<f64>,
    /// New trigger price.
    #[arg(long, value_parser = parse_order_price)]
    pub trigger_price: Option<f64>,
    /// New disclosed quantity.
    #[arg(long)]
    pub disclosed_quantity: Option<i64>,
    /// New order validity.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(VALIDITIES).map(|value| value.to_ascii_uppercase()))]
    pub validity: Option<String>,
    /// New TTL validity in minutes. Used only when --validity TTL.
    #[arg(long)]
    pub validity_ttl: Option<i64>,
    /// -1 for automatic protection, or 0 to 100 percent.
    #[arg(long, value_parser = clap::value_parser!(i64).range(-1..=100), allow_hyphen_values = true)]
    pub market_protection: Option<i64>,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Args, Debug)]
pub struct OrderCancelArgs {
    /// Order variety.
    #[arg(long, default_value = "regular", ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(VARIETIES).map(|value| value.to_ascii_lowercase()))]
    pub variety: String,
    #[expect(
        clippy::doc_markdown,
        reason = "clap uses this doc verbatim as existing CLI help"
    )]
    /// Kite order_id to cancel.
    #[arg(long)]
    pub order_id: String,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Args, Debug)]
pub struct PositionConvertArgs {
    /// Exchange to send the order to.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(EXCHANGES).map(|value| value.to_ascii_uppercase()))]
    pub exchange: String,
    /// Exchange trading symbol, for example INFY.
    #[arg(long)]
    pub tradingsymbol: String,
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(TRANSACTION_TYPES).map(|value| value.to_ascii_uppercase()))]
    pub transaction_type: String,
    /// day or overnight.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(["day", "overnight"]).map(|value| value.to_ascii_lowercase()))]
    pub position_type: String,
    /// Quantity to convert.
    #[arg(long)]
    pub quantity: i64,
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(PRODUCTS).map(|value| value.to_ascii_uppercase()))]
    pub old_product: String,
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(PRODUCTS).map(|value| value.to_ascii_uppercase()))]
    pub new_product: String,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Args, Debug)]
pub struct InstrumentDumpArgs {
    /// Write all parsed instrument records as JSON to this file. Ignores --limit.
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
    /// Limit displayed records. Use 0 for all.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct InstrumentSearchArgs {
    /// Search text matched against tradingsymbol, name, and aliases.
    /// Prefix with `EXCHANGE:` (for example `NSE:infy`) to filter by exchange.
    pub query: String,
    /// Only match instruments on this exchange.
    #[arg(long, ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(EXCHANGES).map(|value| value.to_ascii_uppercase()))]
    pub exchange: Option<String>,
    /// Maximum matches to return. Use 0 for all.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct InstrumentResolveArgs {
    /// Symbol (`INFY`) or exchange-qualified symbol (`NSE:INFY`).
    pub symbol: String,
    /// Only match instruments on this exchange.
    #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(EXCHANGES).map(|value| value.to_ascii_uppercase()), ignore_case = true)]
    pub exchange: Option<String>,
    /// Maximum matches to return. Use 0 for all.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Args, Debug, Clone)]
#[command(group = clap::ArgGroup::new("quote_inputs").required(true).multiple(true).args(["instruments", "instrument"]))]
pub struct QuoteArgs {
    /// Instruments as EXCHANGE:TRADINGSYMBOL, for example NSE:INFY.
    #[arg(value_name = "INSTRUMENT", value_parser = parse_quote_instrument)]
    pub instruments: Vec<String>,
    /// Repeatable alternative to positional instruments.
    #[arg(short = 'i', long = "instrument", value_name = "EXCHANGE:SYMBOL", value_parser = parse_quote_instrument)]
    pub instrument: Vec<String>,
}

#[derive(Args, Debug)]
pub struct HistoricalArgs {
    #[expect(
        clippy::doc_markdown,
        reason = "clap uses this doc verbatim as existing CLI help"
    )]
    /// Numeric instrument_token from `kite market resolve` or instruments CSV.
    #[arg(long)]
    pub instrument_token: u32,
    /// minute, 3minute, 5minute, 10minute, 15minute, 30minute, 60minute, day.
    #[arg(long, value_parser = ["minute", "3minute", "5minute", "10minute", "15minute", "30minute", "60minute", "day"])]
    pub interval: String,
    /// Start timestamp: yyyy-mm-dd hh:mm:ss.
    #[arg(long)]
    pub from: String,
    /// End timestamp: yyyy-mm-dd hh:mm:ss.
    #[arg(long)]
    pub to: String,
    /// Fetch continuous futures data where supported.
    #[arg(long)]
    pub continuous: bool,
    /// Include open interest column where available.
    #[arg(long)]
    pub oi: bool,
}

#[derive(Args, Debug)]
pub struct MarginOrdersArgs {
    #[command(flatten)]
    pub input: JsonArrayInput,
    /// Request compact response (`mode=compact`).
    #[arg(long)]
    pub compact: bool,
}

#[derive(Args, Debug)]
pub struct MarginBasketArgs {
    #[command(flatten)]
    pub input: JsonArrayInput,
    /// Include current positions in basket margin calculation.
    #[arg(long)]
    pub consider_positions: bool,
    /// Request compact response (`mode=compact`).
    #[arg(long)]
    pub compact: bool,
}

#[derive(Args, Debug)]
pub struct MarginChargesArgs {
    #[command(flatten)]
    pub input: JsonArrayInput,
}

#[derive(Args, Debug)]
pub struct JsonArrayInput {
    /// JSON array file.
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
    /// Inline JSON array.
    #[arg(long = "input-json", value_name = "JSON_ARRAY")]
    pub input_json: Option<String>,
    /// Repeatable inline JSON object; objects are combined into an array.
    #[arg(long = "order", value_name = "JSON_OBJECT")]
    pub order: Vec<String>,
}

#[derive(Args, Debug)]
pub struct GttWriteArgs {
    /// GTT type. Defaults to single on create; keeps the stored type on modify.
    #[arg(long = "type", ignore_case = true, value_parser = clap::builder::PossibleValuesParser::new(["single", "two-leg", "trailing-single", "trailing-two-leg"]).map(|value| value.to_ascii_lowercase()))]
    pub trigger_type: Option<String>,
    /// Trailing distance in price points. Required for a trailing GTT unless
    /// condition JSON already contains `trailing_points`.
    #[arg(long = "trailing-points", value_name = "POINTS")]
    pub trailing_points: Option<f64>,
    /// Inline GTT condition JSON object.
    #[arg(long = "condition-json", value_name = "JSON_OBJECT")]
    pub condition_json: Option<String>,
    /// GTT condition JSON file.
    #[arg(long = "condition-file", value_name = "PATH")]
    pub condition_file: Option<PathBuf>,
    /// Inline GTT orders JSON array.
    #[arg(long = "orders-json", value_name = "JSON_ARRAY")]
    pub orders_json: Option<String>,
    /// GTT orders JSON file.
    #[arg(long = "orders-file", value_name = "PATH")]
    pub orders_file: Option<PathBuf>,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Args, Debug)]
pub struct GttModifyArgs {
    /// GTT trigger ID.
    #[arg(long)]
    pub trigger_id: i64,
    #[command(flatten)]
    pub write: GttWriteArgs,
}

#[derive(Args, Debug)]
pub struct GttDeleteArgs {
    /// GTT trigger ID.
    #[arg(long)]
    pub trigger_id: i64,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Args, Debug)]
pub struct MfInstrumentDumpArgs {
    /// Write raw CSV to a file instead of stdout.
    #[arg(long, value_name = "PATH")]
    pub out: Option<PathBuf>,
    /// Limit displayed records. Use 0 for all.
    #[arg(long, default_value_t = 50)]
    pub limit: usize,
}

#[derive(Args, Debug)]
pub struct MfInstrumentSearchArgs {
    /// Fund name or ISIN to search for.
    pub query: String,
    /// Maximum matches to return. Use 0 for all.
    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Args, Debug, Clone)]
pub struct TuiArgs {
    /// Legacy flag; the TUI now picks stream modes per instrument automatically.
    #[arg(long, default_value_t = StreamMode::Quote, value_enum, hide = true)]
    pub mode: StreamMode,
    /// Optional instrument tokens for an ephemeral watchlist (not persisted).
    #[arg(long = "token")]
    pub token: Vec<u32>,
    /// Instrument tokens, separated by spaces or commas. Repeatable.
    #[arg(long = "tokens", num_args = 1.., value_delimiter = ',')]
    pub tokens: Vec<u32>,
}

#[derive(Args, Debug)]
#[command(group = clap::ArgGroup::new("tick_inputs").required(true).multiple(true).args(["instruments", "token", "tokens"]))]
pub struct StreamTicksArgs {
    /// Instruments to stream, in EXCHANGE:SYMBOL form.
    #[arg(value_name = "INSTRUMENT", value_parser = parse_quote_instrument)]
    pub instruments: Vec<String>,
    /// WebSocket tick detail level.
    #[arg(long, default_value_t = StreamMode::Quote, value_enum, ignore_case = true)]
    pub mode: StreamMode,
    /// One instrument token. Repeatable.
    #[arg(long = "token")]
    pub token: Vec<u32>,
    /// Instrument tokens, separated by spaces or commas. Repeatable.
    #[arg(long = "tokens", num_args = 1.., value_delimiter = ',')]
    pub tokens: Vec<u32>,
    /// Stop after this many ticks. Use 0 for no limit.
    #[arg(long, default_value_t = 10)]
    pub limit: usize,
    /// Stop waiting after this many seconds. Use 0 for no timeout.
    #[arg(long, default_value_t = 30)]
    pub timeout_seconds: u64,
}

#[derive(Args, Debug)]
pub struct RawGetArgs {
    /// Kite API path, for example /user/profile. Full URLs are rejected.
    pub path: String,
    /// Query parameter. Repeat for multiple parameters.
    #[arg(short = 'q', long = "query", value_name = "KEY=VALUE")]
    pub query: Vec<String>,
}

#[derive(Args, Debug)]
pub struct RawWriteArgs {
    /// Kite API path, for example /margins/orders. Full URLs are rejected.
    pub path: String,
    /// Query parameter. Repeat for multiple parameters.
    #[arg(short = 'q', long = "query", value_name = "KEY=VALUE")]
    pub query: Vec<String>,
    /// Inline JSON request body.
    #[arg(long = "body-json", value_name = "JSON")]
    pub json_body: Option<String>,
    /// File containing the JSON request body.
    #[arg(long = "body-file", value_name = "PATH")]
    pub json_file: Option<PathBuf>,
    /// Form field. Repeat for multiple fields.
    #[arg(long = "form", value_name = "KEY=VALUE")]
    pub form: Vec<String>,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Args, Debug)]
pub struct RawDeleteArgs {
    /// Kite API path, for example /gtt/triggers/123. Full URLs are rejected.
    pub path: String,
    /// Query parameter. Repeat for multiple parameters.
    #[arg(short = 'q', long = "query", value_name = "KEY=VALUE")]
    pub query: Vec<String>,
    #[command(flatten)]
    pub confirm: ConfirmArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum StreamMode {
    Ltp,
    Quote,
    Full,
}

impl From<StreamMode> for kiteconnect::ticker::Mode {
    fn from(mode: StreamMode) -> Self {
        match mode {
            StreamMode::Ltp => Self::Ltp,
            StreamMode::Quote => Self::Quote,
            StreamMode::Full => Self::Full,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn clap_command_tree_is_valid() {
        Cli::command().debug_assert();
    }
    #[test]
    fn dry_run_takes_precedence_over_confirmation() {
        for (confirm, dry_run, expected) in [
            (false, false, true),
            (false, true, true),
            (true, false, false),
            (true, true, true),
        ] {
            assert_eq!(
                super::ConfirmArgs { dry_run, confirm }.is_dry_run(),
                expected
            );
        }
    }
    #[test]
    fn stream_mode_preserves_its_json_spelling() {
        assert_eq!(
            serde_json::to_string(&super::StreamMode::Quote).unwrap(),
            "\"Quote\""
        );
    }
    #[test]
    fn token_lists_accept_spaces_commas_and_repeated_flags() {
        use clap::Parser;
        for tokens in [
            vec!["--tokens", "1", "2", "--tokens", "3"],
            vec!["--tokens", "1,2,3"],
        ] {
            let mut args = vec!["kite", "stream", "ticks"];
            args.extend(tokens);
            let cli = Cli::try_parse_from(args).unwrap();
            let Some(super::Commands::Stream {
                command: super::StreamCommands::Ticks(args),
            }) = cli.command
            else {
                panic!("expected ticks")
            };
            assert_eq!(args.tokens, vec![1, 2, 3]);
        }
    }
    #[test]
    fn market_reads_require_qualified_instruments_or_tokens() {
        use clap::Parser;
        for args in [
            vec!["kite", "market", "ltp"],
            vec!["kite", "market", "ltp", "INFY"],
            vec!["kite", "stream", "ticks"],
            vec!["kite", "stream", "ticks", "INFY"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        assert!(super::parse_quote_instrument("NSE:INFY").is_ok());
        for value in ["INFY", ":INFY", "NSE:"] {
            assert!(super::parse_quote_instrument(value).is_err());
        }
    }

    #[test]
    fn order_prices_and_tags_refuse_invalid_wire_values() {
        for value in ["NaN", "inf", "-1", "-inf"] {
            assert!(super::parse_order_price(value).is_err());
        }
        for value in ["", "with space", "a-b", "123456789012345678901"] {
            assert!(super::parse_order_tag(value).is_err());
        }
        assert_eq!(super::parse_order_price("0").unwrap(), 0.0);
        assert!(super::parse_order_tag("tag123").is_ok());
    }
}
