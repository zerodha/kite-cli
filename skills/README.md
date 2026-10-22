# Agent skills

Skills teach AI coding agents how to use `kite` safely: read with `--json`, dry-run every write, and leave sign-in to you.

| Skill | Use |
| :--- | :--- |
| [`kite`](kite/SKILL.md) | Portfolio, orders, market data and scripting with the `kite` CLI |

## Install

Install `kite` and sign in first (`kite auth login`). Then copy the skill directory into the skills folder of your agent:

```sh
# Claude Code
mkdir -p ~/.claude/skills && cp -r skills/kite ~/.claude/skills/

# Codex and other agents that read ~/.agents/skills
mkdir -p ~/.agents/skills && cp -r skills/kite ~/.agents/skills/
```

The skill tells the agent to show you every order, GTT or alert request as a dry run and to add `--confirm` only after you approve it. Your agent's own permission settings still apply. Review what it runs.
