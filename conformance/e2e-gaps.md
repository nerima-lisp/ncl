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
