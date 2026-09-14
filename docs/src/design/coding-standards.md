# Coding standards

## 決定

責務ごとの実装コンテキストは次の表で固定する。

| context | crates | 主な契約 |
| --- | --- | --- |
| unsafe platform | `ncl-sys` | pages、threads、OS declarations、roots、code space |
| typed object | `ncl-object`, `ncl-types` | Word、widetag、accessor、Runtime、ThreadContext wrapper |
| language front | `ncl-reader`, `ncl-printer`, `ncl-conditions`, `ncl-clos`, `ncl-compiler-front` | Form、condition、class、IR lowering |
| machine backend | `ncl-ir`, `ncl-codegen`, `ncl-asm-x86-64`, `ncl-asm-aarch64`, `ncl-objfile` | MachineFunction、encoding、Fixup、Relocation、CodeBlob |
| library | `ncl-lib-*` | ANSI/SBCL library surface |
| integration | `ncl-threads`, `ncl-ffi`, `ncl-image`, `ncl-runtime`, `ncl-conformance`, root `ncl` | execution, image, CLI, conformance |

外部 crate はゼロで、OS API は `ncl-sys` の宣言経由だけにする。safe crate は unsafe boundary を再宣言しない。公開型は所有権と root の責務を表し、heap `Word` を未登録 Rust container に保持しない。

## 根拠

境界を crate と型に対応させると、レビュー対象、依存方向、unsafe の範囲が一致する。低レイヤーを先に固定することで 24 以上の Phase 1 lane が同じ契約から作業できる。

## 却下した代替案

- backend 全体を一 crate にする案は encoder と object ownership を混ぜるため却下する。
- 単一の compiler-back 境界は target encoder と file writer の責務を曖昧にするため採用しない。
- unsafe helper を各 crate に分散する案は platform audit を困難にするため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- crate 名、path、依存方向、外部依存ゼロを変更しない。
- unsafe は `ncl-sys` に閉じ込め、public API の root と error semantics を省略しない。
- formatter、parser、test は対象 crate の最小範囲で実行し、設定ファイルを緩めて通過させない。

## Boundary review checklist

Reviewers check that entities retain identity across movement, value objects remain copyable only when cheap, and aggregate mutation goes through named methods. A public field is rejected when it can bypass package, class, root, or barrier invariants. A repository-like registry belongs to Runtime and is not duplicated by a library crate.

Reviewers check that `&mut ThreadContext` is the first heap API argument, `&Runtime` is the shared argument, RootToken protects allocation-crossing values, and `enter_native` brackets blocking work. A raw OS declaration outside ncl-sys is a boundary violation. A third-party crate in a manifest is a dependency-policy violation even if used only in tests.

## Naming and layout

Use CLHS vocabulary in type, module, and documentation names. Prefer `Symbol`, `Package`, `Readtable`, and `GenericFunction` over abbreviations. Rust uses snake case for Lisp names, such as `symbol_value` and `print_base`. Keep unsafe code and target conditionals close to the platform boundary. Do not hide target behavior behind an untyped string or undocumented global.

## Test evidence

Each test states the contract it exercises: lowtag and widetag tests include boundary values; GC tests include promotion, large object, card marking, weak clearing, and finalizer ordering; calling tests include fifth argument, tail cleanup prohibition, multiple values, and status propagation; backend tests include both target encoders, map lookup, relocation bounds, RW-to-RX publication, and FASL header rejection. A passing command with zero selected tests is not evidence.

Lane reports list the exact path, command, exit status, and selected test count. A manual table check is labeled manual. A failure already present on the untouched baseline is recorded separately from a regression. Build artifacts are not source evidence unless the command proves the runner loaded changed source.

## Change discipline

Design contradictions are resolved by updating the contract before implementation. Do not loosen assertions, disable plugins, extend timeouts, or add skip flags to make a gate green. Do not alter configuration as a lane shortcut. A change crossing crate boundaries is reviewed as one contract change even when the files are separate.

## DDD contract details

An entity is identified by lifecycle identity: symbol, package, class, and thread are entities. `Word`, `TypeSpecifier`, pathname components, and character are value objects. Use newtypes for identifiers crossing GC or Runtime boundaries. Aggregates protect invariants: Package mutation goes through `intern`, `unintern`, and `shadow`; hash rehash is driven by movement notification. `Runtime` owns package, function, class, and GC registries; ThreadContext owns TLAB, bindings, roots, handlers, safepoint, and multiple values. `intern`, `compile`, and `macroexpand` are domain services. The typed ncl-object wrapper around ncl-sys is the anti-corruption layer.

## Rust API rules

Keep files near 300 lines and below 500. Do not use `mod.rs`; use `foo.rs` and optionally `foo/`. Default visibility is private, crate sharing is `pub(crate)`, and only listed contracts are `pub`. Use newtypes when type meaning matters, `#[repr(transparent)]` only when ABI representation is fixed, and `#[must_use]` for Result, tokens, and builders. Public enums that may gain variants are `#[non_exhaustive]`.

Each crate owns its error enum and manually implements Error, Display, and required From conversions. Return failures and propagate with `?`; do not use thiserror or `Box<dyn Error>`. Production code has no unwrap, expect, panic, or unbounded todo. `todo!()` is allowed only in the Phase 0 skeleton, and its count is reported by inspection. `unsafe` is confined to ncl-sys and documents alignment, lifetime, ownership, and FFI invariants with a SAFETY reason. Explicit Send and Sync require an actual sharing proof. Clone is derived only for cheap values.

