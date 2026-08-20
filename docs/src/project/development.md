# Development

This page collects the local verification commands for the Rust workspace, the
direct Common Lisp core, and the documentation site.

## Isolated worktrees

Keep independent changes in separate worktrees so that each work unit can be
verified and integrated without changing another task's working tree:

~~~sh
git worktree list
git status --short --branch
git diff --check
~~~

Run the relevant checks from the worktree that contains the change. Once the
change is represented by a tested commit on `main`, remove its source worktree
and delete its branch. Keep a worktree that conflicts with the current `main`
separate until its changes have been reviewed as an independent work unit.

## Rust workspace

The workspace requires Rust 1.97 or newer and pins Rust 1.97.1 in
`rust-toolchain.toml`. Run the Rust checks from the repository root:

~~~sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
~~~

Use `nix develop path:.` when the Rust tools are not installed locally or when
you want the same toolchain and supporting programs used by the project gates.

## Common Lisp core

The direct Common Lisp core is declared by `ncl.asd`. The development shell
provides SBCL, cl-weave, paredit-cli, and the documentation tools:

~~~sh
nix develop path:.
nix run path:. -- --eval '(+ 1 2)'
nix run path:.#test
nix run path:.#coverage
~~~

Coverage defaults to `artifacts/ncl-coverage/`; set `NCL_COVERAGE_DIR` to use a
different output directory. See [Common Lisp core](../guide/common-lisp-core.md)
for the source boundaries and test workflow.

## Rust coverage

Run the Rust workspace tests with LLVM coverage and the current ratchet floors:

~~~sh
nix run path:.#rust-coverage -- --summary-only
~~~

The command requires at least 75% line, 78% function, and 75% region coverage.
For a browsable report, use `--html --output-dir artifacts/rust-coverage` in
place of `--summary-only`. The same ratchet is enforced by the
`ncl-rust-coverage` flake check.

## Documentation

Build the site with strict MkDocs validation:

~~~sh
mkdocs build --strict --config-file docs/mkdocs.yml
~~~

The configuration reads from `docs/src` and writes the ignored `site/`
directory. The `ncl-docs` flake check runs the same strict build.

## Full local gate

The repository-wide Nix check includes the Rust, Common Lisp, coverage, and
documentation gates:

~~~sh
nix flake check --no-write-lock-file
~~~

For focused iteration, run the individual commands above before invoking the
full check.
