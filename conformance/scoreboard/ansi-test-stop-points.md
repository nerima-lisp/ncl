# ANSI-test stop points

This ledger records blockers observed while loading the shared ANSI-test
runtime. Each entry is a reproducible compiler or runtime boundary, not a
test skip.

| Status | Stop point | Minimal reproduction | Cause classification |
| --- | --- | --- | --- |
| fixed | `LOOP FOR (A B) = ...` | `(loop for (a b) = '(2) do (return (list a b)))` | LOOP `=` clauses rejected destructuring patterns; fixed by routing them through the existing destructuring binder. |
| open | compiling `rt.lsp` after LOOP expansion | `(compile-file "rt.lsp")` from the ANSI-test directory after loading `rt-package.lsp` | `ControlError` remains after a compiled nested handler escape; exact top-level form is still being isolated. |

Run the chapter runner with `--timeout` and `--output` to preserve the
chapter-level result and the current stop-point evidence.
