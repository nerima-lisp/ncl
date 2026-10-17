# NCL

The [wave plan](docs/src/project/wave-plan.md) records current decisions, phases, and acceptance criteria. Design contracts are in [docs/src/design/](docs/src/design/).

## Status

The following CLI results have been verified:

```sh
nix develop --command cargo run -p ncl -- --version
ncl 0.1.0

nix develop --command target/debug/ncl --eval '(+ 1 2)'
3
```

## Development

Run the development gates from the repository root:

```sh
nix develop --command cargo test --workspace
nix develop --command cargo clippy --workspace --all-targets -- -D warnings
nix develop --command python3 scripts/check_standards.py
```

Coverage and documentation are separate gates:

```sh
nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
mkdocs build --strict --config-file docs/mkdocs.yml
```

Run Rust commands through `nix develop`; bare `cargo` may fail to link the binary on macOS.

## License

MIT.
