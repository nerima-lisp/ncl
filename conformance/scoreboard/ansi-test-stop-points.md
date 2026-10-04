# ANSI-test stop points

This ledger records blockers observed while loading the shared ANSI-test
runtime. Each entry is a reproducible compiler or runtime boundary, not a
test skip.

| Status | Stop point | Minimal reproduction | Cause classification |
| --- | --- | --- | --- |
| fixed | `LOOP FOR (A B) = ...` | `(loop for (a b) = '(2) do (return (list a b)))` | LOOP `=` clauses rejected destructuring patterns; fixed by routing them through the existing destructuring binder. |
| open | `rt.lsp` top-level form 46, `do-entry` | `(compile-file "rt.lsp")` from the ANSI-test directory after loading `rt-package.lsp` | The nested `handler-bind` / `return-from` function reaches `inline-direct-calls` with `UndefinedValue` and `TypeMismatch` verifier errors. |

## Current form-46 evidence

Direct reproduction from the ANSI-test directory:

```text
target/release/ncl --compile-file /tmp/rt-prefix-46.lsp
```

Exit status: `1`. The failure is `UndefinedValue(ValueId(299))`,
`UndefinedValue(ValueId(300))`, and `SuccessorType(BlockId(21))` during
`inline-direct-calls`. The minimal nested `do-entry` regression passes, but
the full `rt.lsp` prefix does not finish loading, so no chapter scoreboard
exists.

The c1-diag five-point proposal status:

1. No current-block parameter-count fallback: implemented.
2. Explicit lexical-name to SSA mappings: normal edges implemented; nested
   handler edge remains incomplete.
3. Shared mapping for merge arguments and rebinding: partly implemented; the
   handler-side mapping remains the blocker.
4. Explicit `BlockId` block-parameter API: implemented in the IR builder.
5. Construction-order-independent reachability: implemented with CFG
   traversal from the entry block.

The next first task is to enumerate every predecessor of `BlockId(21)`, find
the handler predecessor supplying `ValueId(299)` and `ValueId(300)`, and make
that edge use the same named mapping as merge rebinding.
