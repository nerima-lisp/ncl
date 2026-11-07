# GC and heap contract

## Contract status

This page defines the GC contract for FR-F01, FR-F03, the heap part of FR-A03 and FR-J02, and D-03. It also fixes the GC interfaces required by L1-4, L4-1, and L3-7.

The **Current implementation at baseline** section records source facts at commit `6aa6639d`. Every later section is the target contract, not a claim that the behavior already exists. Unless a narrower path is listed, crate ownership follows [Crate map](crates.md). Unsafe code stays in `ncl-sys`; no external crate is added.

## Requirement reference

- FR-F01 selects the page-based generational collector, TLAB allocation, card barrier, OS reclamation, large objects, conservative pinning, verification/stress modes, and EQ/EQL rehash.
- FR-F03 requires weak references, finalizers, `*AFTER-GC-HOOKS*`, per-thread `GET-BYTES-CONSED`, `ROOM`, GC logs/metrics, and heap dump.
- FR-A03 requires `--dynamic-space-size` and catchable `storage-condition` on exhaustion. FR-J02 requires direct word access without a global lock or hash lookup on the mutator hot path. D-03 selects the page-based generational design.
- FR-F02 makes parallel or concurrent marking optional after FR-F01. V5 is the stress/verification layer: collection at every allocation, heap verification, multithread stress, signal torture, ASan, TSan, and Miri.
- L0-2 freezes this contract; L0-3 measures the baseline; L1-2 owns signals and stack guards; L1-10 migrates features out of `ncl-object`; L3-2 owns prerequisite HIR optimization; L3-7 checks allocation code generation; L4-1 implements threads. `l02d-cond` is the separate condition-mapping contract used for `storage-condition`.

## Current implementation at baseline `6aa6639d`

### Heap and collection

The collector already performs stop-the-world, moving, generational collection. It gathers heap and thread roots, the live multiple-value prefix, conservative snapshots, and native frame values in `packages/sys/src/heap/collect.rs` lines 16-105. It moves live, unpinned objects below generation 2 and advances their survival generation (`packages/sys/src/heap/collect.rs` lines 124-165). It forwards roots and object slots, handles weak hash entries, and removes logically dead objects (`packages/sys/src/heap/collect.rs` lines 169-232,483-498).

Storage is still per object: `Object.words` is a `Box<[u64]>`, and `State` owns a `Vec<Object>`, an object-start `HashMap`, a layout `HashMap`, and a dirty-card `HashSet` (`packages/sys/src/heap_state.rs` lines 5-30). `PageKind` classifies cons, ordinary, large, code, and static objects; it is not a page allocator or page table (`packages/sys/src/heap_types.rs` lines 17-28). Allocation creates a fresh boxed slice and inserts its address in the object-start map (`packages/sys/src/heap/allocation.rs` lines 162-212). Objects of at least 8 KiB start in generation 2; smaller objects start in generation 0.

### Locks, lookup, and storage retention

`Heap` stores mutable state behind `Mutex<State>` (`packages/sys/src/heap.rs` lines 27-40). `find_raw` resolves a pointer through `object_starts` (`packages/sys/src/heap.rs` lines 290-300). Word reads and writes take the mutex and resolve the object (`packages/sys/src/heap.rs` lines 334-350). The write barrier also takes that mutex, resolves its owner, and inserts a dirty-card entry (`packages/sys/src/heap.rs` lines 369-377). The current card constant is 512 bytes (`packages/sys/src/heap.rs` lines 19).

When an object dies, the collector subtracts its size from the logical `used` counter and sets `alive = false`, but leaves the object in `State.objects`; its boxed storage remains allocated (`packages/sys/src/heap/collect.rs` lines 483-498). Forwarded source objects also remain in that vector (`packages/sys/src/heap/collect.rs` lines 124-165). This is why logical reclamation does not release memory to the OS.

### TLAB and code generation

Generated allocation code has a TLAB bump/limit fast-path shape. x86-64 uses the pinned context in R15, R10/R11 for the bump and limit, and RAX for the end/result (`packages/codegen/src/isa_x86_64.rs` lines 3-6, `packages/codegen/src/target_x86_64_lowering/runtime_ops.rs` lines 100-151). AArch64 uses x21 for the context, x16/x17 for bump/limit, and x0 for the end/result (`packages/codegen/src/target.rs` lines 19-27, `packages/codegen/src/target_aarch64_lowering.rs` lines 218-316). `ThreadLayout` exposes the TLAB offsets (`packages/sys/src/thread/layout.rs` lines 3-42).

