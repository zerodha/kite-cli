# CLI reference

This file is generated from `kite --help` and the `--help` of every subcommand.
Do not edit it by hand. After you change help text in `src/cli.rs`, run:

```bash
cargo build && python3 scripts/gen-cli-reference.py
```

For task-based examples, see the [user guide](guide/README.md).

## `kite`

```text
A composable CLI for Zerodha Kite: auth, orders, portfolio, market data, instruments, historical candles, margins, GTTs, alerts, mutual funds, WebSocket ticks, and raw API requests. `kite auth login` signs in with your user ID, password and TOTP or SMS OTP, and saves the access token it returns. KITE_API_KEY and KITE_ACCESS_TOKEN override the saved session. Use --json for stable machine-readable output.

Usage: kite [OPTIONS] [COMMAND]

Commands:
  doctor      Verify install, config, auth sources, and read-only API reachability
  config      Manage the kite config file (see `kite config path`)
  auth        Interactive token login and logout
  user        User profile and available funds/margins
  orders      Order book, trades, order history, and safe order writes
  portfolio   Long-term holdings, intraday/overnight positions, auctions, and conversion
  market      Instruments, quotes, search/resolve helpers, and market data snapshots
  quotes      Backwards-compatible quote commands. Prefer `kite market ...`
  historical  Historical candle data
  margins     Order margin, basket margin, and charges calculators
  gtt         Good Till Triggered (GTT) triggers
  alerts      Server-side price alerts. ATO basket creation and editing stay in the TUI
  mf          Mutual fund orders, SIPs, holdings, and instrument master
  stream      WebSocket live tick stream helpers
  tui         Interactive terminal trading UI: watchlist, positions, orders, holdings
  request     Authenticated raw Kite API escape hatch
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Quick start:
  kite doctor
  kite auth login
  kite user profile

Common reads:
  kite market ltp NSE:INFY NSE:RELIANCE
  kite market resolve NSE:INFY
  kite portfolio holdings
  kite orders list

Safety:
  Order, GTT, alert, portfolio-convert, and raw write commands dry-run by default.
  Add --confirm only when you intend to send a live write to Kite.
```

### `kite doctor`

```text
Verify install, config, auth sources, and read-only API reachability

Usage: kite doctor [OPTIONS]

Options:
      --no-live
          Skip the live /user/margins auth check even if a session is present

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Examples:
  kite doctor
  kite doctor --no-live

`doctor` never prints full credentials. Without auth it reports setup steps instead of failing early.
```

### `kite config`

```text
Manage the kite config file (see `kite config path`)

Usage: kite config [OPTIONS] <COMMAND>

Commands:
  path  Print the config path
  show  Show the saved session, redacted by default
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Examples:
  kite config path
  kite config show
  kite config show --reveal
```

#### `kite config path`

```text
Print the config path

Usage: kite config path [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite config show`

```text
Show the saved session, redacted by default

Usage: kite config show [OPTIONS]

Options:
      --reveal  Reveal secrets. Avoid in shared shells/logs
  -h, --help    Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite auth`

```text
Interactive token login and logout

Usage: kite auth [OPTIONS] <COMMAND>

Commands:
  login   Sign in with a user ID, password, and verification code
  logout  Revoke and clear the saved session
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Examples:
  kite auth login
  kite auth login AB1234
  kite auth logout

`kite auth login` signs in with a user ID, password, and verification code. Saves the access token and user ID to config.
```

#### `kite auth login`

```text
Sign in with a user ID, password, and verification code

Usage: kite auth login [OPTIONS] [USER_ID]

Arguments:
  [USER_ID]  Kite user_id (Zerodha client ID). Prompted if omitted

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite auth logout`

```text
Revoke and clear the saved session

Usage: kite auth logout [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite user`

```text
User profile and available funds/margins

Usage: kite user [OPTIONS] <COMMAND>

Commands:
  profile  Retrieve the logged-in user's profile
  margins  Retrieve funds and margins for all segments or one segment
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite user profile`

```text
Retrieve the logged-in user's profile

Usage: kite user profile [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite user margins`

```text
Retrieve funds and margins for all segments or one segment

Usage: kite user margins [OPTIONS]

Options:
      --segment <SEGMENT>  equity or commodity [possible values: equity, commodity]
  -h, --help               Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite orders`

```text
Order book, trades, order history, and safe order writes

Usage: kite orders [OPTIONS] <COMMAND>

Commands:
  list    Retrieve today's order book
  get     Retrieve history for one order_id
  trades  Retrieve all trades for the day or trades for one order_id
  place   Place an order. Dry-runs unless --confirm is provided
  modify  Modify an open/pending order. Dry-runs unless --confirm is provided
  cancel  Cancel an open/pending order. Dry-runs unless --confirm is provided
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite orders list`

