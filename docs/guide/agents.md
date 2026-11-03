# Using Kite CLI with AI Agents

AI coding agents that can run shell commands—such as Claude Code, Codex, and Cursor—can use `kite` to analyse your portfolio, inspect mutual fund holdings and SIPs, and research live or historical market data directly from your terminal.

Because `kite` provides structured `--json` output on every command, separates `stdout` from `stderr`, and keeps sign-in strictly interactive, an agent can query your account and market data without ever seeing your password, two-factor code, or raw access token.

## Quick setup: give this to your agent

Copy and paste this prompt into your AI coding agent:

```text
Fetch and follow the instructions at https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/prompt.md to set me up with Kite CLI.
```

When the agent runs those instructions, it will:

1. Check if `kite` is installed (and run the official release installer if needed).
2. Download the official [`kite` agent skill](../../skills/kite/SKILL.md) into your agent's skills directory (`~/.claude/skills/kite/SKILL.md` for Claude Code or `~/.agents/skills/kite/SKILL.md` for Codex and compatible agents).
3. Run `kite --json doctor` to verify the installation and check whether an active session is present.

### Sign in from your own terminal

Agents cannot sign in on your behalf. In your own interactive terminal window, sign in once for the day:

```bash
kite auth login
kite doctor
```

Once signed in, your agent shares the active session for read-only queries until the token expires the next morning around 06:00 IST.

### Manual skill installation

If you prefer to install the skill yourself instead of using the setup prompt:

```bash
# Claude Code
mkdir -p ~/.claude/skills/kite
curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/kite/SKILL.md -o ~/.claude/skills/kite/SKILL.md

# Codex and other agents that read ~/.agents/skills
mkdir -p ~/.agents/skills/kite
curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/kite/SKILL.md -o ~/.agents/skills/kite/SKILL.md
```

## Why `kite` works well with agents

- **Structured `--json` output** — Passing `--json` to any command prints Kite's raw `data` payload on `stdout`, making it straightforward for an agent to pipe results into `jq`, Python, or shell scripts:
  ```text
  $ kite --json portfolio holdings | jq -r '.[] | [.tradingsymbol, .quantity, .last_price] | @tsv'
  AARON	1	352.95
  SBIN	16	762.45
  ```
- **Clean `stdout` and `stderr` separation** — Warnings, hints, and empty-state notices go to `stderr`, while `stdout` carries only the data payload.
- **Self-discoverable flags** — Agents can run `kite --help` or `kite <command> --help` to inspect subcommands and flags without needing a separate MCP server.
- **Local instrument search and resolution** — `kite market search` and `kite market resolve` query the local instrument master cache (without requiring an API call) to look up exact trading symbols, lot sizes, and the numeric `instrument_token` values needed for historical candle data.

## Example workflows and prompts

Once `kite` is signed in and the skill is installed, you can ask your agent natural-language questions about your portfolio and the market.

### 1. Portfolio breakdown and concentration check

> **Prompt:** *"Check my Kite equity holdings and mutual fund holdings. Group my overall allocation, list my top gainers and losers by net P&L, and flag any single stock that makes up more than 15% of my equity portfolio."*

Behind the scenes, the agent queries:

```bash
kite --json portfolio holdings
kite --json mf holdings
```

The agent can combine free demat quantities (`quantity`), T1 quantities (`t1_quantity`), collateral (`collateral_quantity`), and MTF quantities (`mtf.quantity`) from the JSON response to compute exact position weights and P&L across your holdings.

### 2. Historical price and volatility research

> **Prompt:** *"Look up `NSE:INFY` and `NSE:TCS`, fetch their daily candles for the past 6 months, and compare their cumulative return, annualized volatility, and maximum drawdown."*

Behind the scenes, the agent resolves each symbol to its numeric `instrument_token` and fetches historical candles:

```bash
kite --json market resolve NSE:INFY
kite --json historical candles --instrument-token 408065 --interval day \
    --from "2026-05-01 09:15:00" --to "2026-11-01 15:30:00"
```

### 3. Mutual fund SIPs and cash planning

> **Prompt:** *"List all my active mutual fund SIPs with their instalment amounts and next due dates, and compare the total monthly commitment against my available equity margin."*

Behind the scenes, the agent runs:

```bash
kite --json mf sips
kite --json user margins --segment equity
```

### 4. Market quotes and depth inspection

> **Prompt:** *"Compare live quotes, 52-week/circuit ranges, and top-of-book bid/ask spreads for `NSE:INFY` and `NSE:SBIN`, and summarise my open positions for today."*

Behind the scenes, the agent runs:

```bash
kite --json market quote NSE:INFY NSE:SBIN
kite --json portfolio positions
```

## Safety and credential boundaries

`kite` and the bundled [`SKILL.md`](../../skills/kite/SKILL.md) enforce strict boundaries when used with an AI agent:

1. **Human-only authentication** — `kite auth login` requires an interactive terminal TTY and refuses to run with `--json` or piped input. The skill instructs the agent never to run `kite auth login` or ask for your user ID, password, or 2FA code.
2. **Credential redaction** — `kite doctor` and `kite config show` redact sensitive credentials by default. The skill instructs the agent never to read `config.json` or print `KITE_ACCESS_TOKEN` / `KITE_API_KEY`.
3. **Dry-run default on writes** — Any state-changing command in `kite` runs as a dry-run preview unless `--confirm` is explicitly passed, and the skill instructs the agent never to send a write without your explicit approval.
4. **No interactive TUI calls** — The skill instructs the agent to use non-interactive `--json` subcommands rather than launching the full-screen terminal UI (`kite` or `kite tui`).

---

[Back to the user guide](README.md)
