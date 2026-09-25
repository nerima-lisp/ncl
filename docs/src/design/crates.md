# Crate architecture

## 決定

外部 crate は `[dependencies]` と `[dev-dependencies]` の双方でゼロとする。OS 呼出しは `ncl-sys` の自前 `extern "C"` 宣言だけを使う。対象 crate と隣接リストは次のとおりで、矢印は依存方向である。

```text
ncl-sys -> none
ncl-object -> ncl-sys
ncl-ir -> none
ncl-asm-x86-64 -> none
ncl-asm-aarch64 -> none
ncl-objfile -> none
ncl-codegen -> ncl-ir, ncl-asm-x86-64, ncl-asm-aarch64, ncl-objfile, ncl-object, ncl-sys
ncl-ownership -> ncl-object
ncl-types -> ncl-object
ncl-reader -> ncl-object
ncl-printer -> ncl-object
ncl-conditions -> ncl-object
ncl-clos -> ncl-object, ncl-types, ncl-conditions
ncl-compiler-front -> ncl-ir, ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-clos
ncl-lib-numbers -> ncl-object, ncl-types, ncl-conditions
ncl-lib-sequences -> ncl-object, ncl-types, ncl-conditions
ncl-lib-strings -> ncl-object, ncl-types, ncl-conditions
ncl-lib-hash-arrays -> ncl-object, ncl-types, ncl-conditions
ncl-lib-streams -> ncl-object, ncl-types, ncl-conditions, ncl-sys
ncl-lib-pathnames -> ncl-object, ncl-types, ncl-conditions, ncl-lib-strings, ncl-sys
ncl-lib-packages -> ncl-object, ncl-types, ncl-conditions
ncl-lib-format -> ncl-object, ncl-types, ncl-conditions, ncl-lib-streams, ncl-lib-strings, ncl-printer
ncl-lib-macros -> ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-lib-sequences
ncl-stdlib -> ncl-types, ncl-reader, ncl-printer, ncl-conditions, ncl-clos,
              ncl-lib-numbers, ncl-lib-sequences, ncl-lib-strings, ncl-lib-hash-arrays,
              ncl-lib-streams, ncl-lib-pathnames, ncl-lib-packages, ncl-lib-format, ncl-lib-macros
ncl-threads -> ncl-object, ncl-sys, ncl-conditions
ncl-ffi -> ncl-object, ncl-sys, ncl-conditions
ncl-image -> ncl-object, ncl-objfile, ncl-sys
ncl-runtime -> ncl-compiler-front, ncl-codegen, ncl-stdlib, ncl-image, ncl-threads, ncl-ffi
ncl-conformance -> ncl-runtime, ncl-ownership
ncl (bin) -> ncl-runtime
```

Paths are `packages/<name>/`, with `packages/asm/x86-64` and `packages/asm/aarch64`, `packages/compiler/front`, `packages/lib/<name>`, `packages/stdlib`, and `packages/ownership`. `ncl-sys` alone permits unsafe code. Its OS surface declares `mmap`, `munmap`, `mprotect`, pthread primitives, mutex and condition-variable operations, `dlopen`, `dlsym`, `dlerror`, `clock_gettime`, `read`, `write`, `open`, `close`, `stat`, `opendir`, `signal`, macOS `pthread_jit_write_protect_np`, and `sys_icache_invalidate` as platform-specific `extern "C"` items. `ncl-ffi` reaches dynamic loading only through `ncl-sys`.

## 根拠

asm crates have no neighbors, so encoding is independent of object model and OS. `ncl-objfile` knows only sections and relocations, while `ncl-codegen` owns their conversion. The graph is acyclic because runtime depends on lower layers and no lower layer depends on runtime.

## 却下した代替案

- a general-purpose dependency crate is rejected because the zero-external-crate contract would be false.
- asm depending on codegen is rejected because target encoding would become cyclic.
- `ncl-object` owning pages and OS calls is rejected because unsafe ownership belongs to `ncl-sys`.
- `ncl-runtime` depending directly on `ncl-lib-*` is rejected because it mixes the independent concern of registration order into runtime.

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- The revised crate list and adjacency list above are frozen by the 2026-09-24 revision, which added `ncl-ownership` and `ncl-stdlib` and turned the `ncl-lib-*` edges into real dependencies. The 2026-09-24 revision is the freeze date.
- No external dependency, hidden workspace member, or reverse edge may be added.
- `ncl-codegen` converts asm `Fixup` values into objfile `Relocation` values; objfile does not import asm types.

### Public API summary

