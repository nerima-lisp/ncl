# GC interface

## 決定

heap、code space、TLAB、shadow root、binding stack、multiple-value area、handler/cleanup/catch の現在ポインタ、stack boundary、safepoint/native state は `ncl-sys` が所有する。`ncl-object` はこの状態を型付き API で包む。Rust builtin が allocation point を跨いで保持する `Word` は `RootToken` または typed handle で明示的に root 化する。

協調 poll の状態遷移は `Running -> PollRequested -> Published -> Collecting -> Running` とする。`Published` で thread は stack bounds と callee-saved register snapshot を `ThreadContext` に公開する。`enter_native` 後は GC 完了まで `Safe` 状態であり、未登録 Rust local の保護を保証しない。保守的 stack/register scan は安全網であり、RootToken の代替ではない。

保守的候補は page table で heap page を特定し、object start reverse lookup を通過した値だけを root として採用する。pin は page attribute とし、cons-only page と header-object page は分離する。weak pointer は強 root ではなく、対象が到達不能になった後に一度だけ finalizer queue へ移す。queue と callback の pending state は GC 完了まで強く保持する。

API は次のとおりである。

```text
alloc(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
alloc_large(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
alloc_code(bytes) -> CodePtr
free_code(CodePtr)
publish_code(CodePtr) // RW -> RX
push_root(&mut ThreadContext, &mut Word) -> RootToken
pop_root(&mut ThreadContext, RootToken)
write_barrier(&mut ThreadContext, Word, Slot)
poll_safepoint(&mut ThreadContext)
enter_native(&mut ThreadContext)
leave_native(&mut ThreadContext)
register_thread(&Heap, &mut Thread) -> Result<(), StorageCondition>
unregister_thread(&Heap, &Thread)
register(&Runtime)
register_layout(&Heap, u8, ReferenceLayout) -> Result<(), LayoutError>
```

`alloc_code` and `free_code` manage non-moving code space. `publish_code` changes a
constructed range from writable to executable only after constants, relocations,
maps, and debug records are installed. Thread and layout registration use the
names and ownership shown here; a lower layer must not invent a second registry.

Native code の `SafepointMap` は 16-byte little-endian header (`pc_offset: u32`, `frame_words: u16`, `slot_words: u16`, `word_slot_count: u16`, `register_mask: u16`, `map_flags: u32`) に続く slot bitmap と `u16` register id 列である。列挙するのは `Word` を持ちうるレジスタだけで、整数、浮動小数点、raw address 専用レジスタは列挙しない。slot bitmap は bit 0..3 を header 語 0..3 に割り当て、header 語 2 は常に 1、他の header 語は常に 0 とする。bit 4 が最初の local slot である。return PC は非移動 code space を指すため更新しない。register id と flags の定義は [Native backend](native-backend.md) に従う。

## 根拠

明示 root は Rust compiler がレジスタ専有した local の寿命を collector に伝えられない問題を解消する。協調 poll と公開済み snapshot は停止要求中の thread を観測可能にし、候補検証と page 分離は conservative scan の false positive を抑える。

## 却下した代替案

- Rust local を暗黙に root とみなす案は register allocation を観測できないため却下する。
- 全 heap を conservative scan する案は false positive を forwarding 対象にし得るため却下する。
- weak object 自体を強 root にする案は到達不能判定を壊すため却下する。
- stack map を encoder ごとの私有形式にする案は collector と metadata の不一致を招くため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- `RootToken`、16-byte map header、slot bitmap、register id 列、page pin、poll 状態を変更しない。
- allocation crossing の `Word` を token/typed handle なしで保持しない。
- safepoint、write barrier、native transition を省略して green test を作らない。

## Collection and root protocol

The three generations are nursery (0), aging (1), and old (2). A nursery survivor is promoted after two collections or when its object size exceeds 8 KiB. Each mutator receives a 32 KiB TLAB; objects of at least 8 KiB use large-object space. The remembered-set card size is 512 bytes. Safepoints occur at allocation slow paths, loop backedges, and immediately before Lisp calls.

