{
  description = "NCL: a Rust runtime and CPS-first Common Lisp core";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    cl-weave.url = "github:nerima-lisp/cl-weave/v1.3.0";
    paredit-cli.url = "github:nerima-lisp/paredit-cli/v1.6.0";
  };

  outputs =
    {
      self,
      nixpkgs,
      cl-weave,
      paredit-cli,
    }:
    let
      systems = [
        "aarch64-darwin"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          ncl = pkgs.rustPlatform.buildRustPackage {
            pname = "ncl";
            version = "0.1.0";
            src = self;
            cargoLock.lockFile = ./Cargo.lock;
            doCheck = true;
          };
          lisp = pkgs.writeShellApplication {
            name = "ncl-lisp";
            runtimeInputs = [ pkgs.sbcl ];
            text = ''
              exec ${pkgs.sbcl}/bin/sbcl --script ${self}/run.lisp "$@"
            '';
          };
        in
        {
          default = ncl;
          inherit lisp ncl;
        }
      );

      apps = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          ncl = self.packages.${system}.ncl;
          lisp = self.packages.${system}.lisp;
          weave = cl-weave.packages.${system}.default;
          test = pkgs.writeShellApplication {
            name = "ncl-test";
            runtimeInputs = [ weave ];
            text = ''
              exec ${weave}/bin/cl-weave run \
                --load ${self}/run-tests.lisp \
                --reporter spec \
                --fail-with-no-tests \
                --max-workers 1 \
                --test-timeout-ms 5000 "$@"
            '';
          };
          coverage = pkgs.writeShellApplication {
            name = "ncl-coverage";
            runtimeInputs = [
              weave
              pkgs.coreutils
            ];
            text = ''
              coverage_dir="''${NCL_COVERAGE_DIR:-artifacts/ncl-coverage}"
              mkdir -p "$coverage_dir"
              mkdir -p "$coverage_dir/report"
              exec ${weave}/bin/cl-weave run ncl \
                --load ${self}/ncl.asd \
                --load ${self}/test/package.lisp \
                --load ${self}/test/support.lisp \
                --load ${self}/test/core.lisp \
                --reporter spec \
                --fail-with-no-tests \
                --max-workers 1 \
                --test-timeout-ms 5000 \
                --coverage \
                --coverage-system ncl \
                --coverage-exclude ${self}/lisp/package.lisp \
                --coverage-exclude ${self}/lisp/constants.lisp \
                --coverage-exclude ${self}/lisp/cps-macros.lisp \
                --coverage-exclude ${self}/lisp/conditions-base.lisp \
                --coverage-output "$coverage_dir/ncl.coverage" \
                --coverage-report-directory "$coverage_dir/report/" "$@"
            '';
          };
        in
        {
          default = {
            type = "app";
            program = "${ncl}/bin/ncl";
            meta = {
              description = "Run the NCL Rust runtime";
            };
          };
          lisp = {
            type = "app";
            program = "${lisp}/bin/ncl-lisp";
            meta = {
              description = "Run the NCL Common Lisp core";
            };
          };
          test = {
            type = "app";
            program = "${test}/bin/ncl-test";
            meta = {
              description = "Run the NCL cl-weave test suite";
            };
          };
          coverage = {
            type = "app";
            program = "${coverage}/bin/ncl-coverage";
            meta = {
              description = "Run NCL tests with cl-weave coverage";
            };
          };
        }
      );

      checks = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          weave = cl-weave.packages.${system}.default;
          paredit = paredit-cli.packages.${system}.default;
          mkdocs = pkgs.python3Packages.mkdocs-material;
        in
        {
          ncl-rust = self.packages.${system}.ncl;
          ncl-tests =
            pkgs.runCommand "ncl-tests"
              {
                nativeBuildInputs = [ weave ];
              }
              ''
                export HOME="$TMPDIR/home"
                export XDG_CACHE_HOME="$TMPDIR/cache"
                mkdir -p "$HOME"
                mkdir -p "$XDG_CACHE_HOME"
                ${weave}/bin/cl-weave run \
                  --load ${self}/run-tests.lisp \
                  --reporter spec \
                  --fail-with-no-tests \
                  --max-workers 1 \
                  --test-timeout-ms 5000
                touch "$out"
              '';
          ncl-paredit =
            pkgs.runCommand "ncl-paredit"
              {
                nativeBuildInputs = [
                  paredit
                  pkgs.ripgrep
                ];
              }
              ''
                files="$(${pkgs.ripgrep}/bin/rg --files ${self} \
                  -g '*.lisp' \
                  -g '*.asd' \
                  -g 'run.lisp' \
                  -g 'run-tests.lisp')"
                test -n "$files"
                while IFS= read -r file; do
                  paredit inspect check --file "$file"
                done <<< "$files"
                touch "$out"
              '';
          ncl-docs =
            pkgs.runCommand "ncl-docs"
              {
                nativeBuildInputs = [ mkdocs ];
              }
              ''
                mkdocs build --strict \
                  --config-file ${self}/docs/mkdocs.yml \
                  --site-dir "$out"
              '';
          ncl-coverage =
            pkgs.runCommand "ncl-coverage"
              {
                nativeBuildInputs = [
                  weave
                  pkgs.coreutils
                  pkgs.python3
                ];
              }
              ''
                export HOME="$TMPDIR/home"
                export XDG_CACHE_HOME="$TMPDIR/cache"
                mkdir -p "$HOME" "$XDG_CACHE_HOME"
                coverage_dir="$TMPDIR/coverage"
                mkdir -p "$coverage_dir/report"
                ${weave}/bin/cl-weave run ncl \
                  --load ${self}/ncl.asd \
                  --load ${self}/test/package.lisp \
                  --load ${self}/test/support.lisp \
                  --load ${self}/test/core.lisp \
                  --reporter spec \
                  --fail-with-no-tests \
                  --max-workers 1 \
                  --test-timeout-ms 5000 \
                  --coverage \
                  --coverage-system ncl \
                  --coverage-exclude ${self}/lisp/package.lisp \
                  --coverage-exclude ${self}/lisp/constants.lisp \
                  --coverage-exclude ${self}/lisp/cps-macros.lisp \
                  --coverage-exclude ${self}/lisp/conditions-base.lisp \
                  --coverage-output "$coverage_dir/ncl.coverage" \
                  --coverage-report-directory "$coverage_dir/report/"
                coverage_index="$coverage_dir/report/cover-index.html"
                test -s "$coverage_index"
                ${pkgs.python3}/bin/python3 - "$coverage_index" <<'PY'
                import re
                import sys

                html = open(sys.argv[1], encoding="utf-8").read()
                rows = re.findall(r"<tr[^>]*>.*?</tr>", html, flags=re.S)
                source_rows = []
                for row in rows:
                    cells = [
                        re.sub(r"<[^>]+>", "", cell).strip()
                        for cell in re.findall(r"<td[^>]*>(.*?)</td>", row, flags=re.S)
                    ]
                    if len(cells) >= 7 and cells[0].endswith(".lisp"):
                        source_rows.append(cells)
                if not source_rows:
                    raise SystemExit("coverage report contains no source rows")
                for cells in source_rows:
                    if cells[3] != "100.0":
                        raise SystemExit(
                            f"expression coverage is {cells[3]}% for {cells[0]}"
                        )
                    if cells[6] not in {"100.0", "-"}:
                        raise SystemExit(
                            f"branch coverage is {cells[6]}% for {cells[0]}"
                        )
                PY
                touch "$out"
              '';
        }
      );

      formatter = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        pkgs.nixfmt
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.rustc
              pkgs.cargo
              pkgs.rustfmt
              pkgs.clippy
              pkgs.sbcl
              cl-weave.packages.${system}.default
              paredit-cli.packages.${system}.default
              pkgs.nixfmt
              pkgs.python3Packages.mkdocs-material
            ];
          };
        }
      );
    };
}
