# ansi-test RT loader blocker

The ANSI test suite is pinned to `ca06bd919661af162c67407c9d994e881870bdb3`.
The RT package bootstrap now loads successfully, but `rt.lsp` cannot finish loading
because the implementation of `COMMON-LISP:LOOP` hangs for a simple `=` binding.

Minimal reproduction:

```lisp
(loop for x = 1 collect x)
```

Observed on the local debug binary: the form does not return. The same defect is
reported as `TypeError` while loading `rt.lsp`, at the `pending-tests` definition
(`rt.lsp`, around line 400), whose body contains `loop for n in notes for note = ...`.
The parser accepts `=` in `packages/lib/macros/src/loop/parser.rs`, but the
expansion path does not complete this form. This is a loop-macro implementation
issue, not an ANSI test failure. The runner therefore reports every discovered
`deftest` as unexecuted until the loop lane supplies the fix.
