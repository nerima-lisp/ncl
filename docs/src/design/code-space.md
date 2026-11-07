# Code space

## Contract

Code space owns executable native bytes, their address-stable metadata, and their publication and retirement. A code object is a logical function or compilation unit with a stable entry offset, code range, relocations, stack maps, and debug records. A code handle is a stable runtime reference to a code object's mapping and metadata. A function cell is the symbol table's current callable function reference; redefinition may replace it while retained function objects and closures continue to refer to prior code handles. Code objects are allocated from page-backed arenas; ordinary function definition must not create one OS mapping per function.

The allocator obtains the operating-system page size at runtime through `sysconf(_SC_PAGESIZE)`. It rounds reservations and protection changes with checked arithmetic. The initial Linux arena size is 16 MiB, with page-rounded dedicated extents for larger objects. macOS uses one process-wide `MAP_JIT` reservation and suballocates the same logical arenas inside it. These sizes are allocator policy; the contract is that the number of VM mappings scales with arenas and large extents, not with code-object count.

The unsafe operating-system boundary stays in `ncl-sys`. The signal-recovery trampoline's machine-code definition and OS-facing entry point belong there. Runtime coordinates Lisp compilation, publication, function cells, and code handles. The exact crate ownership and dependency edges follow the [crate map](crates.md).

## Current implementation

The current source has the following behavior:

- `packages/runtime/src/lib.rs` lines 252–272 calls `alloc_code(compiled.code.len())` for each compiled function. `packages/sys/src/code.rs` line 17 fixes the assumed page size at 4096 bytes; lines 161–200 round each request and call `mmap` for that allocation.
- `packages/sys/src/code.rs` lines 128–150 changes non-macOS mappings from writable to executable with `mprotect` and publishes the entry. Lines 82–97 and 173–180 use `MAP_JIT` on macOS and bracket code writes with `pthread_jit_write_protect_np`. Lines 142–147 invalidate the instruction cache on publish. There is no Linux AArch64 cache-maintenance operation in this path.
- `CodePtr::publish` stores the entry and published state with Release ordering; readers use Acquire ordering in `packages/sys/src/code.rs` lines 114–123 and 148–149.
- `packages/runtime/src/lib.rs` lines 206–249 registers compiled functions and retains their `CodePtr` values in `EvaluatorState`. `packages/runtime/src/evaluator.rs` lines 11–23 use `Rc<RefCell<...>>` for the current function table, so that table is not a cross-thread publication mechanism.
- `packages/sys/src/heap.rs` lines 201–259 has a stop-the-world `release_code` hook that checks saved return PCs in registered frame chains. It does not establish reachability from function objects or closures. `packages/sys/src/code/frames.rs` lines 29–39 records both a return PC and the function object.
- A release-build probe on macOS arm64 used 100,000 distinct `defun` forms. Its final `vmmap -noCoalesce` snapshot had 186,401 total process regions, including 186,156 rows with `rwx`, and 4.5 GiB resident memory. The REPL returned after `LOAD`, but the captured output did not establish whether `LOAD` returned normally. `fboundp` was true for the first generated function and false for the last; subsequent REPL reads reported `Storage(CapacityExceeded)`. The run does not establish that all 100,000 definitions completed or that the OS mapping limit was reached.
- The stale instruction-cache risk on Linux AArch64 follows from the missing cache-maintenance step; it is an inference from the code path, not a reproduced failure. The one-mapping-per-compiled-function behavior follows from the allocation call path.

The probe used these commands after building the release binary. The generated file contains one form per line; `vmmap` runs in a second terminal while NCL is loading it:

```sh
cargo build --release -p ncl
python3 -c 'from pathlib import Path; Path("target/code-space-100k.lisp").write_text("".join(f"(defun code-space-probe-{i:06d} () 0)\n" for i in range(100_000)))'
./target/release/ncl
```

At the NCL prompt, enter `(load "target/code-space-100k.lisp")`; in the second terminal, run `vmmap -noCoalesce <ncl-pid>` and record resident memory and total regions. The captured REPL checks were `(fboundp 'code-space-probe-000000) => T` and `(fboundp 'code-space-probe-099999) => NIL`.

## Allocation and page permissions

Each arena is a contiguous page-aligned reservation. The allocator assigns stable, non-overlapping extents to code objects and stores the logical byte length separately from the page-rounded extent. It never moves a published object because PCs, closures, stack frames, and debug records can refer to its address.

