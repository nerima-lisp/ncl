# Getting started

## Requirements

NCL builds with the stable Rust toolchain at version 1.98.0. The
repository pins that version, with <code>rustfmt</code> and
<code>clippy</code>, in <code>rust-toolchain.toml</code>. Alternatively
run <code>nix develop</code> from the repository root; the development
shell provides Rust 1.98.0, clippy, rustfmt, cargo-llvm-cov, and mkdocs.

## Current CLI state

The command-line interface is a stub during the native rewrite. Only
version output works today:

~~~sh
cargo run -- --version
~~~

<code>--version</code> and its short form <code>-V</code> print the
package version and exit with status 0.

<code>--eval</code> reports the rewrite state and exits with status 1:

~~~sh
cargo run -- --eval '(+ 1 2)'
~~~

That command prints <code>--eval is not implemented during the native
rewrite</code> to standard error. Any other option exits with status 2,
and running the binary without arguments prints a notice that NCL is being
rewritten and exits with status 2.

There is no <code>--file</code>, <code>--repl</code>,
<code>--compiled</code>, <code>--load</code>, <code>--script</code>, or
<code>--quiet</code> option. Evaluation returns when milestone M1 in the
[wave plan](project/wave-plan.md) lands.

## Development gates

From the repository root, run <code>nix develop</code> and then:

~~~sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
python3 scripts/check_standards.py
~~~

Coverage uses LLVM instrumentation through the flake app:

~~~sh
nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
~~~

Build the documentation with:

~~~sh
mkdocs build --strict --config-file docs/mkdocs.yml
~~~
