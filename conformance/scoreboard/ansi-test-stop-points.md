# ANSI-test stop points

This ledger records blockers observed while loading the shared ANSI-test
runtime. Each entry is a reproducible compiler or runtime boundary, not a
test skip.

| Status | Stop point | Minimal reproduction | Cause classification |
| --- | --- | --- | --- |
| fixed | `LOOP FOR (A B) = ...` | `(loop for (a b) = '(2) do (return (list a b)))` | LOOP `=` clauses rejected destructuring patterns; fixed by routing them through the existing destructuring binder. |
| open | `rt.lsp` top-level form 46, `do-entry` | `(compile-file "rt.lsp")` from the ANSI-test directory after loading `rt-package.lsp` | The nested `handler-bind` / `return-from` function reaches `inline-direct-calls` with `UndefinedValue` and `TypeMismatch` verifier errors. |

Run the chapter runner with `--timeout` and `--output` to preserve the
chapter-level result and the current stop-point evidence.
