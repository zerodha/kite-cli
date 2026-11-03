{
  description = "Development environment for kite CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        toolchain = (pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml).override (old: {
          extensions = old.extensions ++ [ "rust-src" ];
        });
      in
      {
        devShells.default = pkgs.mkShell {
          name = "kite-cli";

          packages = with pkgs; [
            toolchain
            cargo-watch
            just
            rust-analyzer
          ];

          shellHook = ''
            export CARGO_HOME="$PWD/.cargo-home"
            export CARGO_TARGET_DIR="$PWD/target"
            mkdir -p "$CARGO_HOME" "$CARGO_TARGET_DIR"

            echo "kite CLI development shell"
            echo "rustc: $(rustc --version)"
            echo "cargo: $(cargo --version)"
            echo "tools: cargo fmt, cargo clippy, cargo test, just"
          '';
        };
      }
    );
}
