# NCL

NCL is being rewritten as a native Common Lisp implementation. The design
contracts for the rewrite are in `docs/src/design/`. The former interpreter,
VM, and syntax crates are retired at commit `d9bbb4ec` and retained only as
legacy tests for later migration.

## 契約との差分

`ncl-sys` は設計契約の実行時境界を次の形で具体化しています。

- `alloc_code` と `publish_code` は OS の mapping/protection 失敗を返す
  `Result` API です。公開後の entry は release store、読み取りは acquire
  load です。
- `ReferenceLayout` は header-inclusive の固定 `reference_words` に加え、
  `boxed_from` からオブジェクト末尾までを参照として扱えます。payload `N`
  は生添字 `N + 1` です。
- `Thread` は `repr(C)` で、生成コードが使う `thread_layout()` の offset を
  ABI として固定します。上位の `ThreadContext` は `Thread` を先頭に置き、
  `*mut ThreadContext` を `*mut Thread` として渡す契約です。
- code object metadata は frame size、function name、source locations を保持し、
  `Heap::release_code(&mut thread, &mut owned_code)` は STW 下で全登録スレッドの
  frame chain を検査し、lookup table から除去してから mapping を解放します。
  live PC が残る間は `CodeError::CodeInUse` を返し、`owned_code` を保持します。
  registry に登録されていない場合も `CodeError::NotRegistered` を返し、再試行できるよう
  `owned_code` を保持します。frame chain は各フレームの `previous` リンクを辿って走査します。
- `Heap::collect` は登録された code registry と frame snapshot を使い、return PC
  ごとの safepoint map、function object、live slots、callee-saved registers を更新します。

契約文書側への正式な反映は、run ブランチでまとめて行います。この worktree では
`docs/src/design/` を編集していません。
