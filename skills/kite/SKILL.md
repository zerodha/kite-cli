---
name: kite
description: Use the `kite` CLI to read a Zerodha Kite account and market data (holdings, positions, orders, trades, margins, quotes, historical candles, instrument search, GTTs, price alerts, mutual funds) and to prepare orders as dry runs. Use when the user asks about their Zerodha or Kite portfolio, P&L, orders, a stock price, an option chain contract, or wants to script Kite data with `--json` and jq.
---

# kite CLI

`kite` is the official command-line client for Zerodha Kite. It acts on the user's live trading account. Treat every write as a real-money action.

## Safety rules

1. **Never send a write without explicit approval.** Order, GTT, alert, conversion and raw `post`/`put`/`delete` commands are dry runs unless `--confirm` is passed. Run the dry run first, show the user the exact request, and add `--confirm` only after the user approves that specific request in this conversation. One approval covers one request.
2. **Never sign in for the user.** `kite auth login` asks for a user ID, password and 2FA code. Do not run it, and never ask for, type, pipe or store those values. If a command fails with `MissingSession` or `session expired`, ask the user to run `kite auth login` in their own terminal.
3. **Never print credentials.** Do not read or print `config.json`, the access token, or `KITE_ACCESS_TOKEN` / `KITE_API_KEY`.
4. **Do not log out** (`kite auth logout`) unless the user asks.
5. **Do not open the TUI** (`kite` with no arguments, or `kite tui`). It is interactive and an agent cannot drive it.

## Before you start

```sh
kite --version
kite --json doctor | jq -e .ok    # exit 0 = installed, configured and signed in
```

Sessions expire every day. A failed `doctor` usually means the user must run `kite auth login`.

## Output contract

- Add `--json` (global flag, any position) for machine output. Prefer it whenever you parse results.
- With `--json`, success prints Kite's `data` payload unchanged on stdout. Errors print `{"ok": false, "error": {"type", "message"}}` on stdout.
- Exit codes: `0` success (includes dry runs and empty results), `1` runtime or API error, `2` usage error (bad flag or command; plain text on stderr even with `--json`).
- Without `--json`, output is a human table. Totals and derived columns in tables (for example combined demat + MTF quantity) are not in the JSON.
- Run `kite <command> --help` when unsure of a flag. The help text is the source of truth.

## Read commands

| Task | Command |
| :--- | :--- |
| Profile | `kite --json user profile` |
| Funds and margins | `kite --json user margins [--segment equity\|commodity]` |
| Holdings | `kite --json portfolio holdings` |
| Positions (`net` and `day` arrays) | `kite --json portfolio positions` |
| Today's orders | `kite --json orders list [--status OPEN] [--tradingsymbol INFY]` |
| One order's history | `kite --json orders get <ORDER_ID>` |
| Trades | `kite --json orders trades [--order-id <ORDER_ID>]` |
| Last price | `kite --json market ltp NSE:INFY NSE:TCS` |
| OHLC + LTP | `kite --json market ohlc NSE:INFY` |
| Full quote with depth | `kite --json market quote NSE:INFY` |
| Find an instrument | `kite --json market search "infosys" [--exchange NSE] [--limit 5]` |
| Instrument token and metadata | `kite --json market resolve NSE:INFY` |
| Historical candles | `kite --json historical candles --instrument-token 408065 --interval day --from "2026-01-01 00:00:00" --to "2026-03-31 23:59:59"` |
| GTT triggers | `kite --json gtt list` |
| Price alerts | `kite --json alerts list` |
| Mutual funds | `kite --json mf holdings`, `kite --json mf sips`, `kite --json mf orders` |
| Order margin and charges | `kite --json margins orders ...`, `kite --json margins charges ...` (see `--help`) |
| Any other GET endpoint | `kite --json request get /path -q key=value` |

Instruments are written `EXCHANGE:TRADINGSYMBOL` (`NSE:INFY`, `NFO:NIFTY26OCT25000CE`, `NSE:NIFTY 50`). Historical candles need the numeric `instrument_token`: get it from `market resolve` (`.matches[0].instrument_token`). Intervals: `minute`, `3minute`, `5minute`, `10minute`, `15minute`, `30minute`, `60minute`, `day`.

Examples:

```sh
# Holdings as symbol, quantity, last price
kite --json portfolio holdings | jq -r '.[] | [.tradingsymbol, .quantity, .last_price] | @tsv'

# Open positions with P&L
kite --json portfolio positions | jq -r '.net[] | select(.quantity != 0) | [.tradingsymbol, .quantity, .pnl] | @tsv'

# Count open orders
kite --json orders list --status OPEN | jq length
```

## Write commands (dry run first)

```sh
kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY \
  --order-type LIMIT --quantity 1 --product CNC --price 1500 --validity DAY
```

This prints the exact request and sends nothing. With `--json`, the dry run is an object with `"dry_run": true`, `method`, `path` and `body`. Show it to the user. After the user approves, run the identical command with `--confirm`.

Writes that follow this rule: `orders place|modify|cancel`, `portfolio convert`, `gtt place|modify|delete`, `alerts create|modify|enable|disable|delete`, `request post|put|delete`.

If a confirmed write times out, its outcome is unknown. Check `kite --json orders list` (or the matching list command) before you retry, so the order is not placed twice.

## Live ticks

```sh
kite --json stream ticks NSE:INFY --limit 5
```

Prints one JSON object per line. By default it stops after 10 ticks or 30 seconds. Never set both `--limit 0` and `--timeout-seconds 0`, because the command then runs until it is killed.

## Troubleshooting

- `MissingSession` or `session expired`: ask the user to run `kite auth login`.
- `CloudflareBlocked`: the request was blocked before it reached Kite. Wait and retry later; do not loop.
- `InputException` and other Kite error types: Kite rejected the request. Show the user the message; do not guess new parameters for a write.
- Instrument not found: run `kite market cache refresh`, then search again.
