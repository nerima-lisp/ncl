# Threads

## 決定

NCL は OS thread を 1 Lisp thread として扱う。`ThreadState` は thread id, native stack bounds, current frame, TLAB, allocation bytes, special-binding stack, handler chain, safepoint epoch, interrupt flags, deadline, and wait state を持つ。main thread も登録対象である。

symbol の special binding は `special_tls_index: u32` で thread-local slot を参照する。slot は value, previous value, binding symbol, dynamic depth の 4 words。`bind` は stack push、`unbind` は depth まで LIFO pop とする。symbol value cell は global default、TLS slot は thread override である。

mutex は ncl-sys の OS mutex wrapper、waitqueue は mutex と condition variable、semaphore は atomic count と waitqueue で実装する。待ち状態へ入る前に thread は safepoint poll を実行し、GC 中は waitqueue を経由して parked になる。

`interrupt-thread` は対象の interrupt flag と safepoint-request bit を atomic store し、対象が waitqueue なら wake する。対象は次の poll で interrupt condition を signal し、handler がなければ debugger condition になる。任意の native instruction を中断して Rust callback を実行してはならない。

`with-deadline` は monotonic nanoseconds の absolute deadline を frame に積む。`with-timeout` は deadline と timeout condition を結び、safepoint、blocking wait、builtin call の各境界で期限を確認する。時間切れは pending non-local exit として cleanup を先に実行する。

## 根拠

thread-local special stacks preserve Common Lisp dynamic binding while allowing a shared heap. Safepoint requests give `interrupt-thread` and STW GC one synchronization boundary. Absolute monotonic deadlines avoid wall-clock adjustments and make nested timeout composition deterministic.

## 却下した代替案

- global special-binding mutex は symbol access を直列化するため却下した。
- green thread only は OS thread と interrupt-thread の決定に反するため却下した。
- signal handler から Lisp code を直接呼ぶ案は async-signal-safety に反するため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- thread の生成、登録、離脱は runtime API を通す。
- special binding を global hash map に保存しない。
- blocking primitive は必ず safepoint/interrupt/deadline protocol と連携する。
- mutex を保持したまま GC request を待つ実装を追加してはならない。
