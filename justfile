
default:
    @just --list

build:
    cargo build

build-release:
    cargo build --release

[no-exit-message]
run *ARGS:
    cargo run -- {{ARGS}}

run-release *ARGS:
    cargo run --release -- {{ARGS}}

test:
    cargo test --workspace

test-mocks:
    cargo test --workspace --test kiteconnect_mocks

perf-replay:
    cargo test --workspace --offline --locked --release --lib tui::app::perf::tick_replay -- --exact --ignored --nocapture --test-threads=1

test-verbose:
    cargo test --workspace -- --nocapture

check:
    cargo check

fmt:
    cargo fmt

fmt-check:
    cargo fmt --all -- --check

lint:
    cargo clippy --workspace --all-targets -- -D warnings

clean:
    cargo clean

watch:
    cargo watch -x build

watch-run *ARGS:
    cargo watch -x "run -- {{ARGS}}"

install:
    cargo install --path .

uninstall:
    cargo uninstall kite

[doc("Build a static x86_64 Linux release tarball in dist/")]
[positional-arguments]
dist LABEL="":
    #!/usr/bin/env bash
    set -euo pipefail
    : "${KITE_API_ROOT:?error: KITE_API_ROOT is required for a release build}"
    : "${KITE_DEFAULT_API_KEY:?error: KITE_DEFAULT_API_KEY is required for a release build}"
    : "${KITE_TICKER_URL:?error: KITE_TICKER_URL is required for a release build}"
    target="x86_64-unknown-linux-musl"
    label="${1:-}"
    if [[ ! "$label" =~ ^[[:alnum:]_.-]*$ ]]; then
        echo "error: release label must contain only letters, numbers, dots, underscores, or hyphens." >&2
        exit 1
    fi

    if ! command -v zig >/dev/null 2>&1; then
        echo "error: zig is required. Install it from https://ziglang.org or your package manager." >&2
        exit 1
    fi
    if ! command -v cargo-zigbuild >/dev/null 2>&1; then
        echo "error: cargo-zigbuild is required. Install it, for example with 'cargo install --locked cargo-zigbuild'." >&2
        exit 1
    fi
    rustup target add "$target" >/dev/null 2>&1 || true

    echo "==> Building static release for $target ..."
    cargo zigbuild --release --target "$target"

    stamp="$(date +%Y-%m-%d-%H%M)"
    version="${stamp}${label:+-$label}"
    name="kite-linux-x86_64-${version}"
    outdir="dist"
    staging="${outdir}/${name}"

    rm -f "${outdir}"/*.tar.gz "${outdir}"/*.tar.gz.sha256 "${outdir}/INSTALL.md"
    rm -rf "$staging"
    mkdir -p "$staging"

    cp "target/${target}/release/kite" "${staging}/kite"
    strip "${staging}/kite" 2>/dev/null || true
    cp docs/INSTALL.md "${staging}/INSTALL.md"
    cp docs/INSTALL.md "${outdir}/INSTALL.md"

    {
        echo "kite (Kite trading CLI/TUI)"
        echo "build:   ${version}"
        echo "commit:  $(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
        echo "target:  ${target} (static, no glibc dependency)"
        echo "built:   $(date '+%Y-%m-%d %H:%M:%S %Z')"
        echo ""
        echo "Run:     chmod +x kite && ./kite        # launches the TUI"
        echo "         ./kite --help                  # CLI usage"
    } > "${staging}/BUILD_INFO.txt"

    tar -czf "${outdir}/${name}.tar.gz" -C "$outdir" "$name"
    rm -rf "$staging"
    (cd "$outdir" && sha256sum "${name}.tar.gz" > "${name}.tar.gz.sha256")

    echo ""
    echo "==> Done."
    echo "    file:   ${outdir}/${name}.tar.gz"
    echo "    size:   $(du -h "${outdir}/${name}.tar.gz" | cut -f1)"
    echo "    sha256: $(cut -d' ' -f1 "${outdir}/${name}.tar.gz.sha256")"
    echo "    guide:  ${outdir}/INSTALL.md (also inside the tarball)"

release-preview TARGET="x86_64-unknown-linux-musl":
    ./scripts/install-rlsr.sh target/release-tools
    RUST_LOG=info target/release-tools/rlsr --config rlsr.toml --skip-publish --split --targets "{{TARGET}}"
    RUST_LOG=info target/release-tools/rlsr --config rlsr.toml --skip-publish --merge
