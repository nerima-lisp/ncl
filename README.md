# NCL

NCL is a Rust-native Common Lisp runtime under active development. It provides
an interpreted evaluator and a stack-bytecode compiler with a bounded
Common Lisp-oriented surface.

## Quick Start

NCL requires Rust 1.85 or newer.

~~~sh
cargo run -- --eval '(+ 1 2)'
cargo run -- --compiled --eval '(+ 1 2)'
cargo run -- --repl
cargo run -- --file path/to/program.lisp
~~~

Multiple <code>--eval</code> options run in the same runtime, so definitions
from an earlier form are available to later forms. Use <code>--quiet</code> to
suppress value output and REPL prompts.

## Install

Build a release binary from a checkout:

~~~sh
cargo build --release
./target/release/ncl --eval '(+ 1 2)'
~~~

The project does not currently publish a package or prebuilt binary.

## Documentation

The detailed documentation is in [docs/src/index.md](docs/src/index.md).
Build it locally with MkDocs:

~~~sh
mkdocs build --strict --config-file docs/mkdocs.yml
~~~

## Development

~~~sh
cargo check --workspace --all-targets
cargo test --workspace --all-targets
cargo fmt --all -- --check
~~~

If Rust is provided through Nix, the equivalent formatter check is:

~~~sh
nix shell nixpkgs#rustc nixpkgs#rustfmt --command cargo fmt --all -- --check
~~~

## Contributing

Keep changes focused, add executable tests for behavior changes, and update
the documentation when the supported language surface changes.

## Support

Report reproducible bugs and documentation issues in the
[NCL issue tracker](https://github.com/nerima-lisp/ncl/issues).

## License

The package metadata declares the MIT license.
