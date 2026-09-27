# E2E gaps

## hash-arrays

The compiled hash/array matrix has 53 normal cases and 3 explicit XFAIL cases.
Each XFAIL must exit with status 1 and retain stderr for the listed substring
assertion.

## macros

`tests/e2emacro.rs` asserts compiled results for the working control macros and
keeps the currently failing loop, binding, iteration, multiple-value, and
structure paths as explicit XFAILs. The observed failures are native IR
verification errors (`pass inline-direct-calls failed`) for loop and generated
iteration control flow, front-end macro-expansion errors for unsupported
destructuring/multiple-value/defstruct paths, and `UnsupportedLiteral` for the
loop maximize probe. These are retained with their stderr and exit status in
the test rather than being silently omitted.

| builtin | source | expected exit | stderr substring | cause |
| --- | --- | ---: | --- | --- |
| `VECTOR-PUSH-EXTEND` | `(vector-push-extend 7 (make-array 0 :fill-pointer 0 :adjustable t))` | 1 | `AArch64 calls support at most four register arguments` | `packages/codegen/src/target_aarch64_lowering.rs:160-163`: AArch64 call lowering is unimplemented beyond four register arguments |
| `VECTOR-POP` | `(vector-pop (make-array 2 :fill-pointer 1 :initial-element 7))` | 1 | `AArch64 calls support at most four register arguments` | `packages/codegen/src/target_aarch64_lowering.rs:160-163`: AArch64 call lowering is unimplemented beyond four register arguments |
| `SBIT` | `(setf (sbit (make-array 2 :element-type 'bit) 0) 1)` | 1 | `UndefinedFunction` | `packages/lib/macros/src/setf_places.rs:223-240`: SETF place expansion for the SBIT CL function is unimplemented |

## Numbers

The compiled numbers builtin probe currently has no XFAIL cases. All 104 registered
builtins have a valid probe expression and assert their ANSI stdout representation,
exit code, and empty stderr. Expressions for floating-point-only builtins use a
floating-point argument; fixed-arity builtins are not probed with extra arguments.

If a future probe cannot execute, add it to the explicit XFAIL table in
`tests/e2enum.rs` with the observed stderr pattern, exit code, and the cause at the
responsible file and line or the missing Common Lisp function.
