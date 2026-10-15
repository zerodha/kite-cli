#!/usr/bin/env python3
"""Write docs/CLI_REFERENCE.md from the `--help` output of every kite command.

Usage: cargo build && python3 scripts/gen-cli-reference.py [path/to/kite]
"""
from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUTPUT = ROOT / "docs" / "CLI_REFERENCE.md"

HEADER = """# CLI reference

This file is generated from `kite --help` and the `--help` of every subcommand.
Do not edit it by hand. After you change help text in `src/cli.rs`, run:

```bash
cargo build && python3 scripts/gen-cli-reference.py
```

For task-based examples, see the [user guide](guide/README.md).
"""


def help_text(binary: str, path: list[str]) -> str:
    result = subprocess.run(
        [binary, *path, "--help"],
        check=True,
        capture_output=True,
        text=True,
        env={"PATH": "/usr/bin:/bin", "NO_COLOR": "1"},
    )
    return "\n".join(line.rstrip() for line in result.stdout.splitlines()).rstrip() + "\n"


def subcommands(text: str) -> list[str]:
    names = []
    in_commands = False
    for line in text.splitlines():
        if line == "Commands:":
            in_commands = True
            continue
        if in_commands:
            if not line.startswith("  "):
                break
            name = line.split()[0]
            if name != "help":
                names.append(name)
    return names


def walk(binary: str, path: list[str], sections: list[str]) -> None:
    text = help_text(binary, path)
    title = " ".join(["kite", *path])
    level = "#" * min(len(path) + 2, 4)
    sections.append(f"{level} `{title}`\n\n```text\n{text}```\n")
    for name in subcommands(text):
        walk(binary, [*path, name], sections)


def main() -> None:
    binary = sys.argv[1] if len(sys.argv) > 1 else str(ROOT / "target" / "debug" / "kite")
    sections: list[str] = []
    walk(binary, [], sections)
    OUTPUT.write_text(HEADER + "\n" + "\n".join(sections))
    print(f"wrote {OUTPUT.relative_to(ROOT)} ({len(sections)} commands)")


if __name__ == "__main__":
    main()
