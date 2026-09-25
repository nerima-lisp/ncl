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
fn add_builtin(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let left = args.required(0)?.as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let right = args.required(1)?.as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    Ok(Word::fixnum(left + right))
}

const REQUIRED: &[Parameter] = &[
    Parameter { name: BuiltinName::new("left"), ty: ParameterType::Fixnum },
    Parameter { name: BuiltinName::new("right"), ty: ParameterType::Fixnum },
];

let implementation = BuiltinImplementation::direct(
    Builtin {
        lambda_list: LambdaList::fixed(REQUIRED),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    },
    add_builtin,
);
let function = runtime.register_builtin(
    &mut ctx,
    BuiltinIdentifier::new(BuiltinPackage::NclTest, BuiltinName::new("ADD")),
    implementation,
)?;
let result = runtime.call_builtin(&mut ctx, function, &[left, right])?;
```

`BuiltinImplementation::direct` is for fixed positional arity. Variadic and
keyword functions use `BuiltinImplementation::adapted` with a
`KeywordAdapter`; the adapter validates and reorders the original argument
slice before the callback is called. The `RustBuiltin` callback receives the
mutable thread context first, followed by the runtime, arguments, and
multiple-value storage. The returned `FunctionObject` is also
stored in the runtime registry. The example package is test-only and does not
claim an ownership-table symbol.

`Runtime::call_builtin` validates direct arity, applies an optional
`KeywordAdapter`, invokes the callback, copies `MultipleValues` into the
thread context, and then consumes pending state. The `NclStatus` enum and the
exported `builtin!` macro are present in `ncl-object`, but no native entry
contract is specified here because this document is limited to the evidenced
typed Rust callback API.

`BuiltinPackage` is a non-exhaustive enum; use `BuiltinPackage::NclTest` for
the test package shown above. `FunctionObject::try_from(word)` is the checked
conversion from a `Word` and returns `ObjectError::TypeError` for an unbound
word or a non-function lowtag.

The complete safe callback order is `(&mut ThreadContext, &Runtime,
&BuiltinArgs, &mut MultipleValues)`, matching allocating object APIs.
`BuiltinArgs` provides checked accessors and does not expose panic-prone indexing.

`Word` is the tagged value re-exported by `ncl-object` from `ncl-sys`. Use
`classify(word)` for immediate and lowtag inspection and
`classify_object(ctx, word)` when the registered widetag is needed. A typed
`ObjectRef` variant and `WordView` are views, not roots. `WordView::try_from_word`
validates only the context-free categories implemented in `typed.rs`, and
returns `typed::TypeError` for a mismatch. Any heap `Word` that survives an
allocation must be held through the existing rooting API.

### Fixed arguments

Fixed positional functions use `BuiltinConvention::Direct(Arity::exact(...))`:

```rust
const FIXED: &[Parameter] = &[
    Parameter { name: BuiltinName::new("first"), ty: ParameterType::Fixnum },
    Parameter { name: BuiltinName::new("second"), ty: ParameterType::Fixnum },
];

fn fixed(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let first = args.required(0)?.as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let second = args.required(1)?.as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    Ok(Word::fixnum(first + second))
}

let implementation = BuiltinImplementation::direct(
    Builtin {
        lambda_list: LambdaList::fixed(FIXED),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    },
    fixed,
);
```

`LambdaList::fixed`, `with_optional`, `with_rest`, and `with_keys` are the
typed descriptor constructors. They keep the parameter shape explicit:

```rust
let fixed = LambdaList::fixed(REQUIRED);
let optional = LambdaList::with_optional(REQUIRED, OPTIONAL);
let rest = LambdaList::with_rest(REQUIRED, REST);
let keys = LambdaList::with_keys(REQUIRED, KEYS, true);
```

`min_arity`, `max_arity`, and `is_direct` expose descriptor properties. The
object layer does not parse Lisp lambda-list syntax; a front-end parser remains
outside this contract.

### Typed adapters, multiple values, and type errors

`TypedRustBuiltin` is the callback shape for a domain function that performs
its own conversion and returns an `ncl-object::LispError`. `typed_builtin!`
generates the raw `RustBuiltin` adapter and uses `FromLispArg` for fixed
arguments:

```rust
let typed: TypedRustBuiltin = |_ctx, _runtime| Ok(Word::NIL);

fn increment(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    value: Fixnum,
) -> Result<Word, LispError> {
    Ok(Word::fixnum(value.value() + 1))
}

typed_builtin!(increment_builtin, increment, (value: Fixnum));
```

The conversion boundary is explicit and returns `LispError` in `ncl-object`:

```rust
let value = Fixnum::from_lisp_arg(&ctx, word)?;
```

For example, `Fixnum::from_lisp_arg(&ctx, Word::NIL)` returns
`Err(LispError::TypeError { .. })`. A function-valued designator can be
constructed through the checked `FunctionObject` conversion:

```rust
let designator = FunctionDesignator::Function(FunctionObject::try_from(function_word)?);
```

`ncl-object` does not depend on `ncl-conditions`. `ncl-conditions::register`
installs a `Runtime::register_lisp_error_converter` callback. The builtin
boundary preserves a `LispError`, invokes that callback, and stores the
allocated condition object in `ThreadContext` pending state.

`MultipleValues` is the explicit result side channel. The callback's returned
`Word` remains the primary value:

```rust
fn quotient_and_remainder(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let dividend = args.required(0)?.as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    let divisor = args.required(1)?.as_fixnum()
        .ok_or(ObjectError::TypeError)?;
    if divisor == 0 {
        return Err(ObjectError::TypeError);
    }
    values.set(&[
        Word::fixnum(dividend / divisor),
        Word::fixnum(dividend % divisor),
    ]);
    Ok(Word::fixnum(dividend / divisor))
}
```

`call_builtin` copies the side channel into `ctx.values()` before consuming
pending state. `ncl-conditions::condition_from_lisp_error` owns the conversion
to a typed condition record. `FunctionDesignator` currently provides the
evidenced tagged view, but no direct `FromLispArg` conversion for the enum is
present:

```rust
let designator = FunctionDesignator::Symbol(Symbol::from_word(symbol_word));
```

Compiler-macro declarations and lambda-list parsing are outside the inspected
object sources and are intentionally not specified here.

### Error and DDD boundaries

`ncl-object` owns the typed Word views, classification, builtin descriptors,
registration, invocation, and `ObjectError` boundary described here. Other
layer ownership and compiler-macro APIs are unresolved dependencies for this
N08b contract and are not asserted by this document.

## Condition signaling from a builtin

`ncl-object` exposes `ObjectError`, typed builtin errors, and
`ThreadContext::take_pending_condition`. `Runtime::call_builtin` copies
multiple values, converts a preserved `LispError` through the registered
condition converter, and consumes ordinary pending state. The condition record
is allocated by `ncl-conditions`; object does not import that crate.

The stable mapping is `TypeError` to `TYPE-ERROR` (`datum`, `expected-type`),
wrong-arity `ProgramError` to `PROGRAM-ERROR` (minimum and maximum), division
by zero to `DIVISION-BY-ZERO`, end-of-file to `END-OF-FILE`, storage failures to
`STORAGE-CONDITION`, and the remaining variants to their corresponding
standard condition class with no slots when their Rust variant has no payload.

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