```text
Retrieve today's order book

Usage: kite orders list [OPTIONS]

Options:
      --status <STATUS>                Filter by status, ignoring case. OPEN includes working and trigger-pending orders
      --tradingsymbol <TRADINGSYMBOL>  Client-side filter by tradingsymbol
  -h, --help                           Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite orders get`

```text
Retrieve history for one order_id

Usage: kite orders get [OPTIONS] <ORDER_ID>

Arguments:
  <ORDER_ID>  Kite order ID

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite orders trades`

```text
Retrieve all trades for the day or trades for one order_id

Usage: kite orders trades [OPTIONS]

Options:
      --order-id <ORDER_ID>  Limit trades to this order ID
  -h, --help                 Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite orders place`

```text
Place an order. Dry-runs unless --confirm is provided

Usage: kite orders place [OPTIONS] --exchange <EXCHANGE> --tradingsymbol <TRADINGSYMBOL> --transaction-type <TRANSACTION_TYPE> --order-type <ORDER_TYPE> --quantity <QUANTITY> --product <PRODUCT>

Options:
      --variety <VARIETY>
          Order variety

          [default: regular]
          [possible values: regular, amo, co, iceberg, auction]

      --exchange <EXCHANGE>
          Exchange to send the order to

          [possible values: NSE, BSE, NFO, BFO, CDS, BCD, MCX, NCO]

      --tradingsymbol <TRADINGSYMBOL>
          Exchange trading symbol, for example INFY, RELIANCE, NIFTY24JANFUT

      --transaction-type <TRANSACTION_TYPE>
          BUY or SELL

          [possible values: BUY, SELL]

      --order-type <ORDER_TYPE>
          MARKET, LIMIT, SL, SL-M

          [possible values: MARKET, LIMIT, SL, SL-M]

      --quantity <QUANTITY>
          Number of shares/contracts/units to transact

      --product <PRODUCT>
          CNC, NRML, MIS, MTF, etc

          [possible values: CNC, NRML, MIS, MTF, CO, BO]

      --price <PRICE>
          Limit price for LIMIT/SL orders. Omit or use 0 for MARKET orders

      --trigger-price <TRIGGER_PRICE>
          Trigger price for SL/SL-M orders

      --disclosed-quantity <DISCLOSED_QUANTITY>
          Quantity disclosed to the market for equity orders

      --validity <VALIDITY>
          DAY, IOC, TTL

          [possible values: DAY, IOC, TTL]

      --validity-ttl <VALIDITY_TTL>
          TTL validity in minutes. Used only when --validity TTL

      --iceberg-legs <ICEBERG_LEGS>
          Number of iceberg legs, between 2 and 50

      --iceberg-quantity <ICEBERG_QUANTITY>
          Quantity per iceberg leg

      --auction-number <AUCTION_NUMBER>
          Auction number for auction variety orders

      --market-protection <MARKET_PROTECTION>
          -1 for auto protection, or 0-100 percent

      --autoslice
          Enable autoslice for freeze-quantity orders

      --tag <TAG>
          Optional alphanumeric order tag, max 20 characters

      --dry-run
          Show the request without sending it

      --confirm
          Required to execute a live write/delete

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Examples:
  kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY --order-type MARKET --quantity 1 --product CNC --validity DAY
  kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY --order-type LIMIT --quantity 1 --product CNC --price 1500 --validity DAY

The command prints a dry-run request by default. Add --confirm only to place a live order.
```

#### `kite orders modify`

```text
Modify an open/pending order. Dry-runs unless --confirm is provided

Usage: kite orders modify [OPTIONS] --order-id <ORDER_ID>

Options:
      --variety <VARIETY>
          Order variety

          [default: regular]
          [possible values: regular, amo, co, iceberg, auction]

      --order-id <ORDER_ID>
          Kite order_id to modify

      --order-type <ORDER_TYPE>
          New order type

          [possible values: MARKET, LIMIT, SL, SL-M]

      --quantity <QUANTITY>
          New quantity

      --price <PRICE>
          New limit price

      --trigger-price <TRIGGER_PRICE>
          New trigger price

      --disclosed-quantity <DISCLOSED_QUANTITY>
          New disclosed quantity

      --validity <VALIDITY>
          New order validity

          [possible values: DAY, IOC, TTL]

      --validity-ttl <VALIDITY_TTL>
          New TTL validity in minutes. Used only when --validity TTL

      --market-protection <MARKET_PROTECTION>
          -1 for automatic protection, or 0 to 100 percent

      --dry-run
          Show the request without sending it

      --confirm
          Required to execute a live write/delete

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Example:
  kite orders modify --order-id 151220000000000 --quantity 2 --price 1500

The command prints a dry-run request by default. Add --confirm only to modify a live order.
```

