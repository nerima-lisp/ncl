# NCL

NCL is a Rust-native Common Lisp implementation. It targets ANSI Common
Lisp plus an NCL-specific extension API, and it is measured against SBCL by
its numbers rather than by compatibility: there is no `SB-*` surface, no
compatibility shim, and no alias package.

## Status

NCL is mid-rewrite. The former interpreter, VM, and syntax crates ended at
commit `d9bbb4ec` and have been removed. The workspace keeps the parts that
do not depend on SBCL and rebuilds the rest:

- Core crates are in place: `ncl-sys`, `ncl-object`, `ncl-ir`,
  `ncl-codegen`, `ncl-objfile`, `ncl-asm-x86-64`, and `ncl-asm-aarch64`.
- The language, library, runtime, and conformance crates exist as skeletons
  and are filled in by the implementation lanes.
- The `ncl` binary currently supports only `--version` / `-V`. `--eval`
  reports that it is not implemented during the native rewrite.
- Source text does not execute as native code yet; milestone M1 is the
  first point that does.

The lane plan, decisions, and acceptance criteria are in
[docs/src/project/wave-plan.md](docs/src/project/wave-plan.md). The frozen
design contracts are in [docs/src/design/](docs/src/design/).

## Development

The workspace pins Rust 1.98.0 and uses zero external crates; `unsafe` is
confined to `ncl-sys`. Enter the development shell and run the gates from
the repository root:

```sh
nix develop
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check_standards.py
```

Coverage and documentation are separate gates:

```sh
nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
mkdocs build --strict --config-file docs/mkdocs.yml
```

Run the Rust commands through `nix develop`; a bare `cargo` may fail to link
the binary on macOS.

## License

MIT.
