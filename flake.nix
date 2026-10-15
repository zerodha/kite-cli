{
  description = "Development environment for kite CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
      in
      {
        devShells.default = pkgs.mkShell {
          name = "kite-cli";

          packages = with pkgs; [
            cargo
            cargo-watch
            clippy
            just
            rust-analyzer
            rustc
            rustfmt
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
