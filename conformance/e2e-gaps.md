# E2E gaps

## hash-arrays

The compiled hash/array matrix has 55 normal cases and 1 explicit XFAIL case.
Each XFAIL must exit with status 1 and retain stderr for the listed substring
assertion. `VECTOR-PUSH-EXTEND` and `VECTOR-POP` were previously XFAILed for
an AArch64 four-register call-argument limit; both now compile and pass as
ordinary cases in `tests/e2ehash.rs`.

## macros

`tests/e2emacro.rs` asserts compiled results for the working control macros and
keeps the currently failing binding, iteration, multiple-value, and structure
paths as explicit XFAILs. `IF`/`WHEN`/`UNLESS` (and the LOOP clauses that lower
through them, e.g. `count`) used to leave a mutated variable's post-branch
value unmerged, producing a native IR verification error
(`pass inline-direct-calls failed`); `lower_if`
(`packages/compiler/front/src/lower/expr.rs`) now merges live variables across
branches through a block parameter, so `loop-count` and ordinary `if`/`when`
mutation moved from XFAIL to `PROBES`. `LOOP` also gained `WHEN`/`UNLESS`/`IF`
conditional clauses (with `AND`/`ELSE`/`END`/`IT`), destructuring `FOR`
variables, and a CLHS-conformant default initial value of `0` for arithmetic
`FOR` clauses without `FROM`/`UPFROM`/`DOWNFROM` (`packages/lib/macros/src/loop`).
The remaining XFAILs are front-end macro-expansion errors for unsupported
destructuring/multiple-value/defstruct paths, `UnsupportedLiteral` for the loop
`maximize`/`minimize` accumulators (the `Word::TRUE` first-value sentinel in
`packages/lib/macros/src/loop/accumulator.rs` is not a literal the front end
lowers), and `loop-with-finally`, whose body has no iteration-control clause at
all: real Common Lisp (confirmed against SBCL 2.6.0) loops forever on this
same input, so NCL's fast compile-time error is preferable to reproducing the
hang and is not a defect to fix. These are retained with their stderr and exit
status in the test rather than being silently omitted.

| builtin | source | expected exit | stderr substring | cause |
| --- | --- | ---: | --- | --- |
| `SBIT` | `(setf (sbit (make-array 2 :element-type 'bit) 0) 1)` | 1 | `UndefinedFunction` | `packages/lib/macros/src/setf_places.rs:223-240`: SETF place expansion for the SBIT CL function is unimplemented |

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
`call-next-method`, `next-method-p`, symbol class designators, slot initargs,
and the tested EQL-specializer path are working. These are covered by value
assertions in the e2e test and are not XFAILs.

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
