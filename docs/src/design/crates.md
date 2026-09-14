# Crates

## 決定

新しい workspace は `packages/<name>/` に配置し、crate name は `ncl-<name>` とする。依存は下層から上層への一方向だけで、中央の builtin registration table は置かない。各 library crate は `pub fn register(rt: &mut Runtime)` を公開し、集約は `ncl-runtime/src/registration.rs` の 1 ファイルだけが行う。

| crate | responsibility and public contract |
| --- | --- |
| `ncl-sys` | unsafe boundary, `Word`, heap, threads, safepoints, roots, FFI primitives |
| `ncl-object` | headers, cons, symbol, strings, vectors, arrays, hash tables, numbers, functions and accessors |
| `ncl-types` | type specifiers, predicates, subtype and type errors |
| `ncl-reader` | character input and Lisp data reader |
| `ncl-printer` | object printing and stream-oriented output |
| `ncl-conditions` | conditions, handlers, restarts, unwind records |
| `ncl-clos` | classes, slots, generic functions, methods and MOP indirection |
| `ncl-compiler-front` | macroexpansion, declarations, type propagation, compiler macros and IR |
| `ncl-compiler-back` | Cranelift lowering, JIT/object modules, stack maps and FASL |
| `ncl-lib-numbers` | numeric tower and numeric builtins |
| `ncl-lib-sequences` | sequence traversal and sequence builtins |
| `ncl-lib-strings` | string and character builtins |
| `ncl-lib-hash-arrays` | arrays and hash-table builtins |
| `ncl-lib-streams` | stream objects and I/O builtins |
| `ncl-lib-pathnames` | pathname and file-name builtins |
| `ncl-lib-format` | FORMAT parser and renderer |
| `ncl-lib-loop` | LOOP parser and expander |
| `ncl-lib-packages` | package and symbol namespace builtins |
| `ncl-lib-macros` | standard macro definitions and compiler-macro registrations |
| `ncl-threads` | OS-thread Lisp API, locks, waits, deadlines and interrupts |
| `ncl-ffi` | dynamic library loading and alien calls |
| `ncl-image` | FASL image serialization, load and relocation |
| `ncl-runtime` | Runtime, eval/compile/load, registration aggregation and public entry points |
| `ncl-conformance` | ansi-test and cl-bench runners; data is supplied under `conformance/` |
| root `ncl` | CLI, `--version`, `--eval`, REPL integration |

`ncl-sys` is the sole crate with `unsafe_code = "allow"`, `unsafe_op_in_unsafe_fn = "deny"`, and `clippy::undocumented_unsafe_blocks = "deny"`. All other crates inherit workspace `unsafe_code = "forbid"`. External dependencies are fixed in Phase 0: Cranelift family (same version) in compiler-back, `libc` in sys, and `libloading` in ffi. No other dependency may be added by a Phase 1 lane.

The Phase 1 signature surface is:

```rust
// ncl-sys
pub type Word = u64;
pub struct RuntimeHandle;
pub struct ThreadHandle;
pub fn alloc(rt: &mut RuntimeHandle, ty: u16, words: usize) -> Result<Word, StorageCondition>;
pub fn register_thread(rt: &mut RuntimeHandle) -> ThreadHandle;
pub fn poll_safepoint(thread: &ThreadHandle);
pub fn push_root(thread: &ThreadHandle, slot: &mut Word) -> RootToken;
pub fn pop_root(thread: &ThreadHandle, token: RootToken);

// object, types, reader, printer, conditions, clos
pub struct Object;
pub struct Runtime;
pub fn read(input: &mut impl Read) -> Result<Word, ReadError>;
pub fn print_object(value: Word, output: &mut impl Write) -> Result<(), PrintError>;
pub fn signal(rt: &mut Runtime, condition: Condition) -> NclStatus;
pub fn register(rt: &mut Runtime);

// compiler-front/back
pub struct Function;
pub fn lower(function: &Function) -> Result<CompiledFunction, CompileError>;
pub fn compile_jit(function: &Function) -> Result<CodeHandle, CompileError>;
pub fn compile_object(function: &Function) -> Result<ObjectFile, CompileError>;

// each ncl-lib-* crate
pub fn register(rt: &mut Runtime);

// threads, ffi, image, runtime, conformance
pub fn spawn(rt: &mut Runtime, entry: Word) -> Result<ThreadId, ThreadError>;
pub fn open_library(path: &str) -> Result<Library, FfiError>;
pub fn load_image(rt: &mut Runtime, bytes: &[u8]) -> Result<(), ImageError>;
pub fn eval(rt: &mut Runtime, source: &str) -> Result<MultipleValues, EvalError>;
pub fn run_ansi_tests(rt: &mut Runtime, suite: &Path) -> Result<TestReport, ConformanceError>;
```

The names `Read`, `Write`, `StorageCondition`, and other result types are public types in their owning crate. The snippets define the cross-crate signatures, not implementation bodies; each crate's `lib.rs` carries the doc comments and concrete module-specific types.

Builtin definitions use one macro convention: `builtin!(name, min..=max, args => body)`. The generated function checks argc, checks each type before conversion, returns `NclStatus::Condition` on failure, and never panics. The registration function inserts the Lisp package/name, function object, lambda-list metadata, and direct-expansion flag into `Runtime`.

Every crate exposes the types and functions listed by this table as documented `pub` signatures, even when a Phase 0 body is `todo!()`. `ncl-sys` is the exception: its Word type, bump allocator without TLAB, thread registration, dummy safepoint, and root API are executable and unit-tested.

## 根拠

The layering lets 24 parallel lanes compile against stable signatures while only one runtime file changes for aggregation. Registration ownership in each library avoids a shared global edit point. Keeping unsafe in ncl-sys makes audits and platform-specific MAP_JIT/W^X, signals, and raw calls explicit.

The dependency policy is applied to the explicitly required Cranelift, libc, and libloading layers: a dependency is accepted only when a dependency-free implementation would exceed 200 lines, an equivalent is available through nixpkgs, it does not claim dependency-free operation, and it belongs at layer L2 or above. `libc` and `libloading` remain the platform boundary exceptions required by the contract.

## 却下した代替案

- A central registration table was rejected because parallel library lanes would edit one file.
- A `cl-cc` base was rejected; the NCL Rust core is authoritative.
- LLVM, a custom backend, and bytecode were rejected in favor of Cranelift.
- Lisp-written standard libraries were rejected; all standard-library crates are Rust.
- Adding dependencies opportunistically in Phase 1 was rejected to keep the workspace contract reproducible.

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- Package paths, crate names, dependency direction, registration signature, and builtin macro behavior are stable.
- A lane may implement only its listed crate and its own tests, without editing runtime aggregation or another lane's public API.
- A lane may not add a dependency, unsafe block, alternate registration mechanism, or alternate backend.
- `conformance/` is data owned by another stream and must not be modified.
- Public signatures require doc comments and must compile under workspace lints.
