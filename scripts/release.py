#!/usr/bin/env python3
"""Build verified release artifacts and prepare rlsr publication metadata."""
from __future__ import annotations

import argparse
from datetime import date
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parent.parent
SETTINGS = ("KITE_API_ROOT", "KITE_DEFAULT_API_KEY", "KITE_TICKER_URL")
WINDOWS_TARGET = "x86_64-pc-windows-msvc"
TARGETS = (
    "x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl",
    "x86_64-apple-darwin", "aarch64-apple-darwin",
    WINDOWS_TARGET,
)


def release_notes(tag: str) -> str:
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    if tag != f"v{version}":
        raise ValueError("release tag must match the Cargo.toml package version")
    changelog = (ROOT / "CHANGELOG.md").read_text()
    section = re.search(
        rf"^## \[{re.escape(version)}\] - \d{{4}}-\d{{2}}-\d{{2}}\n(.*?)(?=^## |^\[Unreleased\]:|\Z)",
        changelog, re.M | re.S,
    )
    if not section or not section.group(1).strip():
        raise ValueError("missing dated changelog section for release tag")
    heading = re.search(
        rf"^## \[{re.escape(version)}\] - (\d{{4}}-\d{{2}}-\d{{2}})$",
        changelog,
        re.M,
    )
    if not heading:
        raise ValueError("missing dated changelog section for release tag")
    try:
        date.fromisoformat(heading.group(1))
    except ValueError as error:
        raise ValueError("release changelog date must be a valid YYYY-MM-DD date") from error
    return section.group(1).strip() + "\n"


def preview_tag() -> None:
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    tag = f"v{version}-preview"
    existing = subprocess.run(
        ["git", "tag", "--list", tag], cwd=ROOT, check=True,
        capture_output=True, text=True,
    ).stdout.strip()
    if existing:
        raise ValueError(f"local preview tag already exists: {tag}")
    subprocess.run(["git", "tag", tag], cwd=ROOT, check=True)


def build(target: str) -> None:
    missing = [name for name in SETTINGS if not os.environ.get(name)]
    if missing:
        raise ValueError("missing release settings: " + ", ".join(missing))
    tool = "zigbuild" if target.endswith("linux-musl") else "build"
    subprocess.run(["cargo", tool, "--locked", "--release", "--target", target], cwd=ROOT, check=True)
    verify_built(target)


def binary_path(target: str) -> Path:
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", "target"))
    suffix = ".exe" if target == WINDOWS_TARGET else ""
    return ROOT / target_dir / target / "release" / f"kite{suffix}"


def verify_built(target: str) -> None:
    missing = [name for name in SETTINGS if not os.environ.get(name)]
    if missing:
        raise ValueError("missing release settings: " + ", ".join(missing))
    binary = binary_path(target).read_bytes()
    for name in SETTINGS:
        if os.environ[name].encode() not in binary:
            raise ValueError(f"compiled binary is missing {name}")


def verify_windows_archive(target: str) -> None:
    if target != WINDOWS_TARGET:
        raise ValueError("Windows archive verification requires the Windows MSVC target")
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    dist = ROOT / "dist"
    archives = [
        path for path in dist.glob(f"kite-{version}*-{target}.zip")
        if path.name in {
            f"kite-{version}-{target}.zip",
            f"kite-{version}-preview-{target}.zip",
        }
    ]
    if len(archives) != 1:
        raise ValueError(f"expected one Windows release archive for {version}, found {len(archives)}")
    archive_path = archives[0]
    archive_name = archive_path.name
    with zipfile.ZipFile(archive_path) as archive:
        entry = archive.getinfo("kite.exe") if "kite.exe" in archive.namelist() else None
        if entry is None or entry.is_dir():
            raise ValueError("Windows release archive must contain kite.exe at its root")
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory) / "kite.exe"
            binary.write_bytes(archive.read(entry))
            version_output = subprocess.run(
                [str(binary), "--version"], check=True, capture_output=True, text=True,
            ).stdout.strip()
            if version not in version_output:
                raise ValueError("Windows binary version does not match Cargo.toml")
            subprocess.run([str(binary), "--help"], check=True, capture_output=True)

    digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    (dist / "checksums.txt").write_text(f"{digest}  {archive_name}\n")
    (dist / "windows-verification.txt").write_text(
        f"target={target}\narchive={archive_name}\nsha256={digest}\n"
        f"version={version_output}\nhelp=passed\ncompiled_settings=passed\n"
    )
    print(f"Verified {archive_name} (SHA-256 {digest})")


def publish_config(repository: str, output: Path) -> None:
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("repository must be OWNER/REPO")
    config = tomllib.loads((ROOT / "rlsr.toml").read_text())
    owner, repo = repository.split("/")
    for release in config["releases"]:
        release["targets"] = {"github": {"owner": owner, "repo": repo, "draft": True}}
    output.write_text(json.dumps(config, indent=2) + "\n")


def normalize_split_manifests() -> None:
    for manifest in (ROOT / "dist").glob("rlsr-split-*.yml"):
        contents = manifest.read_text()
        manifest.write_text(contents.replace("- ./dist\\", "- ./dist/"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("build").add_argument("target", choices=TARGETS)
    commands.add_parser("verify-built").add_argument("target", choices=TARGETS)
    commands.add_parser("verify-windows-archive").add_argument("target", choices=TARGETS)
    commands.add_parser("notes").add_argument("tag")
    commands.add_parser("preview-tag")
    commands.add_parser("normalize-manifests")
    publish = commands.add_parser("publish-config")
    publish.add_argument("repository")
    publish.add_argument("output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "build":
            build(args.target)
        elif args.command == "verify-built":
            verify_built(args.target)
        elif args.command == "verify-windows-archive":
            verify_built(args.target)
            verify_windows_archive(args.target)
        elif args.command == "notes":
            notes = release_notes(args.tag)
            (ROOT / "dist").mkdir(exist_ok=True)
            (ROOT / "dist/release-notes.md").write_text(notes)
        elif args.command == "preview-tag":
            preview_tag()
        elif args.command == "normalize-manifests":
            normalize_split_manifests()
        else:
            publish_config(args.repository, args.output)
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        parser.exit(1, f"release: {error}\n")


if __name__ == "__main__":
    main()
