# Handoff: pprint-2

## Completed

- Added the public FORMAT/logical-block boundary in `packages/printer/src/pretty.rs`.
  `PrettyPrinter` now implements `CharSink`, keeps logical-block state, supports
  newline/indent/tab operations, and exposes `finish()` to flush a trailing
  conditional break.
- Documented the adapter contract in `packages/printer/README.md`.
- Confirmed dynamic special lookup through the current runtime path: `progv`
  updates symbol value cells during its dynamic extent and
  `PrintOptions::from_specials` reads them. Added regression coverage.
- Added priority-aware dispatch entries and basic `CONS`, `LIST`, `ATOM`,
  `SYMBOL`, and `INTEGER` type matching while preserving legacy entries.
- Added `*PRINT-LINES*` output truncation through `LineLimitSink`; once the
  line limit is reached, output is suppressed and `..` is emitted once.
- Commits pushed on `takeokunn-pprint-engine`: `044eb2dd`, `ffae204d`,
  `fc0c38aa`, `60be981f`.

## Remaining

- Standard operator dispatch for `QUOTE`, `LET`, `DEFUN`, and related forms.
- Full Common Lisp type-specifier dispatch beyond the basic built-ins.
- Stateful Lisp `PPRINT-NEWLINE`, `PPRINT-INDENT`, `PPRINT-TAB`, `PPRINT-FILL`,
  `PPRINT-LINEAR`, and `PPRINT-TABULAR` wrappers. They are currently callable
  placeholders; c3 can use the documented `PrettyPrinter` boundary directly.
- FORMAT parser/executor changes for `~_`, `~I`, `~W`, and `~<~:>` belong to
  the c3-format worktree and must not be edited here.
- Verify detailed `*PRINT-CIRCLE*` interaction at truncation boundaries.

## First next task

Add operator-specific standard dispatch without changing the public
`PrettyPrinter` boundary. Start by extending `packages/printer/src/cons.rs`
or a dedicated dispatch formatter, then add CLHS 22.2 tests for `QUOTE`,
`LET`, and `DEFUN` forms.

## Traps

- This checkout is a Git worktree, not a jj workspace. Use Git here.
- Do not edit or merge the c3-format worktree.
- Priority entries use an internal `(type-specifier function . priority)`
  payload; legacy `(type-specifier . function)` entries remain supported.
- `PrettyPrinter::finish()` is needed when a conditional newline is the last
  operation; `end_logical_block()` also flushes before its suffix.
- `*PRINT-LINES*` truncation is a sink-level guard. It preserves circle state
  internally but needs additional behavioral tests for labels at the cutoff.
- `.mediator/` and build artifacts must not be committed.

## Related paths

- `packages/printer/src/pretty.rs`
- `packages/printer/src/print.rs`
- `packages/printer/src/builtins.rs`
- `packages/printer/src/options.rs`
- `packages/printer/src/options/specials.rs`
- `packages/printer/src/cons.rs`
- `packages/printer/tests/builtins.rs`
- `packages/printer/tests/options.rs`
- `packages/printer/tests/printing.rs`
- `packages/printer/README.md`
- `packages/lib/format`

## Verification

- `CARGO_TARGET_DIR=/tmp/ncl-pprint-target cargo test --locked -p ncl-printer`: exit 0 before the final dispatch/line-limit commits; focused dispatch and line-limit tests also exit 0 afterward.
- `nix develop --command cargo clippy --locked -p ncl-printer --all-targets --all-features -- -D warnings`: exit 0 after the final changes.
- `git diff --check`: exit 0.
- `git ls-remote origin refs/heads/takeokunn-pprint-engine`: verified `60be981f`.