Generated reference stores are followed by `WriteBarrier` calls for cons and vector setters (`packages/codegen/src/target_x86_64_lowering/primitives.rs` lines 197-210, `packages/codegen/src/target_aarch64_lowering/primitives.rs` lines 323-345). The runtime shim receives `(Thread*, slot_index, owner)` and forwards to the sys API, whose typed argument order is `(thread, owner, slot_index)` (`packages/runtime/src/support/write_barrier.rs` lines 6-7, `packages/sys/src/lib.rs` lines 389-397). The current sys barrier then locks and resolves the owner (`packages/sys/src/heap.rs` lines 369-377).

That generated shape has no production refill path: `set_tlab` is defined and exported (`packages/sys/src/codegen.rs` lines 3-7, `packages/sys/src/lib.rs` lines 29), while the runtime resolver reports `AllocateSlow` as unsupported (`packages/runtime/src/support.rs` lines 144-147). The existing generated fast path is therefore not a working production TLAB allocator.

### Rooting and thread protocol

`Thread::push_root` and `register_root_set` register caller-owned mutable slots whose addresses must outlive their token; removal is LIFO (`packages/sys/src/thread.rs` lines 110-130). Heap-level roots use stable caller-owned slots and tokens (`packages/sys/src/heap/roots.rs` lines 25-123). Object-layer `with_root` and `with_roots` keep values in interior-mutable slots and reread relocated values after allocation (`packages/object/src/roots.rs` lines 127-179). Callers use the same discipline: hash-table construction roots its argument slice until setup finishes (`packages/lib/hash-arrays/src/hash_tables.rs` lines 73-83,118-120), and condition construction roots both the datum and expected type across allocation (`packages/conditions/src/conversion.rs` lines 52-65). A managed `Word` held across an allocation must use one of these APIs or a precise native frame slot.

The collector scans precise roots and frames together with conservative thread snapshots; a conservative match currently pins an object (`packages/sys/src/heap/collect.rs` lines 26-72). Stop-the-world collection uses an epoch and parks polling mutators (`packages/sys/src/stw.rs` lines 23-95). Native transitions publish a snapshot and require managed values to be rooted (`packages/sys/src/thread/native.rs` lines 58-133). Runtime thread contexts register and unregister their sys thread (`packages/object/src/context.rs` lines 53-101,392-403).

### Existing GC surface and implementation mismatch

The design specifies three generations, a 32 KiB TLAB, an 8 KiB large-object threshold, 512-byte cards, `--dynamic-space-size`, weak references, finalizers, and post-GC hooks. Source confirms stop-the-world movement, generation numbers, the allocation threshold, and the card-size constant, but not page-backed storage, a production TLAB refill, page-level pinning, OS reclamation, the CLI option, or most Lisp APIs. The latter items are requirements, not shipped behavior.

The current default heap configuration is 64 MiB with a 4 MiB allocation-debt threshold (`packages/sys/src/heap.rs` lines 42-48). The allocation path checks the logical used-byte counter against the configured size (`packages/sys/src/heap/allocation.rs` lines 170-176). At baseline, searches for `--dynamic-space-size` and `GET-BYTES-CONSED` in `packages/` return no implementation matches.

Most functions registered by `packages/object/src/gc.rs` lines 163-200 have `Word::UNBOUND` function cells; only `NCL-GC:GC` is implemented there. The internal `fn()` hook vector (`packages/sys/src/heap_state.rs` lines 29, `packages/sys/src/heap.rs` lines 406-407) is not Lisp's `NCL-GC:*AFTER-GC-HOOKS*`. Current finalizer collection queues `Word::NIL` with a callback and calls the callback with NIL (`packages/sys/src/heap/collect.rs` lines 490-492, `packages/sys/src/heap.rs` lines 398-405). `ROOM` is owned by runtime Phase 2 (`packages/runtime/ownership.tsv` lines 40). `ThreadContext::set_gc_stress` forces a full collection before each object allocation (`packages/object/src/context.rs` lines 304-311, `packages/object/src/primitives.rs` lines 21-47); it does not verify heap invariants. Strict forwarding rejects forwarded stale words only when enabled (`packages/sys/src/heap.rs` lines 123-126,309-320).