#### `kite orders cancel`

```text
Cancel an open/pending order. Dry-runs unless --confirm is provided

Usage: kite orders cancel [OPTIONS] --order-id <ORDER_ID>

Options:
      --variety <VARIETY>
          Order variety

          [default: regular]
          [possible values: regular, amo, co, iceberg, auction]

      --order-id <ORDER_ID>
          Kite order_id to cancel

      --dry-run
          Show the request without sending it

      --confirm
          Required to execute a live write/delete

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Example:
  kite orders cancel --order-id 151220000000000

The command prints a dry-run request by default. Add --confirm only to cancel a live order.
```

### `kite portfolio`

```text
Long-term holdings, intraday/overnight positions, auctions, and conversion

Usage: kite portfolio [OPTIONS] <COMMAND>

Commands:
  holdings   Long-term equity holdings
  positions  Short-term positions, returned as net/day groups
  auctions   Holdings available in current auctions
  convert    Convert the margin product of an open position. Dry-runs unless --confirm is provided
  help       Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite portfolio holdings`

```text
Long-term equity holdings

Usage: kite portfolio holdings [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite portfolio positions`

```text
Short-term positions, returned as net/day groups

Usage: kite portfolio positions [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite portfolio auctions`

```text
Holdings available in current auctions

Usage: kite portfolio auctions [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite portfolio convert`

```text
Convert the margin product of an open position. Dry-runs unless --confirm is provided

Usage: kite portfolio convert [OPTIONS] --exchange <EXCHANGE> --tradingsymbol <TRADINGSYMBOL> --transaction-type <TRANSACTION_TYPE> --position-type <POSITION_TYPE> --quantity <QUANTITY> --old-product <OLD_PRODUCT> --new-product <NEW_PRODUCT>

Options:
      --exchange <EXCHANGE>
          Exchange to send the order to

          [possible values: NSE, BSE, NFO, BFO, CDS, BCD, MCX, NCO]

      --tradingsymbol <TRADINGSYMBOL>
          Exchange trading symbol, for example INFY

      --transaction-type <TRANSACTION_TYPE>
          [possible values: BUY, SELL]

      --position-type <POSITION_TYPE>
          day or overnight

          [possible values: day, overnight]

      --quantity <QUANTITY>
          Quantity to convert

      --old-product <OLD_PRODUCT>
          [possible values: CNC, NRML, MIS, MTF, CO, BO]

      --new-product <NEW_PRODUCT>
          [possible values: CNC, NRML, MIS, MTF, CO, BO]

      --dry-run
          Show the request without sending it

      --confirm
          Required to execute a live write/delete

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Example:
  kite portfolio convert --exchange NSE --tradingsymbol INFY --transaction-type BUY --position-type day --quantity 1 --old-product MIS --new-product CNC

The command prints a dry-run request by default. Add --confirm only to send the live conversion.
```

### `kite market`

```text
Instruments, quotes, search/resolve helpers, and market data snapshots

Usage: kite market [OPTIONS] <COMMAND>

Commands:
  instruments  Show instruments, or download JSON to --out
  cache        Inspect and manage the local instrument cache
  search       Search the instrument master by symbol or name
  resolve      Resolve a symbol into stable instrument metadata/token(s)
  quote        Full market quotes; limit: 500 instruments
  ohlc         OHLC + LTP quotes; limit: 1000 instruments
  ltp          LTP quotes; limit: 1000 instruments
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market instruments`

```text
Show instruments, or download JSON to --out

Usage: kite market instruments [OPTIONS]

Options:
      --out <PATH>     Write all parsed instrument records as JSON to this file. Ignores --limit
      --limit <LIMIT>  Limit displayed records. Use 0 for all [default: 50]
  -h, --help           Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market cache`

```text
Inspect and manage the local instrument cache

Usage: kite market cache [OPTIONS] <COMMAND>

Commands:
  path     Print the local instrument cache path
  info     Show cache metadata and freshness
  refresh  Refresh the local instrument cache from instruments.json
  clear    Delete the local instrument cache file
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market cache path`

```text
Print the local instrument cache path

Usage: kite market cache path [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market cache info`

```text
Show cache metadata and freshness

Usage: kite market cache info [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market cache refresh`

```text
Refresh the local instrument cache from instruments.json

Usage: kite market cache refresh [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market cache clear`

```text
Delete the local instrument cache file

Usage: kite market cache clear [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market search`

