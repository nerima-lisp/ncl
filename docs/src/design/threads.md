# Threads

## 決定

NCL は 1 OS thread を 1 Lisp thread として登録する。`ThreadContext` は TLAB、shadow roots、binding stack、multiple values、handler/cleanup/catch pointers、stack bounds、safepoint epoch、register snapshot、native state、interrupt flags、deadline、wait state を持つ。これらと共有 heap は `ncl-sys` が所有する。

symbol の `tls_index: u32` は 1 語の現在値 slot を指し、binding entry は `(old_value, tls_index)` の 2 語とする。mutex、condition variable、semaphore、waitqueue は `ncl-sys` の OS wrapper を使う。blocking 前に poll と interrupt/deadline 検査を行い、`enter_native` から `leave_native` を対にする。

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