## Design contract

### Heap layout and allocation

- Reserve one contiguous dynamic-heap address range whose usable size is `--dynamic-space-size`. The default remains 64 MiB. Parse a positive byte count with optional binary `KiB`, `MiB`, or `GiB` suffix using checked arithmetic; reject malformed, zero, overflowing, or non-page-aligned values. The size must be a multiple of both 32 KiB and the host VM page size. Commit backing pages on demand. The cap includes nursery, aging, old, large-object, static, and TLAB backing pages; it excludes page metadata and nonmoving code space.
- A logical GC page is 32 KiB, matching the per-mutator TLAB size. The OS layer groups logical pages to satisfy the host VM page granularity. A physical backing group is returned only when every logical page in it is free. Direct page metadata is indexed by `(address - heap_base) / 32KiB`; object-start bits, object size/layout data, generation, pin state, and the card bitmap live outside the managed region. Do not use a hash table in mutator address resolution.
- Keep cons-only, header-object, large-object, static, and code storage separate. A conservative match pins the whole 32 KiB logical page for that collection. This limits retention from false candidates and prevents a cons-shaped word from being interpreted as a header object.
- Give each mutator two lazily filled nursery TLABs of 32 KiB, one for cons pages and one for header-object pages. Refill assigns a fresh page of the matching class and initializes its bump and limit fields. Codegen selects that class's fields statically. The fast path accepts only allocations below 8 KiB and succeeds when `end <= tlab_limit`; it bumps the pointer, initializes the allocation metadata, updates per-thread byte counters, and returns the prior bump. Objects of 8 KiB or larger bypass the TLAB and use large-object allocation.
- Nursery survivors move to aging on the first minor collection and to old after surviving the next minor collection. Minor collection examines nursery and aging plus dirty cards in old-generation and large-object pages. Full collection examines every generation, promotes surviving nursery and aging objects to old, compacts movable old pages, sweeps pinned pages in place, and sweeps the large-object area. Large objects do not move. A large-object page may hold multiple objects; objects at least 32 KiB occupy a contiguous page extent.
- Retire both TLABs for every mutator at a stop-the-world boundary and include only each allocated prefix in the object map. Reuse an unused tail only after the page is reinitialized for its allocation class. A TLAB page is not reclaimed while any mutator can still allocate from it.
- After all roots and strong object references are rewritten, clear dead object-start metadata and return wholly empty pages/extents to the OS by decommitting or unmapping their backing. Preserve the reserved virtual range and direct page indices. A page with a conservative pin cannot move or be released during that collection.

### Collection, roots, and safepoints

- Keep collection stop-the-world. A collector increments the epoch, requests polls, waits for every registered Lisp mutator to publish its stack bounds and callee-saved snapshot or enter `Safe`, then scans, updates, and resumes mutators. Concurrent or parallel marking (FR-F02) is deferred until the stop-the-world design passes V5 and performance measurements justify it.
- Use precise `RootToken`/typed handles for Rust values and precise `SafepointMap` slots for generated Lisp frames. Conservative stack and register scanning is an additional safety net, never a substitute for explicit roots. Candidate values must fall within the reserved heap and identify an object start; pin their containing logical page for the collection. Precise roots and mapped slots are rewritten when objects move.
- Preserve the current root API behavior: caller-owned root slots remain address-stable until token removal; thread roots are popped LIFO; heap-level roots can be unregistered by token; `HeapRootedWord` owns an interior-mutable stable slot; `with_root` and `with_roots` reread slots after allocation. Any Rust `Word` held across allocation, collection, or a call that may allocate remains rooted through these APIs.
- Poll at allocation slow paths, loop backedges, and immediately before Lisp calls. Native entry publishes stack/register state; native code must root any managed value it retains. A safepoint cannot occur between a managed reference store and its barrier.
- Handler, cleanup, and catch chains remain part of the root protocol. Their live `Word` fields must be visible at safepoints; IR region metadata describes unwinding and is not a replacement root registry.

### Allocation and barrier ABI