```text
Search the instrument master by symbol or name

Usage: kite market search [OPTIONS] <QUERY>

Arguments:
  <QUERY>  Search text matched against tradingsymbol, name, and aliases. Prefix with `EXCHANGE:` (for example `NSE:infy`) to filter by exchange

Options:
      --exchange <EXCHANGE>  Only match instruments on this exchange [possible values: NSE, BSE, NFO, BFO, CDS, BCD, MCX, NCO]
      --limit <LIMIT>        Maximum matches to return. Use 0 for all [default: 20]
  -h, --help                 Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market resolve`

```text
Resolve a symbol into stable instrument metadata/token(s)

Usage: kite market resolve [OPTIONS] <SYMBOL>

Arguments:
  <SYMBOL>  Symbol (`INFY`) or exchange-qualified symbol (`NSE:INFY`)

Options:
      --exchange <EXCHANGE>  Only match instruments on this exchange [possible values: NSE, BSE, NFO, BFO, CDS, BCD, MCX, NCO]
      --limit <LIMIT>        Maximum matches to return. Use 0 for all [default: 20]
  -h, --help                 Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market quote`

```text
Full market quotes; limit: 500 instruments

Usage: kite market quote [OPTIONS] <INSTRUMENT|--instrument <EXCHANGE:SYMBOL>>

Arguments:
  [INSTRUMENT]...  Instruments as EXCHANGE:TRADINGSYMBOL, for example NSE:INFY

Options:
  -i, --instrument <EXCHANGE:SYMBOL>  Repeatable alternative to positional instruments
  -h, --help                          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market ohlc`

```text
OHLC + LTP quotes; limit: 1000 instruments

Usage: kite market ohlc [OPTIONS] <INSTRUMENT|--instrument <EXCHANGE:SYMBOL>>

Arguments:
  [INSTRUMENT]...  Instruments as EXCHANGE:TRADINGSYMBOL, for example NSE:INFY

Options:
  -i, --instrument <EXCHANGE:SYMBOL>  Repeatable alternative to positional instruments
  -h, --help                          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite market ltp`

```text
LTP quotes; limit: 1000 instruments

Usage: kite market ltp [OPTIONS] <INSTRUMENT|--instrument <EXCHANGE:SYMBOL>>

Arguments:
  [INSTRUMENT]...  Instruments as EXCHANGE:TRADINGSYMBOL, for example NSE:INFY

Options:
  -i, --instrument <EXCHANGE:SYMBOL>  Repeatable alternative to positional instruments
  -h, --help                          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite quotes`

```text
Backwards-compatible quote commands. Prefer `kite market ...`

Usage: kite quotes [OPTIONS] <COMMAND>

Commands:
  ltp   LTP quotes. Prefer `kite market ltp`
  ohlc  OHLC quotes. Prefer `kite market ohlc`
  full  Full quotes. Prefer `kite market quote`
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite quotes ltp`

```text
LTP quotes. Prefer `kite market ltp`

Usage: kite quotes ltp [OPTIONS] <INSTRUMENT|--instrument <EXCHANGE:SYMBOL>>

Arguments:
  [INSTRUMENT]...  Instruments as EXCHANGE:TRADINGSYMBOL, for example NSE:INFY

Options:
  -i, --instrument <EXCHANGE:SYMBOL>  Repeatable alternative to positional instruments
  -h, --help                          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite quotes ohlc`

```text
OHLC quotes. Prefer `kite market ohlc`

Usage: kite quotes ohlc [OPTIONS] <INSTRUMENT|--instrument <EXCHANGE:SYMBOL>>

Arguments:
  [INSTRUMENT]...  Instruments as EXCHANGE:TRADINGSYMBOL, for example NSE:INFY

Options:
  -i, --instrument <EXCHANGE:SYMBOL>  Repeatable alternative to positional instruments
  -h, --help                          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite quotes full`

```text
Full quotes. Prefer `kite market quote`

Usage: kite quotes full [OPTIONS] <INSTRUMENT|--instrument <EXCHANGE:SYMBOL>>

Arguments:
  [INSTRUMENT]...  Instruments as EXCHANGE:TRADINGSYMBOL, for example NSE:INFY

Options:
  -i, --instrument <EXCHANGE:SYMBOL>  Repeatable alternative to positional instruments
  -h, --help                          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite historical`

```text
Historical candle data

Usage: kite historical [OPTIONS] <COMMAND>

Commands:
  candles  Retrieve historical candle records for an instrument token
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite historical candles`

