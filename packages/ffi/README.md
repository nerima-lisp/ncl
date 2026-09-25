# ncl-ffi

`ncl-ffi` owns the `NCL-FFI` surface for NCL: foreign type descriptors,
foreign-call argument marshalling, machine pointers, dynamic loading, and the
105 symbols in `ownership.tsv`.
It depends only on `ncl-object`, `ncl-sys`, and `ncl-conditions`, and it stays
within Rust's safe subset: every raw pointer operation remains behind `ncl-sys`.

## Public API

```text
AlienType, AlienRecord, AlienEnum, AlienRoutine
parse_type_name(&str) -> Result<AlienType, FfiError>
parse_type_specifier(&str) -> Result<AlienType, FfiError>
size_of/align_of(&AlienType) -> usize
offset_of(&AlienRecord, usize) -> Option<usize>
record_size/union_size(&AlienRecord) -> usize
marshal_argument(&ThreadContext, &AlienType, Word) -> Result<Vec<u8>, FfiError>
unmarshal_result(&mut ThreadContext, &Runtime, &AlienType, &[u8]) -> Result<Word, FfiError>
alien_routine, alien_funcall, alien_size, alien_sap, cast, null_alien
SystemAreaPointer, sap_plus, sap_minus, sap_difference, sap_int, from_word
sap_eq, sap_lt, sap_le, sap_gt, sap_ge
load_shared_object, unload_shared_object, dlopen_or_lose, dlerror_message,
extern_alien_name, find_dynamic_foreign_symbol_address,
find_foreign_symbol_address, foreign_symbol_address, foreign_symbol_sap,
foreign_symbol_dataref_sap
allocate_system_memory, deallocate_system_memory, memmove, sap_ref, sap_set,
with_rooted_objects
signal_ffi_error
register(&Runtime) -> Result<(), ObjectError>
FfiError, sys_requirements
```

## Implemented

- Alien type model: integer widths, `float` / `double` / `long double`, pointer,
  `c-string`, `utf8-string`, `system-area-pointer`, array, struct, union, enum,
  and function types, with LP64 size, alignment, and field offsets.
- Type parsing: atomic names (separator- and case-insensitive) and the compound
  forms `(pointer T)`, `(array T N)`, `(struct ...)`, `(union ...)`,
  `(enum ...)`, and `(function RESULT (ARG ...))`.
- Argument marshalling and result unmarshalling for every scalar type, in
  little-endian byte order, with range and shape checks. Composite types report
  `FfiError::MissingSysPrimitive`.
- SAP arithmetic and comparison over a fixnum address; `sap-int` and `int-sap`
  are exact inverses.
- `with_rooted_objects` roots managed values with precise roots across a
  collection, using the public `push_root` / `pop_root`.
- `register` interns the 105 owned symbols and sets each function, class, macro,
  and special bit the ownership gate requires.

## ncl-sys requirements

`ncl-sys` currently exposes only raw `extern "C"` declarations for dynamic
loading and for reading or writing an arbitrary address. `ncl-ffi` cannot call
them and must not re-declare them, so the following primitives are requested.
Each is named by a constant in `sys_requirements`, and the call that needs it
returns `FfiError::MissingSysPrimitive` carrying that constant.

