# NCL

NCL is a Rust-native Common Lisp runtime under active development. It provides
an interpreted evaluator and a stack-bytecode compiler with a bounded
Common Lisp-oriented surface.

## Quick Start

NCL uses the stable Rust toolchain with the Rust 2024 edition. The required
components are declared in `rust-toolchain.toml`; `nix develop` provides the
same tools through the flake. A local `rustup` install also honors
`rust-toolchain.toml` automatically, so `cargo` commands run directly against
a bare `rustup`-managed toolchain (outside `nix develop`) are supported too.

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
cargo build --locked --workspace --release
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
cargo check --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --all-targets
cargo build --locked --workspace --release
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
~~~

If Rust is provided through Nix, enter the reproducible development shell:

~~~sh
nix develop
cargo fmt --all -- --check
cargo test --locked --workspace --all-features --all-targets
~~~

The flake also exposes the repository formatter:

~~~sh
nix fmt
~~~

The intended release lint gate is:

~~~sh
cargo clippy --workspace --all-targets --all-features -- -D warnings
~~~

The workspace currently passes this Clippy gate. The release profile enables
thin LTO, one codegen unit, symbol stripping, and abort-on-panic for a compact
production binary.

Measure test coverage from the reproducible shell, which provides both
`cargo-llvm-cov` and LLVM's profile merger:

~~~sh
llvm_path=$(nix eval --raw nixpkgs#llvmPackages_20.llvm.outPath)
LLVM_COV="$llvm_path/bin/llvm-cov" \
LLVM_PROFDATA="$llvm_path/bin/llvm-profdata" \
cargo llvm-cov --locked \
  --workspace --all-features --all-targets --summary-only --fail-under-lines 88.4
~~~

The project is pursuing 100% coverage. CI currently enforces a minimum of 88.4%
line coverage as a regression gate while the remaining error and platform
branches are covered incrementally.

CI also publishes the generated `lcov.info` file as the `coverage-lcov` artifact
for each workflow run, so coverage changes can be reviewed without relying on
local build output.

## Contributing

Keep changes focused, add executable tests for behavior changes, and update
the documentation when the supported language surface changes.

## Support

Report reproducible bugs and documentation issues in the
[NCL issue tracker](https://github.com/nerima-lisp/ncl/issues).

## License

The package metadata declares the MIT license.
