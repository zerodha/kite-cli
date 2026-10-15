# Scripting

This guide covers using `kite` in shell scripts and pipelines: `--json` output, exit codes, `stdout` and `stderr` separation, terminal colour and width handling, WebSocket streaming, and raw API requests (`kite request`).

```bash
kite --json portfolio holdings | jq -r '.[] | [.tradingsymbol, .quantity, .last_price] | @tsv'
kite --json orders list --status OPEN | jq length
kite --json request get /quote/ltp -q i=NSE:INFY
```

## `--json` output

`--json` is a global flag and can be placed before or after the subcommand. When `--json` is active:

- **Success** — Prints Kite's `data` payload unchanged as pretty-printed JSON on `stdout`. New fields added by Kite's API appear automatically without requiring a `kite` upgrade.
- **Runtime errors** — Prints a structured JSON error envelope on `stdout` and exits with code `1` (see [Errors](#errors)).
- **Non-interactive** — Never prompts for input. `kite auth login` refuses to run with `--json`.

**Exceptions:**
- **Argument parsing errors** (unknown commands, missing required flags, or invalid flag values) are caught before execution; they print plain text to `stderr` and exit with code `2`, even when `--json` is passed.
- **`--help`** always prints plain text.
- **`kite tui`** is interactive and ignores `--json` (running `kite` with no arguments alongside `--json` exits with code `2`).

```text
$ kite --json market ltp NSE:INFY
{
  "NSE:INFY": {
    "instrument_token": 408065,
    "last_price": 1074.35
  }
}
```

```text
$ kite --json portfolio holdings | jq -r '.[] | [.tradingsymbol, .quantity, .last_price] | @tsv'
AARON	1	352.95
SBIN	16	762.45
```

> [!NOTE]
> `--json` emits Kite's raw API response. Derived values computed by `kite` for human tables—such as combined demat + MTF holding quantities and portfolio summary totals in [Portfolio](portfolio.md)—are not added to the raw JSON payload. Client-side filters like `orders list --status OPEN` still apply; when nothing matches, the output is `[]`.

### Dry runs

With `--json`, a dry run prints a single JSON object describing the exact request that `--confirm` would send:

```text
$ kite --json orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY \
    --order-type LIMIT --quantity 1 --product CNC --price 1500 --validity DAY
{
  "body": {
    "form": {
      "exchange": "NSE",
      "order_type": "LIMIT",
      "price": "1500",
      "product": "CNC",
      "quantity": "1",
      "tradingsymbol": "INFY",
      "transaction_type": "BUY",
      "validity": "DAY"
    }
  },
  "dry_run": true,
  "method": "POST",
  "next": "add --confirm to place the order",
  "path": "/orders/regular",
  "query": []
}
```

- `body` is `{"form": {...}}` for form-encoded payloads, `{"json": ...}` for JSON payloads, or `null` when there is no request body.
- Sensitive values are replaced with `"<redacted>"`, matching human dry-run output.
- When executed with `--confirm`, the output is Kite's `data` payload (for example, `{"order_id": "151220000000000"}`).

### Errors

```text
$ kite --json portfolio holdings
{
  "error": {
    "message": "not logged in; run `kite auth login`",
    "type": "MissingSession"
  },
  "ok": false
}
```

`error.type` is one of:

| Type | Meaning |
| :--- | :--- |
| `MissingSession` | No access token is saved in `config.json` or set in `KITE_ACCESS_TOKEN` |
| Kite error type (`InputException`, `NetworkException`, `GeneralException`, etc.) | Kite rejected the request; `message` contains Kite's error message, type, and HTTP status |
| `CloudflareBlocked` | Cloudflare blocked the request before it reached Kite (see [Troubleshooting — Cloudflare blocks](troubleshooting.md#cloudflare-blocks)) |
| `Error` | Any other failure, including an expired session (``session expired; run `kite auth login` ``) |

## Exit codes

| Code | Meaning |
| :--- | :--- |
| `0` | Success (includes dry runs, empty filtered lists, `No results.` from search, and a passing `kite doctor` check) |
| `1` | Runtime or API error (Kite rejected the request, missing/expired session, network failure, unreadable config file, invalid input data, or a failed `kite doctor` check) |
| `2` | CLI usage error from argument parsing (missing required flag, unknown flag/command, or running `kite` with no command outside an interactive terminal) |

`kite doctor` exits with `1` when any setup or API check fails, and sets `"ok": false` in its JSON output so scripts can inspect individual checks:

```bash
kite --json doctor | jq -e .ok
```

## `stdout` and `stderr` separation

- **`stdout`** carries only primary command output: tables, key-value records, JSON payloads, and dry-run previews.
- **`stderr`** carries diagnostics: warnings, `hint:` lines, empty-state notices (such as `No OPEN orders today.`), streaming progress banners, and human-mode error messages.

Redirecting `kite orders list > orders.txt` writes only the table data, and an empty result produces an empty file.

## Human output in pipes

Human output remains the default when piping. When `stdout` is not a TTY:

- **No ANSI colour** — Colour codes are omitted automatically.
- **No column truncation** — All table columns are printed regardless of width. (When writing to a narrow terminal TTY, low-priority columns are hidden rather than truncating numbers; set `COLUMNS` to override the detected terminal width.)
- **Clean SIGPIPE handling** — If a downstream pipe reader closes early (for example, `kite orders list | head -n 1`), `kite` terminates quietly and exits with code `0`.

Colour is also disabled whenever `NO_COLOR` is set (to any value) or `TERM=dumb`. These settings apply to CLI output only; for TUI themes, see [TUI settings](tui.md#settings-tuijson).

## Streaming JSON

`kite --json stream ticks NSE:INFY --limit 0` (or `--token 408065`) emits newline-delimited JSON—one JSON object per line as each tick arrives—until interrupted or until `--timeout-seconds` elapses. See [Market data — Live ticks](market-data.md#live-ticks).

## Raw API calls (`kite request`)

`kite request` sends authenticated requests to any Kite Connect endpoint path, useful for endpoints or parameters not wrapped by a dedicated subcommand.

```text
$ kite request get /quote/ltp -q i=NSE:INFY
content_type                             application/json
http_status                              200
response.data.NSE:INFY.instrument_token  408065
response.data.NSE:INFY.last_price        1074.35
response.status                          success
```

```text
$ kite --json request get /quote/ltp -q i=NSE:INFY
{
  "content_type": "application/json",
  "http_status": 200,
  "response": {
    "data": {
      "NSE:INFY": {
        "instrument_token": 408065,
        "last_price": 1074.35
      }
    },
    "status": "success"
  }
}
```

- **Read vs. write** — `kite request get` executes immediately. `post`, `put`, and `delete` run as dry runs until you add `--confirm`.
- **Query parameters** — Pass `-q KEY=VALUE` (repeatable).
- **Request body** — Pass `--form KEY=VALUE` (repeatable), `--body-json '<json>'`, or `--body-file <path>`.
- **Response envelope** — Successful calls return the full Kite envelope (including `status`) under `response`. HTTP errors and Kite error envelopes follow standard error formatting and exit with code `1`.
- **Path restriction** — Pass a relative Kite API path such as `/orders` (leading `/` is optional). Full URLs are rejected so your access token is never sent to a third-party host:
  ```text
  $ kite request get https://example.com/user/profile
  error: raw request path must be a Kite API path such as /orders, not a full URL
  ```

### CLI command to Kite endpoint mapping

| CLI command | Kite Connect endpoint |
| :--- | :--- |
| `kite user profile` | `GET /user/profile` |
| `kite user margins [--segment S]` | `GET /user/margins`, `GET /user/margins/:segment` |
| `kite orders list` | `GET /orders` |
| `kite orders get <id>` | `GET /orders/:order_id` |
| `kite orders trades [--order-id <id>]` | `GET /trades`, `GET /orders/:order_id/trades` |
| `kite orders place` | `POST /orders/:variety` |
| `kite orders modify` | `PUT /orders/:variety/:order_id` |
| `kite orders cancel` | `DELETE /orders/:variety/:order_id` |
| `kite portfolio holdings` | `GET /portfolio/holdings` |
| `kite portfolio positions` | `GET /portfolio/positions` |
| `kite portfolio auctions` | `GET /portfolio/holdings/auctions` |
| `kite portfolio convert` | `PUT /portfolio/positions` |
| `kite market quote`, `ohlc`, `ltp` | `GET /quote`, `GET /quote/ohlc`, `GET /quote/ltp` |
| `kite historical candles` | `GET /instruments/historical/:instrument_token/:interval` |
| `kite margins orders`, `basket`, `charges` | `POST /margins/orders`, `POST /margins/basket`, `POST /charges/orders` |
| `kite gtt list`, `get` | `GET /gtt/triggers`, `GET /gtt/triggers/:id` |
| `kite gtt place`, `modify`, `delete` | `POST /gtt/triggers`, `PUT /gtt/triggers/:id`, `DELETE /gtt/triggers/:id` |
| `kite alerts list`, `get`, `history` | `GET /alerts`, `GET /alerts/:uuid`, `GET /alerts/:uuid/history` |
| `kite alerts create`, `modify`, `enable`, `disable`, `delete` | `POST /alerts`, `PUT /alerts/:uuid`, `DELETE /alerts` |
| `kite mf orders`, `order <id>` | `GET /mf/orders`, `GET /mf/orders/:order_id` |
| `kite mf sips`, `holdings` | `GET /mf/sips`, `GET /mf/holdings` |
| `kite mf instruments`, `search` | `GET /mf/instruments` (public, no session) |
| `kite market instruments`, `cache refresh` | `GET /instruments.json` (public, no session) |
| `kite auth logout` | `DELETE /session/token` |

## Command aliases

Short aliases are provided for interactive convenience. In scripts, prefer the canonical command names.

| Canonical command | Alias |
| :--- | :--- |
| `orders` | `order` |
| `orders list` | `orders ls` |
| `orders get` | `orders history` |
| `portfolio` | `pf` |
| `portfolio holdings` | `pf h` |
| `portfolio positions` | `pf pos` |
| `market` | `m` |
| `market instruments` | `market instrument` |
| `market quote` | `market full` |
| `gtt` | `gtts` |
| `gtt list` | `gtt ls` |
| `mf` | `mutual-funds` |
| `mf orders` | `mf orderbook` |
| `mf holdings` | `mf h` |
| `stream` | `ws` |
| `request` | `api` |

---

[Back to the user guide](README.md)