```text
Retrieve historical candle records for an instrument token

Usage: kite historical candles [OPTIONS] --instrument-token <INSTRUMENT_TOKEN> --interval <INTERVAL> --from <FROM> --to <TO>

Options:
      --instrument-token <INSTRUMENT_TOKEN>
          Numeric instrument_token from `kite market resolve` or instruments CSV
      --interval <INTERVAL>
          minute, 3minute, 5minute, 10minute, 15minute, 30minute, 60minute, day [possible values: minute, 3minute, 5minute, 10minute, 15minute, 30minute, 60minute, day]
      --from <FROM>
          Start timestamp: yyyy-mm-dd hh:mm:ss
      --to <TO>
          End timestamp: yyyy-mm-dd hh:mm:ss
      --continuous
          Fetch continuous futures data where supported
      --oi
          Include open interest column where available
  -h, --help
          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite margins`

```text
Order margin, basket margin, and charges calculators

Usage: kite margins [OPTIONS] <COMMAND>

Commands:
  orders   Calculate margins for each order in a JSON array
  basket   Calculate basket margins for spread orders
  charges  Calculate virtual contract note / order charges
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite margins orders`

```text
Calculate margins for each order in a JSON array

Usage: kite margins orders [OPTIONS]

Options:
      --file <PATH>              JSON array file
      --input-json <JSON_ARRAY>  Inline JSON array
      --order <JSON_OBJECT>      Repeatable inline JSON object; objects are combined into an array
      --compact                  Request compact response (`mode=compact`)
  -h, --help                     Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite margins basket`

```text
Calculate basket margins for spread orders

Usage: kite margins basket [OPTIONS]

Options:
      --file <PATH>              JSON array file
      --input-json <JSON_ARRAY>  Inline JSON array
      --order <JSON_OBJECT>      Repeatable inline JSON object; objects are combined into an array
      --consider-positions       Include current positions in basket margin calculation
      --compact                  Request compact response (`mode=compact`)
  -h, --help                     Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite margins charges`

```text
Calculate virtual contract note / order charges

Usage: kite margins charges [OPTIONS]

Options:
      --file <PATH>              JSON array file
      --input-json <JSON_ARRAY>  Inline JSON array
      --order <JSON_OBJECT>      Repeatable inline JSON object; objects are combined into an array
  -h, --help                     Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite gtt`

```text
Good Till Triggered (GTT) triggers

Usage: kite gtt [OPTIONS] <COMMAND>

Commands:
  list    List all visible GTT triggers
  get     Retrieve one GTT trigger
  place   Place a GTT trigger. Dry-runs unless --confirm is provided
  modify  Modify a GTT trigger. Dry-runs unless --confirm is provided
  delete  Delete a GTT trigger. Dry-runs unless --confirm is provided
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite gtt list`

```text
List all visible GTT triggers

Usage: kite gtt list [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite gtt get`

```text
Retrieve one GTT trigger

Usage: kite gtt get [OPTIONS] <TRIGGER_ID>

Arguments:
  <TRIGGER_ID>  GTT trigger ID

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite gtt place`

```text
Place a GTT trigger. Dry-runs unless --confirm is provided

Usage: kite gtt place [OPTIONS]

Options:
      --type <TRIGGER_TYPE>
          GTT type. Defaults to single on create; keeps the stored type on modify

          [possible values: single, two-leg, trailing-single, trailing-two-leg]

      --trailing-points <POINTS>
          Trailing distance in price points. Required for a trailing GTT unless condition JSON already contains `trailing_points`

      --condition-json <JSON_OBJECT>
          Inline GTT condition JSON object

      --condition-file <PATH>
          GTT condition JSON file

      --orders-json <JSON_ARRAY>
          Inline GTT orders JSON array

      --orders-file <PATH>
          GTT orders JSON file

      --dry-run
          Show the request without sending it

      --confirm
          Required to execute a live write/delete

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Example:
  kite gtt place --type single --condition-json '{"exchange":"NSE","tradingsymbol":"INFY","trigger_values":[702.0],"last_price":798.0}' --orders-json '[{"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","quantity":1,"order_type":"LIMIT","product":"CNC","price":702.5}]'

The command prints a dry-run request by default. Add --confirm only to place the live GTT.
```

#### `kite gtt modify`

```text
Modify a GTT trigger. Dry-runs unless --confirm is provided

Usage: kite gtt modify [OPTIONS] --trigger-id <TRIGGER_ID>

Options:
      --trigger-id <TRIGGER_ID>       GTT trigger ID
      --type <TRIGGER_TYPE>           GTT type. Defaults to single on create; keeps the stored type on modify [possible values: single, two-leg, trailing-single, trailing-two-leg]
      --trailing-points <POINTS>      Trailing distance in price points. Required for a trailing GTT unless condition JSON already contains `trailing_points`
      --condition-json <JSON_OBJECT>  Inline GTT condition JSON object
      --condition-file <PATH>         GTT condition JSON file
      --orders-json <JSON_ARRAY>      Inline GTT orders JSON array
      --orders-file <PATH>            GTT orders JSON file
      --dry-run                       Show the request without sending it
      --confirm                       Required to execute a live write/delete
  -h, --help                          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite gtt delete`

