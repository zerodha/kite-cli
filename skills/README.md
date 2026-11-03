# Agent skills

Skills teach AI coding agents how to use `kite` safely: query portfolio and market data with `--json`, keep write commands in dry-run mode unless explicitly approved, and leave sign-in to you.

| Skill | Use |
| :--- | :--- |
| [`kite`](kite/SKILL.md) | Portfolio analysis, market data, mutual funds, and scripting with the `kite` CLI |

## Quick setup (give this to your agent)

Paste this prompt into Claude Code, Codex, Cursor, or another coding agent to install `kite` and the official skill automatically:

```text
Fetch and follow the instructions at https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/prompt.md to set me up with Kite CLI.
```

Then sign in from your own terminal with `kite auth login`.

## Manual install

Install `kite` and sign in first (`kite auth login`). Then download the skill into your agent's skills folder:

```sh
# Claude Code
mkdir -p ~/.claude/skills/kite
curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/kite/SKILL.md -o ~/.claude/skills/kite/SKILL.md

# Codex and other agents that read ~/.agents/skills
mkdir -p ~/.agents/skills/kite
curl -fsSL https://raw.githubusercontent.com/zerodha/kite-cli/main/skills/kite/SKILL.md -o ~/.agents/skills/kite/SKILL.md
```

If you have cloned this repository, you can copy `skills/kite` directly (`cp -r skills/kite ~/.claude/skills/` or `cp -r skills/kite ~/.agents/skills/`).

For example workflows and safety details, see [Using with AI agents](../docs/guide/agents.md).
