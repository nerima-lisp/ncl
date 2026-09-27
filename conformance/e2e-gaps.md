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