```text
Delete a GTT trigger. Dry-runs unless --confirm is provided

Usage: kite gtt delete [OPTIONS] --trigger-id <TRIGGER_ID>

Options:
      --trigger-id <TRIGGER_ID>  GTT trigger ID
      --dry-run                  Show the request without sending it
      --confirm                  Required to execute a live write/delete
  -h, --help                     Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite alerts`

```text
Server-side price alerts. ATO basket creation and editing stay in the TUI

Usage: kite alerts [OPTIONS] <COMMAND>

Commands:
  list     List alerts, optionally filtered by status
  get      Retrieve one alert
  history  Retrieve an alert's trigger history
  create   Create a simple alert. Dry-runs unless --confirm is provided
  modify   Modify a simple alert, preserving omitted fields and status. Dry-runs by default
  delete   Delete one or more alerts. Dry-runs unless --confirm is provided
  enable   Enable an alert, preserving its type and basket. Dry-runs by default
  disable  Disable an alert, preserving its type and basket. Dry-runs by default
  help     Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Examples:
  kite alerts list --status enabled
  kite alerts get UUID
  kite alerts history UUID
  kite alerts create --name 'INFY target' --lhs NSE:INFY --operator '>=' --value 1500
  kite alerts create --name 'Index comparison' --lhs 'NSE:NIFTY 50' --operator '>' --rhs 'NSE:NIFTY NEXT 50'
  kite alerts modify UUID --value 1600
  kite alerts disable UUID
  kite alerts enable UUID --confirm
  kite alerts delete UUID --confirm

Writes dry-run by default. Add --confirm to send a live write. Modify, enable and disable read the stored alert before showing the exact request. Use the TUI to create or edit ATO basket alerts.
```

#### `kite alerts list`

```text
List alerts, optionally filtered by status

Usage: kite alerts list [OPTIONS]

Options:
      --status <STATUS>  [possible values: enabled, disabled, deleted]
  -h, --help             Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite alerts get`

```text
Retrieve one alert

Usage: kite alerts get [OPTIONS] <UUID>

Arguments:
  <UUID>

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite alerts history`

```text
Retrieve an alert's trigger history

Usage: kite alerts history [OPTIONS] <UUID>

Arguments:
  <UUID>

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite alerts create`

```text
Create a simple alert. Dry-runs unless --confirm is provided

Usage: kite alerts create [OPTIONS] --name <NAME> --lhs <EXCHANGE:SYMBOL> --operator <OPERATOR> <--value <VALUE>|--rhs <EXCHANGE:SYMBOL>>

Options:
      --name <NAME>
          Alert name

      --lhs <EXCHANGE:SYMBOL>
          Left-hand instrument in EXCHANGE:SYMBOL form

      --attribute <ATTRIBUTE>
          Left-hand attribute. Defaults to `LastTradedPrice` on create

          [possible values: LastTradedPrice, HighPrice, LowPrice, OpenPrice, ClosePrice, DayChange, DayChangePercent, IntraDayChange, IntraDayChangePercent, LastTradedQuantity, AverageTradePrice, VolumeTraded, TotalBuyQuantity, TotalSellQuantity, OpenInterest, OpenInterestDayHigh, OpenInterestDayLow]

      --operator <OPERATOR>
          Comparison operator. Quote it to prevent shell redirection

          [possible values: <=, >=, <, >, ==]

      --value <VALUE>
          Right-hand constant, instead of --rhs

      --rhs <EXCHANGE:SYMBOL>
          Right-hand instrument in EXCHANGE:SYMBOL form, instead of --value

      --rhs-attribute <RHS_ATTRIBUTE>
          Right-hand attribute. Defaults to `LastTradedPrice` for a new instrument condition

          [possible values: LastTradedPrice, HighPrice, LowPrice, OpenPrice, ClosePrice, DayChange, DayChangePercent, IntraDayChange, IntraDayChangePercent, LastTradedQuantity, AverageTradePrice, VolumeTraded, TotalBuyQuantity, TotalSellQuantity, OpenInterest, OpenInterestDayHigh, OpenInterestDayLow]

      --dry-run
          Show the request without sending it

      --confirm
          Required to execute a live write/delete

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Example:
  kite alerts create --name 'INFY target' --lhs NSE:INFY --operator '>=' --value 1500

The attribute defaults to LastTradedPrice. ATO basket alerts can only be created in the TUI. Add --confirm to create the live alert.
```

#### `kite alerts modify`

