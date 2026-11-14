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
coverage run and warns when it is below the goal. The blocking path gate
compares the candidate coverage summary with a parent coverage summary. For
pull requests, CI uses the pull request base commit. For branch pushes, it
fetches the base branch tip. In both cases, CI computes their merge base with
`git merge-base HEAD <base-sha>` and passes the NUL-delimited output of
`git diff --no-ext-diff --name-status --find-renames -z <merge-base> HEAD` to
the gate.

For a detected rename, the candidate path uses the parent summary entry for
the old path. A candidate path missing from the candidate summary contributes
zero candidate regions; a parent path missing from the parent summary
contributes zero parent regions. Clause (a) passes when the changed paths'
combined missed-region count does not exceed the parent count, or their
combined candidate coverage is at least 95.0%. Clause (b) checks each changed
path that has a parent summary entry, including a renamed path matched to its
old name. It passes when the candidate path has no increase in missed regions
or reaches 95.0% coverage individually.
The path gate runs on pull requests and non-main branch pushes. Main pushes
skip it but still upload the workspace summary for later baselines.

To make a local command fail when the workspace total is below 95.0%, include
the threshold:

~~~sh
nix run path:.#rust-coverage -- --summary-only --fail-under-regions 95.0
~~~

For the parent summary, CI first looks for a `coverage-summary` artifact from
the base branch at the exact merge-base commit. If it is unavailable, CI checks
that commit's first-parent ancestors, starting with the nearest one, and uses
the first commit with a matching artifact. “Non-expired” means the GitHub
artifact listing reports `expired: false`. If no matching artifact exists, the
gate fails.

Before pushing a candidate, run `python3 scripts/precheck_changed_path_coverage.py`
from the workspace root. It finds the same parent artifact, measures only Cargo
packages containing changed `.rs` paths with `--all-features --all-targets`, and
passes that summary and the full changed-path list to the CI gate script. The
local run uses `cargo llvm-cov`'s default Cargo test runner. The checker reports
both clauses, and exits successfully only when (a) and (b) pass. This command
requires Python 3.11 or newer, a jj workspace with `main@origin`, an authenticated
`gh` that can read Actions artifacts, `git`, and `cargo llvm-cov` on `PATH`. Use
`nix develop path:.` for the pinned Rust coverage tool. For a historical commit
exported into a source tree, add `--candidate-rev <sha> --source-root <path>`;
parent lookup still uses current `main@origin`.

Package measurement omits tests from other packages that may call changed code,
so a precheck can fail while workspace CI passes: with 10 parent misses, package
tests could report 12 while workspace tests cover three more regions and report
9. A precheck PASS predicts a CI PASS only when candidate, parent artifact,
toolchain, target configuration, features, test selection, and instrumented
region sets match CI and tests are deterministic. For example, with 10 parent
misses and 9 package misses, a workspace run containing that same package test
set can only maintain or reduce the missed count, so it also passes. Feature
unification or build differences that change region sets, or different test
selection, fall outside this prediction.

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
