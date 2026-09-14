# Conformance data

`sbcl/` は SBCL 2.6.0 の外部シンボル表と contrib 表です。`baselines/` は同じ環境で取得した ansi-test、cl-bench、起動、サイズ、compile-file の基準値です。

表面の再生成:

```sh
sbcl --script conformance/sbcl/extract-surface.lisp
```

ansi-test と cl-bench はリポジトリ外の一時領域へ取得してから実行します。数値は測定日と環境に結びついているため、比較は同一マシン・同一日の再計測で行ってください。公開ファイルにはホスト名、ユーザ名、会社名、絶対ホームパスを記録しません。
