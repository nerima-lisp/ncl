# Handoff: PPRINT logical block runtime

## Objective

Finish `PPRINT-LOGICAL-BLOCK`, `PPRINT-POP`, and `PPRINT-EXIT-IF-LIST-EXHAUSTED` runtime behavior, verify a CLHS-style logical-block traversal, inspect PR #82 CI, and leave a pushed branch handoff.

## Authorization and branch

- Branch: `takeokunn-pprint-engine`
- Commit and push are authorized for this lane.
- No changes were made to the default branch or to the c1-ansi worktree.

## Changes

- Added NCL-EXT runtime builtins and per-thread logical-block state in `packages/printer/src/pprint.rs` and `packages/printer/src/pprint_logical_block.rs`.
- `PPRINT-POP` now advances proper lists, honors `*PRINT-LENGTH*`, returns a dotted tail once, and stops on repeated cons identities.
- `PPRINT-EXIT-IF-LIST-EXHAUSTED` shares the same cursor, length, and cycle state.
- The logical-block bridge invokes user thunks through `BuiltinFunctionCaller`, preserving closure execution and non-local exit handling.
- Registered `*PRINT-LENGTH*` and `*PRINT-LEVEL*` as printer-owned special variables in `packages/printer/src/builtins.rs`.
- Kept macro changes limited to PPRINT registration/expansion in `packages/lib/macros/src/pprint.rs`, `packages/lib/macros/src/lib.rs`, and `packages/lib/macros/src/owned_macros.rs`.
- Added runtime tests in `packages/runtime/src/runtime_tests.rs` for a fill-style traversal with `*PRINT-LENGTH*` and for a dotted tail.

## Verification

- `cargo test --locked -p ncl-printer -p ncl-lib-macros -p ncl-lib-format -p ncl-runtime --quiet`: passed; all selected test binaries passed, including 25 printer tests and 24 runtime tests.
- `nix develop --command cargo clippy --locked -p ncl-printer -p ncl-lib-macros -p ncl-lib-format -p ncl-runtime --all-targets --all-features -- -D warnings`: passed.
- `nix run nixpkgs#rustfmt -- --edition 2024 ...`: passed after formatting the changed Rust files.
- `python3 scripts/check_standards.py`: passed with no violations after splitting `packages/printer/src/pprint_logical_block.rs`.
- `git diff --check`: passed.
- `gh pr checks 82`: observed Linux/macOS check failures on the prior pushed revision. The earlier added-lines report identified only branch-added violations; main's latest CI had no equivalent violations. A follow-up run inspection hit GitHub HTTP 403, so `gh api rate_limit` was used. The reported core reset was `2026-10-04 13:09:59 JST`; no further GitHub calls should be made before that time.

## Conflict review

The c1-ansi branch and this branch both add/modify `packages/lib/macros/src/lib.rs` and `packages/lib/macros/src/owned_macros.rs`, so those files remain integration conflict points. This branch's additions in those files are PPRINT module/callback and PPRINT name-list entries only. No LOOP implementation changes were made here. c1-ansi's branch-side additions include LOOP entries and unrelated compiler/IR/runtime escape work.

## Remaining

- Push the final commit containing the source changes and this handoff.
- After the GitHub API reset window, one minimal `gh pr checks 82` call may be made, with no watch/sleep loop. If not called, report the 403/rate-limit gap explicitly.