```text
Modify a simple alert, preserving omitted fields and status. Dry-runs by default

Usage: kite alerts modify [OPTIONS] <--name <NAME>|--lhs <EXCHANGE:SYMBOL>|--attribute <ATTRIBUTE>|--operator <OPERATOR>|--value <VALUE>|--rhs <EXCHANGE:SYMBOL>|--rhs-attribute <RHS_ATTRIBUTE>> <UUID>

Arguments:
  <UUID>


Options:
      --name <NAME>
          Alert name

      --lhs <EXCHANGE:SYMBOL>
          Left-hand instrument in EXCHANGE:SYMBOL form

      --attribute <ATTRIBUTE>
          Left-hand attribute. Defaults to `LastTradedPrice` on create

          [possible values: LastTradedPrice, HighPrice, LowPrice, OpenPrice, ClosePrice, DayChange, DayChangePercent, IntraDayChange, IntraDayChangePercent, LastTradedQuantity, AverageTradePrice, VolumeTraded, TotalBuyQuantity, TotalSellQuantity, OpenInterest, OpenInterestDayHigh, OpenInterestDayLow]

      --operator <OPERATOR>
          Comparison operator. Quote it to prevent shell redirection

          [possible values: <=, >=, <, >, ==]

      --value <VALUE>
          Right-hand constant, instead of --rhs

      --rhs <EXCHANGE:SYMBOL>
          Right-hand instrument in EXCHANGE:SYMBOL form, instead of --value

      --rhs-attribute <RHS_ATTRIBUTE>
          Right-hand attribute. Defaults to `LastTradedPrice` for a new instrument condition

          [possible values: LastTradedPrice, HighPrice, LowPrice, OpenPrice, ClosePrice, DayChange, DayChangePercent, IntraDayChange, IntraDayChangePercent, LastTradedQuantity, AverageTradePrice, VolumeTraded, TotalBuyQuantity, TotalSellQuantity, OpenInterest, OpenInterestDayHigh, OpenInterestDayLow]

      --dry-run
          Show the request without sending it

      --confirm
          Required to execute a live write/delete

  -h, --help
          Print help (see a summary with '-h')

Global options:
      --json
          Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted

      --config <PATH>
          Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)

Example:
  kite alerts modify UUID --value 1600

Reads the stored alert to build the full condition. Use the TUI to edit ATO basket alerts. Add --confirm to modify the live alert.
```

#### `kite alerts delete`

```text
Delete one or more alerts. Dry-runs unless --confirm is provided

Usage: kite alerts delete [OPTIONS] <UUID>...

Arguments:
  <UUID>...

Options:
      --dry-run  Show the request without sending it
      --confirm  Required to execute a live write/delete
  -h, --help     Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite alerts enable`

```text
Enable an alert, preserving its type and basket. Dry-runs by default

Usage: kite alerts enable [OPTIONS] <UUID>

Arguments:
  <UUID>

Options:
      --dry-run  Show the request without sending it
      --confirm  Required to execute a live write/delete
  -h, --help     Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite alerts disable`

```text
Disable an alert, preserving its type and basket. Dry-runs by default

Usage: kite alerts disable [OPTIONS] <UUID>

Arguments:
  <UUID>

Options:
      --dry-run  Show the request without sending it
      --confirm  Required to execute a live write/delete
  -h, --help     Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite mf`

```text
Mutual fund orders, SIPs, holdings, and instrument master

Usage: kite mf [OPTIONS] <COMMAND>

Commands:
  orders       Mutual fund orders from the last 7 days
  order        Retrieve one mutual fund order by id
  sips         Active and paused SIPs
  holdings     Mutual fund holdings in DEMAT
  instruments  Download mutual fund instrument CSV, write to --out, or emit bounded JSON with --json
  search       Search mutual fund instruments by fund name or ISIN
  help         Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite mf orders`

```text
Mutual fund orders from the last 7 days

Usage: kite mf orders [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite mf order`

```text
Retrieve one mutual fund order by id

Usage: kite mf order [OPTIONS] <ORDER_ID>

Arguments:
  <ORDER_ID>  Mutual fund order ID

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite mf sips`

```text
Active and paused SIPs

Usage: kite mf sips [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite mf holdings`

```text
Mutual fund holdings in DEMAT

Usage: kite mf holdings [OPTIONS]

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite mf instruments`

```text
Download mutual fund instrument CSV, write to --out, or emit bounded JSON with --json

Usage: kite mf instruments [OPTIONS]

Options:
      --out <PATH>     Write raw CSV to a file instead of stdout
      --limit <LIMIT>  Limit displayed records. Use 0 for all [default: 50]
  -h, --help           Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite mf search`

