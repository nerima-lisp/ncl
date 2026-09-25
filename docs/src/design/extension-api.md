# Extension API

NCL extensions are workspace crates with explicit ownership and dependency
boundaries. They use only path dependencies and must not introduce external
crates or reverse dependencies.

## Frozen package names

The public extension packages are `NCL-THREADS`, `NCL-FFI`, `NCL-MOP`,
`NCL-GRAY`, `NCL-GC`, `NCL-IMAGE`, `NCL-UNICODE`, `NCL-OS`, `NCL-EXT`, and
`NCL-SYS`. Their owning crates are, in order, `ncl-threads`, `ncl-ffi`,
`ncl-clos`, `ncl-lib-streams`, `ncl-object`, `ncl-image`, `ncl-lib-strings`,
`ncl-os`, `ncl-compiler-front`, and `ncl-sys`.

`NCL-` packages are the only extension namespace. The ASDF and UIOP package
names remain `ASDF/INTERFACE` and `UIOP/DRIVER`.

## Ownership tables

Each owning crate stores a seven-column TSV at `packages/<crate>/ownership.tsv`:
`package`, `symbol`, `kind`, `crate`, `phase`, `direct-expansion`, and `notes`.
The `crate` column is retained so a row is self-describing. COMMON-LISP has
978 rows; ASDF/INTERFACE has 230 rows; UIOP/DRIVER has 427 rows. SB-prefixed
packages and SBCL contrib rows are not part of the contract.

`conformance/ownership/check.py` reads all split tables and checks retained
surface coverage, duplicate `(package, symbol)` keys, valid fields, and the
absence of SB-prefixed packages.

## Registration order

`ncl-stdlib::register_all` is the sole standard-library registration entry
point. It calls registrations in dependency order:

1. `ncl-types`
2. `ncl-reader`
3. `ncl-printer`
4. `ncl-conditions`
5. `ncl-clos`
6. `ncl-lib-numbers`
7. `ncl-lib-sequences`
8. `ncl-lib-strings`
9. `ncl-lib-hash-arrays`
10. `ncl-lib-streams`
11. `ncl-lib-pathnames`
12. `ncl-lib-packages`
13. `ncl-lib-format`
14. `ncl-lib-macros`
15. `ncl-threads`
16. `ncl-ffi`
17. `ncl-image`
18. `ncl-os`
19. `ncl-uiop`
20. `ncl-asdf`
21. `ncl-profiler`
22. `ncl-coverage`
23. `ncl-debug`
24. `ncl-disasm`

Crates without a landed registration implementation contribute no call until
their lane lands. No extension crate calls another crate's registration
function directly.

## Dependency graph

```text
ncl-os -> ncl-object, ncl-sys, ncl-conditions, ncl-lib-streams
ncl-uiop -> ncl-object, ncl-conditions, ncl-lib-streams, ncl-lib-pathnames,
           ncl-lib-strings, ncl-os
ncl-asdf -> ncl-object, ncl-conditions, ncl-clos, ncl-uiop
ncl-profiler -> ncl-object, ncl-sys, ncl-threads, ncl-conditions
ncl-coverage -> ncl-object, ncl-compiler-front
ncl-debug -> ncl-object, ncl-sys, ncl-conditions, ncl-codegen, ncl-lib-streams
ncl-disasm -> ncl-object
```

`ncl-disasm` uses `ncl-asm-x86-64` and `ncl-asm-aarch64` only as development
dependencies for round-trip tests.

## Builtin binding contract

The builtin boundary is owned by `ncl-object`. A library crate describes one
function and binds it to the CL function cell with `Runtime::register_builtin`:

```rust
let implementation = BuiltinImplementation::direct(
    Builtin { arity: 2, direct: true, lambda_list: "left right" },
    add_builtin,
);
let function = runtime.register_builtin(&mut ctx, "NCL-TEST", "ADD", implementation)?;
let result = runtime.call_builtin(&mut ctx, function, &[left, right])?;
```

`BuiltinImplementation::direct` is for fixed positional arity. Variadic and
keyword functions use `BuiltinImplementation::adapted` with a
`KeywordAdapter`; the adapter validates and reorders the original argument
slice before the callback is called. The returned `FunctionObject` is also
stored in the runtime registry, so generated code and Rust callers resolve the
same function object. The example package is test-only and does not claim an
ownership-table symbol.

The fixed native ABI uses a direct context-plus-Word signature. Adapter calls
use the `(ctx, argc, args, values) -> NclStatus` shape. Fixed calls must not be
materialized as an argument-array adapter. Multiple values are written to
`MultipleValues`, copied to `ThreadContext` by `call_builtin`, and exposed to
the caller through `ctx.values()`.

## Condition signaling from a builtin

`ncl-object` does not depend on `ncl-conditions`. A builtin reports a boundary
condition by recording an `ObjectError` in `ThreadContext`'s pending state and
returning the reserved `Word::UNBOUND` marker as its primary result. On return,
`Runtime::call_builtin` copies multiple values, consumes the pending state, and
returns the pending error instead of the marker. A callback may alternatively
return `Err(ObjectError)` directly; it must choose one path for each failure.
The library crate owns construction of the CL condition object and calls the
appropriate `ncl-conditions` function before recording the pending error.
This keeps condition classes, handlers, and restarts above the object layer.

`Word::UNBOUND` is also used for an unbound function cell. `call_builtin`
rejects an unbound handle or missing implementation with `ObjectError::Unbound`
and never invokes the callback.

## Strict ownership binding check

The existing `assert_crate_coverage*` checks remain the general registration
gate and do not require a function cell to contain a callable object. A lane
that needs the stronger contract opts in with
`assert_crate_function_bindings` or
`assert_crate_function_bindings_from_table`. Those checks verify both the
runtime registry and the symbol's function cell for every owned function row.
