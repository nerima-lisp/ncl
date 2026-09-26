# Conformance data

`ownership/` contains the crate-local ownership gate and its fixed
`COMMON-LISP` symbol set. `libraries/` records the fixed library-load inputs.
`baselines/` contains ansi-test, cl-bench, startup, size, and compile-file
reference values.

ansi-test と cl-bench はリポジトリ外の一時領域へ取得してから実行します。数値は測定日と環境に結びついているため、比較は同一マシン・同一日の再計測で行ってください。公開ファイルにはホスト名、ユーザ名、会社名、絶対ホームパスを記録しません。

## Scoreboard runner

`python3 scripts/conformance_scoreboard.py` downloads the pinned upstream
repositories into a cache outside the source tree and supervises one command
per suite. Commands must print a JSON object to standard output: ansi-test
prints `passed`, `failed`, and `unexecuted`; cl-bench prints a positive
`times` array in seconds. Assertion failures remain measurement results;
timeouts and signal termination remain harness failures.

The upstream sources are not vendored. ansi-test is pinned to
`ca06bd919661af162c67407c9d994e881870bdb3`, and cl-bench is pinned to
`553fbcdf88d2ca4e340a0cdf02679055f3279c8f`; both are fetched from the
ansi-test GitLab project. The license files remain in the temporary checkout.
