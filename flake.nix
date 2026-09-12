{
  description = "Reproducible development environment for NCL";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          let
            pkgs = import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            };
            rust = pkgs.rust-bin.stable."1.98.0".default;
          in
          f { inherit pkgs rust; }
        );
    in
    {
      formatter = forAllSystems ({ pkgs, ... }: pkgs.nixfmt);
      packages = forAllSystems (
        { pkgs, rust }:
        {
          default =
            let
              rustPlatform = pkgs.makeRustPlatform {
                cargo = rust;
                rustc = rust;
              };
            in
            rustPlatform.buildRustPackage {
              pname = "ncl";
              version = "0.1.0";
              src = builtins.path {
                path = ./.;
                name = "ncl-source";
                filter = path: type: !pkgs.lib.hasInfix "/target/" (toString path);
              };
              cargoLock.lockFile = ./Cargo.lock;
              cargoBuildFlags = [ "--workspace" ];
              cargoTestFlags = [
                "--workspace"
                "--all-features"
                "--all-targets"
              ];
              doCheck = true;
              meta.description = "A Rust-native Common Lisp runtime";
              meta.mainProgram = "ncl";
            };
        }
      );
      apps = forAllSystems (
        { pkgs, rust }:
        {
          rust-coverage = {
            type = "app";
            program = "${
              pkgs.writeShellApplication {
                name = "ncl-rust-coverage";
                runtimeInputs = [
                  rust
                  pkgs.cargo-llvm-cov
                  pkgs.llvmPackages_20.llvm
                ];
                text = ''
                  export LLVM_COV="${pkgs.llvmPackages_20.llvm}/bin/llvm-cov"
                  export LLVM_PROFDATA="${pkgs.llvmPackages_20.llvm}/bin/llvm-profdata"
                  export NIX_LDFLAGS="-L${pkgs.libiconv}/lib ''${NIX_LDFLAGS:-}"
                  export RUSTFLAGS="-C link-arg=-L${pkgs.libiconv}/lib ''${RUSTFLAGS:-}"
                  exec cargo llvm-cov --locked --workspace --all-features --all-targets "$@"
                '';
              }
            }/bin/ncl-rust-coverage";
          };
        }
      );
      devShells = forAllSystems (
        { pkgs, rust }:
        {
          default =
            let
              docs = pkgs.python313.withPackages (
                pythonPackages: with pythonPackages; [
                  mkdocs
                  mkdocs-material
                  pymdown-extensions
                ]
              );
            in
            assert rust.version == "1.98.0";
            pkgs.mkShell {
              packages = with pkgs; [
                rust
                cargo
                rustfmt
                clippy
                cargo-llvm-cov
                llvmPackages_20.llvm
                libiconv
                docs
              ];
              shellHook = ''
                export CARGO_TERM_COLOR=always
                export RUST_BACKTRACE=1
                export LLVM_COV=${pkgs.llvmPackages_20.llvm}/bin/llvm-cov
                export LLVM_PROFDATA=${pkgs.llvmPackages_20.llvm}/bin/llvm-profdata
              ''
              + pkgs.lib.optionalString pkgs.stdenv.hostPlatform.isDarwin ''
                export NIX_LDFLAGS="-L${pkgs.libiconv}/lib $NIX_LDFLAGS"
                export RUSTFLAGS="-C link-arg=-L${pkgs.libiconv}/lib ''${RUSTFLAGS:-}"
              '';
            };
        }
      );
      checks = forAllSystems (
        { pkgs, rust }:
        {
          default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
          cargo-test =
            let
              rustPlatform = pkgs.makeRustPlatform {
                cargo = rust;
                rustc = rust;
              };
            in
            rustPlatform.buildRustPackage {
              pname = "ncl-cargo-test";
              version = "0.1.0";
              src = builtins.path {
                path = ./.;
                name = "ncl-cargo-test-source";
                filter = path: type: !pkgs.lib.hasInfix "/target/" (toString path);
              };
              cargoLock.lockFile = ./Cargo.lock;
              buildType = "debug";
              cargoBuildFlags = [ "--workspace" ];
              cargoTestFlags = [
                "--workspace"
                "--locked"
              ];
              doCheck = true;
              installPhase = ''
                mkdir -p "$out"
              '';
            };
        }
      );
    };
}
