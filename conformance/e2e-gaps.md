# E2E gaps

## hash-arrays

`tests/e2ehash.rs` currently contains 56 ordinary probes and no XFAILs. This
describes its test coverage, not every supported or unsupported hash/array
operation. `VECTOR-PUSH-EXTEND` and `VECTOR-POP` are ordinary probes in the
compiled matrix. AArch64 lowering passes four call arguments in registers and
stores any additional arguments in its outgoing area. `SETF SBIT` is also an
ordinary probe using a simple bit vector created by `MAKE-ARRAY`.

## macros

`tests/e2emacro.rs` currently asserts compiled results for control macros, LOOP clauses,
`MAXIMIZE`/`MINIMIZE`, `ROTATEF`, `SHIFTF`, and `MULTIPLE-VALUE-SETQ`. The
`MULTIPLE-VALUE-SETQ` probes cover extra values, missing values, no values, an
empty variable list, and return of the primary value. Its expansion captures
the values in a list, assigns each variable from its corresponding position,
and ignores values beyond the last variable. A missing position reads as
`NIL`, including the primary value when there are no values.

`IF`/`WHEN`/`UNLESS` and LOOP clauses that lower through them, such as `COUNT`,
now merge mutated variables across branches through a block parameter in
`packages/compiler/front/src/lower/expr.rs`. LOOP also supports conditional
clauses with `AND`/`ELSE`/`END`/`IT`, destructuring `FOR` variables, and a
default initial value of `0` for arithmetic `FOR` clauses without
`FROM`/`UPFROM`/`DOWNFROM` (`packages/lib/macros/src/loop`). The
`MAXIMIZE`/`MINIMIZE` accumulators use the `NIL` accumulator value to identify
their first iteration, avoiding the unsupported `Word::TRUE` literal.

The only explicit macro XFAIL in `tests/e2emacro.rs` is `loop-with-finally`.
This file records the current probes, not every unsupported LOOP clause. The
XFAIL records NCL's current `UndefinedValue` rejection. The form has no
iteration-control clause, so the equivalent Common Lisp loop does not
terminate; the test records NCL's early rejection instead of running an
unbounded probe.

| case | source | expected exit | stderr substring | cause |
| --- | --- | ---: | --- | --- |
| `loop-with-finally` | `(loop with x = 2 finally (return x))` | 1 | `UndefinedValue` | NCL rejects this form before evaluation because it has no iteration-control clause; the equivalent Common Lisp loop does not terminate. |

## Numbers

The compiled numbers builtin probe currently has no XFAIL cases. All 104 registered
builtins have a valid probe expression and assert their ANSI stdout representation,
exit code, and empty stderr. Expressions for floating-point-only builtins use a
floating-point argument; fixed-arity builtins are not probed with extra arguments.

If a future probe cannot execute, add it to the explicit XFAIL table in
`tests/e2enum.rs` with the observed stderr pattern, exit code, and the cause at the
responsible file and line or the missing Common Lisp function.
## CLOS

The compiled CLOS probe is in `tests/e2eclos.rs`. Built-in class inspection,
class and method definition, inheritance dispatch, standard method qualifiers,
`call-next-method`, `next-method-p`, symbol class designators, and slot initargs
are covered by value assertions there and are not XFAILs.
The EQL-specializer path has no end-to-end value assertion. The unit test
`dispatch_specificity_uses_argument_precedence_and_eql_descriptors` in
`packages/clos/src/dispatch_tests.rs` checks score ordering, not compiled runtime
dispatch.

| case | source | stderr | cause |
| --- | --- | --- | --- |
| unknown generic function | `(clos-unknown-generic 1)` | `UNDEFINED-FUNCTION` | compiled calls resolve function cells through `packages/runtime/src/function_call.rs:133-139`; CLOS does not install a generic-function cell |

The remaining requested CLOS behaviors are tracked as implementation gaps:
`CHANGE-CLASS`, `UPDATE-INSTANCE-FOR-DIFFERENT-CLASS`,
`UPDATE-INSTANCE-FOR-REDEFINED-CLASS`, `ENSURE-GENERIC-FUNCTION`,
`ADD-METHOD`/`REMOVE-METHOD`/`FIND-METHOD`, user-defined method combinations,
`SLOT-UNBOUND`/`SLOT-MISSING`, `PRINT-OBJECT`/`DESCRIBE-OBJECT`,
`WITH-SLOTS`/`WITH-ACCESSORS`, and complete multiple-inheritance CPL.

## streams

`tests/e2estream.rs` covers the 28 builtins registered by
`packages/lib/streams/src/registration.rs`. All compiled-code probes now pass
with exact exit status, stdout, and stderr assertions; there are no remaining
stream XFAILs.
