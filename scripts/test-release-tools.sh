#!/usr/bin/env bash
set -euo pipefail

root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
mkdir -p "$scratch/home"
mkdir -p "$scratch/tmp"
cd "$root"
env -i \
    PATH="$PATH" \
    HOME="$scratch/home" \
    TMPDIR="$scratch/tmp" \
    XDG_CONFIG_HOME="$scratch/config" \
    XDG_CACHE_HOME="$scratch/cache" \
    XDG_DATA_HOME="$scratch/data" \
    XDG_STATE_HOME="$scratch/state" \
    CARGO_HOME="$scratch/cargo" \
    CARGO_TARGET_DIR="$scratch/target" \
    RUSTUP_HOME="$scratch/rustup" \
    MISE_CONFIG_DIR="$scratch/mise-config" \
    MISE_DATA_DIR="$scratch/mise-data" \
    MISE_CACHE_DIR="$scratch/mise-cache" \
    MISE_STATE_DIR="$scratch/mise-state" \
    python3 -I -B -m unittest discover -s tests -p 'test_*.py'
