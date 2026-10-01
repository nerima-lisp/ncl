# Getting started

## Requirements

NCL builds with the stable Rust toolchain at version 1.98.0. The
repository pins that version, with <code>rustfmt</code> and
<code>clippy</code>, in <code>rust-toolchain.toml</code>. Alternatively
run <code>nix develop</code> from the repository root; the development
shell provides Rust 1.98.0, clippy, rustfmt, cargo-llvm-cov, and mkdocs.

## Current CLI state

Evaluate a form with `--eval`:

~~~sh
nix develop --command cargo run -- --eval '(+ 1 2)'
~~~

The command prints <code>3</code>. A recursive source-level run prints
<code>75025</code>:

~~~sh
nix develop --command cargo run -- --eval \
  '(progn (defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 25))'
~~~

The binary also provides an interactive REPL and the <code>--load</code>,
<code>--script</code>, and <code>--compile-file</code> file modes.

## Development gates

From the repository root, run <code>nix develop</code> and then:

~~~sh
nix develop --command cargo test --workspace
nix develop --command cargo clippy --workspace --all-targets -- -D warnings
nix develop --command cargo fmt --check
nix develop --command python3 scripts/check_standards.py
nix develop --command mkdocs build --strict --config-file docs/mkdocs.yml
~~~

Coverage uses LLVM instrumentation through the flake app:

~~~sh
nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
~~~
