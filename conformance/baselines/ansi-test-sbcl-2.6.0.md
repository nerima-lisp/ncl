再現コマンド: `sbcl --noinform --disable-debugger --userinit /dev/null --load doit.lsp`

# ansi-test on SBCL 2.6.0

- 測定日: 2026-09-15
- リポジトリ: ansi-test `ca06bd919661af162c67407c9d994e881870bdb3`
- 全走行対象: 21,942 pending tests
- 結果: 完走せず、合格・失敗・未対応の確定集計なし
- 中断箇所: `INVOKE-DEBUGGER.1`
- 中断理由: テストハーネスが `No format-control for ~S` の `SIMPLE-ERROR` を発生
- 壁時計時間: 3.295 秒（SBCL の `TIME` 出力）

章ごとの完走内訳と失敗テスト名一覧は、ハーネスが中断したため確定できません。走行ログには中断までにロードされたテスト名が出力されています。