The object-layer `ThreadContext` owns a boxed `ncl_sys::Thread`; the pinned pointer passed to generated code is that sys thread, while object-facing APIs accept `ThreadContext` (`packages/object/src/context.rs` lines 52-81). Sys registration and heap operations accept `Thread` directly. Extend `ThreadLayout` with offsets for cumulative consed bytes, allocation debt, heap base, and direct page-metadata base. Expose separate cons and header-object TLAB bump/limit pairs. The x86-64 allocation sequence uses R15 for context, R10/R11 for the selected bump/limit, and RAX for end/result. AArch64 uses x21 for context, x16/x17 for the selected bump/limit, and x0 for end/result. The checked slow path refills the selected TLAB, allocates in large-object space, or collects; its ABI roots all live `Word` values through the safepoint map.

Every managed reference store writes the slot and then calls the leaf `native_write_barrier(Thread*, slot_index, owner)` shim. The slot index is in machine words from the object's base. The C-ABI shim receives context/slot/owner in RDI/RSI/RDX on x86-64 System V and x0/x1/x2 on AArch64; it calls the typed sys API as `write_barrier(thread, owner, slot_index)`. The helper reads the heap base and direct page metadata, then marks the card containing the owner slot using the 512-byte card size. It is nonallocating, nonblocking, and contains no safepoint, global heap mutex, or hash lookup. Card bits are atomic so concurrent mutators may mark a card safely. False-positive marks are allowed; missing an old-to-young edge is not. Minor collection scans marked cards in old-generation and large-object pages. Raw non-managed slots do not call this helper, and every codegen and object setter uses the same contract.

### Weak references, finalizers, movement, and Lisp API

- Weak pointers, weak vectors, and weak hash-table entries are not strong roots. After tracing, clear a weak target that is otherwise unreachable. Support the existing weakness names `:key`, `:value`, `:key-and-value`, and `:key-or-value`.
- Queue a finalizable object once when it first becomes otherwise unreachable. Keep the target and callback strongly rooted until its callback returns or signals. Drain the queue FIFO after mutators resume, then run `NCL-GC:*AFTER-GC-HOOKS*` in list order on the collection-initiating Lisp thread. Neither finalizers nor hooks run under the heap lock or during stop-the-world. Callback conditions use the normal condition system and are not swallowed. Mark a finalizer invoked before calling it; if it signals, do not retry it, keep the uninvoked queue suffix rooted for the next dispatch, and skip hooks for this dispatch. If a hook signals, stop that hook pass; the persistent hook list starts from its first element after the next collection.
- Forward all strong object slots before releasing pages. Then rehash EQ/EQL hash tables whose keys moved, before mutators resume. The `ncl-lib-hash-arrays` feature context owns the rehash algorithm and registers a relocation callback through the typed object boundary; `ncl-sys` does not depend on the feature crate. Rehash uses existing table capacity and preallocated scratch only, performs no managed allocation, and updates weak entries after their reachability decision. Any capacity growth is deferred until normal mutator execution.
- Keep the GC Lisp facade in the existing `NCL-GC` package. Define each listed function in its function namespace with these contracts; initialize the special variables in their value namespace as specified below:

| Function | Contract |
|---|---|
| `GC()` | Request a full collection and return NIL. |
| `CANCEL-FINALIZATION(object)` | Remove a pending finalizer; return true only when one was removed. |
| `FINALIZE(object, callback)` | Register a one-shot callback that receives the object after collection resumes. |
| `GENERATION-BYTES-ALLOCATED(generation)` | Return this thread's cumulative aligned object bytes allocated directly into generation 0, 1, or 2, including headers; promotions and collector copies do not count. |
| `HASH-TABLE-WEAKNESS(table)` | Return NIL for a strong table or its weakness keyword. |
| `MAKE-WEAK-POINTER(value, weakness)` | Create a weak pointer with the selected weakness policy. |
| `MAKE-WEAK-VECTOR(length)` | Create a weak vector of `length` NIL-initialized slots. |
| `WEAK-POINTER-P(value)` / `WEAK-VECTOR-P(value)` | Return true when `value` has the named type. |
| `WEAK-POINTER-VALUE(pointer)` | Return the live target or NIL after clearing. |
| `GET-BYTES-CONSED()` | Return this thread's cumulative allocated object bytes after object-size alignment, including headers but excluding unused TLAB tail, collector copies, and metadata. Never reset at GC. |
| `HEAP-DUMP(stream)` | Write the version 1 dump described below and return NIL. |

