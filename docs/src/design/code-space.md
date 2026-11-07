# Code space

## Contract

Code space owns executable native bytes, their address-stable metadata, and their publication and retirement. A code object is a logical function or compilation unit with a stable entry offset, code range, relocations, stack maps, and debug records. A code handle is a stable runtime reference to a code object's mapping and metadata. A function cell is the symbol table's current callable function reference; redefinition may replace it while retained function objects and closures continue to refer to prior code handles. Code objects are allocated from page-backed arenas; ordinary function definition must not create one OS mapping per function.

The allocator obtains the operating-system page size at runtime through `sysconf(_SC_PAGESIZE)`. It rounds reservations and protection changes with checked arithmetic. The initial Linux arena size is 16 MiB, with page-rounded dedicated extents for larger objects. macOS reserves one process-wide 1 GiB `MAP_JIT` region and suballocates the same logical arenas inside it. Failure to create the reservation returns `CodeError::MappingFailed`; exhausting its free extents returns the typed `CodeError::CapacityExceeded`. Neither case requests a second JIT region or falls back to one mapping per function. The number of VM mappings scales with arenas and large extents, not with code-object count.

The unsafe operating-system boundary stays in `ncl-sys`. The signal-recovery trampoline's machine-code definition and OS-facing entry point belong there. Runtime coordinates Lisp compilation, publication, function cells, and code handles. The exact crate ownership and dependency edges follow the [crate map](crates.md).

## Current implementation

The current source has the following behavior:

- `packages/runtime/src/lib.rs` lines 252–272 calls `alloc_code(compiled.code.len())` for each compiled function. `packages/sys/src/code.rs` line 17 fixes the assumed page size at 4096 bytes; lines 161–200 round each request and call `mmap` for that allocation.
- `packages/sys/src/code.rs` lines 128–150 changes non-macOS mappings from writable to executable with `mprotect` and publishes the entry. Lines 82–97 and 173–180 use `MAP_JIT` on macOS and bracket code writes with `pthread_jit_write_protect_np`. Lines 142–147 invalidate the instruction cache on publish. There is no Linux AArch64 cache-maintenance operation in this path.
- `CodePtr::publish` stores the entry and published state with Release ordering; readers use Acquire ordering in `packages/sys/src/code.rs` lines 114–123 and 148–149.
- `packages/runtime/src/lib.rs` lines 206–249 registers compiled functions and retains their `CodePtr` values in `EvaluatorState`. `packages/runtime/src/evaluator.rs` lines 11–23 store the state in `Rc<EvaluatorState>`; the state has a `RefCell` function table, so it is not a cross-thread publication mechanism.
- `packages/sys/src/heap.rs` lines 201–259 has a stop-the-world `release_code` hook that checks saved return PCs in registered frame chains. It does not establish reachability from function objects or closures. `packages/sys/src/code/frames.rs` lines 29–39 records both a return PC and the function object.
- The stale instruction-cache risk on Linux AArch64 follows from the missing cache-maintenance step; it is an inference from the code path, not a reproduced failure. The one-mapping-per-compiled-function behavior follows from the allocation call path.

## Allocation and page permissions

Each arena is a contiguous page-aligned reservation. The allocator assigns stable, non-overlapping extents to code objects and stores the logical byte length separately from the page-rounded extent. It never moves a published object because PCs, closures, stack frames, and debug records can refer to its address.

On platforms using `mprotect`, a new extent is writable and non-executable while it is being constructed. Publication seals its complete pages read-execute. A sealed page is never written again. Allocation and publication proceed in address order within an arena so adjacent sealed pages coalesce into a bounded executable range; a writer that cannot preserve this order uses a separate arena. Reclaiming an object does not reopen a page while another object in that arena remains live. An arena is unmapped only when every code object in it is retired and no frame or reader can still refer to it. Oversized objects use page-rounded dedicated extents.

The page-granular permission rule means the allocator may pack code objects into one page only when it can publish all of them before sealing that page. Otherwise it advances to a fresh page. This trades some tail space for stable addresses, simple W^X transitions, and bounded mapping growth. On Linux, loading 100,000 simple `(defun name () 0)` forms must increase process RSS by no more than 512 MiB from immediately before `LOAD` to the run's peak. Run five fresh processes and apply the limit to the largest per-run increase; record each peak RSS and VM-map count. This budget covers the complete workload, including sealed code pages, metadata, and Lisp objects.

