# NCL

NCL is a Common Lisp project with two execution layers: a Rust-native runtime
and a direct, CPS-first Common Lisp core. Both layers keep the supported
language surface explicit instead of implying compatibility with a complete
external implementation.

The Rust workspace provides the interpreted evaluator and stack-bytecode
compiler. The Common Lisp core is organized as readable source files for
language experiments, embedding, and direct macro-driven extension.

The current implemented surface includes reader dispatch literals, compiled and
interpreted generalized places, bounded condition definition and signaling, and
string stream forms such as <code>with-input-from-string</code> and
<code>with-output-to-string</code>.

## Quick Start

The workspace requires Rust 1.97 or newer and pins Rust 1.97.1 in
`rust-toolchain.toml`.

~~~sh
cargo run -- --eval '(+ 1 2)'
cargo run -- --compiled --eval '(+ 1 2)'
cargo run -- --compile --eval '(defun square (x) (* x x))'
cargo run -- --repl
cargo run -- --file path/to/program.lisp
~~~

Multiple <code>--eval</code> options run in the same runtime, so definitions
from an earlier form are available to later forms. Use <code>--quiet</code> to
suppress value output and REPL prompts. <code>--compile</code> reads the input,
performs supported compile-time preparation such as macro and package
processing, builds bytecode artifacts, and reports their sizes without
executing ordinary runtime forms. Use <code>--compiled</code> when the input
should also execute through the bytecode VM.

## Install

Build a release binary from a checkout:

~~~sh
cargo build --release
./target/release/ncl --eval '(+ 1 2)'
~~~

The project does not currently publish a package or prebuilt binary.

## Common Lisp core

The Nix flake provides Rust, rustfmt, Clippy, SBCL, cl-weave, paredit-cli, and
documentation tooling:

~~~sh
nix develop path:.
nix run path:. -- --eval '(+ 1 2)'
nix run path:.#test
nix run path:.#coverage
nix run path:.#rust-coverage -- --summary-only
~~~

The source declaration is [ncl.asd](ncl.asd). The CLI and standalone test
entry points load that declared source sequence directly, while coverage and
ASDF consumers use the system declaration. Coverage artifacts are written
below `artifacts/` by default; set `NCL_COVERAGE_DIR` to choose another
directory. The `test` and `coverage` apps default to one worker and a 5000 ms
per-test timeout; pass additional cl-weave options after the app name. The direct core's
source boundaries and extension points are described in the
[Common Lisp core guide](docs/src/guide/common-lisp-core.md).

The `rust-coverage` app runs every Rust workspace target with LLVM coverage.
It enforces the current ratchet floors of 75% line, 78% function, and 75%
region coverage; the long-term target is 100% for each metric. Pass `--html`
and `--output-dir artifacts/rust-coverage` to produce a browsable report.

The Common Lisp tests use [cl-weave](https://github.com/nerima-lisp/cl-weave),
and source formatting and structural checks use
[paredit-cli](https://github.com/nerima-lisp/paredit-cli).

## Documentation

The detailed documentation is in [docs/src/index.md](docs/src/index.md).
Build it locally with MkDocs:

~~~sh
mkdocs build --strict --config-file docs/mkdocs.yml
~~~

## Development

~~~sh
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo fmt --all -- --check
nix run path:.#rust-coverage -- --summary-only
nix flake check path:.
nix flake check --no-build --all-systems path:.
mkdocs build --strict --config-file docs/mkdocs.yml
~~~

If Rust is provided through Nix, run the formatter check from the project
development shell:

~~~sh
nix develop path:. --command cargo fmt --all -- --check
~~~

GitHub Actions runs the Rust formatter, locked workspace build, test, Clippy,
and complete Nix flake gates from
[.github/workflows/ci.yml](.github/workflows/ci.yml). The Nix job uses the
[Determinate Nix Action](https://github.com/determinatesystems/determinate-nix-action)
and [Magic Nix Cache](https://github.com/marketplace/actions/magic-nix-cache)
to provide a reproducible CI environment.

The Nix flake adds repository-level checks for the Lisp test suite, Paredit
syntax validation, strict MkDocs builds, and the Lisp coverage report. CI also
runs the Rust LLVM coverage ratchet. Run `nix flake check path:.` and the
`rust-coverage` app to execute the complete local check set.

## Contributing

Keep changes focused, add executable tests for behavior changes, and update
the documentation when the supported language surface changes.

## Support

Report reproducible bugs and documentation issues in the
[NCL issue tracker](https://github.com/nerima-lisp/ncl/issues).

## License

The package metadata declares the MIT license.