Allocation debt is a separate counter reset at collection. Initialize the special variable `*AFTER-GC-HOOKS*` to NIL. Initialize `*GC-REAL-TIME*` and `*GC-RUN-TIME*` to zero nanoseconds; after each collection, set them to that collection's pause and collector durations. Define every listed function in the function namespace; no listed function may have an `UNBOUND` function cell.
- `COMMON-LISP:ROOM` reports committed and free dynamic bytes, bytes by generation, large-object bytes, pinned-page bytes, and the last collection's reclaimed bytes. Runtime owns this function.
- `--gc-log` writes one JSON Lines record to stderr after each collection. Version 1 uses integer nanoseconds for `pause_ns` and `collector_ns`, integer bytes for `*_bytes`, and integer logical-page counts for `pages_committed` and `pages_released`. Its fields are `format` (`ncl-gc-log`), `version` (`1`), `kind` (`minor` or `full`), `cause` (`allocation-debt`, `capacity`, `manual`, or `stress`), `pause_ns`, `collector_ns`, `used_before_bytes`, `used_after_bytes`, `reclaimed_bytes`, `moved_bytes`, `pinned_bytes`, `pages_committed`, and `pages_released`. `*GC-REAL-TIME*` is the latest `pause_ns`; `*GC-RUN-TIME*` is the latest `collector_ns`; both start at zero and use monotonic elapsed time.
- `NCL-GC:HEAP-DUMP(stream)` writes JSON Lines with header `{"format":"ncl-heap-dump","version":1}`. Page records contain `type` (`page`), per-dump integer `id`, `kind` (`cons`, `header-object`, `large-object`, or `static`), `generation` (0-2 or null for static pages), integer `bytes`, and boolean `pinned` (pinned by conservative reachability in the latest collection). Object records contain `type` (`object`), per-dump integer `id`, integer `page_id`, numeric allocation `tag`, integer `bytes`, generation 0-2, boolean `pinned`, `edges` (an array of strong target IDs), `weak` (an array of `{slot: integer, target_id: integer|null}` entries), and boolean `finalizer` (whether finalization is pending). IDs are assigned by page index and object-start order and are stable only within that dump. Code space is excluded. Do not emit raw object payloads or process addresses. Stop the world only to capture the metadata snapshot; serialize after mutators resume.

### Heap verification and stress modes

- `--gc-stress` forces a full collection before every managed allocation, matching the existing `ThreadContext::set_gc_stress` behavior. It exercises the same allocation, root-update, weak processing, rehash, and release paths as ordinary collection.
- `--heap-verify` verifies after every collection and before page reuse or OS release: page extents are aligned and nonoverlapping; page metadata matches object-start and size maps; object layouts and reference slots are in bounds; all precise roots and strong edges resolve to live objects; forwarding chains terminate; no page is both free and pinned/allocated; dirty cards cover every old-to-young edge; and weak/finalizer states match reachability.
- V5 must run allocation-by-allocation GC stress, heap verification, multithread stress, signal torture, ASan, TSan, and Miri. Require zero crashes and zero invariant violations. The 24-hour soak uses the representative workload fixed by L0-3. Before the soak, measure its peak RSS in five baseline runs and set the RSS cap to the largest baseline peak plus 10 percent. Require post-full-GC live bytes to remain stable and zero crashes during the soak.
- Do not claim a speedup from this contract. L3-7 checks generated allocation size/sequence and measures the affected loops; inspect the emitted code as well as benchmark results.

### Public interface and frame map

The object facade accepts `ThreadContext` and `Runtime`; it owns the boxed sys thread and registers it with the heap. The sys API accepts `Thread` and `Heap` directly. The machine-visible pointer is the stable pointer to the boxed `Thread`, not the object-layer wrapper.