On macOS, the allocator reserves a process-wide 1 GiB `MAP_JIT` region and suballocates it. The JIT write-protection switch is per thread. A code-writer scope disables protection only on the writing thread, performs bounded byte writes, and restores protection before calling Lisp code, invoking callbacks, reaching a safepoint, allocating, or returning to normal runtime work. No allocation or safepoint is permitted while the write window is active, and restoration must occur on every error path. `MAP_JIT` does not make an object callable: the code object and its metadata must still pass the common publication sequence below. Apple documents the single-region and per-thread constraints in its [Apple Silicon JIT porting guide](https://developer.apple.com/documentation/apple-silicon/porting-just-in-time-compilers-to-apple-silicon) and [libpthread header](https://github.com/apple-oss-distributions/libpthread/blob/main/include/pthread/pthread.h#L2524-L2657).

The permission invariant is that NCL never exposes a page as writable and executable through the ordinary mapping path. macOS follows the operating system's `MAP_JIT` model: only the writer thread temporarily receives write access, and that thread does not execute JIT code while write protection is disabled. No code path leaves a writer in that state after publication.

## Instruction-cache synchronization

`ncl-sys` provides one cache-synchronization operation over the final code range:

- Linux AArch64 uses an in-tree `ncl-sys` inline-assembly routine because Rust has no stable instruction-cache-clear intrinsic. It reads D- and I-cache line sizes from `CTR_EL0`, cleans modified data-cache lines with `dc cvau`, executes `dsb ish`, invalidates instruction-cache lines with `ic ivau`, then executes `dsb ish` and `isb sy`. It runs after the last byte or relocation write and before publication. [LLVM compiler-rt documents this AArch64 sequence](https://github.com/llvm/llvm-project/blob/main/compiler-rt/lib/builtins/clear_cache.c#L838-L892); [Arm documents the user-space cache-maintenance contract](https://developer.arm.com/community/arm-community-blogs/b/architectures-and-processors-blog/posts/caches-self-modifying-code-implementing-clear-cache).
- macOS AArch64 calls the operating system's `sys_icache_invalidate` through `ncl-sys` for the written range, after restoring JIT write protection and before publication. This is the selected platform API; it does not rely on a Rust intrinsic.
- x86-64 needs no separate data/instruction cache flush for this operation. Code publication still needs the release/acquire synchronization below. [LLVM compiler-rt target handling](https://github.com/llvm/llvm-project/blob/main/compiler-rt/lib/builtins/clear_cache.c#L721-L734).
- Other supported targets must define their own cache contract before code execution is enabled. Windows is outside the target set.

The cache operation does not publish a function by itself. Cache coherence and Rust atomic ordering solve different parts of the protocol.

## Publication and concurrent calls

A code object becomes visible in this order:

1. Reserve an unpublished extent and write machine bytes.
2. Apply all code relocations and resolve constants without making the function callable.
3. Decode and validate stack maps, construct the code handle, and install stack maps, debug ranges, and registry metadata in a prepared state that PC lookups cannot expose yet.
4. Seal the page range executable and non-writable, or restore macOS JIT write protection.
5. Synchronize instruction and data caches for the written range after the last write and protection transition.
6. Activate the prepared registry entry, then release-publish the new function-cell value.
7. Callers acquire-load the function cell before entering the entry point.

No registry lookup, symbol function cell, or closure may expose a partially relocated code object. A failed step before step 6 discards only unpublished state. Re-definition release-publishes the new function-cell value atomically; callers already holding an old function object or closure may continue to call its old code.

The current `compile_native` order does not meet this contract: `packages/runtime/src/lib.rs` lines 268–272 calls `publish_code` before it encodes and decodes stack maps at lines 273–275, and the registry currently rejects unpublished code at `packages/sys/src/code/registry.rs` lines 11–23. L1-3.2 moves publication after map validation and metadata preparation, and changes registry installation so prepared metadata stays hidden until code sealing and activation complete. The same runtime path formats `CodeError` into a string at lines 269 and 272 and converts the map error to `String` at line 275; `SafepointMap::decode` currently returns `&'static str` at `packages/sys/src/code/frames.rs` line 69. L1-3.2 preserves typed `CodeError`, introduces typed `SafepointMapError`, and maps both into typed runtime errors.

A multi-threaded implementation must use synchronized function cells rather than sharing the current `Rc<EvaluatorState>` and its `RefCell` table. Function-cell loads use Acquire and redefinition publishes the new handle with Release, as specified above. The acquire-load lowering is part of L1-3.2 and changes both `packages/codegen/src/target_x86_64_lowering/calls.rs` and `packages/codegen/src/target_aarch64_lowering/calls.rs`. Do not put a global lock on every call. Additional locks may protect mutation but must not substitute for this publication order.

## Code retirement and the heap collector

The heap collector owns tracing and moving Lisp references. This contract also owns the code-handle layout, code-GC decisions, and handoff that retires executable extents; code GC is part of L1-3.3, not deferred to a later lane.

The existing `widetag::CODE` value 17 is the code-handle object. Its five payload words are `ENTRY` (a fixnum containing the stable native entry address), `SIZE` (a fixnum byte length), `CONSTANTS` (a traced constant-vector reference), `STACK_MAP` (a traced stack-map reference), and `DEBUG` (a traced debug-table reference). The `function_offset::CODE` slot at offset 3 of both `SIMPLE_FUN` and `CLOSURE` contains a heap reference to this `CODE` object, not a raw native address. A moving collection updates this heap reference; it does not move the native code or rewrite the entry address. The layout follows `packages/object/src/layout.rs` lines 17–29 and 93–100, `packages/object/src/code.rs` lines 11–34, and `packages/object/src/function.rs` lines 20–83.

Every Lisp thread, including the main thread, runs on a control stack mapped by the runtime, with a guard page at each end. `--control-stack-size` sets the size. At a code-GC safepoint, the collector captures each mutator's current PC and scans saved return PCs only within the registered bounds of these control stacks. The saved return PC is frame-header word 1 as defined in `packages/sys/src/code/frames.rs` lines 29–39; no unrelated native or foreign stack is scanned.

Code GC follows this handoff:

1. Stop every Lisp mutator at a safepoint and run the ordinary heap trace. Mark each reachable `widetag::CODE` handle, following `SIMPLE_FUN` and `CLOSURE` slot 3 and any other traced references. Keep its entry range and heap references to constants, maps, and debug data alive.
2. Resolve every captured current PC and saved return PC within the registered control-stack bounds to its code range. Mark that range's `CODE` handle and its constant vector, stack maps, and debug data as live roots, even if ordinary heap tracing missed them. Also pin ranges with outstanding compiler, linker, debugger, unwinder, profiler, or registry-reader leases.
3. After heap marks, stack-PC roots, and reader leases have been combined, pass each remaining unreachable handle's stable entry address to the code-space retire queue. Heap sweep may reclaim the handle object, but code space retains the executable extent and metadata as a retired candidate. It must not unmap or reuse the address during this handoff.
4. Before reclaiming a candidate, recheck that no reachable handle, active PC, saved return PC, or metadata lease names its range. Wait until every mutator has left the old range, reached a safepoint, and acknowledged a context-synchronizing retire epoch (`isb sy` on AArch64; a serializing `cpuid` on x86-64). Only after this quiescent point may code space unmap the extent or return it to the arena free set. A page shared with live code remains sealed and allocated.
5. Re-definition release-publishes a new function-cell value. Previously retained function objects and closures keep the old `CODE` handle reachable and callable; active return PCs independently pin old code. The old extent becomes a retire candidate only after both forms of liveness disappear.

The collector never rewrites a live return PC because code addresses are stable. The registry, maps, and debug metadata are retained through the same quiescent epoch as the bytes. `Heap::release_code` currently checks saved return PCs (`packages/sys/src/heap.rs` lines 201–259), but it does not trace `CODE` handles or implement this full handoff; L1-3.3 adds that integration while L02c-GC supplies heap reachability.

## Ordered implementation slices

L1-3 is the code-space implementation lane. The canonical L3-13 table is in [FASL v1](fasl.md); it is kept in one place so its paths and completion conditions cannot diverge. The paths in this table are disjoint within L1-3.

| Order | Commit unit | Primary paths | Completion evidence |
| --- | --- | --- | --- |
| L1-3.1 | Replace per-function mappings with page-backed arena allocation, checked page rounding, stable extents, and whole-arena release. | `packages/sys/src/code/arena.rs`, `packages/sys/src/code.rs` | Tests cover alignment, overflow, runtime page-size discovery, typed capacity failure, and reuse. Five fresh-process runs of 100,000 simple definitions complete without per-function mappings; each Linux run's RSS increase from immediately before `LOAD` to peak is at most 512 MiB, with peak RSS and map count recorded. |
| L1-3.2 | Add platform write scopes and cache synchronization, prepare metadata before publication, preserve typed errors, and implement release/acquire function-cell publication. | `packages/sys/src/code/publish.rs`, `packages/sys/src/code/frames.rs`, `packages/sys/src/code/registry.rs`, `packages/runtime/src/lib.rs`, `packages/runtime/src/evaluator.rs`, `packages/codegen/src/target_x86_64_lowering/calls.rs`, `packages/codegen/src/target_aarch64_lowering/calls.rs` | Generated functions execute after publication on Linux x86-64, Linux AArch64, and macOS arm64. Concurrent redefinition/call tests never observe partial code. Runtime preserves `CodeError` and typed safepoint-map errors instead of formatting them as strings. The AArch64 Linux check passes in the fourth-target CI job being added by L05-CI; current `.github/workflows/ci.yml` has x86-64 Linux and AArch64 macOS jobs, not an AArch64 Linux job (lines 26–29, 96–99). |
| L1-3.3 | Implement the `CODE` code-handle trace and retirement handoff, including redefinition, stack PCs, return PCs, and reader quiescence. | `packages/sys/src/heap.rs`, `packages/sys/src/code/retire.rs`, `packages/object/src/layout.rs`, `packages/object/src/function.rs`, `packages/object/src/gc.rs` | Tests prove old code stays callable through a saved function and closure after redefinition, active PCs and return PCs prevent reclamation, and unreachable code is released only after the context-synchronizing quiescent epoch. |

L1-3.1 and L1-3.2 may proceed after this contract freezes. L1-3.3 implements the code-GC interface defined above and consumes the L02c-GC heap reachability result; it does not defer code reclamation to that lane.
