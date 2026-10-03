# cov-mq handoff

## State

- Branch: `takeokunn-cov-mq-3`, created from `origin/main` at `1814e5fe`.
- The original detached worktree commit `274da21b` was not carried forward because it only changes CI coverage collection and is not part of the current-main source measurement.
- Existing generated `target-cov-mq/` remains uncommitted.

## Investigation

- `tests/e2ehash.rs` contains 55 cases.
- All 55 cases passed individually with the release binary, including the hash-array cases.
- All 55 cases passed individually with the debug instrumented binary.
- `cargo test --locked -p ncl --test e2ehash` on macOS exceeded a 300-second timeout while running the single test. The release test passed in 16.39 seconds. This is a debug-process performance issue in the local environment, not a reproducing single ncl case failure.
- The main coverage run `36798694977` reports `112178` regions and `23771` notcovered (`78.81%`). Aggregated top crates by region notcovered: `packages/codegen` 2953, `packages/compiler` 2566, `packages/lib/macros` 2363, `packages/lib/numbers` 1745, `packages/clos` 1563.
- PR #53 run `37135087581` reports `126679` regions and `10917` notcovered (`91.38%`). Aggregated top crates: `packages/lib/macros` 2968, `packages/codegen` 2906, `packages/compiler` 2238, `packages/object` 2000, `packages/clos` 1822.

## Changes

- Added value assertions for x86-64 constant-word conversion, unsupported runtime-table constants, and all IR comparison conditions in `packages/codegen/src/target_x86_64_lowering/ops.rs`.

## Verification

- `nix develop --command cargo test --locked -p ncl-codegen x86_64_ -- --nocapture`: passed, 21 selected tests, 34 filtered out.
- `nix develop --command cargo fmt --all -- --check`: passed.
- `nix develop --command cargo test --locked -p ncl --test e2ehash --release -- --nocapture`: passed, 1 test.

## Remaining

- The full current-main llvm-cov run was not completed locally because the debug `e2ehash` test exceeded the local timeout.
- No threshold or exclusion was changed.
- No generated coverage files were committed.
