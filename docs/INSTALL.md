# Installation

Install the latest release of `kite` on **Linux** and **macOS**:

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

To install a specific version, set `KITE_VERSION` (shell) or pass `-Version` (PowerShell):

```sh
curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/scripts/install.sh | KITE_VERSION=0.1.0 sh
```

## Upgrading

```sh
kite upgrade --check            # report whether a newer release exists
kite upgrade                    # upgrade to the latest release
kite upgrade --version 0.2.1    # install a specific release, including an older one
```

`kite doctor` also reports when a newer release is available.

`kite upgrade` downloads the installer from the target release's tag and runs it with `KITE_VERSION` and `KITE_INSTALL_DIR` set to the directory of the running `kite`, so it replaces that binary in place. It refuses binaries in the Nix store or reached through a symlink: upgrade those with your package manager. On Windows, a running `kite.exe` cannot be replaced, so `kite upgrade` prints the PowerShell command to run after `kite` exits.

Re-running the install command also upgrades. Without `KITE_INSTALL_DIR` it picks a directory as described above, which may differ from where your current `kite` lives. If both directories are on `PATH`, whichever comes first wins, and the old binary can shadow the new one. Check with `command -v kite` and `kite --version`.

## Verify your installation

After installing, launch the full-screen terminal UI directly:

```sh
kite
```

If you aren't signed in yet, `kite` opens an interactive sign-in screen inside the UI.

To sign in from the shell for non-interactive CLI commands and verify your setup:

```sh
kite auth login
kite doctor
```

- `kite auth login` prompts for your Kite user ID, password, and 2FA code in the terminal (the TUI and CLI share the same session).
- `kite doctor` verifies your configuration and active session.

## Reporting issues & security

When reporting a bug, include:

- The output of `kite --version`
- The exact CLI command or TUI steps to reproduce
- What you expected vs. what happened
- Your operating system and terminal emulator

> [!CAUTION]
> Redact personal account details from screenshots before sharing. **Never share your password, two-factor codes, or `config.json`.** Your `config.json` contains a live access token; if it is ever exposed, run `kite auth logout` immediately to revoke the token.

[Back to documentation](README.md)
