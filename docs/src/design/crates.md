# Crate architecture

## 決定

外部 crate は `[dependencies]` と `[dev-dependencies]` の双方でゼロとする。OS 呼出しは `ncl-sys` の自前 `extern "C"` 宣言だけを使う。対象 crate と隣接リストは次のとおりで、矢印は依存方向である。

```text
ncl-sys -> none
ncl-object -> ncl-sys
ncl-ir -> ncl-object
ncl-types -> ncl-object
ncl-reader -> ncl-object
ncl-printer -> ncl-object
ncl-conditions -> ncl-object
ncl-clos -> ncl-object, ncl-types
ncl-compiler-front -> ncl-ir, ncl-reader, ncl-conditions, ncl-clos
ncl-asm-x86-64 -> none
ncl-asm-aarch64 -> none
ncl-objfile -> ncl-sys
ncl-codegen -> ncl-ir, ncl-asm-x86-64, ncl-asm-aarch64, ncl-objfile, ncl-object, ncl-sys
ncl-lib-numbers -> ncl-object, ncl-types
ncl-lib-sequences -> ncl-object, ncl-types
ncl-lib-strings -> ncl-object, ncl-types
ncl-lib-hash-arrays -> ncl-object, ncl-types
ncl-lib-streams -> ncl-object, ncl-types
ncl-lib-pathnames -> ncl-object, ncl-types
ncl-lib-format -> ncl-object, ncl-types
ncl-lib-loop -> ncl-object, ncl-types
ncl-lib-packages -> ncl-object, ncl-types
ncl-lib-macros -> ncl-object, ncl-types
ncl-threads -> ncl-object, ncl-sys
ncl-ffi -> ncl-object, ncl-sys
ncl-image -> ncl-object, ncl-objfile, ncl-sys
ncl-runtime -> ncl-compiler-front, ncl-codegen, ncl-image, ncl-threads, ncl-ffi
ncl-conformance -> ncl-runtime
ncl (bin) -> ncl-runtime
```

Paths are `packages/<name>/`, with `packages/asm/x86-64` and `packages/asm/aarch64`, `packages/compiler/front`, and `packages/lib/<name>`. `ncl-sys` alone permits unsafe code. Its OS surface declares `mmap`, `munmap`, `mprotect`, pthread primitives, mutex and condition-variable operations, `dlopen`, `dlsym`, `dlerror`, `clock_gettime`, `read`, `write`, `open`, `close`, `stat`, `opendir`, `signal`, macOS `pthread_jit_write_protect_np`, and `sys_icache_invalidate` as platform-specific `extern "C"` items. `ncl-ffi` reaches dynamic loading only through `ncl-sys`.

## 根拠

asm crates have no neighbors, so encoding is independent of object model and OS. `ncl-objfile` knows only sections and relocations, while `ncl-codegen` owns their conversion. The graph is acyclic because runtime depends on lower layers and no lower layer depends on runtime.

## 却下した代替案

- a general-purpose dependency crate is rejected because the zero-external-crate contract would be false.
- asm depending on codegen is rejected because target encoding would become cyclic.
- `ncl-object` owning pages and OS calls is rejected because unsafe ownership belongs to `ncl-sys`.

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- The complete list and adjacency list above are frozen.
- No external dependency, hidden workspace member, or reverse edge may be added.
- `ncl-objfile` consumes asm `Fixup` values as its own `Relocation` values; asm does not import objfile types.

### Public API summary

| crate | public API and responsibility |
| --- | --- |
| ncl-sys | OS `extern "C"`: mmap, mprotect, munmap, pthread create/join/self, stack attributes, mutex, condvar, semaphore, dlopen, dlsym, dlerror, clock_gettime, read, write, open, close, stat, opendir, signals, pthread_jit_write_protect_np, sys_icache_invalidate |
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
| ncl-lib-* | library builtin registration through `register(&Runtime)` |
| ncl-threads | Lisp thread API over ncl-sys |
| ncl-ffi | foreign declarations and calls |
| ncl-image | image save/load |
| ncl-runtime | eval, compile, load and registration orchestration |
| ncl-conformance | conformance runner |

The dependency graph is acyclic. `ncl-ir -> none` and `ncl-objfile -> none` are deliberate. `Runtime`, `ThreadContext`, and `builtin!` live in ncl-object. No external crate is permitted; OS declarations exist only in ncl-sys.

### Registration and ownership rules

`register(&Runtime)` is the only library registration entry point. It installs symbols, functions, classes, and compiler macros into Runtime registries and does not create a second global table. `ThreadContext` is created and registered by the runtime, owns its TLAB, binding stack, roots, handlers, safepoint state, and multiple-value area, and is unregistered before its OS thread exits.

`ncl-sys` owns pages, stack bounds, OS synchronization, dynamic loading, code-space permissions, and platform declarations. `ncl-object` owns the typed boundary and allocation API. `ncl-ir` contains no allocator, OS call, or object ownership. `ncl-codegen` consumes IR and emits machine structures. The assembler crates consume only their own machine model. `ncl-objfile` serializes bytes and never calls mmap, mprotect, or pthread APIs.

Reverse dependencies are forbidden: object does not import front end, codegen does not import compiler front, assembler does not import codegen, and objfile does not import runtime. A dependency review inspects every Cargo.toml edge and confirms the graph remains acyclic.

### API shape by layer

The unsafe layer returns raw handles and OS status values. The object layer converts them into `Word`, `StorageCondition`, `RootToken`, and typed slot operations. The compiler front end returns `Function` and diagnostics. Codegen returns `MachineFunction` and `CodegenError`. Assemblers return bytes and `Fixup`. Objfile returns a byte vector. Runtime is the only layer that sequences allocation, compilation, loading, and registration.

No crate may expose a raw page pointer, mutable global heap, hidden thread registry, or platform-specific register name across its boundary. Platform names stay in ncl-sys or the target assembler. The public API summary and adjacency table are the source of truth for Phase 1 lane ownership.

The library crates do not own compilation or allocation policy. They receive `Runtime` and `ThreadContext` from the integration layer, register builtins, and return typed conditions. This keeps registration, GC ownership, and OS access at their declared boundaries.

Each crate exposes the smallest API needed by its neighboring contract.

`ncl-sys` does not expose Lisp values.

`ncl-object` does not expose platform register names.

`ncl-ir` does not expose heap pointers.

`ncl-objfile` does not expose executable mappings.