```text
// ncl-object facade
alloc(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
alloc_large(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
push_root(&mut ThreadContext, &mut Word) -> RootToken
pop_root(&mut ThreadContext, RootToken)

// ncl-sys collector interface
write_barrier(&mut Thread, owner: Word, slot_index: usize)
poll_safepoint(&mut Thread)
enter_native(&mut Thread)
leave_native(&mut Thread)
register_thread(&Heap, &mut Thread) -> Result<(), StorageCondition>
unregister_thread(&Heap, &Thread)
register_layout(&Heap, u8, ReferenceLayout) -> Result<(), LayoutError>
alloc_code(bytes) -> CodePtr
free_code(CodePtr)
publish_code(CodePtr) // RW -> RX
```

Code allocation remains nonmoving and separate from dynamic heap limits. Publish code only after constants, relocations, maps, and debug records are installed; release it only after registered frame metadata shows no live return PC in the range.

A native `SafepointMap` retains its 16-byte little-endian header: `pc_offset:u32`, `frame_words:u16`, `slot_words:u16`, `word_slot_count:u16`, `register_mask:u16`, and `map_flags:u32`, followed by a slot bitmap and `u16` register IDs. The bitmap assigns bits 0..3 to header words 0..3, with bit 2 always one and the other header bits always zero; header word 2 is the precise function-object root. Bit 4 is the first local, followed by local and outgoing slots. Only registers that may contain a `Word` are listed. Return PCs point into nonmoving code and are not forwarded. `map_flags` bit 0 is call, bit 1 loop-backedge, bit 2 allocation-slow, and bit 3 has-derived-address. Register IDs and flags follow [Native backend](native-backend.md).

PC lookup binary-searches code-relative map offsets. Frame walking reads the four-word header, forwards the function object in header word 2 and live slots/registers, leaves the return PC unchanged, then follows previous FP. Native frames use the conservative boundary recorded by `enter_native`; JIT frames use precise maps.

### Layout registration and closures

`Heap` owns the GC epoch and registered `ReferenceLayout` values. `register_layout(widetag, layout)` accepts header-inclusive raw indices in `reference_words`; `boxed_from`, when present, means every word from that index to the object end is boxed. Re-registering an identical layout succeeds without change; a different layout for the same widetag returns `LayoutError` and does not replace the original. The object layer converts payload-relative offsets with `reference_words()` before registration (`packages/sys/src/heap_types.rs`, `ReferenceLayout`; `packages/object/src/layout.rs`, `reference_words`).

For an IR v2 closure, fixed references are payload `name`, `lambda-list`, and `code` (raw indices 2, 3, and 4 after the header). `boxed_from` starts at raw index 5, the first capture, and extends to the object end. The collector discovers, forwards, and writes back inline captures during the same scan. `entry` is a fixnum-encoded offset into nonmoving code space, so it is scalar metadata and is never treated as a heap reference. The closure therefore needs no separate capture-count metadata word.

### Migration order

1. Freeze the typed allocation, root, safepoint, page metadata, and barrier interfaces in this document. Keep existing root tokens, `HeapRootedWord`, `with_root(s)`, frame maps, and stop-the-world epoch semantics covered by their current tests.
2. Add the reserved page range, direct page/object metadata, large-object extents, OS decommit, and checked `--dynamic-space-size` parsing. Preserve the current typed `StorageCondition` boundary; after l02d-cond's condition mapping lands, translate final capacity exhaustion to catchable `storage-condition` after one minor and one full collection.
3. Implement a real TLAB refill and the large-object slow path. Wire `AllocateSlow`, initialize `ThreadContext` page/TLAB fields, and count inline and slow allocations separately from collection debt.
4. Replace the per-object collector with page tracing, pinning, movement, weak processing, and reclamation while preserving root slots and safepoint maps. Forward all roots and strong slots before clearing or decommitting source pages.
5. Install direct page metadata card marking and convert every managed reference setter. This is the stage that removes the global mutex and object-start hash lookup from mutator word reads, writes, and barriers. Allocation slow paths and the stop-the-world coordinator may still synchronize.
6. Add relocation callbacks, in-place EQ/EQL rehash, Lisp weak/finalizer/hook APIs, `GET-BYTES-CONSED`, `ROOM`, logs, metrics, and heap dump. Run callbacks only after the collector has made the heap consistent.
7. Add `--heap-verify` and `--gc-stress`, then complete V5 and the 24-hour soak before calling L1-4 complete.

### L1-4 implementation slices and dependencies

