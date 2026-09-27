# End-to-end conformance gaps

## Numbers

The compiled numbers builtin probe currently has no XFAIL cases. All 104 registered
builtins have a valid probe expression and assert their ANSI stdout representation,
exit code, and empty stderr. Expressions for floating-point-only builtins use a
floating-point argument; fixed-arity builtins are not probed with extra arguments.

If a future probe cannot execute, add it to the explicit XFAIL table in
`tests/e2enum.rs` with the observed stderr pattern, exit code, and the cause at the
responsible file and line or the missing Common Lisp function.
