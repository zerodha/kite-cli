# Installation

Install the latest stable release of `kite` (or the newest prerelease if no stable release is published yet) on **Linux** and **macOS**:

```sh
curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/scripts/install.sh | sh
```

You can also browse and download release archives directly at [github.com/zerodha/kite-cli/releases](https://github.com/zerodha/kite-cli/releases).

On **Windows x86_64**, run this command in PowerShell:

```powershell
irm https://raw.githubusercontent.com/zerodha/kite-cli/main/scripts/install.ps1 | iex
```

The Windows installer verifies the archive checksum, installs to a user-writable directory, and adds that directory to the user `PATH`. Open a new terminal after installation.

## Supported platforms

| Operating system | Architectures | Notes |
| :--- | :--- | :--- |
| **Linux** | `x86_64`, `aarch64` | Statically linked (`musl`); no `glibc` dependency |
| **macOS** | `x86_64` (Intel), `aarch64` (Apple Silicon) | Native Mach-O binaries |
| **Windows** | `x86_64` | Native MSVC binary |

## How the installer works

1. **Platform detection** — The shell installer detects `Linux` or `Darwin`; Windows uses `scripts/install.ps1` for the x86_64 MSVC build.
2. **Checksum verification** — Downloads the release archive alongside `checksums.txt` and verifies the SHA-256 digest before extracting the `kite` binary.
3. **Install location** — Selects a writable directory in the following order:
   - `$KITE_INSTALL_DIR` (if explicitly set in your environment)
   - `~/.local/bin` (if writable and already on `PATH`)
   - `~/.cargo/bin` (if writable and already on `PATH`)
   - `/usr/local/bin` (if writable by your user)
   - `~/.local/bin` (fallback; prints instructions to add it to your shell's `PATH`)

For safety, the installer never installs into the current working directory and refuses to overwrite an existing `kite` symlink.

## Verify your installation

After installing, sign in to your Kite account and run the built-in diagnostic check:

```sh
kite auth login
kite doctor
```

- `kite auth login` prompts for your Kite user ID, password, and 2FA code in an interactive terminal.
- `kite doctor` verifies your configuration and active session.
- Run `kite` (with no arguments) to launch the full-screen terminal UI.

## Reporting issues & security

When reporting a bug, include:

- The output of `kite --version`
- The exact CLI command or TUI steps to reproduce
- What you expected vs. what happened
- Your operating system and terminal emulator

> [!CAUTION]
> Redact personal account details from screenshots before sharing. **Never share your password, two-factor codes, or `config.json`.** Your `config.json` contains a live access token; if it is ever exposed, run `kite auth logout` immediately to revoke the token.

[Back to documentation](README.md)
