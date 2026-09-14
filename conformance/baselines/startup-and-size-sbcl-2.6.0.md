再現コマンド: `/usr/bin/time -p sbcl --noinform --userinit /dev/null --non-interactive --eval '(+ 1 2)' --quit`

# Startup, executable size, and compile-file

測定日: 2026-09-15。各 wall time は `/usr/bin/time -p` の `real`、単位は秒です。

| 項目 | 回数 | 最小 | 中央値 | 最大 |
| --- | ---: | ---: | ---: | ---: |
| `--non-interactive --eval '(+ 1 2)'` | 10 | 0.01 | 0.01 | 0.01 |
| `--script` の最小スクリプト | 10 | 0.01 | 0.01 | 0.01 |
| 保存 executable の起動と `--eval` | 10 | 0.01 | 0.01 | 0.03 |

`sb-ext:save-lisp-and-die` で生成した executable のサイズは 82,501,760 bytes です。compile-file は cl-bench `files/*.lisp` 16 ファイル、4,136 行、3 回で 0.09 / 0.08 / 0.08 秒、最小 0.08、中央値 0.08、最大 0.09 秒でした。出力 FASL はリポジトリ外の一時領域へ書きました。
