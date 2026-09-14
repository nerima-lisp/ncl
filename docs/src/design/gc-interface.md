# GC interface

## 決定

heap、code space、TLAB、shadow root、binding stack、multiple-value area、handler/cleanup/catch の現在ポインタ、stack boundary、safepoint/native state は `ncl-sys` が所有する。`ncl-object` はこの状態を型付き API で包む。Rust builtin が allocation point を跨いで保持する `Word` は `RootToken` または typed handle で明示的に root 化する。

協調 poll の状態遷移は `Running -> PollRequested -> Published -> Collecting -> Running` とする。`Published` で thread は stack bounds と callee-saved register snapshot を `ThreadContext` に公開する。`enter_native` 後は GC 完了まで `Safe` 状態であり、未登録 Rust local の保護を保証しない。保守的 stack/register scan は安全網であり、RootToken の代替ではない。

保守的候補は page table で heap page を特定し、object start reverse lookup を通過した値だけを root として採用する。pin は page attribute とし、cons-only page と header-object page は分離する。weak pointer は強 root ではなく、対象が到達不能になった後に一度だけ finalizer queue へ移す。queue と callback の pending state は GC 完了まで強く保持する。

API は次のとおりである。

```text
alloc(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
alloc_large(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
push_root(&mut ThreadContext, &mut Word) -> RootToken
pop_root(&mut ThreadContext, RootToken)
write_barrier(&mut ThreadContext, Word, Slot)
poll_safepoint(&mut ThreadContext)
enter_native(&mut ThreadContext)
leave_native(&mut ThreadContext)
```

Native code の `SafepointMap` は 16-byte little-endian header (`pc_offset: u32`, `frame_words: u16`, `slot_words: u16`, `word_slot_count: u16`, `register_mask: u16`, `map_flags: u32`) に続く slot bitmap と register id 列である。slot bit 0 は frame header 語 0、bit 8 は最初の local slot とし、header 0..7 は root にしない。register id と flags の定義は [Native backend](native-backend.md) に従う。

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

Weak API is `make_weak(value)`, `weak_value(weak)`, and `register_finalizer(object, callback)`. A weak target is cleared when otherwise unreachable; a finalizer is queued once, and queue/callback state remains strongly held until completion. Non-moving code space has independent allocation and release.

## SafepointMap wire format

The 16-byte little-endian header is `pc_offset:u32`, `frame_words:u16`, `slot_words:u16`, `word_slot_count:u16`, `register_mask:u16`, and `map_flags:u32`. Bitmap bit 0 denotes header word 0, bit 8 denotes the first local, and later bits denote local or outgoing slots. Header words are metadata and are not roots. `map_flags` bit 0 is call, bit 1 loop-backedge, bit 2 allocation-slow, and bit 3 has-derived-address.

PC lookup binary-searches code-relative map offsets. Frame walking reads the four-word header, uses the function object to find the code object, finds the map, updates live slots and registers through forwarding, then follows previous FP. Native frames use the conservative boundary recorded by `enter_native`; JIT frames use precise maps.
