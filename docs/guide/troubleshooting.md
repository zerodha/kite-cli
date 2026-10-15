# Troubleshooting

This guide explains common errors and how to diagnose and resolve them. When troubleshooting, always start with `kite doctor`:

```bash
kite doctor
kite config show
kite market cache info
```

`kite doctor` validates your configuration file, credentials, and performs a single read-only API check (`GET /user/margins`), reporting any failed step with a remediation hint.

In human mode, errors are printed to `stderr` prefixed with `error:`. With `--json`, errors are printed to `stdout` as `{"ok": false, "error": {...}}` (see [Scripting — Errors](scripting.md#errors)).

## Session expired

```text
error: Invalid `api_key` or `access_token`. (TokenException, HTTP 403)
hint: run `kite auth login` to sign in again
```

With `--json`:

```json
{
  "error": {
    "message": "session expired; run `kite auth login`",
    "type": "Error"
  },
  "ok": false
}
```

- **Cause** — Kite access tokens expire daily around **06:00 IST**, or earlier if you sign out from another session. `kite` does not renew tokens automatically.
- **Fix** — Run `kite auth login`.
- **Still failing after login?** — Check whether `KITE_ACCESS_TOKEN` is set in your environment (`kite config show`), as it overrides the token saved in `config.json` (see [Configuration — `KITE_ACCESS_TOKEN`](configuration.md#kite_access_token)).
- **In the TUI** — The status bar displays `○ auth expired` and automatically opens the sign-in screen.

## Not logged in

```text
error: not logged in; run `kite auth login`
hint: run `kite auth login` to sign in again
```

- **Cause** — No access token is present in `config.json` or `KITE_ACCESS_TOKEN`.
- **Fix** — Run `kite auth login`. If you use `--config` or `KITE_CONFIG`, run `kite config path` to verify that you are pointing to the same file used during login.

## Sign-in problems

### Non-interactive terminal error

```text
error: interactive `auth login` cannot run under --json or without a TTY; run it in an interactive terminal
```

`kite auth login` requires an interactive terminal for password and 2FA prompts. Run it directly in a terminal session without `--json` or piped `stdin`.

### Missing API configuration (source builds)

```text
error: no API root configured; set KITE_API_ROOT or "api_root" in config.json
```

Source builds without embedded build-time settings require `api_root`, `api_key`, and `ticker_url`. Configure them via environment variables or `config.json` as described in [Getting started — From source](getting-started.md#from-source).

### Rejected credentials

When Kite rejects your user ID, password, or 2FA code, `kite` prints Kite's message, error type (`UserException`, `TwoFAException`, etc.), and HTTP status. Your password and 2FA code are never echoed in error messages.

## Rate limits

Kite enforces the following rate limits across endpoints:

| Endpoint category | Rate limit |
| :--- | :--- |
| **Quotes** | 1 request / second |
| **Historical candles** | 3 requests / second |
| **Order placement** | 10 requests / second · 400 / minute · 5,000 / day |
| **Order modification** | 25 modifications per order |
| **Other REST endpoints** | 10 requests / second |
| **WebSocket ticker** | Up to 3,000 instruments per connection · 3 connections per API key |

Exceeding a rate limit returns `HTTP 429`. `kite` prints Kite's error message, exits with code `1`, and does not retry automatically. In scripts, add a delay between requests. In the TUI, polling intervals are controlled by `poll_secs` in `tui.json`.

## Cloudflare blocks

```text
error: Cloudflare blocked the request (HTTP 403, Ray 8a1b2c3d4e5f6789-BOM)
```

With `--json`, the error type is `CloudflareBlocked`. This indicates that Cloudflare rejected the request before it reached Kite's servers. `kite` does not retry automatically—wait and try again, or test from a different network connection. If the block persists, include the **Ray ID** from the error message when reporting the issue.

## Network errors

```text
error: request failed: error sending request: client error (Connect): tcp connect error: Connection refused (os error 111)
```

- **CLI** — Verify your internet connection and check any custom `KITE_API_ROOT` or `api_root` value using `kite config show`. `kite doctor` reports this as `✗ Kite API   request failed`.
- **TUI** — On transient network failures, the TUI keeps the last known data on screen, displays a status toast, and retries automatically. The WebSocket ticker reconnects with exponential backoff while showing `◌ reconnecting` in the header.
- **Tick streaming (`kite stream ticks`)** — Automatically reconnects if dropped. If it cannot establish an initial connection before `--timeout-seconds` elapses, it exits with `0 ticks in 30.0s`; verify `ticker_url` with `kite config show`.

## Instrument cache issues

Symbol lookups (`kite market resolve`, `kite market search`, `kite stream ticks NSE:INFY`, and the TUI's search and option chain) use the local instrument cache (`~90 MB`), which refreshes automatically after **07:00 IST** each day.

- **Symbol not found** — Check the exchange prefix and spelling with `kite market search`. Newly listed instruments appear after the 07:00 IST master publication; run `kite market cache refresh` to force an immediate download.
- **Slow or failed download** — The first symbol lookup of the day waits for the cache refresh. Run `kite market cache info` to inspect cache freshness, or `kite market cache refresh` on a stable connection.
- **Corrupted cache file** — Run `kite market cache clear` to delete the cache; the next lookup will download a fresh copy.

See [Market data — The cache](market-data.md#the-cache) for details.

## Terminal colours and display issues

- **No colour in CLI output** — Colour is disabled automatically when `stdout` is piped, when `NO_COLOR` is set, or when `TERM=dumb`.
- **Disable CLI colour** — Export `NO_COLOR=1`.
- **Incorrect TUI colours** — The TUI uses its own theme engine and ignores `NO_COLOR`. Press `T` inside the TUI to switch themes, or select the `system` theme to inherit your terminal emulator's native palette. See [TUI — Themes](tui.md#themes).
- **Missing glyphs (`?` or boxes)** — `kite` uses Unicode symbols (`₹`, `–`, `✓`, `▶`) and box-drawing characters. Ensure your terminal uses a UTF-8 locale and a font with Unicode coverage.
- **Missing table columns** — On narrow terminals, `kite` hides lower-priority columns rather than truncating numbers. Widen your terminal window, set `COLUMNS`, or pipe the output (`kite orders list | cat`) to print all columns.

## Common messages reference

| Message | Meaning & resolution |
| :--- | :--- |
| `DRY RUN  nothing was sent to Kite` | Write command executed without `--confirm`; no changes were sent (see [Orders — Dry run by default](orders.md#dry-run-by-default)) |
| `hint: add --confirm to place the order` | Pass `--confirm` to execute the previewed write request |
| `warning: no quote returned for NSE:RELIANCE` | Kite returned no quote for that symbol; verify the exchange and trading symbol |
| `warning: excluded 1 incomplete, discrepant or unpriced holdings from totals` | A holding is missing a price or flagged as discrepant and was excluded from totals (see [Portfolio — Excluded holdings](portfolio.md#warning-excluded-holdings)) |
| `error: at least one instrument is required, for example NSE:INFY` | Pass at least one `EXCHANGE:SYMBOL` argument to `market ltp`, `ohlc`, or `quote` |
| `error: raw request path must be a Kite API path such as /orders, not a full URL` | Pass a relative API path (such as `/orders`) to `kite request`, not a full URL |
| `No OPEN orders today.` | No orders matched your `--status` filter (exits with code `0`) |
| `could not decode the response from /path` | Kite's response shape differed from expectations; inspect the raw payload with `kite request get /path` |
| `error: the following required arguments were not provided:` | Missing required CLI flag (exits with code `2`); run the command with `--help` |

## Reporting a problem

When filing an issue, include the output of `kite --version` and `kite doctor`, the exact command or TUI steps, and the full error message. **Never include your password, two-factor codes, or the contents of `config.json`.**

---

[Back to the user guide](README.md)
