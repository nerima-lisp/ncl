# Threads

## 決定

NCL は 1 OS thread を 1 Lisp thread として登録する。`ThreadContext` は TLAB、shadow roots、binding stack、multiple values、handler/cleanup/catch pointers、stack bounds、safepoint epoch、register snapshot、native state、interrupt flags、deadline、wait state を持つ。これらと共有 heap は `ncl-sys` が所有する。

symbol の `tls_index: u32` は 1 語の thread override slot を指す。value cell は global default、TLS slot は thread override を保持し、binding entry は `(old_value, tls_index)` の 2 語とする。bind は現在値を binding stack に保存して TLS slot を新しい値へ設定し、unbind は保存値を復元して entry を pop する。mutex、condition variable、semaphore、waitqueue は `ncl-sys` の OS wrapper を使う。blocking 前に poll と interrupt/deadline 検査を行い、`enter_native` から `leave_native` を対にする。

`interrupt-thread` は対象の interrupt flag と safepoint request bit を設定し、waitqueue を wake する。Lisp callback は safepoint または native transition 復帰時だけ実行する。deadline は monotonic nanoseconds の絶対値で、timeout は pending non-local exit として cleanup 後に報告する。

## 根拠

mutator 固有状態を ThreadContext に閉じ込めることで共有 Runtime と root publication の境界を固定できる。absolute monotonic deadline は wall clock の変更に依存しない。async signal handler から Lisp を呼ばないことで OS の安全条件を守る。

## 却下した代替案

- global binding mutex は special access を直列化するため却下する。
- green thread のみの実装は OS blocking と native transition を表現できないため却下する。
- 任意の native instruction を割り込む設計は async-signal-safety に反するため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- TLS slot は 1 語、binding entry は 2 語、blocking call は native transition とする。
- thread の登録、離脱、poll、root publication を独自の global state に置かない。
- mutex 保持中に GC request を待つ実装を追加しない。
- 同一 OS thread 上に複数の登録 `Thread` がある場合、`collect` を呼ぶ側以外は native 状態でなければならない（STW は active mutator の poll を待つため、poll できない登録 Thread があると停止する）。
- `Runtime` の registry 操作は呼び出し側の `ThreadContext` で確保する（内部 ctx は持たない）。
- `ncl_sys::unregister_thread` は `Thread` の heap 参照から heap を引くため、`Runtime`（heap の所有者）は登録済みの全 `ThreadContext` より長生きしなければならない。`ThreadContext` の Drop は登録済みなら unregister する。

## Poll state machine

`Running -> PollRequested -> Published -> Collecting -> Running` is the normal request path. `Published` records stack bounds, callee-saved registers, current frame, and epoch. A thread blocked in native code is `Safe`; it contributes its registered roots and conservative boundary without delaying collection.

ThreadContext fields are thread id, native stack bounds, current frame, TLAB base/limit, allocation counter, TLS area, binding stack, handler/catch/cleanup pointers, registered roots, safepoint epoch/state, interrupt flags, deadline stack, multiple-value area, wait state, native register spill area, and pending status.

The OS wrappers expose mutex lock/unlock, condition wait/signal/broadcast, semaphore wait/post, and waitqueue park/wake. The protocol is poll, inspect interrupt and deadline, enter native, park, wake, leave native, and poll again. `interrupt-thread` atomically sets interrupt and poll-request bits and wakes a waitqueue. Delivery occurs at a safepoint or native return.

`with-deadline` pushes an absolute monotonic-nanosecond deadline. `with-timeout` derives one and installs a timeout condition. Poll, blocking waits, and builtin boundaries check it; expiration becomes a pending non-local exit after cleanup. A symbol value cell supplies the global default; each thread has one override word in TLS. Binding uses the old value and TLS index, and unbinding restores the old value before removing the entry.

## Phase 1 machine-visible layout

`ncl-sys::Thread` is `repr(C)`. The fields consumed by generated code are dedicated machine words at offsets returned by `thread_layout()`: `tlab_bump`, `tlab_limit`, `safepoint_request`, `pending`, `mv`, `handler`, `cleanup`, and `catch`. The scalar fields consumed directly by generated code are eight bytes wide and eight-byte aligned. `mv` is a Rust `Vec<Word>` descriptor and is not a generated-code scalar word. (`packages/sys/src/thread.rs`, `ThreadLayout`, `thread_layout()`.)

`SafepointState` and `NativeState` use `repr(u8)`, but this representation is not part of the generated-code ABI. Generated code reads the dedicated words only. `safepoint_request == 0` means no poll is pending; a nonzero value requests the slow path. Requesting a safepoint publishes the Rust state and sets the word, and polling consumes the request by clearing it. (`packages/sys/src/thread.rs`, `request_safepoint`, `poll_safepoint`.)

`ThreadContext` pins `Thread` in a `Box`, so moving the context after registration is safe and the address passed to `register_thread` remains stable until unregistration. (`packages/object/src/lib.rs`, `ThreadContext.thread`; `packages/sys/src/lib.rs`, `register_thread`.)

The safepoint slow path receives the pinned context in `x0`, the active generated frame pointer (`x29`) in `x1`, and the continuation PC in `x2`. A Rust callback handling that call must use only the passed frame pointer and PC and must not read its own `x29`/`x30`: a predecessor implementation snapshotted the Rust frame instead, corrupted the saved link register, and looped indefinitely, so this prohibition is part of the contract. A thread in the native state provides only a conservative snapshot; a precise frame snapshot is published only by the thread that entered the slow path. (`packages/codegen/src/target_aarch64_lowering.rs`, `lower_safepoint`.)
