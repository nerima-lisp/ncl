# Coding standards

## Architecture boundaries

The target crate map in `docs/src/design/crates.md` defines the bounded contexts. Crate names use CLHS vocabulary for language features and compiler-stage names for the compiler.

| Context | Crates | Contract |
| --- | --- | --- |
| Platform | `ncl-sys` | OS declarations, pages, synchronization, code space, and signal-recovery trampoline |
| Object kernel | `ncl-object` | Word, tags, layout, heap API, roots, ThreadContext, and generic Runtime lookup registries |
| Feature models | `ncl-arrays`, `ncl-hash-tables`, `ncl-packages`, `ncl-streams`, `ncl-structures`, `ncl-numbers`, `ncl-readtables` | CLHS feature state, operations, and typed errors |
| CLHS interfaces | `ncl-reader`, `ncl-printer`, `ncl-conditions`, `ncl-clos`, `ncl-lib-*` | Reader, printer, condition, object-system, and library behavior |
| Compiler stages | `ncl-compiler-front`, `ncl-hir`, `ncl-ir`, `ncl-opt`, `ncl-codegen`, assemblers, `ncl-objfile` | Source forms through Lisp-semantic HIR, semantics-preserving HIR optimization, machine code, and object-file output |
| Integration | `ncl-stdlib`, `ncl-runtime`, `ncl-image`, `ncl-threads`, `ncl-ffi`, `ncl-conformance`, root `ncl` | Registration composition, execution, type-check failure signaling, images, FFI, CLI, and conformance |

Each crate depends only on direct edges allowed by `docs/src/design/crates.md` and `docs/src/design/crate-dependencies.tsv`. External crates remain prohibited. Unsafe code and signal-recovery trampoline code stay in `ncl-sys`. Safe crates do not redeclare OS boundaries. Heap Word values are held only through the rooting API or registered root collections. The runtime entry point that signals type-check failures belongs to `ncl-runtime`; `docs/src/design/conditions.md` selects its API name.

## DDD review checklist

### Mechanically checkable

- The planned dependency checker compares current Cargo manifest edges with the target allowlist, rejects external manifest dependencies and cycles, and rejects edge endpoints absent from the target map.
- The source-layout gate must reject support.rs, utils, helpers, and tests_*.rs under packages/<crate>/src. It also rejects mod.rs and files over 500 lines.
- The ncl-object boundary check must reject feature model modules and feature API re-exports from its public root.
- For a test move, record the selected test names and count for the source and destination crates before and after the move. The selection must be non-empty when the source had tests, the count must be unchanged, and the same tests must pass.
- A changed manifest edge is accepted only after the allowlist and graph check are updated in the same contract change.

### Human review

- Names, module boundaries, errors, and documentation use CLHS vocabulary. Compiler contexts use the named compiler stage and its input/output terms.
- An entity has identity and a lifecycle: symbols, packages, classes, and threads are examples. A value object has no identity separate from its value: Word, TypeSpecifier, pathname components, and characters are examples.
- An aggregate is a consistency boundary whose root owns every invariant-changing operation. Package changes use named operations such as intern, unintern, export, shadow, and use-package. Hash-table mutation and rehashing go through named operations. Stream transitions and structure-slot changes go through explicit methods.
- Public fields do not let callers bypass aggregate invariants, rooting, or write barriers. Runtime registries are lookup indexes for packages, functions, classes, and GC metadata; feature model crates own their feature state and invariants, and do not create duplicate registries.
- Cross-context calls pass through an anti-corruption layer that translates types and failures. The `ncl-object` boundary adapts `ncl-sys` values; `ncl-compiler-front` translates source forms to HIR; CLHS registration crates adapt Lisp arguments to feature model APIs. The allowed compiler edges are `ncl-compiler-front -> ncl-hir`, `ncl-hir -> ncl-ir`, and `ncl-opt -> ncl-hir`. Whether the HIR type lattice integrates with `ncl-types` and the direction of any edge remain for `docs/src/design/hir.md` to decide.
- Every bounded context owns a typed error enum. New paths do not use String errors. Lisp-facing adapters map those errors to Lisp conditions at the boundary; condition class and mapping rules live in `docs/src/design/conditions.md`.
- Reviewers decide whether each type is an entity or value object, whether the aggregate owns every mutation, and whether the adapter prevents one context's internal vocabulary from leaking into another.

