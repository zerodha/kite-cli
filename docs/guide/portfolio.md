# Portfolio and funds

This guide covers viewing holdings, net and day positions, auction holdings, and account margins, along with how `kite` computes portfolio totals and handles incomplete records.

```bash
kite portfolio holdings
kite portfolio positions
kite user margins
```

All portfolio and margin inspection commands are read-only and require an active session (see [Getting started](getting-started.md)). For position product conversion (`kite portfolio convert`), a dry-run preview is shown unless `--confirm` is passed.

## Holdings

```text
$ kite portfolio holdings
INSTRUMENT    QTY     AVG     LTP    CUR VALUE           P&L   NET CHG  DAY CHG
NSE:AARON   1,001  161.00  352.95  3,53,302.95  +1,92,141.95  +119.22%   +0.17%
BSE:SBIN       16  801.78  762.45    12,199.20       -629.30    -4.91%   -0.52%
───────────────────────────────────────────────────────────────────────────────
Total                              3,65,502.15  +1,91,512.65  +110.07%   +0.15%

Invested ₹1,73,989.50 · Current ₹3,65,502.15 · Day's P&L +₹537.40
```

### Column definitions

| Column | Calculation |
| :--- | :--- |
| `QTY` | Total quantity: `free quantity + T1 quantity + collateral (pledged) quantity + MTF quantity` |
| `AVG`, `LTP` | Kite's `average_price` and `last_price` |
| `CUR VALUE` | `LTP × QTY` |
| `P&L` | `(LTP − AVG) × QTY` |
| `NET CHG` | `P&L ÷ (AVG × QTY)`, expressed as a percentage |
| `DAY CHG` | Kite's `day_change_percentage` |

> [!NOTE]
> `QTY` can exceed the raw `quantity` field in Kite's API response. In the example above, `NSE:AARON` has 1 free share in demat and 1,000 shares held under MTF, giving a total `QTY` of `1,001`. `kite` recomputes `P&L` from the combined quantity rather than using Kite's `pnl` field (which only reflects free quantity).

### Portfolio totals

The `Total` row and summary footer aggregate all holdings that have a valid P&L:

- **Invested** — Sum of `AVG × QTY` across included holdings.
- **Current** — Sum of `LTP × QTY` across included holdings.
- **Day's P&L** — Sum of `(LTP − close_price) × QTY` (falling back to Kite's per-share `day_change` when `close_price` is unavailable).
- **Total `DAY CHG`** — Day's P&L as a percentage of the previous close value.

When you have no holdings, `kite` prints `No holdings.` on `stderr` and nothing on `stdout`.

### Warning: excluded holdings

A holding cannot have its P&L computed when Kite flags it as discrepant, when its `last_price` is zero, or when a required field is missing. In these cases, `kite` displays `–` in the affected columns, excludes the row from portfolio totals, and prints a warning on `stderr`:

```text
$ kite portfolio holdings
INSTRUMENT    QTY     AVG     LTP    CUR VALUE           P&L   NET CHG  DAY CHG
NSE:AARON   1,001  161.00  352.95  3,53,302.95  +1,92,141.95  +119.22%   +0.17%
BSE:SBIN       16  801.78  762.45          –             –        –      -0.52%
───────────────────────────────────────────────────────────────────────────────
Total                              3,53,302.95  +1,92,141.95  +119.22%   +0.17%

Invested ₹1,61,161.00 · Current ₹3,53,302.95 · Day's P&L +₹600.60
warning: excluded 1 incomplete, discrepant or unpriced holdings from totals
```

This matches Kite Web's behaviour. Check the flagged holding in Kite Web or Console if it requires a discrepancy resolution (such as entering an off-market transfer price).

## Positions

```text
$ kite portfolio positions
Net positions
PRODUCT  INSTRUMENT              QTY     AVG        LTP      P&L
NRML     MCX:LEADMINI17DECFUT      1  161.05     161.05     0.00
NRML     MCX:GOLDGUINEA17DECFUT    0    0.00  23,355.00  +801.00
CO       NSE:SBIN                  0    0.00     308.40    -2.00
────────────────────────────────────────────────────────────────
Total                                                    +799.00

Day positions
PRODUCT  INSTRUMENT              QTY        AVG        LTP     P&L
NRML     MCX:GOLDGUINEA17DECFUT   -3  23,340.00  23,355.00  -93.00
NRML     MCX:LEADMINI17DECFUT      1     161.05     161.05    0.00
CO       NSE:SBIN                  0       0.00     308.40   -2.00
──────────────────────────────────────────────────────────────────
Total                                                       -95.00
```

- **Net positions** include overnight positions carried forward from previous sessions.
- **Day positions** show activity from today's trading session only.
- `P&L` displays Kite's `pnl` field for each row, and `Total` is the sum of that column.
- A row with `QTY` of `0` represents a closed position (dimmed on colour terminals). A negative `QTY` indicates a short position.

## Convert a position

`kite portfolio convert` converts an open position's product type (for example, between `MIS` and `CNC`). Because it modifies your account state, it runs as a dry run until you add `--confirm`:

```text
$ kite portfolio convert --exchange NSE --tradingsymbol INFY --transaction-type BUY \
    --position-type day --quantity 1 --old-product MIS --new-product CNC
DRY RUN  nothing was sent to Kite

PUT /portfolio/positions
exchange          NSE
tradingsymbol     INFY
transaction_type  BUY
quantity          1
new_product       CNC
old_product       MIS
position_type     day
hint: add --confirm to convert the position
```

See [Orders — Dry run by default](orders.md#dry-run-by-default) for details on write previews.

## Auction holdings

`kite portfolio auctions` lists holdings eligible for sale in an exchange auction, including the `AUCTION_NUMBER` required when placing an auction order:

```text
$ kite portfolio auctions
AUCTION_NUMBER  EXCHANGE  TRADINGSYMBOL  QUANTITY  AVERAGE_PRICE  LAST_PRICE    PNL
20              NSE       ASHOKLEY              1         131.95      142.50  10.55
34              NSE       BHEL                  5          75.95       81.10  25.75
7529            NSE       SBIN                  3         573.40      593.75  61.05
```

## Funds and margins

```text
$ kite user margins
                       EQUITY    COMMODITY
Available margin    99,725.05  1,00,661.70
Used margin       1,45,706.55         0.00
Opening balance   2,45,431.60  1,00,661.70
Cash              2,45,431.60  1,00,661.70
Collateral               0.00         0.00
Payin                    0.00         0.00
SPAN              1,01,989.00         0.00
Exposure            38,981.25         0.00
Options premium          0.00         0.00
Delivery                 0.00         0.00
M2M realised           761.70         0.00
M2M unrealised           0.00         0.00
```

- **Available margin** — Kite's `net` figure (funds available for new trades).
- **Used margin** — Kite's `utilised.debits`.
- **Breakdown rows** — Populated from Kite's `available` and `utilised` margin objects.
- Pass `--segment equity` or `--segment commodity` to view a single segment.

To calculate required margins and statutory charges before placing an order or basket, see [Orders — Margins and charges](orders.md#margins-and-charges).

## Mutual fund holdings

Mutual fund holdings are queried via `kite mf holdings`. See [Mutual funds](mutual-funds.md).

---

[Back to the user guide](README.md)
