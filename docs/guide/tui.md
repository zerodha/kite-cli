# TUI

This page covers `kite tui`, the full-screen terminal trading interface: navigation, workspaces and panes, watchlists, order and GTT tickets, price alerts, charts, notes, the command palette, keyboard shortcuts, and `tui.json` settings.

```bash
kite                      # Opens the TUI (shows the sign-in screen first if needed)
kite tui                  # Same as above
kite tui --token 408065   # Opens with a temporary watchlist of instrument tokens
```

> [!IMPORTANT]
> The TUI operates on your live Kite account. Orders, GTT triggers, and position conversions always go through a review step that displays the exact request before anything is sent. Certain non-order actions—such as watchlist edits and toggling a simple alert—are sent immediately when you press the key. See [Safety](#safety) for the complete breakdown.

## Contents

- [Start, sign in and quit](#start-sign-in-and-quit)
- [The screen](#the-screen)
- [Getting around](#getting-around)
- [Workspaces and panes](#workspaces-and-panes)
- [Watchlists](#watchlists)
- [Instrument search](#instrument-search)
- [Order ticket](#order-ticket)
- [GTT ticket](#gtt-ticket)
- [Price alerts](#price-alerts)
- [Chart](#chart)
- [Notes](#notes)
- [Command palette and action menu](#command-palette-and-action-menu)
- [Keys reference](#keys-reference)
- [Settings: tui.json](#settings-tuijson)
- [Safety](#safety)
- [Files the TUI writes](#files-the-tui-writes)
- [Connection diagnostics](#connection-diagnostics)
- [What the TUI does not do](#what-the-tui-does-not-do)

---

## Start, sign in and quit

- **Launch:** Running `kite` with no arguments in an interactive terminal opens the TUI. Under `--json` or without a TTY, it prints a usage hint and exits with code `2`.
- **Sign in:** When no saved session exists, the TUI opens the sign-in screen first and prompts for your user ID, password, and two-factor authentication code. When Kite offers multiple 2FA methods (such as TOTP and SMS OTP), press `Tab` to switch methods. For SMS, the TUI requests Kite to dispatch the OTP.
- **Session expiry:** When your access token expires, the TUI ends the active session and returns to the sign-in screen with your user ID pre-filled. Press `Ctrl-U` on that screen to sign in as a different user. After signing in, the TUI starts a fresh session; it does not renew tokens automatically in the background.
- **Temporary token mode:** `kite tui --token 408065` (repeat `--token` for multiple instruments) displays those instruments in a temporary, in-memory watchlist. A `--token` session does not read or modify your account's watchlists and does not write to `notes.json`. Theme selections and pane divider positions are still saved to `tui.json`.
- **Quit:** Press `q` or `Ctrl-C` to exit. Inside the notes editor and search or filter inputs, `q` types the letter `q`; `Ctrl-C` quits from anywhere.

Bracketed paste inserts text into the focused field. Single-line fields accept only the first line and never treat a pasted newline as confirmation. Pasting on a review screen is ignored. AltGr characters are accepted as text, and `Ctrl-H` acts as `Backspace`.

---

## The screen

```text
 kite  1 Overview  2 Trading  3 Portfolio  4 Derivatives  5 Orders  6 Invest   ● live | open | 14:32:05 │ tabs + status
┌ > Watchlist · Default ──────┐┌ Chart | NSE:INFY | 5m ──────────────────────┐
│  SYMBOL       LTP     CHG%  ││                                             │
│ ▶NSE:INFY 🔔 1523.40  +0.80  ││   candles of the selected watch row          │ Trading: the
│  NSE:RELIANCE 2891.15 -0.42  ││                                             │ watch list on
│  NSE:NIFTY 50 24312.4 +0.11  │└─────────────────────────────────────────────┘ the left, and
│  ...                         │┌ Depth | INFY ─────┐┌ Orders · time ▼ ────────┐ what it selects
│                              ││ 5-level ladder     ││ the account's orders    │ on the right
└──────────────────────────────┘└────────────────────┘└─────────────────────────┘
 ⚠ order rejected: Insufficient funds …                                          toasts/messages
 Ctrl-K:commands  Tab:pane  ?:help │ b:buy  s:sell  g:gtt  a:alert │ /:add        key hints
```

- **Header:** Displays one tab per workspace followed by a status strip:
  - **WebSocket state:** `● live`, `◌ connecting`, `◌ reconnecting`, `○ offline`, or `○ auth expired`.
  - **Session indicators:** Session uptime, IST clock, market phase (`pre-open`, `open`, or closed), and available equity margin.
  - **Responsive truncation:** When the terminal width is narrow, the status strip drops the clock and market phase first, then available funds, and then uptime. Workspace tabs drop their number keys first, then the `kite` mark, without truncating workspace names—all tabs fit at 80 columns. Below 80 columns, the header keeps the active tab and as many adjacent tabs as fit, displaying `‹` or `›` on sides with hidden tabs.
- **Main area:** Displays the panes of the active workspace.
- **Message and key-hint bar:** Shows transient status messages (order acknowledgements, errors) colour-coded by severity. When no message is active, it displays contextual key hints grouped by function and separated by a dim `│`:
  1. **Entry points:** `Ctrl-K:commands  ?:help` on a pane, or `Enter`/`Esc` inside a modal.
  2. **Order & alert actions:** `b:buy  s:sell  g:gtt  a:alert`.
  3. **Row actions:** `x:cancel`, `m:modify`, `a:authorise`.
  4. **Navigation:** `j/k:move`, `[ ]:lists`, `←/→:workspaces`.
  5. **Pane controls:** `c:chart`, `S:sort`, `r:refresh`, `i:interval`.
  6. **Global shortcuts:** `n:notes  T:theme  q:quit`.

  When the terminal is too narrow to show all groups, entire groups are dropped from least to most important—individual hints are never cut in half. Every omitted shortcut remains listed in `?` help. `Ctrl-K:commands` leads every hint line outside a modal. See `?` or the [Keys reference](#keys-reference) for full descriptions.
- **Modals:** Floating overlays for the command palette, theme picker, instrument search, order and GTT tickets, confirmations, order detail, the alert form, funds breakdown, event log, notes, and help. Modals stack cleanly (for example, order ticket → review confirmation, or notes → delete confirmation).

---

## Getting around

- **Switch workspaces:** Press `1` through `6`, cycle with `←` and `→`, or click a workspace tab in the header.
- **Workspace picker:** Press `` ` `` to open a list of all workspaces with their number keys and constituent panes. Navigate with `j`/`k`, arrow keys, or the scroll wheel; click a row, press `Enter`, or press a number key to switch; press `Esc` to close.
- **Switch panes:** Press `Tab` and `Shift-Tab` to move focus between panes in the active workspace, or click anywhere inside a pane.
- **Zoom a pane:** Press `z` to expand the focused pane to the full workspace area. Press `z` again to restore the split layout.
- **Resize panes:** Drag the border between two panes with the mouse. Divider positions are persisted to `tui.json`. Press `=` to reset all borders in the workspace to their default proportions.
- **Move selection:** Press `j`/`k` or `↓`/`↑` to move up and down tables. Press `g` and `G` to jump to the first and last row (except in Watch and Holdings, where `g` opens a GTT ticket).
- **Refresh data:** Press `r` to refresh the focused pane from Kite (on the Alerts pane, `r` toggles the selected alert and `R` refreshes).
- **Help overlay:** Press `?` to view all keyboard shortcuts for the focused pane. Scroll with `j`/`k`, `PageUp`/`PageDown`, and `g`/`G`. Close with `?`, `q`, or `Esc`.
- **Funds modal:** Press `F` to open the full equity and commodity margin breakdown (`available` and `utilised` figures behind the header's summary number).
- **Event log:** Press `Ctrl-L` to inspect the session's message history (newest first), including messages that have left the bottom bar. Scroll the list with `j`/`k`/`g`/`G` and scroll the selected message's detail with `PgUp`/`PgDn`. Close with `Ctrl-L` or `Esc`.
- **Theme picker:** Press `T` to preview and switch colour themes. See [Themes](#themes).

**Mouse support:** Click workspace tabs, table rows, column headers (to sort by that column), section labels, option-chain legs, modal list rows, form fields, or positions inside an open note (to place the cursor). Click any key hint in the bottom bar to trigger that shortcut. Drag pane borders to resize splits, and use the scroll wheel to scroll the list or panel under the pointer one row per notch.

**Colours and column fitting:** Gains, buys, and bids render in green; losses, sells, and asks render in red. When a table column does not fit the available pane width, the TUI hides the entire column rather than truncating numbers.

**Small terminals:** Below 120 columns or 30 rows, a workspace renders only its focused pane across the full body. The pane title indicates how many sibling panes are hidden, and `Tab` cycles through them.

---

## Workspaces and panes

Each workspace consists of a fixed arrangement of panes. Every pane maintains its own cursor position, sort order, scroll offset, and batch marks independently.

| Key | Workspace | Panes |
| --- | --- | --- |
| `1` | **Overview** | Positions and Holdings snapshots, Indices, Movers |
| `2` | **Trading** | Watch on the left; Chart above Depth and Orders on the right |
| `3` | **Portfolio** | Positions on the left; Holdings above Funds on the right |
| `4` | **Derivatives** | Option Chain on the left; underlying spot Chart on the right |
| `5` | **Orders** | Orders on the left; GTT above Alerts on the right |
| `6` | **Invest** | Mutual Funds (MF) on the left; Bids on the right |

The **Orders** pane in the Trading workspace and the **Orders** pane in the Orders workspace display the same underlying order book as two independent views: sorting, marking, or scrolling one does not affect the other.

### Overview

The default workspace at session start. It is strictly read-only.

- **Summary tiles:** Four header tiles showing portfolio value, day P&L, total P&L, and available funds.
- **Positions and Holdings snapshots:** Press `[` and `]` to switch focus between the two snapshots, and press `Enter` to jump to the selected instrument in the Portfolio workspace. The focused snapshot is titled `> Positions (N)` with an accented border. Clicking inside a snapshot focuses it; clicking a row also selects that row.
- **Indices:** Displays the index instruments present in the Trading workspace's active watchlist.
- **Movers:** Lists the top five gainers and losers for an NSE index universe. Press `u` to cycle the universe: `Nifty 50` (default), `Nifty 100`, `Nifty 500`, `Nifty F&O Stocks`, and `All (illiquid)`. Your selection is saved to `tui.json`. Movers cover **NSE only**. The `All` universe is unfiltered and includes suspended or untraded stocks that may show extreme percentage changes on zero volume. Movers load at session start, when switching to Overview, and when pressing `r`.
- **Account summary strip:** Shows item counts such as `Positions 12 (5 open)`, `Orders 3 (1 open)`, `Holdings 40`, and `GTT 2`. On narrower terminals, this condenses to `Pos 5/12  Ord 1/3  Hold 40  GTT 2` (`open/total`). Counts that have not finished loading display `?`, never `0`.
- **Responsive layout:** With 28 or more rows, the summary tiles and account strip render with borders; with fewer rows, borders compact to give more vertical space to the snapshots. Below 12 rows or 70 columns, Indices and Movers are hidden and only the focused snapshot is drawn (`[`/`]` or `Tab` switches between them).

### Trading: Watch, Chart, Depth, Orders

**Watch** displays your active watchlist: symbol, LTP, day change (`%` and absolute), high, low, and volume. Prices flash green or red as ticks arrive.

- **Add and remove:** Press `/` to search and add an instrument (see [Instrument search](#instrument-search)). Press `d` to remove the selected row. Both actions are sent to your Kite account immediately without confirmation, and the row updates once Kite confirms the change.
- **Manage lists and order:** Press `[` and `]` to switch between watchlists. Press `w` to open the command palette pre-filled with `wl ` to switch or create a list. Press `J` and `K` to move the selected row down or up within its group. See [Watchlists](#watchlists).
- **Tickets and alerts:** Press `b` or `s` to open the order ticket, `g` for the GTT ticket, or `a` for the price-alert form on the selected instrument.
- **Refresh:** Press `r` to reload watchlists from your account (picking up changes made on Kite Web or Kite mobile).
- **Alert indicator:** A bell icon (`🔔`) next to a symbol indicates one or more price alerts on that instrument—rendered in the warning colour while at least one alert is enabled, and dimmed when all alerts for that symbol are disabled.
- **Indices:** Index rows (such as `NSE:NIFTY 50`) stream price ticks without volume or market depth. Attempting to open an order or GTT ticket on an index is rejected with `indices are not tradable`.

**Chart** and **Depth** always track the instrument selected in **Watch**. Moving the cursor in the **Orders** pane does not change the charted or depth instrument.

- **Chart:** Renders historical and live candlesticks for the selected watchlist row. See [Chart](#chart).
- **Depth:** Renders the 5-level bid and ask ladder (price, quantity, order count) along with lower circuit (`LC`) and upper circuit (`UC`) limits beneath the bid and ask totals.
- **Full-screen Depth & Quote:** Pressing `Enter` or `z` on Depth expands it to fill the workspace and adds a quote summary above the ladder: OHLC values plus visual range bars for circuit limits and the day's high-low range (plus the 52-week range once the `1D` chart interval has been loaded for that instrument). Press `Enter` or `z` again to restore the split layout.

**Orders** shows today's order book: timestamp, symbol, side, order type, product, quantity (`filled/total`), price, trigger price, and status (colour-coded by status). Orders are listed newest first, with open orders sorted ahead of completed orders at the same timestamp.

- **Cancel:** Press `x` to cancel the selected open order after confirmation. Press `Space` to mark individual open orders or `Ctrl-A` to mark all open orders, then press `x` to cancel the marked batch.
- **Modify:** Press `m` on an open order to open the order ticket in modify mode.
- **Inspect and sort:** Press `Enter` to view full order details and state history. Press `S` to cycle the sort order (`time`, `status`, `symbol`). Press `c` to open an inline candlestick chart for the order's instrument.
- **Live updates:** The pane polls every 5 seconds and immediately after any order action. In addition, real-time order updates over the WebSocket display an `order update: SYMBOL STATUS` toast and trigger an immediate refresh of both orders and positions.

### Portfolio: Positions, Holdings, Funds

**Positions** displays your open and closed positions for the day: product, symbol, net quantity, average price, live LTP, live P&L, and day change `%`. A summary header above the table shows total P&L (Kite's `pnl`, including carried-forward derivatives positions) broken down into realised and unrealised P&L, alongside the `open/total` position count.

- **Trade or exit:** Press `b` or `s` to open the order ticket. Pressing `s` on a long position (or `b` on a short position) pre-fills the quantity required to exit.
- **Convert product:** Press `c` to open the position conversion modal (for example, `MIS` to `CNC` or `NRML`). Because `c` is bound to conversion on this pane, open a position's chart via `Ctrl-K` → **Toggle chart**.
- **Alerts and sorting:** Press `a` to create a price alert for the selected position. Press `S` to cycle the sort column (`P&L`, `quantity`, `day %`, `symbol`).
- **Live streaming:** Positions and holdings subscribe to WebSocket LTP ticks automatically, so P&L updates in real time even for instruments not present in your watchlist.

**Holdings** displays your demat equity holdings: symbol, quantity, average cost, live LTP, current value, P&L, P&L `%`, and day `%`. The pane title displays total holdings value, total P&L, and day P&L (omitting summary figures if the pane is too narrow rather than truncating them). Discrepant or unpriced holdings display `N/A` and are excluded from portfolio totals, matching Kite Web. The pane refreshes every 30 seconds.

- **Trade:** Press `b` or `s` to open the order ticket. Pressing `s` pre-fills the free (unpledged) quantity.
- **GTT stop-loss / target:** Press `g` to open a `SELL` GTT ticket pre-filled with the holding's free quantity.
- **CDSL authorisation and alerts:** On the Holdings pane, `a` opens the CDSL TPIN/OTP authorisation flow (for accounts requiring sell consent), `B` opens the price-alert form for the selected holding, and `A` jumps to the Alerts workspace pane.
- **Sort and chart:** Press `S` to cycle the sort order. Press `c` to toggle an inline candlestick chart.

**Funds** displays the equity and commodity margin tables side by side. Pressing `F` from anywhere in the TUI opens the same breakdown in a modal.

### Derivatives: option chain

Builds a call/put strike ladder for a single underlying and expiry using the cached instrument master (requiring no extra HTTP calls to build the ladder).

- **Select underlying:** Press `u` to cycle through built-in index underlyings (`NIFTY`, `BANKNIFTY`, `FINNIFTY`, `MIDCPNIFTY`, `SENSEX`, `BANKEX`). Press `/` to search for any F&O stock underlying; if the chosen instrument has no option contracts, the TUI notifies you and keeps the current chain.
- **Navigate strikes and expiries:** Press `[` and `]` to switch expiry dates. Press `h` and `l` to focus the Call (`CE`) or Put (`PE`) side. Press `j` and `k` to move across strikes.
- **Trade or alert on a leg:** Press `b` or `s` to open an order ticket for the focused option contract at the selected strike (defaulting to `NRML`). Press `a` to set a price alert on that option leg.
- **Live streaming:** While the Derivatives workspace is visible, the underlying spot and the expiry's call and put contracts stream in `full` mode (providing live LTP, volume, OI, and bid/ask). Subscriptions populate outward from the at-the-money (ATM) strike up to Kite's 3,000-instrument WebSocket limit.
- **ATM centring and chart:** Whenever the ladder is rebuilt (on underlying or expiry change), the selection centres on the ATM strike. The **Chart** pane beside the chain always plots the underlying spot price, not the focused option leg.

### Orders: Orders, GTT, Alerts

- **Orders:** An independent view of today's order book with the same shortcuts and batch cancellation controls as the Trading workspace's Orders pane.
- **GTT:** Lists all Good Till Triggered rules on your account. Press `m` on an `active` trigger to modify it in the GTT ticket. Press `x` to delete the selected trigger after confirmation. Press `S` to cycle the sort order. The pane refreshes every 60 seconds. New GTT triggers are created with `g` from the Watch or Holdings panes.
- **Alerts:** Lists all server-side price alerts on your Kite account: name, symbol, condition, status, disable reason (if disabled by Kite), trigger count, alert type (`SIMPLE` or `ATO n` for an Alert-to-Order with `n` basket items), and creation date. Selecting an `ATO` alert displays its basket of orders in a sub-panel below the table. See [Price alerts](#price-alerts).

### Invest

- **MF:** Displays your mutual fund holdings: fund name, units, average NAV, current NAV, invested amount, current value, P&L, net return `%`, and day change `%`. This pane is read-only and refreshes alongside equity holdings (every 30 seconds by default). Press `S` to cycle the sort order (`name`, `value`, `P&L`, `day change`). To view mutual fund orders, SIPs, or fund factsheets, use the [`kite mf`](mutual-funds.md) CLI commands.
- **Bids:** Displays IPOs, government securities (G-Secs), auctions, corporate actions, and Social Stock Exchange (SSE) IPOs. Press `[` and `]` (or click a section tab) to switch sections:
  - Press `a` in the **IPO** section to toggle between open issues and your submitted applications.
  - Press `Enter` to open the active section on Kite Web in your browser to apply or bid.
  - Press `s` in the **Auctions** section to open a pre-configured `SELL` / `CNC` / `LIMIT` / `DAY` order ticket for the selected auction holding.
  - Press `r` to refresh the active section.

---

## Watchlists

Watchlists in the TUI are your live Kite account watchlists—the exact same lists shown on Kite Web and the Kite mobile app. The TUI fetches them at startup and stores no separate local watchlist copy. Every edit is sent to Kite immediately without confirmation (except `:wl delete`, which prompts first). Adding an instrument in the TUI reflects on your phone right away; changes made in the Kite app appear in the TUI when you press `r`.

- **Switch, create, rename, delete:**
  - Press `[` and `]` to cycle through your watchlists in account order.
  - Run `:wl NAME` in the command palette to switch to the list named `NAME`, creating it on your account if it does not exist.
  - Run `:wl rename NAME` to rename the currently open watchlist.
  - Run `:wl delete` to delete the open watchlist after confirmation (including the last remaining watchlist).
  - Press `w` on the Watch pane to open the command palette pre-filled with `wl `.
- **Duplicate watchlist names:** Kite allows multiple watchlists to share the same name. When duplicates exist, switching by name requires disambiguation: the command palette lists each matching watchlist alongside its numeric ID so you can pick the exact one, or you can run `:wl ID` directly. Creating a new named watchlist requires the watchlist catalog to have loaded cleanly first.
- **Optimistic vs. confirmed updates:**
  - **Add (`/`) and remove (`d`):** Sent to Kite first; the row appears or disappears on screen only after Kite confirms the write and returns the updated list. Only one mutation is allowed in flight per list at a time (indicated in the pane title). If a write fails, the pane restores the server state.
  - **Reorder (`J`/`K`):** Moves the row immediately on screen and dispatches the update in the background. If you press `J` or `K` again while a reorder request is already in flight, the row continues moving smoothly on screen and a single follow-up request syncs the final order. If the server rejects the reorder, the list reverts to the server's order while keeping your cursor on the moved instrument.
- **Watchlist groups:** Group headers, IDs, and item memberships created on Kite Web are preserved intact by the TUI. The TUI renders the watchlist as a flat table and does not edit group structure, so `J`/`K` reordering stops at group boundaries.
- **Curated lists:** Kite's built-in curated watchlists are marked `read-only` in the pane title and reject all edits.
- **Error and staleness handling:** The pane explicitly indicates whether it is loading, failed (`r` to retry), on an account with zero watchlists, or viewing an empty list. If a background refresh fails after a watchlist has already loaded, existing rows stay on screen marked `stale`, and edits are blocked until a fresh read succeeds.
- **Persistence:** `tui.json` remembers the last open watchlist per user ID, falling back to the first watchlist if the saved list no longer exists.

---

## Instrument search

Press `/` on the Watch or Option Chain pane to open the instrument search modal. Search performs a fuzzy character-order match against both trading symbols and company/contract names in the local instrument cache, ranking exact prefix matches first. Prefix your query with an exchange (such as `NSE:`) to filter by exchange. Press `Enter` to add the selected instrument to the active watchlist (or load its option chain in Derivatives).

If the instrument master fails to load, the search modal displays the error and remains open; press `Ctrl-R` to retry downloading the master. Modifying an existing order, GTT trigger, or ATO basket item also relies on the cached instrument master to validate exact tick and lot sizes.

The instrument master is cached locally at `~/.cache/kite/instruments.json` (see [Market data — The cache](market-data.md#the-cache)). The TUI loads it asynchronously in the background and refreshes it automatically when the file is older than the most recent 07:00 IST daily publication.

---

## Order ticket

Pressing `b` (buy) or `s` (sell) on an instrument row opens the order ticket modal:

```text
┌ BUY NSE:INFY ────────────────────────────────────────────────┐
│ LTP 1523.40  ·  lot 1  ·  tick 0.05                          │
│                                                              │
│ ▶      Qty   10                                              │
│       Type   MARKET  LIMIT  SL  SL-M                         │
│      Price   1523.40                                         │
│    Trigger   n/a                                             │
│    Product   CNC  MIS  NRML  MTF                             │
│   Validity   DAY  IOC  TTL                                   │
│   TTL mins   5                                               │
│  Autoslice   off  on                                         │
│    Variety   regular  iceberg                                │
│                                                              │
│   Enter to review - nothing is sent yet                      │
└──────────────────────────────────────────────────────────────┘
```

- **Field navigation and stepping:** Press `Tab` and `Shift-Tab` to move between fields. Press `↑` and `↓` to increment or decrement numeric values: quantity steps by the contract's lot size, price and trigger step by the tick size, `TTL mins` steps through Kite's supported minute values, and iceberg legs step by `1`. Press `←`/`→` or `Space` to change a selector row.
- **Contextual fields:** Fields that do not apply to the current selection remain visible for layout stability but are skipped during `Tab` navigation (`Price` for `MARKET` and `SL-M` orders, `Trigger` for `MARKET` and `LIMIT` orders, and `TTL mins` unless validity is `TTL`).
- **Lot and tick validation:** The subtitle displays the instrument's lot size and tick size. Quantities must be multiples of the lot size and prices must be multiples of the tick size; invalid inputs are flagged before you can proceed to the review screen.
- **Defaults:** `Price` initialises to the current LTP. `Product` defaults to `CNC` for equities and `NRML` for futures and options (customisable via `default_product` in `tui.json`), offering only products valid for that exchange and segment.
- **Freeze quantity, Autoslice, and Iceberg:** Entering a quantity above the exchange's freeze limit automatically enables `Autoslice` so Kite splits the order into child orders rather than having the exchange reject it. `Variety` offers `iceberg` (2 to 50 legs) for new orders on supported exchanges (all except `MCX` and `NCO`). Order modifications do not use Autoslice or Iceberg.
- **Modify mode:** Pressing `m` on an open order opens the ticket with only `Qty`, `Type`, `Price`, and `Trigger` editable. `Product` and `Validity` are shown dimmed and read-only because Kite does not permit changing them on an open order.
- **Review and confirmation:** Pressing `Enter` on the ticket **never sends the order immediately**. It opens a review screen displaying the exact order parameters alongside a live margin and brokerage/charges preview fetched from Kite. Press `y` (or `Enter` on the confirm button) to transmit the order, or `Esc` to return to the ticket.
- **Result feedback:** When Kite accepts the order, the TUI displays the new order ID in the message bar. If Kite rejects the request, the TUI displays Kite's rejection reason.

---

## GTT ticket

Press `g` on a Watch or Holdings row to open the Good Till Triggered (GTT) ticket to create a trigger, or press `m` on an active row in the GTT pane to modify it (titled `MODIFY GTT <id>`). The GTT ticket renders with a yellow warning border so you can immediately distinguish a stored trigger from an immediate order ticket.

- **Fields:** `Qty`, `Side`, `Price`, `Trigger`, `Product`, `GTT type` (`single` or `OCO`), and `Trailing`. Because GTT rules always place `LIMIT` orders when triggered, order type and validity selectors are omitted.
- **Two-leg (OCO) triggers:** Setting `GTT type` to `OCO` configures a stop-loss leg below the current price and a target leg above the current price. Initial limit prices match their respective trigger prices; verify both before saving.
- **Holdings and MTF awareness:** Opening a `SELL` or `GTT` ticket from an MTF-only holding pre-selects the `MTF` product and your MTF quantity. Holdings with both `CNC` and `MTF` quantities default to the available `CNC` quantity. NSE equity GTT tickets offer `CNC` and `MTF`; derivative GTT tickets offer `NRML`.
- **Trailing GTT:** Available for `SELL` triggers (both single-leg and OCO), adding a trailing distance in price points. Switching the side to `BUY` disables trailing automatically.
- **Review:** Saving a GTT trigger goes through the same two-step review and confirmation screen as an order ticket.

---

## Price alerts

**Price alerts live on your Kite account, not locally inside the TUI.** Kite evaluates the condition on its servers, fires the alert, and sends notifications. Consequently:

- Alerts continue monitoring the market after you quit the TUI.
- Alerts created on Kite Web or the Kite mobile app appear in the TUI automatically.
- Pressing `d` on the Alerts pane deletes the alert from your Kite account (after confirmation), removing it from Kite Web and mobile as well.
- You can create multiple alerts on the same instrument; only an exact duplicate of an existing simple alert condition is refused.

**Creating an alert:**

- Press `a` on a Watch row, a Positions row, or the focused leg of the Option Chain to open the alert form for that instrument. On the Holdings pane, press `B` (since `a` triggers CDSL authorisation).
- Pressing `a` on the Alerts pane, running `:alert` in the command palette, or selecting **New alert…** in the `Ctrl-K` menu prompts you to pick an instrument first. Running `:alert SYMBOL` opens the alert form for `SYMBOL` directly.
- Choose the `Attribute`, `Operator`, and `Level`. Use `↑`/`↓` to step the level by tick size, `Space` or `←`/`→` to change selector values, `Enter` to save, and `Esc` to cancel.
  - For a **Simple** alert, pressing `Enter` creates or updates the alert on your account immediately without a separate confirmation screen.
  - For an **ATO** alert, pressing `Enter` opens a mandatory review screen first (see [Alert to order (ATO)](#alert-to-order-ato)).
- The `Level` field initialises to the attribute's current value and tracks live ticks until you edit it; typing a digit replaces the pre-filled number.
- When creating a new alert, a level that the market already satisfies is refused locally because it would fire immediately. Editing an existing alert skips this local check because the TUI may not have a live tick for the edited instrument.
- Alerts on indices (such as `NIFTY 50`) are submitted under the `INDICES` exchange as required by Kite.

**Managing alerts:**

Press `A` from any pane to jump to the Alerts pane and refresh the alert list from Kite.

- **Edit (`e`):** Opens the selected alert in the alert form. Editing preserves the alert's current status—an alert that has already fired (and is therefore `disabled`) remains disabled after editing, which the form notes explicitly.
- **Enable / disable (`r`):** Toggles the selected alert between `enabled` and `disabled`. Simple alerts and alert disables take effect immediately. Enabling an **ATO** alert opens a confirmation modal displaying its condition and basket orders first, because enabling it arms live orders on your account. Concurrent toggles on the same alert are blocked while a request is in flight.
- **Trigger history (`Enter`):** Opens a sub-panel below the table listing every timestamp the alert fired and the market price at that moment. Press `Esc` to close the history panel while staying in the Alerts pane.
- **Refresh (`R`) and sort (`S`):** Because `r` toggles the selected alert on this pane, refresh is bound to `R`. Press `S` to cycle the sort order. The Alerts pane refreshes automatically every 60 seconds, whenever you jump to it with `A`, and after every alert modification.

**When an alert fires:** The TUI does not evaluate alert conditions locally against the tick stream. Instead, whenever an Alerts refresh detects that an alert's `triggered_count` has increased during the current session, the TUI displays a notification in the message bar and rings the terminal bell (up to one poll interval after the server trigger). Historical trigger counts already present when the session starts do not fire notifications.

### Alert to order (ATO)

An Alert-to-Order (ATO) alert executes a basket of up to 20 orders on Kite's servers when the alert condition is met. Set the alert form's `Type` selector to `ATO` to build or edit an order basket (new ATO alerts start with an empty basket).

- **Basket controls:** Focus the basket section (`Tab` past the condition fields) and use:
  - `a` to add an order (selecting the instrument first)
  - `e` or `Enter` to edit the selected basket order
  - `d` to remove the selected order
  - `c` to duplicate the selected order
  - `[` and `]` to move the selected order earlier or later in execution sequence
  - `j`/`k` and `PgUp`/`PgDn`/`Home`/`End` to navigate the basket
- **Basket order fields:** Each basket item specifies `Side`, `Qty`, `Product` (`CNC`, `MIS`, `NRML`), `Type` (`MARKET`, `LIMIT`, `SL`), `Price`, `Trigger`, and `Validity` (`DAY`, `IOC`, `TTL`) on `NSE`, `BSE`, `NFO`, or `BFO`. Pressing `Enter` inside a basket item's editor returns the item to the basket—it never places an immediate order.
- **ATO review screen:** Pressing `Enter` to save an ATO alert opens a dedicated review screen detailing the trigger condition, resulting status, every order being armed, every order being removed, and two operational notes: Kite converts `MARKET` basket orders into price-protected `LIMIT` orders upon execution, and saving arms the basket on your account. Press `Enter` to submit or `Esc` to return to editing. If Kite rejects the submission, the form remains open with Kite's error message so your hand-built basket is never lost.
- **Unsupported order varieties:** If an existing ATO alert created on Kite Web contains order parameters the TUI form does not edit (such as `co` or `iceberg` varieties, `MTF`, `SL-M`, disclosed quantity, GTT legs, tags, or unknown fields), the TUI preserves those items verbatim. You can reorder, duplicate, or delete them, and their row indicates that parameter editing requires Kite Web.
- **Converting ATO to Simple:** Changing an existing ATO alert's type to `Simple` deletes its order basket on your account, which is highlighted on the review screen.
- **Validation and safeguards:** Saving an ATO with zero basket items is refused (since Kite would silently downgrade it to a simple alert). Pressing `Esc` on a modified basket prompts for confirmation before discarding changes. ATO is not supported for conditions on `MCX`, `NCO`, `CDS`, `BCD`, or global indices excluded by Kite Web (such as `GIFT NIFTY`, `US500`, and `HANGSENG`).

---

## Chart

The **Trading** and **Derivatives** workspaces include a dedicated **Chart** pane. On the **Orders** and **Holdings** panes, pressing `c` opens an inline candlestick chart for the selected row's instrument across the workspace; press `c` or `Esc` (or switch workspaces) to close it. On **Positions**, `c` is bound to product conversion, so open a position's chart via `Ctrl-K` → **Toggle chart**.

- **Expand (`C`):** Toggles whether the chart fills the entire workspace or shares the split layout with its sibling panes.
- **Interval (`i`):** Cycles the candle interval across `1m`, `5m`, `15m`, `1h`, and `1D` (with lookback windows of 7, 30, 90, 180, and 250 days, respectively).
- **Volume (`v`):** Shows or hides the volume histogram subplot beneath the candles.
- **Inspect candles (`h`/`l`):** Steps backward or forward one candle at a time, displaying that bar's timestamp, `O`, `H`, `L`, `C`, and volume in the chart header.
- **Live updates:** The rightmost candle updates in real time from incoming WebSocket ticks, and the current LTP renders as a horizontal price line with a tag on the vertical price axis.
- **Time axis:** Labels along the bottom axis adapt automatically to the interval and terminal width—showing clock times (`09:15`), session dates (`Thu20`), weeks (`24Aug`), months (`Sep`), or years (`2026`), stepping to coarser spacing rather than crowding or clipping labels.

### Pointing at a candle

- **Hover readout:** Move the mouse pointer over the candle area to inspect any historical bar: its timestamp, OHLC, volume, and bar offset (`N bars back`). The hovered bar's close price is tagged on the price axis with `C ` to distinguish it from the live LTP tag.
- **Scroll lock while hovering:** While the mouse pointer rests over the candles, live chart auto-scrolling pauses so the bar under your cursor does not shift. Moving the pointer off the chart restores the keyboard cursor state.
- **Click to pin:** Click a candle to pin the keyboard inspection cursor to that bar. You can then step from that bar with `h` and `l`; stepping past the newest bar returns the chart to `LIVE` mode.
- **Wheel isolation:** Mouse wheel events over the chart are ignored so scrolling never accidentally changes the selected instrument in an adjacent pane.
- **Narrow charts:** When horizontal space is tight, the header readout drops volume first, then `L`, `H`, and `O`, and finally the timestamp—prices are always shown in full or omitted cleanly.

### Indicators

Press `I` to cycle the technical indicator overlay: `none`, `SMA(20)`, `EMA(20)`, `BB(20,2.0)`, `RSI(14)`, and `MACD(12,26,9)`. The active indicator is saved to `tui.json`. One indicator is displayed at a time with standard periods:

- **Overlays (`SMA`, `EMA`, `Bollinger Bands`):** Plotted directly over the candlesticks on the price axis.
- **Oscillators (`RSI`, `MACD`):** Plotted in a dedicated 6-row subplot beneath the candles (automatically hidden if the main candle area would shrink below 5 rows).

---

## Notes

Press `n` from anywhere to open the local scratchpad and trading journal—useful for recording trade theses, price levels, or position notes. Notes are stored strictly on your local machine (`~/.config/kite/notes.json`) and require no network calls once the TUI is running, remaining accessible even if your network connection or session drops.

- **List navigation:** Press `j`/`k` to select a note, `n` to create a note attached to the currently selected instrument (on Watch, Positions, Holdings, or Overview snapshots), `Shift-N` to create a general note without an instrument tag, `e` or `Enter` to edit the selected note, `d` to delete it (after confirmation), and `Esc` or `q` to close the pad. Instrument-linked notes store the exchange and trading symbol (`NSE:INFY`) so they survive contract rollovers.
- **Editor controls:** Inside the note editor, all printable keys insert text—**`Esc` is the only key that exits the editor**. `Enter` inserts a newline, arrow keys and `Home`/`End` move by full Unicode graphemes, `Tab` inserts four spaces, and multi-line bracketed paste preserves line breaks.
- **Automatic saving:** Pressing `Esc` saves the note immediately (empty notes are discarded with a notice). Open edits are also flushed to disk automatically if you quit, log out, your session expires, or the process receives `SIGTERM` or `SIGHUP`. If a late save fails on exit, the error is printed after the terminal screen is restored.
- **File safety:** An empty or whitespace-only `notes.json` initialises an empty pad, and UTF-8 BOMs are accepted. If `notes.json` contains malformed JSON or cannot be read, the TUI refuses to overwrite it and warns you in the notes pad title.

---

## Command palette and action menu

Press `:` to open the command palette. Press `Tab` to autocomplete, `Enter` to execute the selected command, and `Esc` to close. Commands are case-insensitive.

| Input | Action |
| --- | --- |
| `buy SYMBOL [QTY] [@PRICE]`, `b …` | Open a `BUY` order ticket pre-filled with the given symbol, quantity, and limit price |
| `sell SYMBOL [QTY] [@PRICE]`, `s …` | Open a `SELL` order ticket |
| `alert SYMBOL` | Open the price-alert form for `SYMBOL` |
| `alert` | Pick an instrument first, then open the price-alert form |
| `SYMBOL` | Search for `SYMBOL` to add it to the active watchlist or jump to it |
| `watch`, `positions`, `orders`, `holdings`, `mf`, `gtt`, `alerts`, `bids` (or unique prefix) | Jump directly to that pane |
| `overview`, `trading`, `portfolio`, `derivatives`, `invest`, `workspace NAME` | Switch to that workspace |
| `auction` | Open the Bids pane (use `[`/`]` to switch to the Auctions section) |
| `wl NAME` | Switch to the account watchlist named `NAME`, or create it if it does not exist |
| `wl rename NAME` | Rename the currently open watchlist on your account |
| `wl delete` | Delete the currently open watchlist from your account (after confirmation) |
| `theme [NAME]` | Open the theme picker, or apply a matching theme by prefix |
| `q`, `quit`, `exit` | Quit the TUI |

- **Symbol syntax:** Symbols can be bare (`infy`) or exchange-prefixed (`nse:infy`). In `buy` and `sell` commands, the symbol is a single token (multi-word index names like `NSE:NIFTY 50` work in search and alerts, while indices are not tradable in orders anyway).
- **Alert vs. Alerts pane:** Typing `alert` creates a new alert; typing `alerts` (or short prefixes `a` / `al`) navigates to the Alerts pane.
- **Safe completion and selection:**
  - **`Tab` only completes text; only `Enter` runs a command.** For example, `bu` completes to `buy `, `theme dra` completes to `theme dracula`, and `sell nse:inf 10 @ 1520` completes the symbol token while preserving the quantity and price arguments.
  - **Ambiguous symbols wait for explicit choice:** Because `INFY` trades on both `NSE` and `BSE`, the palette never guesses an exchange—`Tab` and `Enter` wait until you highlight a specific row using `↑`/`↓`, `Ctrl-N`/`Ctrl-P`, the scroll wheel, or a mouse click.
  - **Mouse clicks only select:** Clicking a suggestion row highlights it without executing it, preventing an accidental double-click from opening or confirming an order.
  - **Inline syntax and validation:** The second line of the input displays expected arguments or validation errors (for example, `buy SYMBOL [QTY] [@PRICE]`, `Quantity must be 1..4294967295`, or `Price must be finite and > 0`). Invalid inputs cannot be executed.

**Action menu (`Ctrl-K`):** Opens the same modal with an empty query, presenting contextual actions organised by group:

- **Suggested:** Refresh, instrument search, new alert, theme picker, notes, and help.
- **Switch workspace:** All six workspaces (`1` through `6`).
- **Panes:** All panes belonging to the active workspace (including panes hidden on narrow terminals).
- **View:** Toggle chart, cycle interval, and toggle volume—plus contextual controls where visible (chart indicator, movers universe, alert trigger history on Alerts, and **Create or switch watchlist…** on Watch).
- **App:** Show settings file path, log out, and quit.

Menu items that would modify a persistent setting without any visible effect on the current pane are omitted so you never make a blind change.

---

## Keys reference

Press `?` anywhere in the TUI to view the contextual shortcut list for the currently focused pane.

### Global

| Key | Action |
| --- | --- |
| `1` – `6` | Switch workspace (`Overview`, `Trading`, `Portfolio`, `Derivatives`, `Orders`, `Invest`). `7` – `0` are unbound |
| `→` / `←` | Next / previous workspace |
| `Tab` / `Shift-Tab` | Next / previous pane in the active workspace |
| `` ` `` | Open the workspace picker |
| `z` | Zoom focused pane to full screen / restore split layout |
| `j` / `k`, `↓` / `↑` | Move selection down / up |
| `G` | Jump to last row |
| `g` | Jump to first row (except in Watch and Holdings, where `g` opens a GTT ticket) |
| `=` | Reset all dragged pane borders to default proportions |
| `:` | Open command palette |
| `Ctrl-K` | Open action menu |
| `T` | Open theme picker |
| `n` | Open notes pad |
| `F` | Open funds and margin breakdown modal |
| `A` | Jump to the Alerts pane and refresh alerts from Kite |
| `Ctrl-L` | Open session event log |
| `r` | Refresh focused pane (on Alerts: toggle selected alert; use `R` to refresh) |
| `?` | Open keyboard help overlay |
| `Esc` | Close top modal or step back (closes inline chart or alert history sub-panel) |
| `q`, `Ctrl-C` | Quit (`q` inserts text inside notes and search inputs; `Ctrl-C` quits from anywhere) |

### Chart

Available when a Chart pane is focused or when an inline chart is open on Orders, Holdings, or Positions:

| Key | Action |
| --- | --- |
| `c` | Open / close inline chart on Orders or Holdings (on Positions: use `Ctrl-K` → **Toggle chart**) |
| `C` | Expand chart to fill workspace / restore split layout |
| `i` | Cycle candle interval (`1m`, `5m`, `15m`, `1h`, `1D`) |
| `v` | Toggle volume histogram |
| `I` | Cycle technical indicator (`SMA`, `EMA`, `Bollinger`, `RSI`, `MACD`, `none`) |
| `h` / `l` | Step to older / newer candle |
| Mouse move | Inspect candle under pointer |
| Mouse click | Pin inspection cursor to clicked candle |

### By pane

| Key | Pane | Action |
| --- | --- | --- |
| `[` / `]` | Overview | Focus Positions / Holdings snapshot |
| `Enter` | Overview | Open selected row in the Portfolio workspace |
| `u` | Overview (Movers) | Cycle movers index universe |
| `/` | Watch | Search and add an instrument |
| `d` | Watch | Remove selected instrument from watchlist (immediate, no confirmation) |
| `w` | Watch | Create or switch watchlist (opens `:wl `) |
| `[` / `]` | Watch | Previous / next watchlist |
| `J` / `K` | Watch | Move selected row down / up within its group |
| `a` | Watch, Positions | Open price-alert form for selected row |
| `a` | Option Chain | Open price-alert form for focused option leg |
| `B` | Holdings | Open price-alert form for selected holding |
| `a` | Holdings | Open CDSL TPIN/OTP authorisation (for accounts requiring consent) |
| `a` | Alerts | Pick an instrument, then open the price-alert form |
| `a` / `e` / `d` / `c` | Alert form (basket focused) | Add / edit / remove / duplicate a basket order |
| `[` / `]` | Alert form (basket focused) | Move selected basket order earlier / later |
| `b` / `s` | Watch, Positions, Holdings, Chain | Open Buy / Sell order ticket (Chain: focused leg) |
| `g` | Watch, Holdings | Open GTT ticket |
| `c` | Positions | Open product conversion form |
| `S` | Positions, Orders, Holdings, MF, GTT, Alerts | Cycle sort column (`▲`/`▼` indicator; clicking a column header also sorts) |
| `Space` / `Ctrl-A` | Orders | Mark selected open order / mark all open orders |
| `x` | Orders | Cancel marked orders or selected open order (with confirmation) |
| `x` | GTT | Delete selected trigger (with confirmation) |
| `m` | Orders, GTT | Modify selected open order or active GTT trigger |
| `Enter` | Orders | View order detail and status history |
| `Enter` / `z` | Trading (Depth) | Expand 5-level depth ladder with full quote and range block |
| `Enter` | Alerts | Toggle trigger history panel for selected alert |
| `e` | Alerts | Edit selected alert |
| `d` | Alerts | Delete selected alert from account (with confirmation) |
| `r` / `R` | Alerts | Enable or disable selected alert (confirming first when enabling an ATO) / refresh alerts |
| `u` | Option Chain | Cycle built-in index underlying |
| `/` | Option Chain | Search for a stock underlying |
| `[` / `]` | Option Chain | Previous / next expiry |
| `h` / `l` | Option Chain | Focus Call (`CE`) / Put (`PE`) side |
| `[` / `]` | Bids | Previous / next section |
| `a` | Bids (IPO) | Toggle between open IPO issues and your applications |
| `Enter` | Bids | Open active section on Kite Web |
| `s` | Bids (Auctions) | Open sell order ticket for selected auction holding |

---

## Settings: tui.json

The TUI reads `~/.config/kite/tui.json` (on macOS, `~/Library/Application Support/kite/tui.json`) once at startup. Every field is optional; omitted fields or unrecognised enum values (such as an unknown preset theme name) fall back to that field's default.

> [!WARNING]
> If `tui.json` contains malformed JSON or a field has the wrong JSON type (for example, `"chart_volume": "invalid"` instead of a boolean), the TUI ignores the entire file and uses built-in defaults for all settings. Manual edits take effect on the next launch. Use `Ctrl-K` → **Show settings file path** to check the active path.

Interactive preferences changed inside the TUI are written back to `tui.json` immediately: theme (`T`), chart indicator (`I`), movers universe (`u`), pane split ratios, active workspace and focused panes, and the open watchlist per user ID. Saving updates only the modified keys while preserving any custom fields you added, formatting the file with sorted object keys. UTF-8 BOMs are accepted, and if duplicate keys exist in a hand-edited file, the last key wins.

```json
{
  "theme": "dracula",
  "default_product": "MIS",
  "chart_interval": "5m",
  "chart_volume": true,
  "chart_indicator": "ema",
  "movers_universe": "nifty500",
  "splits": { "trading_root": 0.45 },
  "poll_secs": {
    "orders": 5,
    "positions": 5,
    "holdings": 30,
    "margins": 30,
    "gtt": 60,
    "alerts": 60
  }
}
```

| Field | Supported values & behaviour |
| --- | --- |
| `theme` | Preset theme name or custom theme object (see [Themes](#themes)). |
| `default_product` | `CNC`, `MIS`, `NRML`, or `MTF`. Overrides the order ticket's initial product selection. |
| `chart_interval` | Initial chart interval at startup: `1m`, `5m`, `15m`, `1h` (also accepts `60m` or `hour`), or `1d`. |
| `chart_volume` | Boolean (`true` by default when omitted). Controls whether charts open with the volume subplot visible. Pressing `v` toggles volume for the current session only. |
| `chart_indicator` | `none`, `sma`, `ema`, `bollinger` (or `bb`), `rsi`, `macd`. Updated automatically when you press `I`. Unrecognised values default to `none`. |
| `movers_universe` | `nifty50`, `nifty100`, `nifty500`, `nifty_fno`, or `all`. Updated automatically when you press `u`. Unrecognised values inside a present `tui.json` fall back to `nifty500`. |
| `splits` | Object mapping split keys to the fractional width/height (`0.0`–`1.0`) occupied by the first pane. Saved when a border drag ends; cleared by pressing `=`. |
| `workspaces` | Per-user map tracking the active workspace, focused pane, and zoomed state. Managed automatically by the TUI. |
| `selected_watch` | Per-user map tracking the last open watchlist ID. Managed automatically by the TUI. |
| `poll_secs` | Background refresh intervals in seconds (clamped to `2`–`600`) for `orders`, `positions`, `holdings`, `margins`, `gtt`, and `alerts`. Mutual fund holdings share the `holdings` interval. |

- **`splits` keys:** `chart_right`, `chart_bottom`, `overview_main`, `overview_books`, `trading_root`, `trading_right`, `trading_bottom`, `portfolio_root`, `portfolio_right`, `derivatives_root`, `orders_root`, `orders_right`, `invest_root` (legacy `watch_detail` is ignored).
- **`workspaces` schema migration:** Written with a `version` field. The TUI transparently migrates `version: 1` state (from before multi-pane workspaces) to the corresponding modern workspaces and preserves unknown future versions untouched.

### Themes

`theme` accepts either a built-in preset name or a custom palette object. Built-in presets (in picker order):

`kite`, `ayu`, `catppuccin`, `catppuccin-frappe`, `catppuccin-macchiato`, `cobalt2`, `dracula`, `everforest`, `github-dark`, `gruvbox`, `kanagawa`, `material`, `matrix`, `monokai`, `nightowl`, `nord`, `one-dark`, `rose-pine`, `solarized`, `solarized-light`, `system`, `tokyo-night`.

- **Flexible matching:** Preset names ignore surrounding whitespace, letter case, and `-`/`_` separators (`tokyonight` and `tokyo_night` both match `tokyo-night`).
- **Default selection:** `kite` is the default on truecolour terminals. On Apple Terminal (when truecolour is not advertised) and when `TERM=dumb`, the default falls back to `system` unless an explicit `theme` is configured in `tui.json`.
- **`system` theme:** Inherits your terminal emulator's native background, foreground, and ANSI palette without overriding background or border fills. Note that the TUI does not read `NO_COLOR`.
- **Interactive switching:** Press `T`, select **Switch theme…** in `Ctrl-K`, or run `:theme NAME`. Typing in the theme picker filters the list and previews each theme live as you move the highlight. Press `Enter` to save or `Esc` to revert.
- **Preserving custom themes:** If you have a custom theme object in `tui.json` and switch to a built-in preset using the picker, the TUI backs up your custom object under `"saved_custom_theme"` in `tui.json` so you can copy it back to `"theme"` at any time.

A custom theme object can specify any subset of the keys below using standard 16-colour ANSI names (case-insensitive) or `#RRGGBB` hex strings. Omitted or invalid keys fall back to terminal defaults (the legacy three-key `{ "accent", "up", "down" }` format continues to work):

```json
{
  "theme": {
    "background": "#1e1e2e",
    "surface": "#313244",
    "text": "#cdd6f4",
    "text_muted": "#b6b8c2",
    "border": "#45475a",
    "selection": "#45475a",
    "accent": "#cda9f7",
    "up": "green",
    "down": "#ff9f9f",
    "warning": "#f9e2af",
    "error": "#f5a0b8",
    "info": "#92bafa"
  }
}
```

| Key | UI elements coloured |
| --- | --- |
| `background` | Main terminal background (omit to keep your terminal's native background) |
| `surface` | Modal and floating panel backgrounds |
| `text` | Primary body text (omit to keep your terminal's default foreground) |
| `text_muted` | Labels, key hints, dividers, and table column headers |
| `border` | Inactive pane and table borders |
| `selection` | Selected table row background (omit to use reversed terminal colours) |
| `accent` | The `kite` header mark, active tab, focused pane/modal borders, and input prompts |
| `up` | Gains, buy tags, bids, live connection indicator, and success messages |
| `down` | Losses, sell tags, asks, and offline/expired status indicators |
| `warning` | GTT and confirmation borders, pending states, pre-open phase, and reconnecting indicator |
| `error` | Error toasts, form validation errors, and failed lookups |
| `info` | Informational messages |

#### Contrast

In every built-in preset except `system`, every foreground text token (`text`, `text_muted`, `accent`, `up`, `down`, `warning`, `error`, `info`) is verified to maintain a contrast ratio of at least **4.5:1** ([WCAG 2.2 AA minimum for normal text](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html)) against that preset's `background`, `surface`, and `selection` fills:

- **`selection` contrast:** Selected table rows preserve per-cell semantic colours (gains remain green and losses remain red on the highlighted row), so foreground tokens are contrast-checked against `selection` as well.
- **Tuned editor palettes:** Presets inspired by popular editor themes preserve their signature background and border hues while adjusting foreground lightness where needed to meet the 4.5:1 threshold.
- **Solid badges:** Text rendered atop solid accent or status fills (the active workspace tab, active selector pills, and the notes cursor) automatically picks black or white for maximum legibility.
- **Non-colour cues:** Row selection never relies on colour alone—the selected row is marked with a `▶ ` prefix and bold text, and active Bids section tabs are underlined.
- **Custom and `system` palettes:** Custom theme objects are applied verbatim without automatic contrast modification.

---

## Safety

**Actions that always require explicit review and confirmation:**

The following actions open a confirmation modal showing the exact parameters or changes before anything is transmitted. The modal footer states whether you are confirming a live Kite API request, a local change, or a session logout:

- Placing, modifying, or cancelling (`x`) an order (including auction sell orders)
- Creating, modifying, or deleting (`x`) a GTT trigger
- Converting a position's product (`c` on Positions)
- Saving an Alert-to-Order (`ATO`) alert, enabling a disabled `ATO` alert (`r` on Alerts), or deleting any alert (`d` on Alerts)
- Deleting a watchlist (`:wl delete`)
- Starting CDSL TPIN/OTP authorisation (`a` on Holdings), logging out, or deleting a local note (`d` in Notes)

**Actions sent immediately without a confirmation modal:**

- Toggling a **Simple** alert between enabled and disabled, or disabling an **ATO** alert (`r` on the Alerts pane)
- Saving a **Simple** (notify-only) alert (`Enter` in the alert form)
- Watchlist edits: adding (`/`), removing (`d`), reordering (`J`/`K`), creating (`:wl NAME`), or renaming (`:wl rename NAME`) a watchlist

**Connection, session, and timeout safeguards:**

- **Expired authentication:** If Kite rejects your access token, the header shows `○ auth expired`, trading shortcuts display ``auth expired, run `kite auth login` ``, and the active session terminates cleanly back to the sign-in screen. While an order request is in flight, trading keys wait for it to finish before initiating another order action.
- **Network drops:** On transient network failures, the TUI keeps the last known data on screen, displays an error toast, and retries automatically. The WebSocket ticker reconnects with exponential backoff up to 30 seconds. Empty panes explain why data is unavailable and show the key to retry.
- **Credential privacy:** The TUI never displays passwords or access tokens on screen or in logs.
- **Write timeouts:** Because a timed-out order, GTT, or conversion write has an uncertain server outcome, the TUI **never retries it automatically**. Instead, it refreshes the affected book from Kite and prompts you to inspect your orders/positions before retrying manually.

---

## Files the TUI writes

| File | Contents |
| --- | --- |
| `~/.config/kite/tui.json` | Theme, preferences, pane border splits, poll intervals, workspace focus, and open watchlist per user ID |
| `~/.config/kite/notes.json` | Local trading notes and scratchpad entries |
| `~/.config/kite/config.json` | Saved session credentials written by the sign-in screen (see [Configuration](configuration.md)) |
| `~/.cache/kite/instruments.json` | Cached exchange instrument master |

Every file save writes to a temporary file in the same directory and atomically renames it into place, ensuring an interrupted write never leaves a corrupted or truncated file. `notes.json` is only rewritten when note contents actually change. Symlinked configuration, settings, and notes files are respected (writes replace the resolved target file, while broken symlinks report an error).

Running two TUI instances concurrently in separate terminals does not synchronise local state: the last instance to save wins, and concurrent note edits do not merge. A `kite tui --token` session never writes to `notes.json`.

---

## Connection diagnostics

Press `Ctrl-L` inside the TUI to inspect the current session's event log (no debug log is written to disk). To test your REST session and WebSocket ticker independently from the shell, run:

```bash
kite user profile
kite stream ticks --token 256265 --limit 1 --timeout-seconds 10
```

`kite user profile` verifies the signed-in REST session (there is no `auth whoami` subcommand).

A WebSocket ticker handshake rejection does not necessarily mean your REST session has expired. The ticker error includes the server's sanitised rejection reason—for example, `Invalid auth token.` can indicate that a custom ticker endpoint does not accept the configured authentication mode. Check your endpoint and authentication settings (or run `kite doctor`) before signing in again.

---

## What the TUI does not do

- **Multi-leg options strategy builder:** The Derivatives workspace provides a live option chain ladder and single-leg order ticket, not payoff graphs or multi-leg strategy execution.
- **Mutual fund order placement or SIP creation:** The Invest workspace's MF pane is read-only (use [`kite mf`](mutual-funds.md) CLI commands to inspect orders, SIPs, and fund lists).
- **Multi-account switching:** Switching to a different Kite user ID requires logging out and signing back in.
- **Custom columns, keybindings, or indicator periods:** Table columns, keyboard shortcuts, and technical indicator lookback periods are fixed.
- **OS desktop notifications:** Kite dispatches push/email alerts from its servers; the TUI adds an in-app message and terminal bell while open.
- **Job control suspension (`Ctrl-Z`):** The TUI does not suspend on `Ctrl-Z`, and modifier combinations (`Alt`/`Ctrl` + letter) do not trigger single-letter pane shortcuts.

[Back to the guide](README.md)
