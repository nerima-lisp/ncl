# Threads

## 決定

NCL-THREADS は 1 OS thread を 1 Lisp thread として登録する。`ncl-sys::Thread` は TLAB、shadow roots、handler/cleanup/catch pointers、stack bounds、safepoint epoch、register snapshot、native state、interrupt flag などの機械可視状態を持ち、`ncl-object::ThreadContext` はこれを binding stack、multiple values、pending state と組み合わせる。共有 heap と機械可視 thread state は `ncl-sys` が所有し、deadline などの API 状態は `ncl-threads` が管理する。`NCL-THREADS` の名前と API が契約の対象であり、SBCL の内部表現や動作をそのまま前提にしない。

per-thread の special value を導入する場合の NCL-THREADS 契約は、symbol の `tls_index: u32` を 1 語の thread override slot に対応させ、value cell を global default とすることである。その場合の binding entry は `(old_value, tls_index)` の 2 語で、bind は旧値を保存して override を設定し、unbind は旧値を復元して entry を pop する。これは契約上のレイアウトであり、現 Phase 1 の `ThreadContext` は `(u32, Word)` の binding stack を持つだけで、symbol value の TLS lookup はまだ実装していない。mutex、condition variable、semaphore、waitqueue の待機は `ncl-threads` の API を通じて行い、blocking call は `enter_native` と `leave_native` で囲む。

`interrupt-thread` は対象の cooperative interrupt を記録し、待機中の `ncl-threads` 操作を wake する。`ncl-sys` の safepoint request bit は safepoint API が管理し、Lisp callback を任意の native instruction 上で実行しない。deadline は monotonic nanoseconds の絶対値である。現 Phase 1 の `with-deadline` と `with-timeout` は保護本体の復帰後に期限を検査し、期限切れを `ThreadError::Timeout` として返す。一般の pending non-local exit や cleanup 後の条件報告までを実装済みとはしない。

## 根拠

mutator 固有状態を ThreadContext に閉じ込めることで共有 Runtime と root publication の境界を固定できる。absolute monotonic deadline は wall clock の変更に依存しない。async signal handler から Lisp を呼ばないことで OS の安全条件を守る。

## 却下した代替案

- global binding mutex は special access を直列化するため却下する。
- green thread のみの実装は OS blocking と native transition を表現できないため却下する。
- 任意の native instruction を割り込む設計は async-signal-safety に反するため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- per-thread special value を実装する際は TLS slot を 1 語、binding entry を 2 語とする。現 Phase 1 の binding API は `ThreadContext::bind`/`unbind` である。
- thread の登録、離脱、poll、root publication を独自の global state に置かない。
- mutex 保持中に GC request を待つ実装を追加しない。
- 同一 OS thread 上に複数の登録 `Thread` がある場合、`collect` を呼ぶ側以外は native 状態でなければならない（STW は active mutator の poll を待つため、poll できない登録 Thread があると停止する）。
- `Runtime` の registry 操作は呼び出し側の `ThreadContext` で確保する（内部 ctx は持たない）。
- `ncl_sys::unregister_thread` は `Thread` の heap 参照から heap を引くため、`Runtime`（heap の所有者）は登録済みの全 `ThreadContext` より長生きしなければならない。`ThreadContext` の Drop は登録済みなら unregister する。

## Poll state machine

`Running -> PollRequested -> Published -> Collecting -> Running` is the normal request path. `Published` records stack bounds, callee-saved registers, current frame, and epoch. A thread blocked in native code is `Safe`; it contributes its registered roots and conservative boundary without delaying collection. This is the `ncl-sys` safepoint contract, not a claim that every `ncl-threads` interrupt operation directly sets the safepoint word.

The machine-visible `ncl-sys::Thread` fields include native stack bounds, TLAB state, registered roots, safepoint epoch/state, interrupt flag, frame/register snapshots, multiple-value descriptor, handler/catch/cleanup pointers, and pending status. `ncl-object::ThreadContext` additionally owns the binding vector and multiple-value storage. The deadline stack and the `ncl-threads` thread registry are currently separate runtime state; they are not asserted to be fields of `ThreadContext`.

The `ncl-threads` API exposes mutex lock/unlock, condition wait/signal/broadcast, semaphore wait/post, and waitqueue operations over shared blocking state. A blocking operation checks its cooperative interrupt, enters native state before parking, wakes, leaves native state, and returns the result. `interrupt-thread` records the interrupt under the thread registry lock and notifies the registry condition variable; it does not by itself establish an asynchronous callback or a generated-code safepoint.

`with-deadline` pushes an absolute monotonic-nanosecond deadline for the calling thread. `with-timeout` sets `NCL-THREADS:*TIMEOUT-EXIT*` around the body and uses the same deadline entry point. In the current API, both report `ThreadError::Timeout` when the protected body returns after the deadline; the blocking primitives independently return `Timeout` when their wait expires. A symbol value cell is currently global, and `ThreadContext::bind`/`unbind` records `(u32, Word)` entries; the TLS override lookup and cleanup-driven non-local exit described by the eventual contract are not yet implemented.

## Phase 1 machine-visible layout

`ncl-sys::Thread` is `repr(C)`. The fields consumed by generated code are dedicated machine words at offsets returned by `thread_layout()`: `tlab_bump`, `tlab_limit`, `safepoint_request`, `pending`, `mv`, `handler`, `cleanup`, and `catch`. The scalar fields consumed directly by generated code are eight bytes wide and eight-byte aligned. `mv` is a Rust `Vec<Word>` descriptor and is not a generated-code scalar word. (`packages/sys/src/thread.rs`, `ThreadLayout`, `thread_layout()`.)

`SafepointState` and `NativeState` use `repr(u8)`, but this representation is not part of the generated-code ABI. Generated code reads the dedicated words only. `safepoint_request == 0` means no poll is pending; a nonzero value requests the slow path. Requesting a safepoint publishes the Rust state and sets the word, and polling consumes the request by clearing it. (`packages/sys/src/thread.rs`, `request_safepoint`, `poll_safepoint`.)

`ThreadContext` pins `Thread` in a `Box`, so moving the context after registration is safe and the address passed to `register_thread` remains stable until unregistration. (`packages/object/src/lib.rs`, `ThreadContext.thread`; `packages/sys/src/lib.rs`, `register_thread`.)

The safepoint slow path receives the pinned context in `x0`, the active generated frame pointer (`x29`) in `x1`, and the continuation PC in `x2`. A Rust callback handling that call must use only the passed frame pointer and PC and must not read its own `x29`/`x30`: a predecessor implementation snapshotted the Rust frame instead, corrupted the saved link register, and looped indefinitely, so this prohibition is part of the contract. A thread in the native state provides only a conservative snapshot; a precise frame snapshot is published only by the thread that entered the slow path. (`packages/codegen/src/target_aarch64_lowering.rs`, `lower_safepoint`.)
