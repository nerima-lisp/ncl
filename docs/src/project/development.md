# Development

This page collects the local verification commands for the Rust workspace and
the documentation site.

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
empty, the snapshot has been fully superseded: there is no work to port.
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

## Rust workspace

The workspace pins Rust 1.98.0 in `rust-toolchain.toml`. Run the Rust checks
from the repository root:

~~~sh
cargo fmt --all -- --check
cargo check --locked --workspace --all-targets --all-features
cargo test --locked --workspace --all-features --all-targets
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
~~~

Use `nix develop path:.` when the Rust tools are not installed locally or when
you want the same toolchain and supporting programs used by the project gates.

## Rust coverage

Run the Rust workspace tests with LLVM coverage and inspect the workspace report:

~~~sh
nix run path:.#rust-coverage -- --summary-only
~~~

The workspace-wide 95.0% region figure is a goal. CI reports it on every
coverage run and warns when it is below the goal. The blocking path gate reads
the NUL-delimited `git diff --no-ext-diff --name-status --find-renames` output.
For a detected rename, the candidate path uses the parent summary entry for
the old path. A path missing from its summary contributes zero regions. The
changed paths pass when their combined missed region count does not exceed the
parent's count, or their combined coverage is at least 95.0%. Each changed
path with a parent summary entry, including a renamed path matched to its old
name, must also have no increase in missed regions or reach 95.0% coverage
individually.
The path gate runs on pull requests and non-main branch pushes. Main pushes
skip it but still upload the workspace summary for later baselines.

To make a local command fail when the workspace total is below 95.0%, include
the threshold:

~~~sh
nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
~~~

For the path gate, CI uses the exact merge-base coverage artifact when
available. If that main run was cancelled or has no artifact, CI uses the
nearest ancestor run with a non-expired coverage artifact. The gate fails when
no such ancestor artifact is available.

For a browsable report, use `--html --output-dir artifacts/rust-coverage` in
place of `--summary-only`. The flake app supplies the pinned Rust and LLVM
tools.

## Documentation

Build the site with strict MkDocs validation:

~~~sh
nix develop --command mkdocs build --strict --config-file docs/mkdocs.yml
~~~

The configuration reads from `docs/src` and writes the ignored `site/`
directory.

## Full local gate

The repository-wide Nix check validates the flake outputs. Run the Rust and
documentation gates explicitly as shown above:

~~~sh
nix flake check --all-systems
~~~

For focused iteration, run the individual commands above before invoking the
full check.
