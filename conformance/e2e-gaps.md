# E2E gaps

## hash-arrays

The compiled hash/array matrix has 56 normal cases and no XFAIL cases.
`VECTOR-PUSH-EXTEND` and `VECTOR-POP` were previously XFAILed for an AArch64
four-register call-argument limit; both now compile and pass as ordinary cases
in `tests/e2ehash.rs`. `SBIT` SETF is also covered as an ordinary case.

## macros

`tests/e2emacro.rs` asserts compiled results for the working control macros and
keeps only `loop-with-finally` as an explicit XFAIL. `IF`/`WHEN`/`UNLESS` (and the LOOP clauses that lower
through them, e.g. `count`) used to leave a mutated variable's post-branch
value unmerged, producing a native IR verification error
(`pass inline-direct-calls failed`); `lower_if`
(`packages/compiler/front/src/lower/expr.rs`) now merges live variables across
branches through a block parameter, so `loop-count` and ordinary `if`/`when`
mutation moved from XFAIL to `PROBES`. `LOOP` also gained `WHEN`/`UNLESS`/`IF`
conditional clauses (with `AND`/`ELSE`/`END`/`IT`), destructuring `FOR`
variables, and a CLHS-conformant default initial value of `0` for arithmetic
`FOR` clauses without `FROM`/`UPFROM`/`DOWNFROM` (`packages/lib/macros/src/loop`).
The remaining XFAIL is `loop-with-finally`, whose body has no iteration-control
clause at all: real Common Lisp (confirmed against SBCL 2.6.0) loops forever on
this same input, so NCL's fast compile-time error is preferable to reproducing
the hang and is not a defect to fix. It remains retained with its stderr and
exit status in the test rather than being silently omitted. LOOP
`maximize`/`minimize`, multiple-value assignment, SHIFTF/ROTATEF, and SBIT are
covered by ordinary probes.

## Numbers

The compiled numbers builtin probe currently has no XFAIL cases. All 104 registered
builtins have a valid probe expression and assert their ANSI stdout representation,
exit code, and empty stderr. Expressions for floating-point-only builtins use a
floating-point argument; fixed-arity builtins are not probed with extra arguments.

If a future probe cannot execute, add it to the explicit XFAIL table in
`tests/e2enum.rs` with the observed stderr pattern, exit code, and the cause at the
responsible file and line or the missing Common Lisp function.
## CLOS

The compiled CLOS probe is in `tests/e2eclos.rs`. Class inspection for built-in
objects is currently working. The following cases are explicit XFAILs and assert
exit code 1 plus the observed stderr category:

| case | source | stderr | cause |
| --- | --- | --- | --- |
| `defclass` | `(defclass point () ())` | `MacroExpansion` | `packages/compiler/front/src/owned_symbols.rs:62-66` marks the form as a macro, but no CLOS macro expander is registered |
| `defgeneric` | `(defgeneric area (object))` | `MacroExpansion` | `packages/compiler/front/src/owned_symbols.rs:63-64` marks the form as a macro, but no CLOS macro expander is registered |
| `defmethod` | `(defmethod area ((object integer)) object)` | `MacroExpansion` | `packages/compiler/front/src/owned_symbols.rs:66` marks the form as a macro, but no CLOS macro expander is registered |
| `make-instance` with a symbol | `(make-instance 'standard-object)` | `TypeError` | `packages/clos/src/initialization.rs:197-210` passes the class argument directly to the instance allocator; symbol-to-class lookup is absent |
| `find-class` | `(find-class 'standard-object)` | `UNDEFINED-FUNCTION` | no `FIND-CLASS` builtin is registered by `packages/clos/src/lib_registration.rs:27-36` |
| `call-next-method` | `(call-next-method)` | `UNDEFINED-FUNCTION` | no `CALL-NEXT-METHOD` builtin is registered by `packages/clos/src/lib_registration.rs:27-36`, and no method invocation context exists |
| unknown generic function | `(clos-unknown-generic 1)` | `UNDEFINED-FUNCTION` | compiled calls resolve function cells through `packages/runtime/src/function_call.rs:133-139`; CLOS does not install a generic-function cell |

The remaining requested CLOS behaviors, including inheritance dispatch,
qualifiers, EQL specializers, initialization methods, slot-name access,
`with-slots`, and `print-object`, depend on the missing class/method definition
and generic-function runtime path and are not silently excluded from this gap.

## streams

`tests/e2estream.rs` covers the 28 builtins registered by
`packages/lib/streams/src/registration.rs`. All compiled-code probes now pass
with exact exit status, stdout, and stderr assertions; there are no remaining
stream XFAILs.
