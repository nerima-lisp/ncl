# NCL

NCL is a Rust-native Common Lisp implementation. It targets ANSI Common
Lisp and an NCL-specific extension API. SBCL is used only as a recorded
performance and conformance baseline; NCL does not provide an SBCL
compatibility surface.

## Current state

The native reader, compiler, runtime, and command-line path are connected.
This command evaluates source text and prints `75025`:

```sh
nix develop --command cargo run -- --eval \
  '(progn (defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 25))'
```

The binary also supports an interactive REPL and `--load`, `--script`, and
`--compile-file`. The remaining ANSI surface, performance work, and library
load matrix are tracked in [the wave plan](docs/src/project/wave-plan.md).

## Development

The workspace pins Rust 1.98.0 and has no external crates. Unsafe code is
confined to `ncl-sys`. Run checks from the repository root:

```sh
nix develop --command cargo test --workspace
nix develop --command cargo clippy --workspace --all-targets -- -D warnings
nix develop --command cargo fmt --check
nix develop --command python3 scripts/check_standards.py
```

Coverage and documentation are separate gates:

```sh
nix develop --command nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
nix develop --command mkdocs build --strict --config-file docs/mkdocs.yml
```

## License

MIT.