Stop-the-world collection issues an epoch, each mutator publishes its snapshot and enters `Published` or `Safe`, the last required thread enters `Collecting`, the collector scans and moves objects, then release returns all threads to `Running`. `--dynamic-space-size` bounds the heap and failure is reported as `storage-condition`. `sb-ext:gc` requests the same protocol; `*after-gc-hooks*` run after release, and `bytes-consed-between-gcs` updates the allocation threshold.

Conservative scanning polls first, obtains stack bounds, saves callee-saved registers, and scans only the published interval. On macOS use `pthread_get_stackaddr_np` and `pthread_get_stacksize_np`; on Linux use `pthread_getattr_np`. Candidate words must pass page-table membership and object-start reverse lookup. Pages are pinned while examined. `RootToken` is the standard path for live Rust values.

Weak API is `make_weak(value, weakness)`, `weak_value(weak)`, and `register_finalizer(object, callback)`. The four weakness names are `:key`, `:value`, `:key-and-value`, and `:key-or-value`. A weak target is cleared when otherwise unreachable; a finalizer is queued once, and queue/callback state remains strongly held until completion. Non-moving code space has independent allocation and release.

## SafepointMap wire format

The 16-byte little-endian header is `pc_offset:u32`, `frame_words:u16`, `slot_words:u16`, `word_slot_count:u16`, `register_mask:u16`, and `map_flags:u32`. Bitmap bits 0..3 denote header words 0..3, with bit 2 always one and the other header bits always zero; bit 4 denotes the first local, followed by local and outgoing slots. The function object in header word 2 is a precise moving-heap root. Return PC is not updated because code space is non-moving. Register ids are `u16` and list only registers that may contain `Word`. `map_flags` bit 0 is call, bit 1 loop-backedge, bit 2 allocation-slow, and bit 3 has-derived-address.

PC lookup binary-searches code-relative map offsets. Frame walking reads the four-word header, forwards the function object in word 2 and live slots/registers, leaves the return PC unchanged, then follows previous FP. Native frames use the conservative boundary recorded by `enter_native`; JIT frames use precise maps.

## Phase 1 registration and release contract

`Heap` owns the current GC epoch and the registered `ReferenceLayout` values. `register_layout(widetag, layout)` accepts header-inclusive raw indices in `reference_words`; `boxed_from`, when present, means every word from that index to the object end is boxed. A widetag registration is not silently replaced. The object layer therefore converts payload-relative offsets with `reference_words()` before registration. (`packages/sys/src/heap_types.rs`, `ReferenceLayout`; `packages/object/src/layout.rs`, `reference_words`.)

Heap-level registry roots are address-stable slots. `push_heap_root` stores the address of a caller-owned `Word` and returns a LIFO `RootToken`; `pop_heap_root` removes only the most recent matching slot. A managed `Word` retained across allocation or collection must use this API or the corresponding thread root. (`packages/sys/src/lib.rs`, `push_heap_root`, `pop_heap_root`.)

Code allocation is non-moving. A code range is writable while it is constructed, and `publish_code` makes it executable only after its bytes and metadata are installed. `release_code` removes or quiesces the published PC range by scanning registered frame metadata before the range can be unmapped. A published return PC is therefore not forwarded by the moving heap. (`packages/sys/src/code.rs`, `publish_code`; `packages/sys/src/heap.rs`, `release_code`.)

The collector increments the heap epoch for a collection and exposes that epoch to registered mutators. Native frame scanning uses the registered `SafepointMap`: the function object in header word 2 and live slots/registers are forwarded, while the return PC is left unchanged because it points into non-moving code. (`packages/sys/src/heap_state.rs`, `gc_epoch`; `packages/sys/src/code.rs`, `scan_frame_chain_with_registry`.)

Object-layer allocation functions root managed arguments received by value and each element of a `&[Word]` before allocating, then re-read those values from the rooted slots after allocation. (`packages/object/src/roots.rs`, `with_root`, `with_roots`; `packages/object/src/lib.rs`.)

`ThreadContext::set_gc_stress` and `Runtime::set_gc_stress` select a test-only mode that forces `collect(true)` on every allocation. The sys heap resolves stale addresses through forwarding, so a stale word cannot be detected, and the basis of correctness rests on static audit. (`packages/object/src/lib.rs`, `set_gc_stress`; `packages/object/src/registry_extensions.rs`, `Runtime::set_gc_stress`; `packages/object/tests/gc_stress.rs`.)
