{
  description = "Reproducible development environment for NCL";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
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
          in
          f pkgs
        );
    in
    {
      formatter = forAllSystems (pkgs: pkgs.nixfmt);
      packages = forAllSystems (pkgs: {
        default =
          let
            rust = pkgs.rust-bin.stable."1.98.0".default;
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
      });
      devShells = forAllSystems (pkgs: {
        default =
          let
            rust = pkgs.rust-bin.stable."1.98.0".default;
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
              export NIX_LDFLAGS="-L${pkgs.libiconv}/lib $NIX_LDFLAGS"
              export RUSTFLAGS="-C link-arg=-L${pkgs.libiconv}/lib ''${RUSTFLAGS:-}"
              export LLVM_COV=${pkgs.llvmPackages_20.llvm}/bin/llvm-cov
              export LLVM_PROFDATA=${pkgs.llvmPackages_20.llvm}/bin/llvm-profdata
            '';
          };
      });
    };
}