On platforms using `mprotect`, a new extent is writable and non-executable while it is being constructed. Publication seals its complete pages read-execute. A sealed page is never written again. Allocation and publication proceed in address order within an arena so adjacent sealed pages coalesce into a bounded executable range; a writer that cannot preserve this order uses a separate arena. Reclaiming an object does not reopen a page while another object in that arena remains live. An arena is unmapped only when every code object in it is retired and no frame or reader can still refer to it. Oversized objects use page-rounded dedicated extents.

The page-granular permission rule means the allocator may pack code objects into one page only when it can publish all of them before sealing that page. Otherwise it advances to a fresh page. This trades some tail space for stable addresses, simple W^X transitions, and bounded mapping growth. Tests record both resident memory and VM-map count so a map-count improvement cannot hide unacceptable space growth.

On macOS, the allocator reserves a process-wide `MAP_JIT` region and suballocates it. The JIT write-protection switch is per thread. A code-writer scope disables protection only on the writing thread, performs bounded byte writes, and restores protection before calling Lisp code, invoking callbacks, or returning to normal runtime work. Restoration must occur on every error path. `MAP_JIT` does not make an object callable: the code object and its metadata must still pass the common publication sequence below. Apple documents the single-region and per-thread constraints in its [Apple Silicon JIT porting guide](https://developer.apple.com/documentation/apple-silicon/porting-just-in-time-compilers-to-apple-silicon) and [libpthread header](https://github.com/apple-oss-distributions/libpthread/blob/main/include/pthread/pthread.h#L2524-L2657).

The permission invariant is that NCL never exposes a page as writable and executable through the ordinary mapping path. macOS follows the operating system's `MAP_JIT` model: only the writer thread temporarily receives write access, and that thread does not execute JIT code while write protection is disabled. No code path leaves a writer in that state after publication.

## Instruction-cache synchronization

`ncl-sys` provides one cache-synchronization operation over the final code range:

- Linux AArch64 cleans modified data-cache lines to the point of unification, waits for completion, invalidates instruction-cache lines, waits again, and executes the required instruction synchronization barrier. Use the platform's compiler-supported clear-cache primitive or an equivalent audited implementation. The operation runs after the last byte or relocation write and before publication. [GCC documents `__builtin___clear_cache`](https://gcc.gnu.org/onlinedocs/gcc-15.1.0/gcc/Other-Builtins.html); [LLVM compiler-rt shows the AArch64 sequence](https://github.com/llvm/llvm-project/blob/main/compiler-rt/lib/builtins/clear_cache.c#L838-L892).
- x86-64 needs no separate data/instruction cache flush for this operation; the compiler clear-cache primitive is a no-op on this target. Code publication still needs the release/acquire synchronization below. [LLVM compiler-rt target handling](https://github.com/llvm/llvm-project/blob/main/compiler-rt/lib/builtins/clear_cache.c#L721-L734).
- Other supported targets must define their own cache contract before code execution is enabled. Windows is outside the target set.

The cache operation does not publish a function by itself. Cache coherence and Rust atomic ordering solve different parts of the protocol.

## Publication and concurrent calls

A code object becomes visible in this order:

1. Reserve an unpublished extent and write machine bytes.
2. Apply all code relocations and resolve constants without making the function callable.
3. Install stack maps, debug ranges, and registry metadata for the complete code range.
4. Synchronize instruction and data caches for the written range.
5. Seal the page range executable and non-writable, or restore macOS JIT write protection.
6. Release-publish the code handle and new entry through the function cell.
7. Callers acquire-load the function cell before entering the entry point.

No registry, symbol function cell, or closure may expose a partially relocated code object. A failed step before step 6 discards only unpublished state. Re-definition publishes the new function cell atomically; callers already holding the old function object may continue to call its old code.

A multi-threaded implementation must use synchronized function cells rather than sharing the current `Rc<RefCell<...>>` table. Function-cell loads use Acquire and redefinition publishes the new handle with Release, as specified above. Do not put a global lock on every call. Additional locks may protect mutation but must not substitute for this publication order.

## Code retirement and the heap collector

Lisp heap reachability and executable-page reclamation are related but separate responsibilities. The L02c-GC lane owns heap tracing and moving references. The code-space contract owns code handles, executable page state, and safe retirement. The integration API must let the collector enumerate or update code references without making the heap collector responsible for OS mappings.

Every Lisp thread, including the main thread, runs on a control stack mapped by the runtime, with one guard page at each end. `--control-stack-size` sets the control-stack size. Code GC scans saved return addresses only within the registered bounds of these control stacks and checks current PCs at mutator safepoints; it does not scan unrelated stacks.

A code object remains live while any of these references exist:

- The current function cell or a reachable function object points to it.
- A closure or another heap object retains a code handle for it.
- A registered mutator has its current PC in the code range or a saved return PC in a frame that will return into it.
- A compiler, linker, debugger, unwinder, profiler, or code-registry reader has an outstanding reference to its metadata.

Redefinition only replaces the current function-cell reference. It does not invalidate an older function object or closure. Return addresses and active PCs must be checked at safepoints; a frame's function object is also a heap root, but it does not replace the return-PC check. The code registry, stack maps, and debug information stay available until no stack walk or asynchronous reader can use them.

When an object becomes unreachable, mark it retired first. Reclaim its pages only after the collector has established a quiescent point for every registered mutator and verified that no active PC, saved return PC, live function object, closure, or metadata reader refers to the extent. Because code is non-moving, a live return PC is never rewritten. The collector must not unmap code while a thread can execute it.

The existing `Heap::release_code` return-PC scan is a useful low-level check, not the complete liveness contract. L1-3 and L02c-GC must agree on the code-handle root protocol before enabling reclamation.

## Ordered implementation slices

L1-3 is the code-space implementation lane; L3-13 is the FASL and image lane. The table orders their commits and gives each unit its owned paths and observable completion evidence. Tests belong under each crate's `tests/` directory unless they exercise a private invariant.

| Order | Commit unit | Primary paths | Completion evidence |
| --- | --- | --- | --- |
| L1-3.1 | Replace per-function mappings with page-sized arena allocation, checked page rounding, stable extents, and whole-arena release. | `packages/sys/src/code.rs`, `packages/sys/src/code/*` | Allocator tests cover alignment, overflow, page-size discovery, exhaustion, and reuse. Defining 100,000 functions completes without mapping one region per function; report map count and peak RSS. |
| L1-3.2 | Add platform publication: macOS JIT write scopes, Linux AArch64 cache sync, x86-64 cache policy, metadata-before-entry ordering, and release/acquire function-cell publication. | `packages/sys/src/code.rs`, `packages/runtime/src/lib.rs`, `packages/runtime/src/evaluator.rs` | Generated functions execute after publication on Linux x86-64, Linux AArch64, and macOS arm64. A concurrent redefinition/call test never observes partial code. The AArch64 CI job passes. |
| L1-3.3 | Add code handles, retirement, and collector coordination for functions, closures, active PCs, saved return PCs, stack maps, and debug readers. | `packages/sys/src/heap.rs`, `packages/sys/src/code/*`, `packages/object/src/function.rs`, `packages/runtime/src/evaluator.rs` | Tests prove old code stays callable through a saved function and closure after redefinition, active frames prevent reclamation, and unreachable code is eventually released after a quiescent collection. |
| L3-13.1 | Implement the canonical FASL v1 writer, parser, relocation checks, compatibility metadata, and `invalid-fasl` mapping. | `packages/objfile/src/fasl.rs`, `packages/objfile/src/fasl_relocation.rs`, `packages/objfile/src/error.rs` | Golden-byte tests establish reproducibility; malformed-input tests cover each rejection stage; FASL fuzzing runs without panic, out-of-bounds access, or unbounded allocation. |
| L3-13.2 | Compile units to code, constants, relocations, symbols, stack maps, debug records, and top-level execution records; load by linking and publishing without source recompilation. | `packages/runtime/src/compile.rs`, `packages/runtime/src/load.rs`, `packages/runtime/src/lib.rs` | A compile-file/load round trip executes the same functions and top-level effects. The load test proves the compiler path is not entered and does not require the original source file. |
| L3-13.3 | Version and restore process images independently from FASL, including runtime options and relocated code. | `packages/image/src/*`, `packages/runtime/src/*` | Save and restore in a fresh process preserves roots, functions, and top-level state. Unsupported image, runtime ABI, and feature versions fail before publication. |
| L3-13.4 | Complete disassembly and debug records, including PC-to-form maps, variable locations, and profiler maps. | `packages/disasm/src/lib.rs`, `packages/image/src/*`, `packages/runtime/src/*` | Disassembly resolves the loaded architecture; a stack trace maps PCs and variables to source records; perf map or jitdump output matches published ranges. |

L1-3.1 and L1-3.2 can proceed after this contract freezes. L1-3.3 also depends on the L02c-GC heap-root and safepoint contract. L3-13 depends on L1-3.2 for publication and on L1-3.3 for image/code lifetime.