## Naming and layout

Use CLHS vocabulary in type, module, and documentation names. Prefer `Symbol`, `Package`, `Readtable`, and `GenericFunction` over abbreviations. Rust uses snake case for Lisp names, such as `symbol_value` and `print_base`. Do not introduce support.rs, utils, or helpers as technical-layer modules. Keep unsafe code and target conditionals close to the platform boundary.

## Test placement and evidence

Place behavior tests in packages/<crate>/tests/. Keep a test inside packages/<crate>/src only when it verifies a private invariant that an integration test cannot observe. Such a test stays in the implementation module as a private #[cfg(test)] module; do not add a tests_*.rs file under src.

For the D-08 test-move procedure, start the feature lane with one behavior-neutral move-only commit. Before moving tests, list each source and destination crate with `cargo test -p <crate> --all-targets -- --list`; record all full test names and the combined count for that crate set. The selection must be non-empty if the source contained tests. After moving, list the same crate set again; the names and count must match. Run every listed test by its exact name with `cargo test -p <crate> --all-targets <test-name> -- --exact`, recording each command and exit status. Put behavior changes in a later commit.

Evaluate coverage by the `notcovered` count from one coverage report and one unchanged scope, including the same changed paths and test set. For the CI changed-path gate, use the Coverage job's `coverage-summary` artifact or its log JSON. Do not substitute a percentage for this count.

The baseline inventory at 6aa6639d has 101 Rust paths under `packages/**/src/**` whose path contains `test`, and 168 `#[cfg(test)]` attributes in 114 src files. Paths under `packages/**/src/tests/` remain source tests for FR-J05. These are file and attribute counts, not test-case counts; recount at the start of each feature lane.

| Crate path | Source test paths |
| --- | ---: |
| packages/asm/x86-64 | 1 |
| packages/clos | 5 |
| packages/codegen | 14 |
| packages/compiler/opt | 9 |
| packages/conditions | 2 |
| packages/image | 3 |
| packages/ir | 4 |
| packages/lib/macros | 22 |
| packages/lib/packages | 4 |
| packages/lib/pathnames | 2 |
| packages/lib/strings | 2 |
| packages/object | 6 |
| packages/objfile | 5 |
| packages/runtime | 11 |
| packages/sys | 11 |

The inventory used `rg --files` with the source-path `src/.*test.*.rs` filter and `rg -n` with the `#[cfg(test)]` filter. Runtime declares test-only modules in `packages/runtime/src/lib.rs:7-27,480-483` and has test support in `packages/runtime/src/support.rs:13-52`. Codegen declares test modules in `packages/codegen/src/lib.rs:16-32`; its inline regalloc tests are in `packages/codegen/src/regalloc.rs:345-346`.

Each test states the contract it exercises. A passing command with zero selected tests is not evidence. Lane reports list the exact path, command, exit status, and selected test count. A baseline failure is separated from a regression.

## Change discipline

Update a design contract before implementing a change that conflicts with it. Do not loosen assertions, disable plugins, extend timeouts, or add skip flags to make a gate green. A cross-crate migration is reviewed as one contract change. Keep behavior changes out of the D-08 move-only commit.

## Rust API rules

Keep files near 300 lines and below 500. Do not use `mod.rs`; use `foo.rs` and optionally `foo/`. Default visibility is private, crate sharing is `pub(crate)`, and only listed contracts are `pub`. Use newtypes when type meaning matters, `#[repr(transparent)]` only when ABI representation is fixed, and `#[must_use]` for Result, tokens, and builders. Public enums that may gain variants are `#[non_exhaustive]`.

