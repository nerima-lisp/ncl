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
  one-shot callbacks drained after the collector releases its heap lock.
- Native frames use a four-word header and safepoint maps use the fixed
  16-byte little-endian header followed by bitmap and `u16` register IDs.

## Platform scope

The handwritten OS declarations are cfg-gated in `src/os.rs`. The current
runtime tests execute on the host target only. Linux-specific declarations are
compiled but not exercised by the macOS arm64 lane, and macOS JIT permission
transitions are not exercised in this worktree because `CodePtr` still owns a
safe byte buffer rather than an executable mapping.

## Safety, errors, and panics

Unsafe code is confined to this crate. Each unsafe block documents the pointer
or register invariant it relies on. Allocation reports `StorageCondition` for
invalid sizes, unregistered threads, and dynamic-space exhaustion. Collection
does not intentionally panic; poisoned internal mutexes are recovered to keep
the runtime from losing its heap state.
