# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.2.3] - 2026-11-12

### Changed

- Positions shows `CHG%`, each position's move from its average price like Kite Web's `Chg.`, instead of the instrument's day change. A short shows a price fall as negative, as Kite does. `S` sorts by it as `chg%`.
- Holdings renames its `P&L%` column to `CHG%` to match. The values are unchanged.

### Fixed

- `kite alerts modify --rhs` on a constant alert now defaults the right-hand attribute to `LastTradedPrice`, as the help says. It previously sent an empty attribute.
- `kite alerts modify` treats an empty stored right-hand exchange, symbol or attribute as missing. It previously sent the empty value.
- `kite orders place --autoslice` lists each slice's order ID, or the reason it failed, and the headline counts failed slices. It previously showed only the parent order ID, so a rejected slice went unnoticed.

## [0.2.2] - 2026-11-11

### Added

- Press `Enter` on Positions to open the selected position's chart.
- Chart any instrument without adding it to a watchlist: press `/` on the Trading chart pane, or run **Chart any instrument…** from the command palette.
- Holdings shows an invested, current, P&L and day change summary like Mutual funds.
- The order detail popup shows the exchange order ID and exchange time. Orders that never reached the exchange, such as rejected orders, show `not assigned`.
- `kite doctor` reports when a newer release is available. `--no-live` skips the check, and a failed check does not fail `doctor`.
- `kite upgrade` installs the latest release, or `--version X`, by running the official installer from that release's tag. `kite upgrade --check` only reports whether an update exists. Nix store and symlinked installs are refused; on Windows it prints the installer command to run after `kite` exits.

### Changed

- The Overview Positions panel shows each position's P&L% against its average price, signed for shorts, instead of the instrument's day change. Closed positions show no percentage.
- The Overview Total P&L tile shows the holdings P&L as a percentage of invested value.

### Fixed

- Positions counts a closed position's P&L as realised. Kite reports `realised` as `0` for positions squared off today, so the summary showed it as unrealised.
- Closed positions show their symbol and P&L greyed out.
- Prices with six or more digits are no longer cut off in the watchlist, orders, GTT, holdings and depth panes.

## [0.2.1] - 2026-10-27

### Fixed

- The Overview movers panel loads again. The screener service now accepts only form-encoded request bodies and rejected the JSON body with HTTP 415.

## [0.2.0] - 2026-10-23

### Changed

- The Kite Connect client is now the [`kiteconnect`](https://crates.io/crates/kiteconnect) 1.0 crate from [zerodha/kiteconnect-rs](https://github.com/zerodha/kiteconnect-rs), with its own release cycle.

## [0.1.0] - 2026-10-22

First public release.

### Terminal UI

- Six workspaces: Overview, Trading, Portfolio, Option Chain, Orders, and IPO & Bids. Switch with `1` to `6`, resize or maximise panes, and search instruments from the command palette.
- Live quotes, indices, 5-level market depth and top movers across Kite watchlists or custom instrument lists.
- Candlestick charts with volume, multiple intervals, and SMA, EMA, Bollinger Bands, RSI and MACD.
- Option chains with open interest, expiry selection and a chart of the underlying.
- Holdings, positions, mutual fund holdings, funds and live P&L in Portfolio.
- Orders, GTT triggers and price alerts, including alert-to-order baskets.
- IPOs, government securities, auctions and corporate actions in IPO & Bids.
- Local trading notes and 22 themes with live preview (`T`).

### CLI

- Commands for profile, margins, holdings, positions, orders, trades, quotes, historical candles, instrument search, GTTs, price alerts and mutual funds.
- `--json` on every command for scripts, with stable exit codes.
- Order, GTT and alert writes print the request as a dry run unless `--confirm` is passed.
- `kite stream ticks` for live ticks, and `kite request` for any other Kite Connect endpoint.
- `kite doctor` to check the install and session.

### Install

- Binaries for Linux and macOS (x86_64 and aarch64) and Windows (x86_64).
- Shell and PowerShell installers that verify the archive checksum.

### Agents

- Official agent skill in `skills/kite` so AI coding agents use the CLI safely.
