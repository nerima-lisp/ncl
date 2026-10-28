# NCL

NCL is a Rust-native Common Lisp implementation. It targets ANSI Common Lisp
and the selected SBCL public APIs, contribs, and library-referenced symbols
recorded in the [wave plan](docs/src/project/wave-plan.md). SBCL 2.6.0 supplies
the conformance and performance baselines; NCL has its own implementation and
extension API.

The [wave plan](docs/src/project/wave-plan.md) records current decisions, phases, and acceptance criteria. Design contracts are in [docs/src/design/](docs/src/design/).

## Install

The v0.1.0 release supports these four platforms, with a binary for each:

| Platform | Archive |
|---|---|
| x86-64 Linux | `ncl-v0.1.0-x86_64-unknown-linux-gnu.tar.gz` |
| AArch64 Linux | `ncl-v0.1.0-aarch64-unknown-linux-gnu.tar.gz` |
| Intel macOS | `ncl-v0.1.0-x86_64-apple-darwin.tar.gz` |
| Apple Silicon macOS | `ncl-v0.1.0-aarch64-apple-darwin.tar.gz` |

Each archive contains the `ncl` executable at its root. Download the archive and its `.sha256` sidecar from the [v0.1.0 release](https://github.com/nerima-lisp/ncl/releases/tag/v0.1.0). Choose the target from the table above, then verify and extract the archive:

```sh
TARGET=aarch64-apple-darwin
BASE_URL="https://github.com/nerima-lisp/ncl/releases/download/v0.1.0"
ASSET="ncl-v0.1.0-${TARGET}.tar.gz"

curl -LO "${BASE_URL}/${ASSET}"
curl -LO "${BASE_URL}/${ASSET}.sha256"

if command -v sha256sum >/dev/null 2>&1; then
  sha256sum -c "${ASSET}.sha256"
else
  shasum -a 256 -c "${ASSET}.sha256"
fi

tar -xzf "${ASSET}"
./ncl --version
```

## Usage

From the directory containing the extracted binary, evaluate an expression:

```sh
./ncl --eval '(+ 1 2)'
```

The command prints `3` and exits with status 0. This example was verified with the dry-run AArch64 macOS release asset.

## Status and limitations

The release workflow built assets for all four platforms listed above. The [ANSI scoreboard run 40651047770](https://github.com/nerima-lisp/ncl/actions/runs/40651047770) at commit [`d41da4a0`](https://github.com/nerima-lisp/ncl/commit/d41da4a0) reported 21,732 tests. Every test errored before reaching its assertions with the shared compiler front-end `UnsupportedLiteral` failure. The zero passing count is not a conformance coverage measure. The first post-release task is to fix this blocker and rerun the ANSI suite.

The [latest complete cl-bench run, 40626164969](https://github.com/nerima-lisp/ncl/actions/runs/40626164969) at commit [`215a0f1ff9560e378552b9693ab5d10efca78326`](https://github.com/nerima-lisp/ncl/commit/215a0f1ff9560e378552b9693ab5d10efca78326) reported:

Load failures occur while benchmark source files are loaded. Execute failures occur while a registered benchmark's setup or function runs.

| Status | Rows |
|---|---:|
| Load failure | 38 |
| Execute failure | 27 |
| Passed | 0 |
| Total | 65 |

The [latest complete workspace coverage run, 40677969202](https://github.com/nerima-lisp/ncl/actions/runs/40677969202) at commit [`aafb21a2c70af214b211a47178a2ee6cf14a5ba9`](https://github.com/nerima-lisp/ncl/commit/aafb21a2c70af214b211a47178a2ee6cf14a5ba9) reported 84.31% Rust workspace region coverage: tests exercised 131,620 of the 156,112 code regions counted by LLVM coverage. This measures Rust code coverage, not ANSI conformance coverage. Performance is not yet competitive with SBCL. The remaining ANSI surface, performance work, and library load matrix are tracked in [the wave plan](docs/src/project/wave-plan.md).

## Development

Install Nix with flakes enabled. Open a development shell before running Rust commands; bare `cargo` may fail to link the binary on macOS:

```sh
nix develop
```

From the development shell, run formatting, lint, and standards checks:

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
python3 scripts/check_standards.py
```

For local lane work, run `cargo test -p <crate>` for each changed crate. If a
test under `tests/` changes, run `cargo test -p ncl --test <name>`. CI runs
workspace tests in debug and release on x86-64 Linux and AArch64 macOS:

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
