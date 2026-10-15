# Getting started

This guide covers installing `kite`, signing in to your Kite account, verifying your session, and running your first commands.

> [!IMPORTANT]
> `kite` connects to your **live Zerodha Kite account**. CLI trading and write commands preview their requests (dry-run mode) unless you explicitly pass `--confirm`. Login and logout take effect immediately. See [Orders](orders.md) for CLI write rules and [TUI safety](tui.md#safety) for terminal workspace confirmation behaviour.

## Install

Follow the [Installation guide](../INSTALL.md) to install a prebuilt release binary. Release binaries include the built-in API settings required for login and market data.

### From source

To build from source, install the Rust toolchain specified in `rust-toolchain.toml`, then run:

```bash
git submodule update --init --recursive
cargo install --locked --path .
```

Source builds require an API root, API key, and WebSocket ticker URL. You can supply these via environment variables, `config.json`, or build-time variables. See [Configuration — Environment variables](configuration.md#environment-variables) for the full list and precedence rules.

## Sign in

Run `kite auth login` in an interactive terminal:

```bash
kite auth login
```

1. **User ID** — Enter your Kite user ID (or pass it directly as `kite auth login AB1234`).
2. **Password** — Enter your password (masked as you type).
3. **Verification method** — Select a 2FA method if your account offers more than one.
4. **Two-factor code** — Enter your TOTP or OTP code (for SMS verification, `kite` requests the OTP before prompting you).

Press `Enter` at any prompt to accept a default value shown in brackets.

- **Credential storage** — `kite` saves only your `user_id` and `access_token` to `config.json`. It **never** stores your password or two-factor code. On Unix systems, `config.json` is written with mode `0600` (readable and writable only by your user).
- **Interactive requirement** — `kite auth login` requires an interactive TTY and refuses `--json` or piped stdin. You can also sign in directly through the TUI by running `kite`.

To print the path to your session file:

```bash
kite config path
```

## Check your session

```bash
kite doctor
```

`kite doctor` validates your local configuration, checks stored credentials, and performs a single read-only API call to `GET /user/margins`. It also displays the active API root and WebSocket ticker URL while redacting sensitive credentials.

- Pass `--no-live` (`kite doctor --no-live`) to validate local settings without making a network call.
- If any check fails, follow the printed hint or consult [Troubleshooting](troubleshooting.md).

## Run your first commands

```bash
kite user profile             # Account profile and enabled exchanges
kite user margins             # Available and utilised funds
kite portfolio holdings       # Demat and MTF holdings with P&L
kite orders list              # Today's order book
kite market ltp NSE:INFY      # Last traded price
kite tui                      # Full-screen terminal UI (or simply `kite`)
```

Run `kite --help` to list all command groups, or `kite <command> --help` to inspect flags for a specific command.

## Sign out

```bash
kite auth logout
```

`kite auth logout` revokes the saved access token on Kite's servers and removes `access_token` from `config.json` while retaining your `user_id` for the next login. It takes effect immediately and does not require `--confirm`.

- If remote revocation fails (for example, due to a network error), `kite` warns you and still removes the local token. To ensure the session is terminated on Kite's side, sign in to Kite Web and log out there.
- If you override your session with the `KITE_ACCESS_TOKEN` environment variable, see [`KITE_ACCESS_TOKEN` logout rules](configuration.md#kite_access_token) — `kite auth logout` only revokes the token stored in `config.json`.

## Session expiry

Kite access tokens expire daily around **06:00 IST** (or earlier if invalidated by another login or logout). `kite` does not automatically renew expired tokens:

- **CLI** — Commands fail with a session-expired error and prompt you to run `kite auth login`.
- **TUI** — The workspace returns to the sign-in screen with your user ID pre-filled.

---

[Back to the user guide](README.md)
