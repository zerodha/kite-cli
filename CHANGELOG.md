# Changelog

All notable changes to this project are documented in this file.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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
