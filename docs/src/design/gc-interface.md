# GC interface

## 決定

NCL は共有ヒープ上の精密・世代別・移動 GC とする。世代は nursery (0)、aging (1)、old (2)、pinned (3) の 4 つ。nursery は minor collection、old は major collection で処理する。nursery survivor は 2 回の collection、または 8 KiB を超える survivor で昇格する。old の major collection は mark, weak processing, compact/copy, sweep の順である。

`ncl-sys` は次の API を所有する。

```text
Word alloc(RuntimeHandle, TypeTag, usize) -> Result<HeapPtr, StorageCondition>
Word alloc_large(RuntimeHandle, TypeTag, usize) -> Result<HeapPtr, StorageCondition>
RootToken push_root(ThreadHandle, *mut Word)
void pop_root(ThreadHandle, RootToken)
void register_thread(ThreadHandle)
void unregister_thread(ThreadHandle)
void poll_safepoint(ThreadHandle)
```

各 OS thread は 32 KiB TLAB を持つ。8 KiB 以上は large-object space に直接割り当て、large object は old 扱いとする。割り当て slow path、後方分岐、Lisp call の直前に safepoint を置く。各 thread は `registered`, `at_safepoint`, `parked`, `unregistering` の状態を持つ。

Lisp フレームは Cranelift user stack map に live `Word` の stack slot を登録する。Cranelift の現行契約では非 tail call が safepoint であり、producer が `declare_needs_stack_map` と stack-map entries を付与する。Rust builtin は native frame として登録され、collector は stop-the-world 中に各 native stack と保存レジスタを保守的に走査する。tag `011` で heap page を指す候補は pinned 扱いにし、指された page 内のオブジェクトはその cycle の移動対象から除外する。これが Rust frame の raw word を allocation point 越しに保持するための契約である。

Rust builtin が複数 allocation point を越えて値を保持する場合は `RootToken` による shadow root を使う。stack scanning は互換性と人間工学のための安全網であり、複数回の allocation をまたぐ長寿命参照の標準手段にはしない。

世代間 pointer store は card table の 512-byte card を dirty にする。remembered set は dirty card のみを minor collection で走査する。forwarding pointer は header flag 5 と payload word 0 に記録し、すべての `Word` root と strong field を forwarding に更新する。

weak pointer は referent の forwarding/mark 状態を見て dead なら nil にする。weak hash table は key/value weakness に従い entry を削除し、finalizer は object が unreachable と判定された後に finalizer queue へ一度だけ移す。queue 自体は strong root である。

GC 開始 thread は全 registered thread に epoch を発行し、各 mutator が stack/register state を公開して parked になるまで待つ。最後の thread は collector を実行し、roots を更新後、全 thread の epoch を release して再開する。thread が safepoint に到達しない場合は `interrupt-thread` の ncl-sys primitive が poll flag を設定する。

`--dynamic-space-size` は heap 上限 bytes として起動時に固定する。TLAB refill、large allocation、GC 後に空きが不足すれば `storage-condition` を生成する。`sb-ext:gc` は full collection を同期実行し、`*after-gc-hooks*` は全 thread の hooks を collector thread が順に呼ぶ。`bytes-consed-between-gcs` は次回 collection の nursery threshold として扱う。

## 根拠

Rust builtin 全体を明示的 handle API にする案 (a) は安全だが、数百の builtin と多値処理で root の付け忘れを増やす。SBCL 型の conservative native-stack scan と pinning (b) は native code と moving GC の境界を小さくし、Lisp frame だけ精密にできる。Cranelift の user stack maps が Lisp 側の精密 root を支えるため、b を採用する。TLAB と card size は固定値にして lane 間の allocation protocol を曖昧にしない。

## 却下した代替案

- 全参照を GC handle にする案は builtin の API と性能上の負担が大きいため主方式にしない。
- non-moving mark-sweep は fragmentation と compact object layout の利点を失うため却下した。
- shared-nothing heap は決定済みの共有 heap、OS thread、STW 契約に反するため却下した。
- safepoint を call だけに限定する案は allocation-free loop を停止できないため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- `Word` は allocation point を越えて安定とは限らない。Rust builtin は `RootToken` または登録 native frame を使う。
- object pointer を Rust reference として保持してはならない。移動後は accessor で再取得する。
- 全 thread は entry 前に登録し、離脱前に unregister する。
- 各 write barrier、weak/finalizer hook、safepoint API の契約を bypass してはならない。
- collector の世代数、昇格回数、TLAB size、card size は lane 側で変更しない。
