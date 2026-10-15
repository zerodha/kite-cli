# Configuration

This guide covers configuration file paths, environment variables, build-time overrides, session management, and local file permissions.

```bash
kite config path
kite config show
KITE_CONFIG=~/kite-test.json kite doctor --no-live
```

## Configuration file (`config.json`)

`kite` reads a single JSON configuration file, resolved in the following order:

1. `--config <path>` (global CLI flag)
2. `KITE_CONFIG` environment variable (when non-empty)
3. Operating system default path:

| Operating system | Default configuration path |
| :--- | :--- |
| **Linux** | `$XDG_CONFIG_HOME/kite/config.json` (or `~/.config/kite/config.json` when unset) |
| **macOS** | `~/Library/Application Support/kite/config.json` |
| **Windows** | `%APPDATA%\kite\config.json` |

Run `kite config path` to print the active configuration path. The file does not need to exist beforehand—`kite auth login` creates it automatically.

### Supported fields

| Field | Written by | Description |
| :--- | :--- | :--- |
| `user_id` | `kite auth login` | Your Kite user ID (offered as the default on subsequent logins) |
| `access_token` | `kite auth login` | Active session token (removed by `kite auth logout`) |
| `app_uuid` | `kite auth login` | Random v4 UUID sent as the app identity (`kite` generates an in-memory identity for read commands if none is saved) |
| `api_key` | User | Kite API key (required only when running a source build without a built-in key) |
| `api_root` | User | Base REST API URL (required only when running a source build without a built-in URL) |
| `ticker_url` | User | WebSocket ticker URL (required only when running a source build without a built-in URL) |

Official release binaries embed `api_key`, `api_root`, and `ticker_url` at build time, so release users never need to configure those three fields manually. Source builds require them; see [Getting started — From source](getting-started.md#from-source).

When reading `config.json`, `kite` ignores unknown fields for forward compatibility. `kite auth login` and `kite auth logout` rewrite only the supported fields.

### Inspecting effective configuration (`kite config show`)

`kite config show` displays the active configuration after environment variable overrides are applied, redacting the user ID and truncating the access token. Pass `--reveal` to print full unredacted values.

```text
$ kite config show
effective.access_token  abcd…2345
effective.api_key       my_api_key
effective.api_root      https://api.example.invalid
effective.app_uuid      00000000-0000-4000-8000-000000000000
effective.ticker_url    wss://ticker.example.invalid
effective.user_id       ********
exists                  true
path                    /home/user/.config/kite/config.json
```

## Environment variables

### Runtime variables

| Variable | Effect |
| :--- | :--- |
| `KITE_CONFIG` | Override the path to `config.json` |
| `KITE_API_KEY` | Override `api_key` |
| `KITE_API_ROOT` | Override `api_root` |
| `KITE_TICKER_URL` | Override `ticker_url` |
| `KITE_USER_ID` | Override `user_id` |
| `KITE_ACCESS_TOKEN` | Override `access_token` (see [`KITE_ACCESS_TOKEN`](#kite_access_token) below) |
| `NO_COLOR` | Any value (including empty) disables ANSI colour in CLI output |
| `TERM` | `dumb` disables ANSI colour in CLI output |
| `COLUMNS` | Override detected terminal width for CLI table formatting |
| `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` | Override default config and cache base directories on Linux |

Empty or whitespace-only `KITE_*` environment variables are ignored.

For `api_key`, `api_root`, and `ticker_url`, precedence is:
1. Runtime environment variable (`KITE_API_KEY`, `KITE_API_ROOT`, `KITE_TICKER_URL`)
2. `config.json` field
3. Value compiled into the binary at build time

> [!NOTE]
> Public instrument-master and mutual-fund scheme downloads always use the standard Kite Connect host independently of `api_root`. Because `kite doctor` checks the configured account API (`api_root`), a passing `doctor` check does not test connectivity to the public instrument download host.

### Build-time variables

When compiling from source with `cargo build` or `cargo install`, these environment variables embed default endpoints and keys into the binary (which runtime environment variables and `config.json` can still override):

| Variable | Sets built-in default for |
| :--- | :--- |
| `KITE_API_ROOT` | `api_root` |
| `KITE_DEFAULT_API_KEY` | `api_key` |
| `KITE_TICKER_URL` | `ticker_url` |

## Sessions

`kite auth login` writes `user_id`, `access_token`, and `app_uuid` to `config.json`. It never stores your password or 2FA code.

- **Daily expiry** — Kite access tokens expire daily around **06:00 IST**. `kite` never renews tokens automatically. When Kite rejects an expired token with `TokenException`, CLI commands exit with an error prompting you to run `kite auth login`, and the TUI returns to its sign-in screen.
- **Logout** — `kite auth logout` revokes the saved token on Kite's servers and removes `access_token` from `config.json`, while preserving `user_id`, `app_uuid`, and endpoint settings. If remote revocation fails, `kite` warns you and still deletes the local token; sign in to Kite Web and log out there to ensure server-side revocation.

### `KITE_ACCESS_TOKEN`

Setting `KITE_ACCESS_TOKEN` in your environment instructs `kite` to use that token instead of reading `access_token` from `config.json`—useful when sharing a token obtained by another tool without writing a config file.

- If you run `kite auth login` while `KITE_ACCESS_TOKEN` is set in your shell, login writes the new token to `config.json`, but `KITE_ACCESS_TOKEN` continues to take precedence until you `unset KITE_ACCESS_TOKEN`.
- `kite auth logout` only revokes and removes the token stored in `config.json`; it does not revoke `KITE_ACCESS_TOKEN`.
- `kite doctor` and `kite config show` report the active token from `KITE_ACCESS_TOKEN` (redacted by default).

## Local files and permissions

| File | Location & contents |
| :--- | :--- |
| `config.json` | Session token and API settings, located in the OS config directory or at `--config` / `KITE_CONFIG` |
| `tui.json` | TUI theme, layout splits, and preferences (always in the OS default config directory; see [TUI settings](tui.md#settings-tuijson)) |
| `notes.json` | Local trading notes created in the TUI (always in the OS default config directory) |
| `instruments.json` | Cached instrument master in the OS cache directory (`~/.cache/kite/` on Linux, `~/Library/Caches/kite/` on macOS) |

- **Permissions** — On Unix, `config.json`, `tui.json`, and `notes.json` are created with mode `0600` (accessible only by your user). `instruments.json` contains public data and uses default OS file permissions.
- **Atomic writes** — All four files are written to a sibling temporary file and atomically renamed into place so an interrupted write never leaves a corrupted or partial file.

> [!CAUTION]
> `config.json` contains a live access token. Never share it or commit it to version control. If `config.json` is ever exposed, run `kite auth logout` immediately and verify that revocation succeeded.

---

[Back to the user guide](README.md)