Each crate owns its error enum and manually implements Error, Display, and required From conversions. Return failures and propagate with `?`; do not use thiserror or `Box<dyn Error>`. Production code has no unwrap, expect, panic, or unbounded todo. A `todo!()` is allowed only in a file explicitly designated as a Phase 0 skeleton, is reported by inspection, and cannot stand in for behavior a lane claims to implement. `unsafe` is confined to ncl-sys and documents alignment, lifetime, ownership, and FFI invariants with a SAFETY reason. Explicit Send and Sync require an actual sharing proof. Clone is derived only for cheap values.

Prefer iterators and return-position `impl Trait`. Use traits for replaceable strategies such as ISA or allocator and enums for closed sets. Every public item has documentation; public functions document `# Errors`, `# Panics`, and `# Safety` when applicable. Do not add a boilerplate `# Panics` section when the function cannot panic.

This Phase 0 API sketch shows the newtype and error boundary; `todo!()` marks an unimplemented skeleton operation:

```rust
#[repr(transparent)]
pub struct Word(u64);

pub struct SymbolId(Word);

#[must_use]
pub fn alloc(thread: &mut ThreadContext, runtime: &Runtime, kind: TypeTag, words: usize)
    -> Result<Word, ObjectError> { todo!() }
```

Reader code propagates its domain error with `?`:

```rust
pub fn read_form(input: &mut Input) -> Result<Form, ReaderError> {
    let token = input.next_token()?;
    parse_token(token)
}
```

## Typed Word contract

`Word` is defined by `ncl-sys` and re-exported by `ncl-object` (`packages/object/src/lib.rs:4`). It is the only raw value crossing the object boundary. Use `Word` for storage
and ABI boundaries, not `u64`, `usize`, or an untyped string. The object layer
exposes `ObjectRef` as a non-exhaustive classification enum. `classify(word)`
uses immediate and lowtag information; `classify_object(ctx, word)` consults
the registered widetag and falls back to `classify` when no heap widetag is
available.

The additive typed view is `WordView`. `WordView::try_from_word(word,
expected)` returns `WordView` or `typed::TypeError`; it validates fixnums,
characters, conses, symbols, functions, and instances from the context-free
classification. Heap widetag validation is not performed by this method. Use
`WordView::as_word` to recover the ABI word. `ObjectRef`, `WordView`, and the
typed word newtypes are views, not roots; a heap word remains subject to the
existing rooting and movement rules.

```rust
let view = WordView::try_from_word(word, ObjectType::Fixnum)?;
let word = view.as_word();
```

Allocation APIs take `&mut ThreadContext` first and `&Runtime` second. A
`RootToken` protects a `Word` while an allocation may move it. `ThreadContext`
owns pending errors and multiple values; `Runtime` owns the builtin registry.
Do not expose a `Word`-accepting helper that can allocate without the mutable
context and an explicit root lifetime.

`ncl-object` APIs return `Result<T, ObjectError>`. `WordView::try_from_word`
returns the typed view's `typed::TypeError`. This is
distinct from `ObjectError::TypeError`, the object API's error variant, and
`ObjectErrorKind::Type`, the non-exhaustive category returned by
`ObjectError::kind()`. The same projection reports a missing builtin as
`ObjectErrorKind::Unbound`. Do not document condition classes or
pending-condition construction as part of this typed contract: those APIs are
not present in the inspected object sources.

## Typed builtin contract

The safe builtin callback is the public `RustBuiltin` function pointer type:

```rust
pub type RustBuiltin = fn(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError>;
```

`BuiltinArgs::new` wraps a `&[Word]`; `len`, `is_empty`, and `get` provide
checked access, while `required` returns `ObjectError::TypeError` when the
requested slot is absent. `BuiltinImplementation::direct` constructs a
fixed-arity implementation. `BuiltinImplementation::adapted` additionally
stores a `KeywordAdapter`, which receives the original `BuiltinArgs` and
returns a normalized `Vec<Word>` or `ObjectError`.

`RustBuiltin` is defined in `packages/object/src/builtin.rs:309`; typed adapters and `TypedRustBuiltin` are defined in `packages/object/src/typed.rs:57-114`. `BuiltinArgs` wraps the argument slice and exposes checked access in `packages/object/src/builtin.rs:255-303`.