The L1-4 epic coordinator is the single integration owner for shared ABI files: `packages/sys/src/heap.rs`, `packages/sys/src/heap_state.rs`, `packages/sys/src/heap_types.rs`, `packages/sys/src/thread.rs`, `packages/sys/src/thread/layout.rs`, and `packages/sys/src/lib.rs`. Child slices use new modules or their exclusive files below and request shared-interface changes from that owner. No two slices edit these shared files concurrently.

| Slice | Owned paths | Depends on | Completion evidence |
|---|---|---|---|
| Heap pages and allocator | `packages/sys/src/heap/allocation.rs`, new `packages/sys/src/heap/pages.rs`, new `packages/sys/tests/heap_pages.rs` | L0-2 contract; shared ABI owner | TLAB refill and large allocation; page reuse/decommit tests; capped exhaustion returns typed capacity error |
| Collector and roots | `packages/sys/src/heap/collect.rs`, `packages/sys/src/heap/scan.rs`, `packages/sys/src/heap/roots.rs`, `packages/sys/src/stw.rs`, new `packages/sys/tests/gc_collect.rs` | Heap page metadata interface | Moving roots/frames and page pin tests; minor/full reachability checks; no release before forwarding |
| Barrier | new `packages/sys/src/heap/barrier.rs`, `packages/runtime/src/support/write_barrier.rs`, `packages/codegen/src/target_x86_64_lowering/ops.rs`, `packages/codegen/src/target_x86_64_lowering/primitives.rs`, `packages/codegen/src/target_aarch64_lowering/ops.rs`, `packages/codegen/src/target_aarch64_lowering/primitives.rs` | Page metadata and ABI; heap core | Generated store-before-barrier checks on both targets; old-to-young references survive minor GC; helper has no lock/hash/allocation |
| Weak API and finalizers | `packages/sys/src/heap/collect/weak_mark.rs`; `packages/object/src/gc.rs` only after L1-10 selects its final feature location | Collector reachability; L1-10 before object API moves; l02d-cond for callback conditions | Each weakness mode, clear-on-death, one-shot finalization, rooted callback state, hooks run after resume |
| EQ/EQL rehash | `packages/lib/hash-arrays/src/`, new `packages/lib/hash-arrays/tests/gc_rehash.rs` | Relocation callback ABI; weak-processing order; L1-10 | Moved EQ/EQL keys remain findable; weak entries remain correct; no managed allocation during rehash |
| Verify and stress | new `packages/sys/src/heap/verify.rs`, new `packages/sys/tests/gc_verify.rs`, new `packages/sys/tests/gc_stress.rs` | Page metadata and collector invariants | `--gc-stress`, `--heap-verify`, V5 zero crashes/violations, then soak/RSS cap from L0-3 |
| Runtime integration and observability | `src/main.rs`, `packages/runtime/src/error.rs`, new `packages/runtime/src/room.rs`, new `packages/runtime/src/gc_diagnostics.rs`; exclude `packages/runtime/src/support/write_barrier.rs` | HeapConfig API; l02d-cond; collector metrics | CLI parsing rejects invalid sizes; handler-case catches exhaustion; `ROOM`, GC log, metrics, and dump report the defined fields |

L1-4 depends on L0-2. Its sys slices can proceed against this frozen contract. Any API implementation in `ncl-object` waits until L1-10 completes its kernel/feature migration, and final symbol ownership follows `docs/src/design/crates.md`; do not create a new crate in this lane. L1-4 completes only after V5 and the soak evidence are recorded.

L4-1 depends on L1-4 and L1-2. It owns `packages/threads/` and the thread-facing feature paths, not the shared heap ABI. Each Lisp thread has one registered context and lazy 32 KiB TLABs for cons and header-object pages, plus roots, safepoint state, and allocation counters. It must test stop-the-world rendezvous, registration/unregistration, and multithread stress against the frozen heap API.

L3-7 depends on L3-2 and L1-4. It owns allocation transformations in `packages/opt/`, allocation lowering in `packages/codegen/src/target_x86_64_lowering/runtime_ops.rs` and `packages/codegen/src/target_aarch64_lowering.rs`, and the new `packages/codegen/tests/allocation_codegen.rs`; it does not edit the page layout or barrier helper. It verifies the fast-path size and instruction contract, slow-path maps, and generated allocation-byte counts.


