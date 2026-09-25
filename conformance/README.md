# Conformance data

`ownership/` contains the crate-local ownership gate and its fixed
`COMMON-LISP` symbol set. `libraries/` records the fixed library-load inputs.
`baselines/` contains ansi-test, cl-bench, startup, size, and compile-file
reference values.

ansi-test と cl-bench はリポジトリ外の一時領域へ取得してから実行します。数値は測定日と環境に結びついているため、比較は同一マシン・同一日の再計測で行ってください。公開ファイルにはホスト名、ユーザ名、会社名、絶対ホームパスを記録しません。
