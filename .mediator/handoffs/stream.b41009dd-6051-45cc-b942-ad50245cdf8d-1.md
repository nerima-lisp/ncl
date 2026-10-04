# c1 ansi continuation handoff

## Current state

The verified control-flow regression work is pushed on `takeokunn-c1-ansi`:

- `e16e767f` preserves lexical SSA values across nested control exits.
- `7821435a` replaces current-block parameter guessing with explicit lexical-name mappings, adds CFG fixed-point reachability, and carries live values through dynamic-binding merges.

The working tree is clean after `7821435a`.

## Reproduction

Build in the Nix development shell, then run from the ansi-test directory:

```text
cargo build --release
target/release/ncl --compile-file /tmp/rt-prefix-46.lsp
```

The command still fails in `inline-direct-calls` with:

```text
UndefinedValue(ValueId(299)), SuccessorType(BlockId(21)),
UndefinedValue(ValueId(300)), SuccessorType(BlockId(21))
```

The minimal nested `do-entry` regression passes, as do the escape and front walker tests.

## Findings

The normal catch edge currently resolves live names explicitly. A diagnostic run showed the catch merge at `BlockId(47)` receives `S=ValueId(314)` and `ENTRY=ValueId(315)`. The remaining `ValueId(299/300)` values originate from the terminated nested escaping handler path, not that normal edge.

The unresolved path is therefore the handler edge from the nested escaping block into the enclosing catch/merge. Do not reintroduce a fallback that infers live values from the current block parameter count. Keep handler payload parameters, `binding_targets`, exit parameters, and environment rebinding derived from one named live mapping.

## Verified commands

```text
cargo fmt --all -- --check
cargo test --test escape do_entry_nested_handler_return_from_compiles
cargo test -p ncl-compiler-front --test analysis_walker_matrix
```

All three completed successfully before `7821435a` was pushed. The full release prefix remains the blocking integration check. `gh` was not called in this continuation.

## Remaining work

1. Dump the final predecessor/argument table for `BlockId(21)` and identify which handler predecessor supplies `ValueId(299/300)`.
2. Make the handler edge use the same explicit `(lexical name, SSA value)` mapping as the exit rebind.
3. Re-run the release prefix and the full `rt.lsp` forms.
4. Run the 25-chapter ansi-test runner with output, record chapter scores and stop-point causes, then commit and push the scoreboard.
5. Check PR #53 CI only with the rate-limited `gh pr checks 53` policy.