```text
Search mutual fund instruments by fund name or ISIN

Usage: kite mf search [OPTIONS] <QUERY>

Arguments:
  <QUERY>  Fund name or ISIN to search for

Options:
      --limit <LIMIT>  Maximum matches to return. Use 0 for all [default: 20]
  -h, --help           Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite stream`

```text
WebSocket live tick stream helpers

Usage: kite stream [OPTIONS] <COMMAND>

Commands:
  ticks  Display live WebSocket ticks; use --json for JSON lines/arrays
  tui    Terminal UI for live ticks
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite stream ticks`

```text
Display live WebSocket ticks; use --json for JSON lines/arrays

Usage: kite stream ticks [OPTIONS] <INSTRUMENT|--token <TOKEN>|--tokens <TOKENS>...>

Arguments:
  [INSTRUMENT]...  Instruments to stream, in EXCHANGE:SYMBOL form

Options:
      --mode <MODE>
          WebSocket tick detail level [default: quote] [possible values: ltp, quote, full]
      --token <TOKEN>
          One instrument token. Repeatable
      --tokens <TOKENS>...
          Instrument tokens, separated by spaces or commas. Repeatable
      --limit <LIMIT>
          Stop after this many ticks. Use 0 for no limit [default: 10]
      --timeout-seconds <TIMEOUT_SECONDS>
          Stop waiting after this many seconds. Use 0 for no timeout [default: 30]
  -h, --help
          Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite stream tui`

```text
Terminal UI for live ticks

Usage: kite stream tui [OPTIONS]

Options:
      --token <TOKEN>       Optional instrument tokens for an ephemeral watchlist (not persisted)
      --tokens <TOKENS>...  Instrument tokens, separated by spaces or commas. Repeatable
  -h, --help                Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite tui`

```text
Interactive terminal trading UI: watchlist, positions, orders, holdings

Usage: kite tui [OPTIONS]

Options:
      --token <TOKEN>       Optional instrument tokens for an ephemeral watchlist (not persisted)
      --tokens <TOKENS>...  Instrument tokens, separated by spaces or commas. Repeatable
  -h, --help                Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

### `kite request`

```text
Authenticated raw Kite API escape hatch

Usage: kite request [OPTIONS] <COMMAND>

Commands:
  get     Authenticated raw GET. Keeps the full API response shape
  post    Authenticated raw POST. Requires --confirm for live request
  put     Authenticated raw PUT. Requires --confirm for live request
  delete  Authenticated raw DELETE. Requires --confirm for live request
  help    Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite request get`

```text
Authenticated raw GET. Keeps the full API response shape

Usage: kite request get [OPTIONS] <PATH>

Arguments:
  <PATH>  Kite API path, for example /user/profile. Full URLs are rejected

Options:
  -q, --query <KEY=VALUE>  Query parameter. Repeat for multiple parameters
  -h, --help               Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite request post`

```text
Authenticated raw POST. Requires --confirm for live request

Usage: kite request post [OPTIONS] <PATH>

Arguments:
  <PATH>  Kite API path, for example /margins/orders. Full URLs are rejected

Options:
  -q, --query <KEY=VALUE>  Query parameter. Repeat for multiple parameters
      --body-json <JSON>   Inline JSON request body
      --body-file <PATH>   File containing the JSON request body
      --form <KEY=VALUE>   Form field. Repeat for multiple fields
      --dry-run            Show the request without sending it
      --confirm            Required to execute a live write/delete
  -h, --help               Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite request put`

```text
Authenticated raw PUT. Requires --confirm for live request

Usage: kite request put [OPTIONS] <PATH>

Arguments:
  <PATH>  Kite API path, for example /margins/orders. Full URLs are rejected

Options:
  -q, --query <KEY=VALUE>  Query parameter. Repeat for multiple parameters
      --body-json <JSON>   Inline JSON request body
      --body-file <PATH>   File containing the JSON request body
      --form <KEY=VALUE>   Form field. Repeat for multiple fields
      --dry-run            Show the request without sending it
      --confirm            Required to execute a live write/delete
  -h, --help               Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```

#### `kite request delete`

```text
Authenticated raw DELETE. Requires --confirm for live request

Usage: kite request delete [OPTIONS] <PATH>

Arguments:
  <PATH>  Kite API path, for example /gtt/triggers/123. Full URLs are rejected

Options:
  -q, --query <KEY=VALUE>  Query parameter. Repeat for multiple parameters
      --dry-run            Show the request without sending it
      --confirm            Required to execute a live write/delete
  -h, --help               Print help

Global options:
      --json           Emit machine-readable JSON. Diagnostics/errors stay JSON-safe and credentials are redacted
      --config <PATH>  Config path. Defaults to $KITE_CONFIG or the kite config file (see `kite config path`)
```
