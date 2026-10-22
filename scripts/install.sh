#!/bin/sh
set -eu

RELEASES=https://github.com/zerodha/kite-cli/releases

fail() {
    printf 'kite install: %s\n' "$*" >&2
    exit 1
}

case "$(uname -s)" in
    Linux) os=unknown-linux-musl ;;
    Darwin) os=apple-darwin ;;
    *) fail "unsupported operating system: $(uname -s). Use scripts/install.ps1 on Windows." ;;
esac
case "$(uname -m)" in
    x86_64|amd64) arch=x86_64 ;;
    aarch64|arm64) arch=aarch64 ;;
    *) fail "unsupported CPU architecture: $(uname -m). Releases support x86_64 and aarch64." ;;
esac
target=$arch-$os

command -v curl >/dev/null 2>&1 || fail "curl is required."
command -v tar >/dev/null 2>&1 || fail "tar is required."
if command -v sha256sum >/dev/null 2>&1; then
    hash_file() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
    hash_file() { shasum -a 256 "$1" | awk '{print $1}'; }
else
    fail "sha256sum or shasum is required."
fi

# GitHub redirects /releases/latest to /releases/tag/<tag>, so no API call is needed.
if [ -n "${KITE_VERSION:-}" ]; then
    tag=v${KITE_VERSION#v}
else
    latest=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "$RELEASES/latest") || fail "could not find the latest release."
    tag=${latest##*/}
fi
case "$tag" in v[0-9]*) ;; *) fail "invalid release tag: $tag." ;; esac
case "$tag" in *[!A-Za-z0-9.+_-]*) fail "invalid release tag: $tag." ;; esac

tmp=$(mktemp -d "${TMPDIR:-/tmp}/kite-install.XXXXXX") || fail "could not create a temporary directory."
trap 'rm -rf "$tmp"' EXIT HUP INT TERM

archive="kite-${tag#v}-$target.tar.gz"
curl -fsSL "$RELEASES/download/$tag/$archive" -o "$tmp/$archive" || fail "could not download $archive."
curl -fsSL "$RELEASES/download/$tag/checksums.txt" -o "$tmp/checksums.txt" || fail "could not download checksums.txt."
expected=$(awk -v name="$archive" '$2 == name || $2 == "*" name {print $1; found=1} END {if (!found) exit 1}' "$tmp/checksums.txt") || fail "checksums.txt has no entry for $archive."
[ "$expected" = "$(hash_file "$tmp/$archive")" ] || fail "SHA-256 check failed for $archive."

if [ -n "${KITE_INSTALL_DIR:-}" ]; then
    install_dir=$KITE_INSTALL_DIR
else
    install_dir=
    for preferred in "${HOME:-}/.local/bin" "${HOME:-}/.cargo/bin"; do
        [ -n "${HOME:-}" ] || continue
        case ":${PATH:-}:" in *":$preferred:"*)
            if [ -d "$preferred" ] && [ -w "$preferred" ]; then install_dir=$preferred; break; fi
        esac
    done
    if [ -z "$install_dir" ] && [ -w /usr/local/bin ]; then install_dir=/usr/local/bin; fi
    if [ -z "$install_dir" ] && [ -n "${HOME:-}" ]; then install_dir=$HOME/.local/bin; fi
    [ -n "$install_dir" ] || fail "no writable install directory. Set KITE_INSTALL_DIR to a user-writable directory on PATH."
fi
mkdir -p "$install_dir" || fail "cannot create install directory: $install_dir."
[ -w "$install_dir" ] || fail "install directory is not writable: $install_dir."
tar -xzf "$tmp/$archive" -C "$tmp" kite || fail "could not extract kite from $archive."
target_file=$install_dir/kite
[ ! -L "$target_file" ] || fail "refusing to replace symlink: $target_file."
staged=$install_dir/.kite-install.$$
trap 'rm -rf "$tmp"; rm -f "$staged"' EXIT HUP INT TERM
install -m 755 "$tmp/kite" "$staged" 2>/dev/null || cp "$tmp/kite" "$staged"
chmod 755 "$staged"
mv -f "$staged" "$target_file" || fail "could not install kite to $target_file."
printf 'Installed kite %s to %s/kite\n' "$tag" "$install_dir"
case ":${PATH:-}:" in
    *":$install_dir:"*) ;;
    *) printf 'Add this directory to PATH: %s\n' "$install_dir" ;;
esac
