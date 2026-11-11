{
  description = "kite: Kite trading CLI and TUI";

  inputs = {
    # Tests embed fixtures from the mock_responses submodule.
    self.submodules = true;
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      inherit (nixpkgs) lib;
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
      ];
      forAllSystems =
        f:
        lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ rust-overlay.overlays.default ];
            }
          )
        );

      # rust-toolchain.toml stays the single source of truth for rustup and Nix.
      toolchainFor = pkgs: pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

      src = lib.fileset.toSource {
        root = ./.;
        fileset = lib.fileset.unions [
          ./Cargo.toml
          ./Cargo.lock
          ./src
          ./tests
          ./mock_responses
        ];
      };

      cargoToml = lib.importTOML ./Cargo.toml;

      kiteFor =
        pkgs:
        let
          toolchain = toolchainFor pkgs;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
        in
        rustPlatform.buildRustPackage {
          pname = cargoToml.package.name;
          inherit (cargoToml.package) version;
          inherit src;
          cargoLock.lockFile = ./Cargo.lock;

          meta = {
            description = "Terminal UI and scriptable CLI for Zerodha Kite";
            homepage = cargoToml.package.repository;
            license = lib.licenses.mit;
            mainProgram = "kite";
          };
        };
    in
    {
      packages = forAllSystems (pkgs: rec {
        kite = kiteFor pkgs;
        default = kite;
      });

      checks = forAllSystems (
        pkgs:
        let
          kite = self.packages.${pkgs.stdenv.hostPlatform.system}.kite;
          toolchain = toolchainFor pkgs;
        in
        {
          # The package build runs `cargo test`.
          inherit kite;

          clippy = kite.overrideAttrs (old: {
            pname = "${old.pname}-clippy";
            nativeBuildInputs = old.nativeBuildInputs ++ [ toolchain ];
            buildPhase = ''
              runHook preBuild
              cargo clippy --workspace --all-targets --offline --locked -- -D warnings
              runHook postBuild
            '';
            doCheck = false;
            installPhase = "touch $out";
          });

          fmt = pkgs.runCommand "kite-fmt" { nativeBuildInputs = [ toolchain ]; } ''
            cd ${src}
            cargo fmt --all -- --check
            touch $out
          '';

          nixfmt = pkgs.runCommand "kite-nixfmt" { nativeBuildInputs = [ pkgs.nixfmt ]; } ''
            nixfmt --check ${./flake.nix}
            touch $out
          '';
        }
      );

      formatter = forAllSystems (pkgs: pkgs.nixfmt-tree);

      devShells = forAllSystems (pkgs: {
        default = pkgs.mkShell {
          name = "kite-cli";

          packages = [
            ((toolchainFor pkgs).override (old: {
              extensions = old.extensions ++ [
                "rust-src"
                "rust-analyzer"
              ];
            }))
            pkgs.cargo-watch
            pkgs.cargo-zigbuild
            pkgs.zig
            pkgs.just
            pkgs.python3
          ];

          shellHook = ''
            export CARGO_HOME="$PWD/.cargo-home"
            export CARGO_TARGET_DIR="$PWD/target"
          '';
        };
      });
    };
}
