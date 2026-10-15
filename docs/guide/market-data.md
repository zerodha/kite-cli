# Market data

This guide covers fetching live prices (LTP, OHLC, and full quotes with 5-level market depth), historical candle data, instrument search and caching, and real-time WebSocket tick streaming.

```bash
kite market ltp NSE:INFY NSE:RELIANCE
kite market resolve NSE:INFY
kite stream ticks NSE:INFY --limit 5
```

Specify instruments in `EXCHANGE:TRADINGSYMBOL` format (for example, `NSE:INFY` or `NFO:NIFTY24JANFUT`). All live market data commands require an active session, except local instrument search and cache commands.

## Last price, OHLC, and quote

Pass one or more instruments as positional arguments or via repeated `-i` / `--instrument` flags. At least one instrument is required.

```text
$ kite market ltp NSE:INFY
INSTRUMENT       LTP  TOKEN
NSE:INFY    1,074.35  408065
```

```text
$ kite market ohlc NSE:INFY
NSE:INFY  1,075.00 -0.80 -0.07%

    OPEN      HIGH       LOW     CLOSE  VOLUME  AVG PRICE
1,085.80  1,085.90  1,070.90  1,075.80       –          –
```

`kite market quote` includes volume, last trade details, total buy/sell quantities, lower/upper circuit limits, and 5 levels of market depth:

```text
$ kite market quote NSE:INFY
NSE:INFY  1,412.95 +23.30 +1.68%

    OPEN      HIGH       LOW     CLOSE     VOLUME  AVG PRICE
1,396.00  1,421.75  1,395.55  1,389.65  73,60,198   1,412.47

Last trade  5 @ 15:45:52
Buy / sell  0 / 5,191
Circuits    1,250.70 – 1,528.60
Updated     2021-06-08 15:45:56 IST

BID QTY  ORDERS  BID       ASK  ORDERS  ASK QTY
      –       –    –  1,412.95      13    5,191
```

- The net change on the header line is computed against the previous close. A `–` indicates that Kite returned no value for that field.
- If Kite returns no data for a requested symbol (for example, due to a typo), `kite` prints the valid quotes and emits a warning on `stderr`:
  ```text
  $ kite market ltp NSE:INFY NSE:RELIANCE
  INSTRUMENT       LTP  TOKEN
  NSE:INFY    1,074.35  408065
  warning: no quote returned for NSE:RELIANCE
  ```
- **API limits** — Up to `500` instruments per `quote` call, `1,000` instruments per `ltp` or `ohlc` call, and `1` quote request per second.
- **Legacy alias** — `kite quotes ltp|ohlc|full` is supported as an alias for `kite market ltp|ohlc|quote`.

## Historical candles

Historical candle queries take a numeric `--instrument-token` rather than a trading symbol. Look up the token first using `kite market resolve`.

```text
$ kite historical candles --instrument-token 408065 --interval minute \
    --from "2017-12-15 09:15:00" --to "2017-12-15 09:20:00"
TIME                          OPEN      HIGH       LOW     CLOSE  VOLUME  OI
2017-12-15T09:15:00+0530  1,704.50  1,705.00  1,699.25  1,702.80   2,499   –
2017-12-15T09:16:00+0530  1,702.00  1,702.00  1,698.15  1,698.15   1,271   –
2017-12-15T09:17:00+0530  1,698.15  1,700.25  1,698.00  1,699.25     831   –
2017-12-15T09:18:00+0530  1,700.00  1,700.00  1,698.30  1,699.00     771   –
2017-12-15T09:19:00+0530  1,699.00  1,700.00  1,698.10  1,699.80     543   –
2017-12-15T09:20:00+0530  1,699.80  1,700.00  1,696.55  1,696.90     802   –
```

- **`--interval`** — One of `minute`, `3minute`, `5minute`, `10minute`, `15minute`, `30minute`, `60minute`, or `day`.
- **`--from` and `--to`** — Formatted as `yyyy-mm-dd hh:mm:ss`.
- **`--oi`** — Includes Open Interest (`OI`) for futures and options contracts.
- **`--continuous`** — Requests continuous historical data for futures contracts.
- **API limits** — Kite allows up to `3` historical requests per second and enforces maximum date-range spans per interval.

## Instruments: search, resolve, and the cache

`kite` stores Kite's daily master list of tradable instruments in a local cache file. `search` and `resolve` query this local cache and do not require an authenticated session.

```text
$ kite market resolve NSE:INFY
EXCHANGE  TRADINGSYMBOL  NAME     INSTRUMENT_TOKEN  INSTRUMENT_TYPE  SEGMENT  EXPIRY  STRIKE  TICK_SIZE  LOT_SIZE
NSE       INFY           INFOSYS            408065  EQ               NSE      –            –       0.05         1
```

