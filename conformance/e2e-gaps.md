# E2E gaps

## hash-arrays

The compiled hash/array matrix has 55 normal cases and 1 explicit XFAIL case.
Each XFAIL must exit with status 1 and retain stderr for the listed substring
assertion.

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