| crate | public API and responsibility |
| --- | --- |
| ncl-sys | OS `extern "C"`: mmap, mprotect, munmap, pthread_create, pthread_join, pthread_self, pthread stack attributes, mutex, condvar, semaphore, dlopen, dlsym, dlerror, clock_gettime, read, write, open, close, stat, opendir, signal, pthread_jit_write_protect_np, sys_icache_invalidate |
| ncl-object | `Word`, `TypeTag`, accessors, `Runtime`, `ThreadContext`, `RootToken`, `register`, `builtin!`, allocation and package/intern API |
| ncl-ir | independent IR data types and descriptors only |
| ncl-types | type specifiers, predicates, and type errors |
| ncl-reader/printer | readtable, reader and printer interfaces |
| ncl-conditions | condition, handler, restart, catch and cleanup interfaces |
| ncl-clos | class, slot, generic-function and MOP interfaces |
| ncl-compiler-front | macroexpand, declarations, compiler macros, IR lowering |
| ncl-codegen | MachineFunction, register allocation, frame and safepoint metadata |
| ncl-asm-x86-64 / ncl-asm-aarch64 | target instruction model and encoder, no codegen dependency |
| ncl-objfile | FASL and native object writer, returns bytes and calls no OS API |
| ncl-ownership | `symbols.tsv` の行型、`rows_for_crate`、`assert_crate_coverage(&Runtime, &mut ThreadContext, crate)`。テスト支援専用で Lisp 値を公開しない |
| ncl-stdlib | `register_all(&Runtime)`: 全 `ncl-lib-*` と types/reader/printer/conditions/clos の `register` を依存順に呼ぶ唯一の入口。順序表を持つ |
| ncl-lib-numbers | 数値塔の builtin 登録(所有表 164 行)。bignum/ratio/float/complex の演算、`ash`、`random`、`boole` |
| ncl-lib-sequences | list/sequence/tree の builtin 登録(150 行)。`equal`/`equalp`、`sort`、`map` 系 |
| ncl-lib-strings | 文字・文字列の builtin 登録(108 行)。文字述語、`char=` 系、名前と符号 |
| ncl-lib-hash-arrays | 配列と hash table の builtin 登録(55 行)。`aref` 系、`adjust-array`、synchronized hash table |
| ncl-lib-streams | stream の builtin 登録(121 行)。fd-stream、standard stream 変数、`read-byte`/`write` 系、external format |
| ncl-lib-pathnames | pathname と file system の builtin 登録(43 行)。`native-namestring`、directory 操作 |
| ncl-lib-packages | package と symbol の builtin 登録(58 行)。package-local nickname、`gensym`/`gentemp` |
| ncl-lib-format | `format` の全 directive(所有表 1 行、directive 集合は CLHS 22.3 全部) |
| ncl-lib-macros | 標準マクロの Rust expander 登録(97 行)。`defun`/`when`/`setf`/`loop` など、`macroexpand`、`funcall`/`apply` |
| ncl-threads | Lisp thread API over ncl-sys |
| ncl-ffi | foreign declarations and calls |
| ncl-image | image save/load |
| ncl-runtime | eval, compile, load; calls `ncl-stdlib::register_all` once at startup |
| ncl-conformance | conformance runner |

The dependency graph is acyclic. `ncl-ir -> none` and `ncl-objfile -> none` are deliberate. `Runtime`, `ThreadContext`, and `builtin!` live in ncl-object. No external crate is permitted; OS declarations exist only in ncl-sys.

### Registration and ownership rules

Each `ncl-lib-*` crate's `register(&Runtime)` is its per-crate registration function. It installs symbols, functions, classes, and compiler macros into Runtime registries and does not create a second global table. `ncl-stdlib::register_all(&Runtime)` is the only integrated entry point and calls those per-crate `register` functions in dependency order, holding the order table. `ncl-runtime` calls only `register_all`, once at startup; a second call returns `ObjectError`. `ThreadContext` is created and registered by the runtime, owns its TLAB, binding stack, roots, handlers, safepoint state, and multiple-value area, and is unregistered before its OS thread exits.

`ncl-stdlib::register_all(&Runtime)` が標準ライブラリ登録の唯一の入口である。各
`ncl-lib-*` の `register(&Runtime)` は他 crate の `register` を呼ばない。`ncl-runtime` は
起動時に `register_all` を一度だけ呼び、二度目の呼出は `ObjectError` を返す。

`ncl-sys` owns pages, stack bounds, OS synchronization, dynamic loading, code-space permissions, and platform declarations. `ncl-object` owns the typed boundary and allocation API. `ncl-ir` contains no allocator, OS call, or object ownership. `ncl-codegen` consumes IR and emits machine structures. The assembler crates consume only their own machine model. `ncl-objfile` serializes bytes and never calls mmap, mprotect, or pthread APIs.

Reverse dependencies are forbidden: object does not import front end, codegen does not import compiler front, assembler does not import codegen, and objfile does not import runtime. A dependency review inspects every Cargo.toml edge and confirms the graph remains acyclic.

### API shape by layer

The unsafe layer returns raw handles and OS status values. The object layer converts them into `Word`, `StorageCondition`, `RootToken`, and typed slot operations. The compiler front end returns `Function` and diagnostics. Codegen returns `MachineFunction` and `CodegenError`. Assemblers return bytes and `Fixup`. Objfile returns a byte vector. Runtime is the only layer that sequences allocation, compilation, loading, and registration.

No crate may expose a raw page pointer, mutable global heap, hidden thread registry, or platform-specific register name across its boundary. Platform names stay in ncl-sys or the target assembler. The public API summary and adjacency table are the source of truth for Phase 1 lane ownership.

The library crates do not own compilation or allocation policy. They receive `Runtime` and `ThreadContext` from the integration layer, register builtins, and return typed conditions. This keeps registration, GC ownership, and OS access at their declared boundaries.

Each crate exposes the smallest API needed by its neighboring contract.

## Extension crates

The Gate 0 extension crates and their dependency edges are:

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

These crates are workspace members. `ncl-disasm` keeps both assembler crates
in dev-dependencies for encoder round-trip tests only.

`ncl-sys` does not expose Lisp values.

`ncl-object` does not expose platform register names.

`ncl-ir` does not expose heap pointers.

`ncl-objfile` does not expose executable mappings.