`Runtime::register_builtin` writes the function object into the symbol's function cell and registers the Lisp function name (`packages/object/src/builtin.rs:364-410`). A symbol's function cell is its function binding slot, separate from its value cell. Registration returns a `FunctionObject`. `Runtime::call_builtin` rejects an unbound or unregistered function with `ObjectError::Unbound`, validates a direct arity,
applies the optional adapter, invokes the callback, copies
`MultipleValues` into the thread context, and then resolves the pending error.
`Runtime::builtin_descriptor` returns the registered `Builtin` descriptor when
one is present. `BuiltinPackage` is a non-exhaustive enum; the test package is
selected with `BuiltinPackage::NclTest`. `FunctionObject::try_from(word)` is
the checked conversion from a `Word` and returns `ObjectError::TypeError` for
an unbound word or a non-function lowtag.

`MultipleValues` is an explicit side channel. Its evidenced operations are
`new`, `clear`, `set`, `push`, `as_slice`, `len`, and `is_empty`; the callback's
returned `Word` remains its primary result. The `NclStatus` enum and exported
`builtin!` macro exist in `packages/object/src/builtin.rs`, but their native
entry behavior is not part of this typed Rust callback contract.

Fixed positional arguments use a direct descriptor and validate each `Word`:

```rust
const REQUIRED: &[Parameter] = &[
    Parameter { name: BuiltinName::new("left"), ty: ParameterType::Fixnum },
    Parameter { name: BuiltinName::new("right"), ty: ParameterType::Fixnum },
];

fn add(
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

let add = BuiltinImplementation::direct(
    Builtin {
        lambda_list: LambdaList::fixed(REQUIRED),
        convention: BuiltinConvention::Direct(Arity::exact(2)),
    },
    add,
);
```

`OPTIONAL` and `KEYS` are `&[Parameter]`; `REST` is one `Parameter` (`packages/object/src/builtin.rs:86-102`). The other descriptor shapes are explicit constructors:

```rust
let optional = LambdaList::with_optional(REQUIRED, OPTIONAL);
let rest = LambdaList::with_rest(REQUIRED, REST);
let keys = LambdaList::with_keys(REQUIRED, KEYS, true);
```

Typed adapters keep conversion and condition placement visible:

```rust
fn increment(_ctx: &mut ThreadContext, _runtime: &Runtime, value: Fixnum)
    -> Result<Word, LispError> {
    Ok(Word::fixnum(value.value() + 1))
}
typed_builtin!(increment_builtin, increment, (value: Fixnum));

let value = Fixnum::from_lisp_arg(&ctx, word)?; // Result<Fixnum, LispError>
let type_error = Fixnum::from_lisp_arg(&ctx, Word::NIL); // Err(LispError::TypeError { .. })
let designator = FunctionDesignator::Function(FunctionObject::try_from(function_word)?);
```

`typed_builtin!` generates a `RustBuiltin` adapter that converts arguments and stores any `LispError` on `ThreadContext` (`packages/object/src/typed.rs:57-114`). `TypedRustBuiltin` is the separate callback shape `fn(&mut ThreadContext, &Runtime) -> Result<Word, LispError>`. `MultipleValues` remains the callback side channel; `LispError` is defined in `ncl-object`, while conversion to higher-layer `ncl-conditions` objects belongs to `docs/src/design/conditions.md`.


## Lisp, GC, and OS boundary

`Word` is `#[repr(transparent)] struct Word(u64)`. Heap APIs take `&mut ThreadContext` first and `&Runtime` for shared state. Never store managed Word in an unregistered Vec, HashMap, static, or Box. Exceptions are pushed roots or Runtime-registered root collections. Every field store uses a write barrier and every blocking I/O, sleep, or foreign call pairs enter_native with leave_native.

