# Contributing

## Set up

The shared API fixtures from [`kiteconnect-mocks`](https://github.com/zerodha/kiteconnect-mocks) live in a git submodule at `mock_responses/`. The tests need it. Initialize it once after you clone:

```bash
git submodule update --init --recursive
```

`rustup` installs the Rust version pinned in `rust-toolchain.toml` on the first build.

## Tasks

The `justfile` has the common tasks:

```bash
just build          # debug build
just run -- <args>  # cargo run with args
just fmt            # format code
just lint           # clippy on all targets, as CI runs it
just test           # run all tests
just test-mocks     # run the kiteconnect-mocks integration tests
just install        # install to ~/.cargo/bin
```

Before you send a change, run what CI runs:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/test-release-tools.sh
```

## Tests

- Tests run against `httpmock` servers and the fixtures in `mock_responses/`. They never call a real Kite account.
- `tests/cli_output.rs` runs the built binary against a mock server and compares stdout, stderr and exit codes with the files in `tests/fixtures/cli-output/`. If you change human output on purpose, update those files in the same change.
- Assert literal expected values. Do not assert only that a call did not fail.

## Documentation

- The user guide is in `docs/guide/`. Every example output in it comes from running the binary against the fixtures. Do not write example output by hand.
- `docs/CLI_REFERENCE.md` is generated. After you change help text in `src/cli.rs`, run `cargo build && python3 scripts/gen-cli-reference.py`.
- Add user-visible changes to `CHANGELOG.md` under `[Unreleased]`.

## Code layout

```text
src/main.rs           binary entrypoint
src/cli.rs            clap command tree and help text
src/commands/         one handler module per Kite noun
src/app/              config, sessions, output and error display
src/app_api/      CLI-only Kite routes: sign-in, screener
src/instruments/      instrument cache, search, resolve and option-chain helpers
src/tui.rs, src/tui/  terminal trading UI
src/util.rs           form builders, JSON input, dry-run payloads, atomic file writes
mock_responses/       kiteconnect-mocks submodule used by the tests
tests/                integration tests
scripts/              repository checks and the CLI reference generator
```

The Kite Connect API client is the [`kiteconnect`](https://github.com/zerodha/kiteconnect-rs) crate. Change it in that repository.
Keep CLI-only routes in `src/app_api/` or the TUI module that uses them.

API methods return `Call<T>`. Await a call for typed data, or use `.raw()` for JSON.
Use `.request()` to inspect a request without sending it.
CLI handlers return `Output` and use the shared runtime for sessions and configuration.
Trading writes use the shared confirmation path.

TUI input, ticker events, and API results reach `App::update` through one event channel.
Rendering reads the app state. Session tasks stop when the session ends.
Place pane logic in `app/panes/` and rendering in `ui/views/`.
Keep numeric table cells complete, or hide their column when space is insufficient.

## Release builds

`just dist` builds a static x86_64 Linux archive in `dist/`. Tagged releases are built by `.github/workflows/release.yml`. Both need `KITE_API_ROOT`, `KITE_DEFAULT_API_KEY` and `KITE_TICKER_URL` at build time. The release archive includes `docs/INSTALL.md`, so keep that file short and self-contained.


Release automation uses pinned `rlsr` through `.github/workflows/release.yml`.
It builds target archives and a shared `checksums.txt` file.
The release tag must match the CLI version and a dated section in `CHANGELOG.md`.
