# GC interface

## 決定

NCL は共有ヒープ、OS thread、stop-the-world safepoint を使う精密・世代別・移動 GC である。世代は nursery (0)、aging (1)、old (2) の 3 つだけで、pin はページ属性である。nursery survivor は 2 回の collection または 8 KiB 以上で昇格する。静的領域は起動時に固定し、GC が走査するが移動しない。

ヒープに触る API の第一引数は常に &mut ThreadContext、共有状態は &Runtime とする。Runtime は Sync な共有状態で package table、function registry、class table、GC settings を持つ。ThreadContext は TLAB、binding stack、handler chain、registered roots、safepoint state、multiple-value 領域を持つ。

API の契約は次の通りである。

    Word alloc(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
    Word alloc_large(&mut ThreadContext, &Runtime, TypeTag, usize) -> Result<Word, StorageCondition>
    RootToken push_root(&mut ThreadContext, &mut Word)
    void pop_root(&mut ThreadContext, RootToken)
    void write_barrier(&mut ThreadContext, Word object, Slot slot)
    void register_thread(&Runtime, &mut ThreadContext)
    void unregister_thread(&Runtime, &mut ThreadContext)
    void poll_safepoint(&mut ThreadContext)
    void enter_native(&mut ThreadContext)
    void leave_native(&mut ThreadContext)

各 thread は 32 KiB TLAB を持ち、8 KiB 以上は large-object space に直接割り当てる。allocation slow path、後方分岐、Lisp call の直前に safepoint を置く。世代間 store は 512-byte card を dirty にし、write_barrier を通った slot を remembered set に追加する。

Rust 側の Vec<Word>、HashMap<_, Word>、static、Box 内に GC-managed Word を置いてはならない。例外は push_root 済み slot、または Runtime に root 集合として登録した Rust 構造体だけである。package/function/class table は heap object または登録 root とする。

Rust builtin/FFI が I/O、sleep、外部関数でブロックする場合、前後に enter_native/leave_native を呼ぶ。native 中の thread は safe state として保守的に stack/register を走査され、GC の待ちを妨げない。通常の長寿命参照は RootToken を使う。

Cranelift 0.134.3 の user stack map は producer が live slot を spill し、declare_needs_stack_map と entries を safepoint に付与する契約である。非 tail call が safepoint であり、collector は stack map、registered roots、native 中の保守走査で roots を更新する。

## 根拠

&mut RuntimeHandle は共有 heap 上の複数 OS thread の同時割当を Rust の型で表せない。TLAB と &mut ThreadContext なら mutator ごとの排他的状態を表し、共有 Runtime は内部同期で扱える。native transition はブロック中の thread を safe state にして STW の永久待ちを防ぐ。

## 却下した代替案

- Runtime の mutable handle を全 thread の割当入口にする案は却下した。
- pin を第 4 世代にする案は却下した。pin はページ属性である。
- 全参照を handle にする案は root 操作を過剰に増やすため主方式にしない。
- non-moving GC と call だけの safepoint は fragmentation と停止不能 loop を招くため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- API の第一引数、3 世代、32 KiB TLAB、8 KiB 閾値、512-byte card を変更しない。
- 未登録 Rust memory に Word を保持せず、field store は write barrier を通す。
- blocking call は native transition を対にし、user stack map の live slot を producer が明示する。