OS calls are handwritten `extern "C"` declarations in ncl-sys: `mmap`, `munmap`, `mprotect`, `pthread_create`, `pthread_join`, `pthread_self`, pthread stack attributes, mutexes, condition variables, semaphores, `dlopen`, `dlsym`, `dlerror`, `clock_gettime`, `read`, `write`, `open`, `close`, `stat`, `opendir`, `signal`, macOS `pthread_jit_write_protect_np`, and `sys_icache_invalidate`. cfg and ABI differences remain in ncl-sys.

External crates are zero. Standard HashMap is allowed, but security or reproducibility-sensitive maps use an in-tree SipHash implementation. Randomness comes through an OS entropy wrapper. The Unicode symbol set exposed by the `NCL-UNICODE` package (FR-003) is generated from UnicodeData into static Rust tables. Bignum uses little-endian u32 limbs. A regular-expression crate is not introduced.

## Examples

```rust
pub fn unintern(package: &mut Package, name: SymbolName) -> Result<bool, PackageError> {
    package.remove_symbol(name)
}
```

An API returning a dynamic error after `expect("symbol exists")` is rejected because it hides a domain invariant. A trait such as `InstructionSelector::select(&Form) -> Result<Instruction, CodegenError>` is preferred over a string ISA with unchecked branch coverage.

```rust
// bad: panic and an untyped error hide the package invariant
pub fn unintern(package: &mut Package, name: &str) -> Box<dyn Error> {
    package.table.remove(name).expect("symbol exists");
}
```

```rust
// bad: a closed set is represented by unchecked strings
pub fn select(isa: &str) -> Result<Vec<String>, String> { todo!() }
```

## Tests, benchmarks, and lanes

Use only standard `#[test]`; table-driven helpers replace rstest. Do not use criterion. Benchmarks run through the ncl-conformance runner. Tests must not disable production contracts with unsafe, skip, or weak assertions. Feature lanes edit only their assigned crates and tests, not other crates, conformance, or configuration. Manifests use workspace lints and add no dependency.

Verification uses the repository's pinned Nix development environment. Reports name changed files, exact commands, exit status, and selected test count. Commits use explicit pathspecs; lane landing instructions govern bookmark and push operations.

## Mechanical inspection

`python3 scripts/check_standards.py` scans `packages/*` for non-path dependencies, unsafe outside `packages/sys`, Rust files over 500 lines, `mod.rs`, and production unwrap/expect/panic. It reports todo counts outside tests. Any violation exits nonzero. The script is not a Rust parser and does not replace cargo, clippy, or review. Skeleton violations are reported rather than hidden, with target file count and all output.

The added-lines gate runs `python3 scripts/check_added_lines.py --base <revision>` locally and in CI. Pull requests use the event base revision; pushes use the previous commit, with the parent commit as the initial-push fallback. It reports only violations on newly added Rust lines, while existing violations remain outside this gate.

## Review examples

An accessor that accepts `&mut Word` without a ThreadContext is incomplete when it can allocate. A field store that omits the write barrier is incorrect even if the unit test uses only old objects. A `HashMap<Word, Word>` in a static registry is incorrect unless its slots are registered roots. An OS call in a safe crate is incorrect even when the declaration is copied verbatim.

An encoder test that checks only vector length is insufficient; it must compare exact bytes or execute the emitted instruction. A FASL test that only parses a header is insufficient; it must reject out-of-bounds offsets and mismatched target metadata. A green selector that ran no tests is reported as no coverage.

## Lane handoff

Handoffs identify the contract section, changed files, verification command, exit status, and remaining gap. A lane must not silently broaden its scope to another crate or configuration file. Shared API changes are coordinated at the contract owner and checked against every consumer.

Documentation examples are normative only when they name ownership, root lifetime, error behavior, and target assumptions. Examples that omit those facts are illustrative and cannot replace a test. Generated tables include their source command and are regenerated only through the project workflow.

Before handoff, inspect the complete diff with `jj diff --git --from <base> --to @`, confirm only assigned paths changed, and confirm the strict site build contains all documents. Keep unrelated worktree entries untouched.

For documentation-only changes, the canonical docs gate is `mkdocs build --strict --config-file docs/mkdocs.yml`; cargo checks are separate and are not substituted for it.

No generated site output is committed unless the repository explicitly tracks it.

