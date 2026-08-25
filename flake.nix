{
  description = "Reproducible development environment for NCL";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      flake-utils,
      nixpkgs,
      rust-overlay,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        rust = pkgs.rust-bin.stable."1.97.0".default.override {
          extensions = [
            "rust-src"
            "rustfmt"
            "clippy"
            "llvm-tools-preview"
          ];
        };
      in
      {
        devShells.default = pkgs.mkShell {
          packages = [
            rust
            pkgs.cargo-llvm-cov
            pkgs.pkg-config
          ];
          RUST_BACKTRACE = "1";
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "ncl";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          doCheck = true;
          meta.mainProgram = "ncl";
        };

        formatter = pkgs.nixfmt;
        checks.default = self.packages.${system}.default;
      }
    );
}
