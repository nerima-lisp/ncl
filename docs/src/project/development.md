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

Run the relevant checks from the worktree that contains the change. Once each
work unit is represented by a tested commit on `main`, remove its disposable
source worktree and delete its branch. Keep a worktree that conflicts with the
current `main` separate until its changes have been reviewed as an independent
work unit.

### Porting work from a stale worktree

Before porting anything, confirm the worktree actually diverges from `main`:

~~~sh
git merge-base --is-ancestor <worktree-head> main && echo "already merged"
git diff --stat main...<worktree-head>
~~~

When `<worktree-head>` is already an ancestor of `main` and the diffstat is
empty, the snapshot has been fully superseded — there is no work to port.
Skip the procedure below and remove the worktree directly. An untracked
planning document left in such a worktree (for example a next-step execution
spec) describes future work, not a deliverable to merge; read it for context,
then discard the worktree with `git worktree remove`.

When a detached worktree was started from an older `main`, inspect its changes
before integrating them. Staged and unstaged changes may be separate work
units. Preserve that boundary by committing and testing each unit
independently. When multiple snapshots overlap, choose one canonical
implementation, port unique documentation or configuration deliberately, and
resolve implementation conflicts in a temporary worktree based on the current
`main`:

~~~sh
git -C <source-worktree> status --short
git -C <source-worktree> diff --check
git -C <source-worktree> diff --name-status
git worktree add -b <integration-branch> <integration-worktree> main
~~~

Run the focused regression test for each work unit before committing it, then
run the workspace checks from the integration worktree. If the current tree
has since been refactored, port the behavior into its new files instead of
merging an incompatible stale snapshot wholesale. Move `main` to the tested
tip only after verifying that the expected old `main` ref is unchanged:

~~~sh
git update-ref refs/heads/main <tested-tip> <expected-old-main>
~~~

After integration, remove a source or temporary integration worktree only
when it is clean or its remaining changes have been explicitly shown to be
represented by tested commits. Inspect a dirty worktree before using `git
worktree remove --force`; keep active, concurrent, or unresolved worktrees and
branches for their owners.

Completed work units are integrated by preserving their commit boundaries,
cherry-picking the tested commits into the integration worktree, and running
the workspace tests again on `main`. The source worktree can then be removed
once `git status --short --branch` is clean. Keep the integration worktree
until documentation updates and final verification are complete.

Recent runtime refactors keep evaluator and builtin sequence responsibilities in
separate modules. When splitting another large module, preserve the existing
public(super) builtin surface, run the focused runtime tests, and integrate the
change as its own commit before removing the source worktree.

## Rust workspace

The workspace requires Rust 1.98.0 and pins Rust 1.98.0 in
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

The direct Common Lisp core is declared by `ncl.asd`. Run its ASDF entry points
with the Common Lisp implementation and dependencies installed locally:

~~~sh
sbcl --script run.lisp --eval '(+ 1 2)'
~~~

The Common Lisp test entry point currently reports test results only; it does
not generate coverage artifacts. See [Common Lisp core](../guide/common-lisp-core.md)
for the source boundaries and test workflow.

## Rust coverage

Run the Rust workspace tests with LLVM coverage and the current ratchet floors:

~~~sh
nix run path:.#rust-coverage -- --summary-only
~~~

To inspect coverage against the configured CI minimum, include the threshold:

~~~sh
nix run path:.#rust-coverage -- --summary-only --fail-under-lines 88.4
~~~

CI enforces 88.4% line coverage. The displayed workspace `TOTAL` can differ
from the value used by cargo-llvm-cov for the threshold check; verify both the
reported summary and the command exit status.

For a browsable report, use `--html --output-dir artifacts/rust-coverage` in
place of `--summary-only`. The flake app supplies the pinned Rust and LLVM
tools; CI remains the authoritative 88.4% line-coverage regression gate.

## Documentation

Build the site with strict MkDocs validation:

~~~sh
mkdocs build --strict --config-file docs/mkdocs.yml
~~~

The configuration reads from `docs/src` and writes the ignored `site/`
directory. The `ncl-docs` flake check runs the same strict build.

## Full local gate

The repository-wide Nix check validates the flake outputs. Run the Rust and
documentation gates explicitly as shown above:

~~~sh
nix flake check --no-write-lock-file
~~~

For focused iteration, run the individual commands above before invoking the
full check.
