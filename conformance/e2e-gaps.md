# E2E gaps

## hash-arrays

The compiled hash/array matrix has 53 normal cases and 3 explicit XFAIL cases.
Each XFAIL must exit with status 1 and retain stderr for the listed substring
assertion.

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

## streams

`tests/e2estream.rs` covers the 28 builtins registered by
`packages/lib/streams/src/registration.rs`. The following compiled-code probes
remain explicit XFAILs; each asserts exit status 1 and the listed stderr
substring.

| builtin | source shape | expected exit | stderr substring | cause |
| --- | --- | ---: | --- | --- |
| `WRITE-STRING` with `:start`/`:end` | `(write-string "abc" stream :start 1 :end 2)` | 1 | `AArch64 calls support at most four register arguments` | `packages/codegen/src/target_aarch64_lowering.rs:160-163` does not lower calls beyond four register arguments |
| `MAKE-STRING-INPUT-STREAM` with bounds | `(make-string-input-stream "abc" :start 1 :end 2)` | 1 | `AArch64 calls support at most four register arguments` | `packages/codegen/src/target_aarch64_lowering.rs:160-163` does not lower calls beyond four register arguments |
| `FRESH-LINE` on string output | `(fresh-line (make-string-output-stream))` | 1 | `ncl: object error: TypeError` | `packages/lib/streams/src/character/input.rs:31-38` rejects reading a string-output stream; `fresh_line_adapter` calls `peek_character` at `character/adapters.rs:91-94` |
| `WITH-OUTPUT-TO-STRING` | `(with-output-to-string ...)` | 1 | `MacroExpansion` | The macro is listed but its compiled expansion currently returns `TypeError`; `packages/lib/macros/src/lib.rs:471` |
| `WITH-INPUT-FROM-STRING` | `(with-input-from-string ...)` | 1 | `MacroExpansion` | The macro is listed but its compiled expansion currently returns `TypeError`; `packages/lib/macros/src/lib.rs:471` |
| `PRINC` | `(princ "x")` | 1 | `undefined function UNDEFINED-FUNCTION: PRINC` | `packages/printer/src/builtins.rs:58` registers an unbound placeholder and no callable replacement is installed |
| `PRIN1` | `(prin1 "x")` | 1 | `undefined function UNDEFINED-FUNCTION: PRIN1` | `packages/printer/src/builtins.rs:58` registers an unbound placeholder and no callable replacement is installed |
| `PRINT` | `(print "x")` | 1 | `undefined function UNDEFINED-FUNCTION: PRINT` | `packages/printer/src/builtins.rs:58` registers an unbound placeholder and no callable replacement is installed |
| `FORMAT` | `(format nil "~a/~s/~d~%~&" "x" "y" 12)` | 1 | `undefined function UNDEFINED-FUNCTION: FORMAT` | `packages/stdlib/src/lib.rs:65-69` omits the `ncl-lib-format` registration call despite listing it in `REGISTRATION_ORDER` |
