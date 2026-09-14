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
