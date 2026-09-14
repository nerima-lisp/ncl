# Threads

## 決定

NCL は OS thread を 1 Lisp thread として扱う。ThreadContext は thread id、native stack bounds、current frame、32 KiB TLAB、allocation bytes、TLS 値領域、special-binding stack、handler chain、safepoint epoch、interrupt flags、deadline、multiple-value 領域、wait state を持つ。main thread も登録する。

symbol は tls_index: u32 を持つ。TLS 領域の slot は現在値の 1 語だけであり、tls_index で引く。束縛スタックの各 entry は (old_value, tls_index) の 2 語である。bind は現在値を entry に保存して TLS slot を新値にし、unbind は entry を LIFO で戻す。symbol value cell は global default、TLS slot は thread override とする。

mutex、condition variable、semaphore、waitqueue は ncl-sys の OS wrapper で実装する。待ち状態へ入る前に poll_safepoint と interrupt/deadline 検査を行い、enter_native を経てブロックする。GC 中は waitqueue の parked state を使う。

interrupt-thread は対象の interrupt flag と safepoint-request bit を atomic store し、waitqueue なら wake する。対象は次の safepoint または native transition 復帰時に interrupt condition を signal する。任意の native instruction を中断して Lisp callback を実行しない。

with-deadline は monotonic nanoseconds の absolute deadline を ThreadContext の dynamic record に積む。with-timeout は deadline と timeout condition を結び、safepoint、blocking wait、builtin call の各境界で期限を確認する。時間切れは pending non-local exit として cleanup を先に実行する。

## 根拠

TLS slot と binding entry を分けることで、通常の special access は index lookup 1 語で済み、束縛解除に必要な履歴だけを 2 語で持てる。OS thread と共有 heap の組合せは ThreadContext に mutator 状態を閉じ込めることで Rust の借用規則と整合する。absolute monotonic deadline は wall clock 変更の影響を受けない。

## 却下した代替案

- TLS slot 自体に旧値、symbol、depth を埋め込む 4 語形式は通常アクセスを大きくするため却下した。
- global special-binding mutex は symbol access を直列化するため却下した。
- green thread only と signal handler からの直接 Lisp 呼出しは OS thread および async-signal-safety に反するため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- TLS slot は 1 語、binding entry は 2 語で、special binding を global hash map に保存しない。
- thread の生成、登録、離脱は Runtime/ThreadContext API を通す。
- blocking primitive は safepoint、native transition、interrupt、deadline protocol と連携する。
- mutex を保持したまま GC request を待つ実装を追加しない。