| constant | required signature | needed by |
| --- | --- | --- |
| `DLOPEN_SHARED_OBJECT` | `dlopen_shared_object(path: &str, lazy: bool) -> Result<SharedObject, DlError>` | `load-shared-object`, `dlopen-or-lose` |
| `DLCLOSE_SHARED_OBJECT` | `dlclose_shared_object(handle: SharedObject) -> Result<(), DlError>` | `unload-shared-object` |
| `DLSYM_FOREIGN_SYMBOL` | `dlsym_foreign_symbol(handle: SharedObject, name: &str) -> Result<usize, DlError>` | `find-foreign-symbol-address`, `foreign-symbol-sap` |
| `DLERROR_MESSAGE` | `dlerror_message() -> Option<String>` | `dlerror` |
| `CALL_FOREIGN_FUNCTION` | `call_foreign_function(address: usize, arguments: &[u8], result: &mut [u8]) -> Result<(), CallError>` | `alien-funcall` |
| `READ_SYSTEM_MEMORY` | `read_system_memory(address: usize, size: usize, out: &mut [u8]) -> Result<(), MemoryError>` | `sap-ref-*`, `deref`, `slot` |
| `WRITE_SYSTEM_MEMORY` | `write_system_memory(address: usize, bytes: &[u8]) -> Result<(), MemoryError>` | `(setf sap-ref-*)` |
| `ALLOCATE_SYSTEM_MEMORY` | `allocate_system_memory(size: usize) -> Result<usize, MemoryError>` | `make-alien`, `allocate-system-memory` |
| `DEALLOCATE_SYSTEM_MEMORY` | `deallocate_system_memory(address: usize, size: usize) -> Result<(), MemoryError>` | `free-alien`, `deallocate-system-memory` |
| `MEMMOVE_SYSTEM_MEMORY` | `memmove_system_memory(destination: usize, source: usize, count: usize) -> Result<(), MemoryError>` | `memmove` |
| `PIN_OBJECT` | `pin_object(thread: &mut Thread, object: Word) -> Result<PinToken, StorageCondition>` | `with-pinned-objects` |
| `UNPIN_OBJECT` | `unpin_object(thread: &mut Thread, token: PinToken) -> bool` | `with-pinned-objects` |
| `OBJECT_ADDRESS` | `object_address(thread: &Thread, object: Word) -> Option<usize>` | `alien-sap` of a heap object |

### Why `invoke_entry` is not the call primitive

`ncl_sys::invoke_entry` enters published NCL generated code with the NCL
register convention: context in `r15` / `x21`, `argc` in `rdi`, four argument
words, `rest`, and a `(value, count)` result. A C function expects the C ABI and
does not read a context register, so `invoke_entry` cannot call one.
`call_foreign_function` is the missing adapter: it takes a marshalled argument
buffer and calls the function pointer with the C ABI.

## The `strlen` acceptance test

The acceptance target is a call to libc `strlen` declared by this crate:

```rust
let strlen = alien_routine("strlen", vec![AlienType::CString], AlienType::SizeT, false);
alien_funcall(&ctx, &runtime, &strlen, &[address])
```

Marshalling succeeds; the call returns
`FfiError::MissingSysPrimitive(CALL_FOREIGN_FUNCTION)` because `ncl-sys` has no
C function-pointer call primitive and no safe `dlsym` wrapper.
`tests/ffi.rs::strlen_call_is_blocked_on_the_ncl_sys_call_primitive` pins this
gap. When `CALL_FOREIGN_FUNCTION` and `DLSYM_FOREIGN_SYMBOL` land, that test
becomes a real call and should assert the returned length.

## Owned symbols

`register` installs the 105 Phase-1 symbols from
`ownership.tsv`: 105 symbols in `NCL-FFI`. Functions are registered with an
unbound placeholder, classes with a
minimal descriptor vector, and macros and variables with their macro and special
bits. `macro` rows reserve the name and set the bit; the expanders live in
`ncl-lib-macros` (L19).

## Known gaps

- Dynamic loading, foreign calls, unmanaged memory, and `sap-ref-*` are blocked
  on the `ncl-sys` primitives above.
- `long-float` has no Phase-1 marshalling and reports
  `FfiError::UnsupportedType`.
- A SAP is a fixnum address. A distinct SAP object kind is an `ncl-object`
  requirement; until then `sap-int` and `int-sap` are the conversion.
- `with_rooted_objects` gives precise roots, which keep objects live and rewrite
  their slots, but not a non-moving pin; `PIN_OBJECT` is the separate
  requirement.
- `register` installs unbound function placeholders. Callable function objects
  need the codegen and runtime layers (L13/L14/L21).

## Verification

```sh
nix develop --command cargo test --locked -p ncl-ffi
nix develop --command cargo clippy --locked -p ncl-ffi --all-targets --all-features -- -D warnings
python3 scripts/check_standards.py
python3 scripts/reachability.py
```