Prefer iterators and return-position `impl Trait`. Use traits for replaceable strategies such as ISA or allocator and enums for closed sets. Every public item has documentation; public functions document Errors, Panics, and Safety as applicable, including `Panics: Never` when relevant.

Required API examples preserve the newtype and error boundaries:

```rust
#[repr(transparent)]
pub struct Word(u64);

pub struct SymbolId(Word);

#[must_use]
pub fn alloc(thread: &mut ThreadContext, runtime: &Runtime, kind: TypeTag, words: usize)
    -> Result<Word, StorageCondition> { todo!() }
```

Reader code propagates its domain error with `?`:

```rust
pub fn read_form(input: &mut Input) -> Result<Form, ReaderError> {
    let token = input.next_token()?;
    parse_token(token)
}
```

## Lisp, GC, and OS boundary

`Word` is `#[repr(transparent)] struct Word(u64)`. Heap APIs take `&mut ThreadContext` first and `&Runtime` for shared state. Never store managed Word in an unregistered Vec, HashMap, static, or Box. Exceptions are pushed roots or Runtime-registered root collections. Every field store uses a write barrier and every blocking I/O, sleep, or foreign call pairs enter_native with leave_native.

OS calls are handwritten `extern "C"` declarations in ncl-sys: `mmap`, `munmap`, `mprotect`, `pthread_create`, `pthread_join`, `pthread_self`, pthread stack attributes, mutexes, condition variables, semaphores, `dlopen`, `dlsym`, `dlerror`, `clock_gettime`, `read`, `write`, `open`, `close`, `stat`, `opendir`, `signal`, macOS `pthread_jit_write_protect_np`, and `sys_icache_invalidate`. cfg and ABI differences remain in ncl-sys.

External crates are zero. Standard HashMap is allowed, but security or reproducibility-sensitive maps use an in-tree SipHash implementation. Randomness comes through an OS entropy wrapper. The 47 symbols required by `sb-unicode` are generated from UnicodeData into static Rust tables. Bignum uses little-endian u32 limbs. A regular-expression crate is not introduced.

## Examples

```rust
pub fn unintern(package: &mut Package, name: SymbolName) -> Result<bool, PackageError> {
    package.remove_symbol(name)
}
```

An API returning a dynamic error after `expect("symbol exists")` is rejected because it hides a domain invariant. A trait such as `InstructionSelector::select(&Form) -> Result<Instruction, CodegenError>` is preferred over a string ISA with unchecked branch coverage.

The two corresponding bad examples are a public `unintern` that returns
`Box<dyn Error>` after `expect("symbol exists")`, and a selector that accepts
`&str` for a closed ISA set. They hide domain failure and exhaustiveness.

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

Use only standard `#[test]`; table-driven helpers replace rstest. Do not use criterion. Benchmarks run through the ncl-conformance runner. Tests must not disable production contracts with unsafe, skip, or weak assertions. A lane edits only its crate and tests, not other crates, conformance, or configuration. Manifests use workspace lints and add no dependency.

Verification uses `nix develop path:. --command cargo ...`. Reports name changed files, exact commands, exit status, and selected test count. Commits, when explicitly requested, use a pathspec; push and PR are outside lane scope.

## Mechanical inspection

`python3 scripts/check_standards.py` scans `packages/*` for non-path dependencies, unsafe outside `packages/sys`, Rust files over 500 lines, `mod.rs`, and production unwrap/expect/panic. It reports todo counts outside tests. Any violation exits nonzero. The script is not a Rust parser and does not replace cargo, clippy, or review. Skeleton violations are reported rather than hidden, with target file count and all output.

## Review examples

An accessor that accepts `&mut Word` without a ThreadContext is incomplete when it can allocate. A field store that omits the write barrier is incorrect even if the unit test uses only old objects. A `HashMap<Word, Word>` in a static registry is incorrect unless its slots are registered roots. An OS call in a safe crate is incorrect even when the declaration is copied verbatim.

An encoder test that checks only vector length is insufficient; it must compare exact bytes or execute the emitted instruction. A FASL test that only parses a header is insufficient; it must reject out-of-bounds offsets and mismatched target metadata. A green selector that ran no tests is reported as no coverage.

The reviewer records the file, line, command, and observed output for each finding. Inferred consequences are separated from directly read facts. If a check cannot run, the report names the missing tool or environmental condition and gives the exact command to resume. No completion claim is made from static inspection when compilation or the canonical gate remains unrun.

## Lane handoff

Handoffs identify the contract section, changed files, verification command, exit status, and remaining gap. A lane must not silently broaden its scope to another crate or configuration file. Shared API changes are coordinated at the contract owner and checked against every consumer.

Documentation examples are normative only when they name ownership, root lifetime, error behavior, and target assumptions. Examples that omit those facts are illustrative and cannot replace a test. Generated tables include their source command and are regenerated only through the project workflow.

The same review applies to documentation tables: numeric fields have one owner, cross-document values are checked together, and a renamed crate is updated in the context table, adjacency list, and API examples. A shortened replacement is not accepted when it removes a layout, numeric threshold, or failure condition.

Before handoff, inspect the complete diff with `git diff --no-ext-diff`, confirm only requested design documents changed, and confirm the generated site contains all eight documents. Keep unrelated worktree entries untouched.

The canonical gate is the strict MkDocs build; cargo checks are separate and are not substituted for it.

No generated site output is committed unless the repository explicitly tracks it.

Reviewers distinguish verified commands from inferred contract implications.

The final report includes gaps instead of silently treating them as passed.

The report names every checklist item by document section.
