# User Guide

Start with [Getting started](getting-started.md) to install `kite`, sign in to your Kite account, and verify your session. Then explore the task guides below.

## Setup & Configuration

| Guide | Topics covered |
| :--- | :--- |
| [Getting started](getting-started.md) | Installation, interactive login, `kite doctor` checks, first commands, and session expiry |
| [Configuration](configuration.md) | `config.json` paths, environment variables, build-time overrides, sessions, and file permissions |
| [Troubleshooting](troubleshooting.md) | Session errors, rate limits, Cloudflare blocks, network failures, cache issues, and display fixes |

## Trading, Portfolio & Market Data

| Guide | Topics covered |
| :--- | :--- |
| [Portfolio and funds](portfolio.md) | Holdings, P&L calculations, excluded holdings warnings, net/day positions, auctions, and margins |
| [Orders](orders.md) | Dry-run previews, placing/modifying/cancelling orders, GTT triggers, position conversion, and margin/charge calculators |
| [Market data](market-data.md) | Live LTP/OHLC/quotes, 5-level depth, historical candles, instrument search/cache, and WebSocket tick streaming |
| [Price alerts](alerts.md) | Creating and managing simple alerts, cross-instrument conditions, and alert-to-order (ATO) baskets |
| [Mutual funds](mutual-funds.md) | Viewing mutual fund holdings, recent orders, active/paused SIPs, and searching schemes |

## Terminal UI & Automation

| Guide | Topics covered |
| :--- | :--- |
| [Terminal UI (TUI)](tui.md) | Workspaces (`1`–`6`), watchlists, order/GTT tickets, ATO baskets, charts, local notes, keybindings, and `tui.json` themes |
| [Scripting](scripting.md) | `--json` output, exit codes, stdout/stderr separation, piping, raw API calls (`kite request`), and command aliases |
| [Agent skills](../../skills/README.md) | Installing the official `kite` skill so AI coding agents use the CLI safely |

## Command Reference

For a complete list of every command, subcommand, and flag, see the [CLI reference](../CLI_REFERENCE.md) or run `kite <command> --help`.

[Back to documentation index](../README.md)
