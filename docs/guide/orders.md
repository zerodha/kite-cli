# Orders

This guide covers viewing, placing, modifying, and cancelling orders, managing Good Till Triggered (GTT) triggers, converting positions, and calculating margins and brokerage charges before trading.

```bash
kite orders list
kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY \
    --order-type LIMIT --quantity 1 --product CNC --price 1500 --confirm
```

## Dry run by default

All state-changing trading and API write commands print the exact request they would send—without sending anything to Kite—until you pass `--confirm`:

- `orders place`, `orders modify`, `orders cancel`
- `gtt place`, `gtt modify`, `gtt delete`
- `portfolio convert`
- `alerts create`, `alerts modify`, `alerts enable`, `alerts disable`, `alerts delete`
- `request post`, `request put`, `request delete`

### Dry-run rules

- **Without `--confirm`** — The command prints a preview and exits with code `0`.
- **With `--confirm`** — The command sends the live request to Kite.
- **`--dry-run` always takes precedence** — Passing both `--dry-run` and `--confirm` performs a dry run. Use this in scripts to inspect the exact payload that a subsequent `--confirm` run will execute.
- **Offline previews** — Most dry runs do not read `config.json` and do not require an active session (exceptions that preserve existing server state—such as `gtt modify` without `--type`, `alerts modify`, `alerts enable`, and `alerts disable`—fetch the existing record first and therefore require a session).
- **Authentication commands** — `kite auth login` and `kite auth logout` take effect immediately and do not use `--confirm` (see [Getting started — Sign out](getting-started.md#sign-out)).

### Reading a dry-run preview

```text
$ kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY \
    --order-type LIMIT --quantity 1 --product CNC --price 1500 --validity DAY
DRY RUN  nothing was sent to Kite
BUY 1 NSE:INFY  LIMIT @ 1,500.00  CNC  DAY

POST /orders/regular
exchange          NSE
tradingsymbol     INFY
transaction_type  BUY
order_type        LIMIT
quantity          1
product           CNC
price             1500
validity          DAY
hint: add --confirm to place the order
```

1. **`DRY RUN  nothing was sent to Kite`** confirms that no network write occurred.
2. **Order summary** — Shows the side, quantity, instrument, order type, price, product, and validity on a single line.
3. **HTTP method and endpoint** — For example, `POST /orders/regular` for the `regular` variety.
4. **Request parameters** — Every form field that `--confirm` will send, exactly as Kite receives it. Optional flags you omit are not sent.
5. **Hint** — Printed on `stderr`, reminding you which flag executes the request.

Sensitive fields (tokens, passwords, checksums, API secrets, and API keys) are automatically replaced with `<redacted>` in dry-run output, while non-secret fields like `instrument_token` remain visible:

```text
$ kite request post /session/token --form api_key=my-key --form access_token=secret-token \
    --form instrument_token=408065
DRY RUN  nothing was sent to Kite

POST /session/token
access_token      <redacted>
api_key           <redacted>
instrument_token  408065
hint: add --confirm to send the raw write request
```

With `--json`, a dry run emits a single structured JSON object. See [Scripting — Dry runs](scripting.md#dry-runs).

## Read orders and trades

```text
$ kite orders list
TIME              SIDE  INSTRUMENT             TYPE    PRODUCT      QTY       PRICE  STATUS     ORDER ID
2021-05-31 09:18  BUY   CDS:USDINR21JUNFUT     LIMIT   NRML         0/1     72.0000  CANCELLED  100000000000000
2021-05-31 15:20  BUY   NSE:IOC                LIMIT   CNC          1/1    109.40    COMPLETE   300000000000000
2021-05-31 15:20  SELL  NSE:IOC                MARKET  CNC          1/1    109.35    COMPLETE   500000000000000
2022-05-24 12:26  BUY   NSE:SBIN               LIMIT   CNC        0/200    463.00    REJECTED   220524001859672
2021-05-31 16:00  BUY   MCX:GOLDPETAL21JUNFUT  LIMIT   NRML         1/1  4,852.00    COMPLETE   700000000000000
2021-05-31 16:08  BUY   MCX:GOLDPETAL21JUNFUT  LIMIT   NRML         1/1  4,852.00    COMPLETE   9000000000000000
2023-06-12 14:00  SELL  NSE:BHEL               LIMIT   CNC         0/60     85.00    CANCELLED  98000000000000000
2025-01-17 11:49  BUY   NSE:SBIN               LIMIT   MTF          0/1    702.00    CANCELLED  250117800776785
2025-08-07 11:56  BUY   NSE:TATASTEEL          LIMIT   CNC          1/1    156.70    COMPLETE   1953341975595868160
2025-08-07 13:38  BUY   NFO:NIFTY2580724400CE  MARKET  NRML     150/150     12.30    COMPLETE   1953367517686685697

Rejected orders
220524001859672  Insufficient funds. Required margin is 95417.84 but available margin is 74251.80. Check the orderbook for open orders.
```

- **`QTY`** — Shown as `filled_quantity / total_quantity`.
- **`TIME`** — Shows only the timestamp when all orders are from today, or includes the date when orders span multiple dates.
- **`Rejected orders`** — Lists the rejection reason returned by Kite/the exchange for each rejected order.
- **Filtering** — Pass `--status <STATUS>` or `--tradingsymbol <SYMBOL>` to filter orders locally:
  - `orders list --status OPEN` matches all working orders (including `OPEN`, `TRIGGER PENDING`, and request-pending states).
  - Any other `--status` value matches that exact status (case-insensitive).
  - When no orders match the filter, `kite` prints a message on `stderr` (for example, `No OPEN orders today.`) and nothing on `stdout`.

### Order history and fills

- `kite orders get <order_id>` — Displays the full state-transition history of a single order.
- `kite orders trades` — Lists all executed fills for today.
- `kite orders trades --order-id <order_id>` — Lists executed fills for a specific order.

## Place an order

Required flags: `--exchange`, `--tradingsymbol`, `--transaction-type`, `--order-type`, `--quantity`, and `--product`.

| Flag | Supported values |
| :--- | :--- |
| `--variety` | `regular` (default), `amo`, `co`, `iceberg`, `auction` |
| `--exchange` | `NSE`, `BSE`, `NFO`, `BFO`, `CDS`, `BCD`, `MCX`, `NCO` |
| `--transaction-type` | `BUY`, `SELL` |
| `--order-type` | `MARKET`, `LIMIT`, `SL`, `SL-M` |
| `--product` | `CNC`, `NRML`, `MIS`, `MTF`, `CO`, `BO` |
| `--validity` | `DAY`, `IOC`, `TTL` |

**Optional flags:** `--price`, `--trigger-price`, `--disclosed-quantity`, `--validity-ttl`, `--iceberg-legs`, `--iceberg-quantity`, `--auction-number`, `--market-protection`, `--autoslice`, and `--tag`. Run `kite orders place --help` for descriptions of each flag.

**Parameter validation rules:**
- Prices must be finite and non-negative.
- Iceberg legs (`--iceberg-legs`) must be between `2` and `50`.
- Order tags (`--tag`) must be `1` to `20` ASCII alphanumeric characters, following the [Kite Connect order parameters specification](https://kite.trade/docs/connect/v3/orders/#regular-order-parameters).

When Kite accepts the order:

```text
$ kite orders place --exchange NSE --tradingsymbol INFY --transaction-type BUY \
    --order-type LIMIT --quantity 1 --product CNC --price 1500 --validity DAY --confirm
✓ Order submitted  BUY 1 NSE:INFY  LIMIT @ 1,500.00  CNC  DAY
  Order ID  151220000000000
hint: check its status with `kite orders get 151220000000000`
```

> [!IMPORTANT]
> **"Order submitted"** means Kite accepted the order request for processing—it does not guarantee that the exchange accepted the order or that it has been filled. Always verify execution status with `kite orders get <order_id>` or `kite orders list`.

## Modify and cancel

```bash
kite orders modify --order-id 151220000000000 --quantity 2 --price 1490
kite orders cancel --order-id 151220000000000
```

Both commands run as dry runs until you pass `--confirm`. `--variety` defaults to `regular`; specify `--variety` if the order belongs to a different variety (`amo`, `co`, `iceberg`, `auction`).

`kite orders modify` sends only the fields you explicitly provide:

```text
$ kite orders modify --order-id 151220000000000 --quantity 2 --price 1490
DRY RUN  nothing was sent to Kite

PUT /orders/regular/151220000000000
quantity  2
price     1490
hint: add --confirm to modify the order
```

```text
$ kite orders cancel --order-id 151220000000000 --confirm
✓ Order cancellation submitted
  Order ID  151220000000000
hint: check its status with `kite orders get 151220000000000`
```

## Rejections, timeouts, and errors

- **API rejections** — When Kite rejects a request immediately, `kite` prints Kite's error message, error type, and HTTP status on `stderr`, and exits with code `1`:
  ```text
  error: Insufficient funds. Required margin is 95417.84 but available margin is 74251.80. (InputException, HTTP 400)
  ```
  `kite` never automatically retries a rejected write. An order rejected asynchronously by the exchange after submission appears as `REJECTED` in `kite orders list` with its rejection reason.
- **Connection drops or timeouts on writes** — When a confirmed write (`--confirm`) loses its connection or times out, its outcome is **unknown** because Kite may have received and processed the request before the connection dropped. Always inspect `kite orders list`, `kite gtt list`, or `kite portfolio positions` before retrying.
- **Missing required flags** — Flag validation runs locally before any network request. Missing or invalid arguments print a usage error on `stderr` and exit with code `2`:
  ```text
  $ kite orders place --exchange NSE --tradingsymbol INFY
  error: the following required arguments were not provided:
    --transaction-type <TRANSACTION_TYPE>
    --order-type <ORDER_TYPE>
    --quantity <QUANTITY>
    --product <PRODUCT>

  Usage: kite orders place --exchange <EXCHANGE> --tradingsymbol <TRADINGSYMBOL> --transaction-type <TRANSACTION_TYPE> --order-type <ORDER_TYPE> --quantity <QUANTITY> --product <PRODUCT>

  For more information, try '--help'.
  ```

## GTT triggers

A Good Till Triggered (GTT) rule places one or two limit orders when the market price reaches your trigger condition.

```text
$ kite gtt list
ID      INSTRUMENT  TYPE             TRIGGER  ORDERS                         STATUS     CREATED     EXPIRES
112127  NSE:INFY    single            702.00  BUY 1 @ 702.50                 active     2019-09-12  2020-09-12
105099  NSE:RAIN    two-leg  102.00 / 103.70  SELL 1 @ 1.00 / SELL 1 @ 1.00  triggered  2019-09-09  2020-01-01

Failed GTT orders
105099  leg 2: Your order price is lower than the current lower circuit limit of 70.65. Place an order within the daily range.
```

Use `kite gtt get <trigger_id>` to inspect a single trigger.

### Placing, modifying, and deleting GTTs

To place a GTT trigger, supply the condition as a JSON object and the orders as a JSON array, either inline (`--condition-json`, `--orders-json`) or from files (`--condition-file`, `--orders-file`):

```text
$ kite gtt place --type single \
    --condition-json '{"exchange":"NSE","tradingsymbol":"INFY","trigger_values":[702.0],"last_price":798.0}' \
    --orders-json '[{"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","quantity":1,"order_type":"LIMIT","product":"CNC","price":702.5}]'
DRY RUN  nothing was sent to Kite

POST /gtt/triggers
condition  {"exchange":"NSE","last_price":798.0,"tradingsymbol":"INFY","trigger_values":[702.0]}
orders     [{"exchange":"NSE","order_type":"LIMIT","price":702.5,"product":"CNC","quantity":1,"tradingsymbol":"INFY","transaction_type":"BUY"}]
type       single
hint: add --confirm to place the GTT trigger
```

With `--confirm`:

```text
✓ GTT submitted
  Trigger ID  123
```

- **`--type`** — Supports `single` (default for `gtt place`), `two-leg`, `trailing-single`, or `trailing-two-leg`.
- **Trailing GTTs** — Require `--trailing-points <points>`, unless `trailing_points` is already present in the condition JSON.
- **Modifying a GTT (`kite gtt modify --trigger-id <id>`)** — Accepts the same condition and order flags as `gtt place`. When `--type` is omitted, `gtt modify` reads the existing GTT from Kite (including during a dry run, which therefore requires a session) to preserve its stored trigger type. Pass `--type` to change the trigger type explicitly.
- **Deleting a GTT (`kite gtt delete --trigger-id <id>`)** — Deletes the trigger (dry run unless `--confirm` is passed).

## Convert a position

`kite portfolio convert` converts an open position's product (for example, from `MIS` to `CNC`). It runs as a dry run until `--confirm` is passed; with `--confirm`, `kite` prints `✓ Position conversion submitted` and the submitted fields. See [Portfolio — Convert a position](portfolio.md#convert-a-position) for an example.

## Margins and charges

Margin and charge commands are read-only calculators. They accept orders in any of three formats: `--file orders.json`, `--input-json '[...]'`, or one or more `--order '{...}'` flags.

```bash
kite margins orders --order '{"exchange":"NSE","tradingsymbol":"INFY","transaction_type":"BUY","variety":"regular","product":"CNC","order_type":"MARKET","quantity":1,"price":0,"trigger_price":0}'
kite margins basket --file basket.json --consider-positions
kite margins charges --file charges.json
```

- **`kite margins orders`** — Calculates required margin for each order individually.
- **`kite margins basket`** — Calculates combined margin for a basket of orders. Pass `--consider-positions` to net against your existing open positions. Returns `initial` (un-netted margin), `final` (netted margin), `charges`, and `orders` (per-order breakdown).
- **`kite margins charges`** — Calculates brokerage, STT/CTT, exchange turnover charges, GST, SEBI charges, and stamp duty for each order.
- **`--compact`** — Supported by `margins orders` and `margins basket` to request Kite's compact margin response.

In human output, margin and charge values are formatted to two decimal places:

```text
[0].charges.total                     1.79255122
[0].charges.transaction_tax           1.498
[0].charges.transaction_tax_type      stt
[0].exchange                          NSE
```

With `--json`, raw numeric precision from Kite's API is preserved.

## Rate limits

Kite enforces order rate limits of **10 requests per second**, **400 per minute**, and **5,000 per day**, with a maximum of **25 modifications per order**. Exceeding a rate limit returns `HTTP 429`; see [Troubleshooting — Rate limits](troubleshooting.md#rate-limits).

---

[Back to the user guide](README.md)
