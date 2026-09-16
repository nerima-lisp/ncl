# ncl-sys

`ncl-sys` is the unsafe boundary for the NCL runtime. Other crates use its
tagged words, heap, precise roots, safepoints, and platform surface.

## Invariants

- `Word` is one 64-bit tagged value. Fixnums have bit 0 clear, and pointers
  are eight-byte aligned.
- Cons objects contain two payload words and have the list lowtag. Header
  objects reserve word 0 for the widetag and use registered layouts for scans.
- Nursery objects are copied on collection when they are not pinned. A
  surviving object advances through aging to old after two collections.
- Large objects (8 KiB and above) are allocated directly in old generation.
- Rust values live across allocation only through `RootToken` or a registered
  root set. Conservative scanning is a safety net, not a replacement.
- A write barrier records stores from old objects in the remembered set.
- Weak values are cleared after reachability is computed. Finalizers are
  queued once and run by `run_pending_finalizers` after collection releases its heap lock.
- Native frames use a four-word header and safepoint maps use the fixed
  16-byte little-endian header followed by bitmap and `u16` register IDs.
- Fields of `Thread` read by generated code are dedicated eight-byte words at
  eight-byte-aligned offsets. Generated code must not read Rust enum or `bool`
  representations directly. The `safepoint_request` word is zero when no poll
  is requested and nonzero when a poll is requested. `request_safepoint` sets
  it while publishing the Rust safepoint state, and `poll_safepoint` clears it
  when the request is consumed.

## Platform scope

The handwritten OS declarations are cfg-gated in `src/os.rs`. Code space uses
page-backed mappings, writes code while writable, then publishes it. macOS
arm64 uses `MAP_JIT`, `pthread_jit_write_protect_np`, and
`sys_icache_invalidate`; Linux allocates RW pages and uses `mprotect` to RX.
The machine-code smoke test has AArch64 and x86-64 byte sequences. The current
host run executes the AArch64 branch; the x86-64 branch is cfg-gated and not
executed in this run.

## Safety, errors, and panics

Unsafe code is confined to this crate. Each unsafe block documents the pointer
or register invariant it relies on. Allocation reports `StorageCondition` for
invalid sizes, unregistered threads, and dynamic-space exhaustion. Collection
does not intentionally panic; poisoned internal mutexes are recovered to keep
the runtime from losing its heap state.
