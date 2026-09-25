# Builtin extension API

This document freezes the N06 contract between a Rust extension and
`ncl-object`. It describes the safe Rust registration boundary and the
optional ownership check. It does not make `ncl-conditions` part of the object
layer ABI.

## Registration

The embedding or the library crate registers one builtin with
`Runtime::register_builtin`:

```rust
pub fn register_builtin(
    &self,
    ctx: &mut ThreadContext,
    package: &str,
    name: &str,
    implementation: BuiltinImplementation,
) -> Result<FunctionObject, ObjectError>
```

`BuiltinImplementation` contains a `Builtin` descriptor, the Rust callback,
and an optional keyword adapter:

```rust
pub struct Builtin {
    pub arity: u8,
    pub direct: bool,
    pub lambda_list: &'static str,
}

pub type RustBuiltin = fn(
    &mut ThreadContext,
    &[Word],
    &mut MultipleValues,
) -> Result<Word, ObjectError>;

pub type KeywordAdapter = fn(&[Word]) -> Result<Vec<Word>, ObjectError>;
```

The constructors express the two supported forms:

```rust
BuiltinImplementation::direct(descriptor, callback)
BuiltinImplementation::adapted(descriptor, callback, adapter)
```

Registration ensures the package, interns `name`, creates a function object,
writes that object to the symbol's function cell, adds the package/name lookup,
and stores the implementation behind the returned `FunctionObject`. The
returned handle is the value accepted by `Runtime::call_builtin`.

The `builtin!` macro is the native ABI metadata and entry-point helper. Its
descriptor records the fixed arity and lambda list. For fixed arities it emits
a context-plus-arguments direct entry point; it also emits the variadic entry
point shape `(ctx, argc, args, values) -> NclStatus`. The safe Rust API above is
the registration and testable callback boundary; code generation may consume
the native entry-point metadata independently.

## Direct calls and adapters

`Runtime::call_builtin` has this contract:

```rust
pub fn call_builtin(
    &self,
    ctx: &mut ThreadContext,
    function: FunctionObject,
    args: &[Word],
) -> Result<Word, ObjectError>
```

For a direct implementation (`descriptor.direct == true`), the caller must
pass exactly `descriptor.arity` words. A mismatch returns
`ObjectError::TypeError` and the callback is not called.

For an adapted implementation, the original argument slice is passed to the
registered `KeywordAdapter`. The adapter owns validation, keyword processing,
reordering, and construction of the callback argument vector. The callback
then receives the adapter's vector. An adapter error is returned unchanged and
the callback is not called. The descriptor's `lambda_list` is metadata for the
function object; it does not itself perform keyword parsing.

An implementation must use the direct constructor for fixed positional
arguments and the adapted constructor when argument translation is required.
The adapter is not an implicit second callback and does not receive a
`ThreadContext`.

## Primary and multiple values

The callback returns one primary `Word` and writes any secondary results to
`MultipleValues`:

```rust
let primary = callback(ctx, args, &mut values)?;
values.set(&[second, third]);
```

`MultipleValues::set` replaces the area, `push` appends, and an empty area is a
valid result. After the callback returns, `call_builtin` copies the area into
`ThreadContext`; callers read it with `ctx.values()`. The primary return value
is not automatically inserted into that area. The area is copied before the
pending-condition check, including when the callback returns an error.

## Pending conditions

`ThreadContext::set_pending` records one `ObjectError` for delivery at the
builtin boundary. After a callback returns `Ok(primary)`, `call_builtin`
consumes the pending error with `take_pending` and returns that error instead of
the primary value. With no pending error it returns `Ok(primary)`.

If the callback itself returns `Err(error)`, that error is returned directly by
the `Result` chain and a pending error is not consumed by that call. Extensions
must therefore choose one reporting path for a failure: return the
`ObjectError`, or return a primary value after recording a pending error for the
boundary to deliver.

## Unbound and missing implementations

`Word::UNBOUND` is the unbound sentinel. `FunctionObject::is_unbound` detects
that sentinel. `call_builtin` returns `ObjectError::Unbound` when the supplied
handle is unbound or when no registered implementation is found for the
function word. An unbound function is never passed to an extension callback.

This is separate from argument failure: a direct arity mismatch is
`ObjectError::TypeError`, and an adapter may return its own `ObjectError` for
invalid keyword payloads.

## Ownership checks are opt-in

`ncl-ownership` is a test-support crate, not an automatic part of builtin
registration. Registering a builtin does not consult `symbols.tsv` and does not
fail because another owned symbol is absent. A test or integration check opts
in explicitly:

```rust
ncl_ownership::assert_crate_coverage(&runtime, &mut ctx, "ncl-lib-numbers")?;
```

The checker reads `conformance/ownership/symbols.tsv`, selects Phase 1 rows for
the requested crate, and rejects an empty selection with `OwnershipError::NoRows`.
It then checks package and symbol interning and verifies the applicable
function, class/condition, macro, variable, and constant state. Missing items
are accumulated in `OwnershipError::Missing`; the check does not silently pass
an incomplete or empty ownership set. `assert_crate_coverage_from_table` is the
same opt-in check with an explicitly supplied table, useful for isolated tests.

The ownership checker depends on `ncl-object` only. It does not register
anything and does not expose Lisp values as an alternative runtime API.

## Conditions dependency direction

The dependency direction is from the object boundary outward:

```text
ncl-sys -> ncl-object -> ncl-conditions -> higher-level libraries
                         \\-> ncl-ownership (test support)
```

More precisely, `ncl-object` depends on `ncl-sys`; `ncl-conditions` depends on
`ncl-object`; and library crates such as `ncl-lib-numbers` depend on both
`ncl-object` and `ncl-conditions`. `ncl-ownership` depends on `ncl-object` and
does not depend on `ncl-conditions`.

Therefore a builtin callback returns `ncl_object::ObjectError` at the N06
boundary. Condition classes, handlers, and restarts belong to
`ncl-conditions` and the higher layer that uses them. `ncl-object` must not
import `ncl-conditions` to implement registration, adapters, multiple values,
pending delivery, or unbound handling; doing so would reverse the declared
dependency direction.

## Code anchors

The normative implementation points are:

- `packages/object/src/builtin.rs`: descriptors, constructors, callback types,
  `MultipleValues`, and the native ABI macro.
- `packages/object/src/lib.rs`: `Runtime::register_builtin`,
  `Runtime::call_builtin`, `ThreadContext::values`, and pending delivery.
- `packages/ownership/src/lib.rs`: explicit Phase 1 ownership checks.
- `packages/*/Cargo.toml`: crate dependency edges, including
  `ncl-conditions -> ncl-object`.
