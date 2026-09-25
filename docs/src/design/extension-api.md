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

let implementation = BuiltinImplementation::direct(
    Builtin { lambda_list: LambdaList::new("left right"), convention: BuiltinConvention::Direct(Arity::exact(2)) },
    add_builtin,
);
let function = runtime.register_builtin(
    &mut ctx,
    BuiltinIdentifier::new(BuiltinPackage::new("NCL-TEST"), BuiltinName::new("ADD")),
    implementation,
)?;
let result = runtime.call_builtin(&mut ctx, function, &[left, right])?;
```

`BuiltinImplementation::direct` is for fixed positional arity. Variadic and
keyword functions use `BuiltinImplementation::adapted` with a
`KeywordAdapter`; the adapter validates and reorders the original argument
slice before the callback is called. The `RustBuiltin` callback receives the
`&Runtime` first, followed by the thread context, arguments, and multiple-value
storage. The returned `FunctionObject` is also
stored in the runtime registry, so generated code and Rust callers resolve the
same function object. The example package is test-only and does not claim an
ownership-table symbol.

The fixed native ABI uses a direct context-plus-Word signature. Adapter calls
use the `(ctx, argc, args, values) -> NclStatus` shape. Fixed calls must not be
materialized as an argument-array adapter. Multiple values are written to
`MultipleValues`, copied to `ThreadContext` by `call_builtin`, and exposed to
the caller through `ctx.values()`.

The complete safe callback order is `(&mut ThreadContext, &Runtime,
&BuiltinArgs, &mut MultipleValues)`, matching allocating object APIs.
`BuiltinArgs` provides checked accessors and does not expose panic-prone indexing.

`Word` is a `#[repr(transparent)]` 64-bit tagged value from `ncl-sys`. Use
`classify(word)` for immediate and lowtag inspection and
`classify_object(ctx, word)` when the registered widetag is needed. A typed
`ObjectRef` variant is a view, not a root. Any heap `Word` that survives an
allocation must be held through a `RootToken` or a runtime-owned root table.
Object accessors validate the widetag and return `ObjectError::TypeError` for
the wrong object kind.

### Fixed, optional, rest, and key arguments

Fixed positional functions use `BuiltinConvention::Direct(Arity::exact(...))`:

```rust
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
    Builtin { lambda_list: LambdaList::new("first second"), convention: BuiltinConvention::Direct(Arity::exact(2)) },
    fixed,
);
```

The front-end lambda-list shapes corresponding to the supported argument
sections are:

```lisp
(lambda (required &optional (count 1 count-p)) ...)
(lambda (required &rest remaining) ...)
(lambda (required &key (width 80 width-p) height &allow-other-keys) ...)
```

The parser stores required, optional, rest, key, and auxiliary parameters in
separate typed fields. Sections must appear in the order
`&whole`, `&environment`, required, `&optional`, `&rest` or `&body`, `&key`,
`&allow-other-keys`, `&aux`. `&whole`, `&environment`, `&body`, and nested
destructuring patterns are macro-only. An adapter for a variadic or keyword
function receives the original slice, checks the rest/keyword shape, applies
defaults, and passes the callback its normalized order.

### Multiple values and type errors

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

`call_builtin` copies the side channel into `ctx.values()` before consuming a
pending condition. A callback that receives a non-fixnum, too few arguments, or
an invalid keyword reports `ObjectError::TypeError`; it must not use
`unwrap_or`, panic, or silently substitute a value. A higher layer may create a
CL `TYPE-ERROR`, record the pending object error, and return `Word::UNBOUND`.
The boundary then returns the pending error and does not expose the marker as a
successful result.

### Macro declaration shape

Compiler macros are front-end declarations, not builtin descriptors. Their
shape is:

```rust
pub struct CompilerMacro {
    pub name: SymbolRef,
    pub arity: ArityPattern,
    pub expander: MacroExpander,
    pub feature: Option<&'static str>,
}

type MacroExpander = fn(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    form: Word,
) -> Result<Option<Word>, FrontError>;
```

The callback receives the whole call form. `Some(expansion)` replaces it and
`None` declines. `ArityPattern { minimum, maximum }` is checked before the
callback. Ordinary and macro lambda lists share one typed representation, but
only macro lists may contain `&whole`, `&environment`, `&body`, or nested
destructuring. Global macro calls use `MacroCaller::call_macro(ctx, runtime,
name, form)` and surface expansion failures as `FrontError::MacroExpansion`.

### Error and DDD boundaries

`ncl-sys` owns the unsafe tagged heap and platform ABI. `ncl-object` owns the
typed Word view, widetags, roots, `Runtime`, `ThreadContext`, builtin
registration, and the `ObjectError` boundary. `ncl-types` interprets type
specifiers. `ncl-compiler-front` parses forms and macro declarations without a
reverse dependency on the runtime. `ncl-conditions` owns CL condition classes,
handlers, and restarts. Library crates own CL-facing functions and condition
construction. An extension stays in its layer and uses the typed API above;
it does not read object slots through `ncl-sys` or duplicate runtime registries.

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
