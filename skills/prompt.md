# Kite CLI — Agent Setup Instructions

Follow these steps to install the `kite` CLI, add the official `kite` skill to your agent environment, and verify readiness.

## 1. Check or install `kite`

Check whether `kite` is already installed:

```sh
kite --version
```

If `kite` is not installed, run the official release installer for the user's platform:

- **Linux and macOS:**
  ```sh
  curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/scripts/install.sh | sh
  ```
- **Windows (PowerShell):**
  ```powershell
  irm https://raw.githubusercontent.com/zerodha/kite-cli/main/scripts/install.ps1 | iex
  ```

After installation, re-run `kite --version` (or `~/.local/bin/kite --version` if `~/.local/bin` is not yet on `PATH`).

## 2. Install the `kite` agent skill

Download [`SKILL.md`](https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/kite/SKILL.md) into the skills directory for your agent:

- **Claude Code:**
  ```sh
  mkdir -p ~/.claude/skills/kite
  curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/kite/SKILL.md -o ~/.claude/skills/kite/SKILL.md
  ```
- **Codex and agents that read `~/.agents/skills`:**
  ```sh
  mkdir -p ~/.agents/skills/kite
  curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/kite/SKILL.md -o ~/.agents/skills/kite/SKILL.md
  ```

If you are inside a local checkout of `zerodha/kite-cli`, you can copy `skills/kite/SKILL.md` directly instead of downloading it.

## 3. Read the skill and verify the session

1. Read the installed `SKILL.md` so its safety rules and `--json` command patterns are in your context.
2. Check installation and session status:
   ```sh
   kite --json doctor
   ```
3. **Never run `kite auth login` yourself**, and never ask the user for their Kite user ID, password, or 2FA code.
   - If `kite --json doctor` returns `"ok": true`, confirm to the user that `kite` and the skill are ready.
   - If the session is missing or expired, ask the user to run `kite auth login` in their own terminal and let you know when they are done.
