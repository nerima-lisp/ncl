# NCL

NCL is a Rust-native Common Lisp implementation. It targets ANSI Common Lisp and an NCL-specific extension API. SBCL is a recorded performance and conformance baseline; NCL does not provide an SBCL compatibility surface.

The [wave plan](docs/src/project/wave-plan.md) records current decisions, phases, and acceptance criteria. Design contracts are in [docs/src/design/](docs/src/design/).

## Status

The native reader, compiler, runtime, and command-line path are connected. These CLI results have been verified:

```sh
nix develop --command cargo run -p ncl -- --version
ncl 0.1.0

nix develop --command cargo run -- --eval '(+ 1 2)'
3

nix develop --command cargo run -- --eval '(progn (defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 25))'
75025
```

The binary also supports an interactive REPL and `--load`, `--script`, and `--compile-file`. The remaining ANSI surface, performance work, and library load matrix are tracked in [the wave plan](docs/src/project/wave-plan.md).

## Development

Open a development shell before running Rust commands; bare `cargo` may fail to link the binary on macOS:

```sh
nix develop
```

From the development shell, run formatting, lint, and standards checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
python3 scripts/check_standards.py
```

For local lane work, test the changed crate. If a test under `tests/` changes, run `cargo test -p ncl --test <name>`. CI runs workspace tests in debug and release on x86-64 Linux and AArch64 macOS:

```sh
cargo test --locked --workspace --all-features --all-targets
cargo test --locked --workspace --all-features --all-targets --release
```

Coverage and documentation are separate checks:

```sh
nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
mkdocs build --strict --config-file docs/mkdocs.yml
```

## License

MIT.