```text
$ kite market search BANK --exchange NSE --limit 3
EXCHANGE  TRADINGSYMBOL  NAME                    INSTRUMENT_TOKEN  INSTRUMENT_TYPE  SEGMENT  EXPIRY  STRIKE  TICK_SIZE  LOT_SIZE
NSE       BANKBEES       RELIANCE ETF BANK BEES           2928385  EQ               NSE      –         0.00       0.01         1
NSE       BANKBARODA-BE  BOB - DEPO SETT                  1219329  EQ               NSE      –         0.00       0.05         1
NSE       CENTRALBK-BE   CENTRAL BANK OF INDIA            3813889  EQ               NSE      –         0.00       0.05         1
```

- **`kite market resolve <SYMBOL>`** — Accepts `INFY` or `NSE:INFY` (use `--exchange` to disambiguate a bare symbol). Exits with code `1` and an empty match list if no instrument matches.
- **`kite market search <QUERY>`** — Matches against `tradingsymbol`, instrument `name`, and known aliases (does not match numeric tokens). Filter by exchange with `--exchange NSE` or by prefixing the query as `NSE:BANK`. Exits with code `0` and prints `No results.` on `stderr` when nothing matches.
- **`kite market instruments`** — Lists cached instruments (`50` rows by default). Pass `--out instruments.json` to export the full instrument master to a file.
- **Row limits (`--limit`)** — `resolve` and `search` return up to `20` rows by default (`50` for `instruments`). Pass `--limit 0` to return all matching rows without truncation.

### The cache

Kite publishes a refreshed instrument master every morning around **07:00 IST**. The local cache is considered fresh if downloaded after the most recent 07:00 IST boundary, and stale otherwise. When a command needs instrument data and the cache is missing or stale, `kite` automatically downloads the updated list (~90 MB, no session required).

```text
$ kite market cache info
exists                    true
fresh_until_unix_seconds  1792027800
modified_unix_seconds     1791986679
path                      /home/user/.cache/kite/instruments.json
record_count              100
size_bytes                24516
stale                     false
```

Human cache output formats timestamps in IST; `--json` output preserves raw Unix timestamps.

| Command | Description |
| :--- | :--- |
| `kite market cache path` | Print the local cache file path |
| `kite market cache info` | Show file size, record count, freshness status, and expiry timestamp |
| `kite market cache refresh` | Force an immediate download of the instrument master |
| `kite market cache clear` | Delete the cached file (`cleared` is `false` if the file did not exist) |

**Cache locations:**
- **Linux:** `~/.cache/kite/instruments.json` (or `$XDG_CACHE_HOME/kite/instruments.json`)
- **macOS:** `~/Library/Caches/kite/instruments.json`

## Live ticks

`kite stream ticks` connects to Kite's WebSocket ticker and streams live market ticks as they arrive:

```text
$ kite stream ticks NSE:INFY --mode full --limit 3
Streaming NSE:INFY in full mode; stops after 3 ticks (--limit 0 for no limit)
TIME      INSTRUMENT         LTP         CHG      CHG%        VOLUME     BUY QTY    SELL QTY
10:41:27  NSE:INFY      1,573.70       +5.90    +0.38%     11,92,471    2,56,443    3,63,009
10:41:27  NSE:INFY      1,573.70       +5.90    +0.38%     11,92,471    2,56,443    3,63,009
10:41:27  NSE:INFY      1,573.70       +5.90    +0.38%     11,92,471    2,56,443    3,63,009
3 ticks in 0.7s
```

- **Specifying instruments** — Pass one or more `EXCHANGE:SYMBOL` arguments (resolved via the local instrument cache) or numeric tokens via repeated `--token <TOKEN>` flags (bypasses the cache). At least one instrument or token is required.
- **`--mode`** — `ltp`, `quote` (default), or `full` (adds 5-level market depth and exchange timestamps).
- **`--limit`** — Stops after receiving `N` ticks (default `10`). Pass `--limit 0` to stream indefinitely.
- **`--timeout-seconds`** — Stops after `N` seconds (default `30`). Pass `0` for no timeout.
- **Stream resiliency** — Status banners print to `stderr` while tick rows print to `stdout`. If the WebSocket connection drops, `kite` reconnects automatically. Kite supports up to `3,000` instruments per WebSocket connection.

### Streaming JSON (`jq` pipelines)

When combined with `--json` and `--limit 0`, `kite` emits newline-delimited JSON (one JSON object per line as each tick arrives), ideal for piping into `jq`:

```text
$ kite --json stream ticks --token 408065 --mode full --limit 0 --timeout-seconds 1 \
    | jq -c '{instrument_token, last_price, volume}'
{"instrument_token":408065,"last_price":1573.7,"volume":1192471}
{"instrument_token":408065,"last_price":1573.7,"volume":1192471}
{"instrument_token":408065,"last_price":1573.7,"volume":1192471}
```

With a non-zero `--limit`, `--json` collects the requested ticks and outputs a single summary object (`{"count", "mode", "ticks": [...]}`) upon completion.

For an interactive live watchlist with candlestick charts and market depth, launch the [Terminal UI (TUI)](tui.md).

---

[Back to the user guide](README.md)
