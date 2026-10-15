# Mutual funds

This guide covers viewing mutual fund holdings, recent orders, active and paused SIPs, and searching mutual fund schemes.

```bash
kite mf holdings
kite mf sips
kite mf search "Index Fund"
```

> [!NOTE]
> `kite` provides **read-only** access to mutual fund data. It does not place, modify, or cancel mutual fund orders or SIPs.

## Holdings

```text
$ kite mf holdings
FUND                                      FOLIO       QUANTITY  AVERAGE_PRICE  LAST_PRICE   PNL
INVESCO INDIA TAX PLAN - DIRECT PLAN      3108290884    382.49          78.43       84.86  2,459.40
Indiabulls Liquid Fund - Direct Plan      5102495241      1.33       1,874.10    2,081.50  276.67
BOI AXA TAX ADVANTAGE FUND - DIRECT PLAN  9104386836    257.06         116.70      101.13  -4,002.38
```

- **`LAST_PRICE`** — The scheme's latest Net Asset Value (NAV), updated once per business day.
- **`PNL`** — Computed in human table output as `QUANTITY × (LAST_PRICE − AVERAGE_PRICE)`. With `--json`, Kite's raw `pnl` field is preserved.

## Orders

`kite mf orders` lists mutual fund orders from the last 7 days:

```text
$ kite mf orders
ORDER_ID                              FUND                                        TRANSACTION_TYPE  STATUS    QUANTITY    AMOUNT  AVERAGE_PRICE  LAST_PRICE  STATUS_MESSAGE
271989e0-a64e-4cf3-b4e4-afb8f38dd203  HDFC Balanced Advantage Fund - Direct Plan  BUY               REJECTED      0.00  1,000.00           0.00       30.68  AMC SIP: Insufficient balance.
ef7e696c-2fa6-400b-b180-eb25e6a04ccf  Kotak Flexicap Fund - Direct Plan           BUY               REJECTED      0.00  2,000.00           0.00       52.80  SIP: Insufficient balance.
2b6ad4b7-c84e-4c76-b459-f3a8994184f1  BOI AXA Arbitrage Fund - Direct Plan        BUY               OPEN          0.00  5,000.00           0.00       10.43  Insufficient fund. 1/5
40410882-b1f8-4938-bb08-4bef2765cbfb  HDFC Balanced Advantage Fund - Direct Plan  BUY               REJECTED      0.00  1,000.00           0.00      271.75  AMC SIP: Insufficient balance.
e67b8741-5054-4fd5-a2da-8c672e1f494a  ICICI Prudential Bond Fund - Direct Plan    BUY               OPEN          0.00  5,000.00           0.00       11.52  Insufficient fund. 3/5
```

Use `kite mf order <order_id>` to inspect a single mutual fund order.

## SIPs

`kite mf sips` lists your active and paused Systematic Investment Plans (SIPs):

```text
$ kite mf sips
SIP_ID           FUND                                             STATUS  INSTALMENT_AMOUNT  FREQUENCY  NEXT_INSTALMENT  COMPLETED_INSTALMENTS
892741486820670  Aditya Birla Sun Life Liquid Fund - Direct Plan  ACTIVE             500.00  weekly     2021-05-12                           0
109195857904698  HDFC Balanced Advantage Fund - Direct Plan       ACTIVE           1,000.00  weekly     2021-06-01                           0
846479755969168  HDFC Balanced Advantage Fund - Direct Plan       ACTIVE           1,000.00  monthly    2021-07-12                           1
749073272501476  HDFC Balanced Advantage Fund - Direct Plan       ACTIVE           1,000.00  monthly    2021-07-30                           1
576440634181776  HDFC Hybrid Equity Fund - Direct Plan            ACTIVE           7,427.00  quarterly  2021-02-19                           0
```

## Search schemes and instruments

- **`kite mf search <query>`** — Searches Kite's mutual fund scheme master by fund name or ISIN and displays up to `20` matches (override with `--limit`).
- **`kite mf instruments`** — Prints the mutual fund scheme master (`50` rows by default, or `--limit 0` for all schemes). Pass `--out funds.csv` to save the complete CSV file to disk.

Both commands download the public scheme list directly on each run and do not require an authenticated session. Unlike the equity/derivatives instrument master, the mutual fund scheme list is not cached locally.

## In the Terminal UI

The **Invest** workspace (`6`) displays your mutual fund holdings with current value, P&L, and day change in a sortable table. See [TUI — Invest](tui.md#invest).

---

[Back to the user guide](README.md)
