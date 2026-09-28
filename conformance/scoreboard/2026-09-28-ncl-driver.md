# cl-bench NCL driver

このドライバは upstream の `cl-bench.lisp` をロードしません。実行時に
`package.lisp`、ドライバ内の最小 `CL-BENCH` ハーネス、`files/*.lisp`、
`tests.lisp` の順にロードします。

## 置換した関数

ドライバが置換する upstream ハーネス要素は次のとおりです。

- `DEFBENCH`: `tests.lisp` の関数・実行回数・setup を記録します。
- `BENCH-RUN-1`: setup、NCL GC、`GET-INTERNAL-REAL-TIME` による各サンプルを記録します。
- `BENCH-RUN`: 登録順に全ベンチを実行し、scoreboard 用の `{"times":[...]}` を出力します。
- 時間計測: `GET-INTERNAL-REAL-TIME` と `INTERNAL-TIME-UNITS-PER-SECOND` を使います。
- GC: `NCL-GC:GC` を動的に呼び出します。`asdf` や `trivial-garbage` の名前は NCL runtime / `ncl-object` に追加しません。
- 結果記録: `*RESULTS*` にベンチ名とサンプル列を保存します。

## 実行

固定 cl-bench checkout のルートで、次を実行します。

```sh
<repo>/conformance/cl-bench/ncl-driver.sh <ncl>
```

shell driver は cl-bench checkout をカレントディレクトリとして Lisp driver を起動し、
`bench-run` が出力する実測サンプルを JSON の `times` に出力します。ロードまたはベンチ
マークが失敗した場合は非ゼロ終了にし、部分的な値を成功した測定結果として扱いません。

scoreboard runner に渡す場合は、cl-bench checkout を current directory
として同じ `--load` コマンドを指定します。標準出力は `times` 配列を含む
JSON で、`scripts/conformance_scoreboard.py` の cl-bench 入力形式に合わせます。

## 確認状況

確認結果:

- `package.lisp` のロードは exit 0 になりました。
- `DEFPACKAGE` の export リストを4引数以下の `CONS` 呼び出しへ分解する修正後、
  `package.lisp` は exit 0 になりました。
- TAK（`files/gabriel.lisp:1561-1570`）と BOYER（`files/gabriel.lisp:565-584`）は
  `files/gabriel.lisp` の後続定義ロード中に `ncl: object error: TypeError` で
  exit 1 になりました。
- FIB（`files/math.lisp:29-35`）と ACKERMANN（`files/math.lisp:54-62`）は
  `files/math.lisp` の `run-fib-ratio` 定義付近（44行）で
  `ncl: native error: unsupported operation: function entry constant is unavailable`
  となり exit 1 でした。
- 4件とも関数本体の実行時間と値は未取得です。停止点は lambda-list の
  `&optional`/`&key` ではなく、ベンチ本体前のロード/codegen 経路です。

`load_times` は、ベンチ本体が未到達の場合のロード停止までの実測プロセス経過時間
です。benchmark の性能値ではありません。

個別ロードのプロセス経過時間（`target/debug/ncl --eval '(load "...")'`）は次のとおりです。

| benchmark | status | elapsed | reason |
| --- | --- | ---: | --- |
| TAK | failed before benchmark | 1.87 s | `TypeError` in `files/gabriel.lisp` load |
| BOYER | failed before benchmark | 1.87 s | `TypeError` in `files/gabriel.lisp` load |
| FIB | failed before benchmark | 1.50 s | `function entry constant is unavailable` near `files/math.lisp:44` |
| ACKERMANN | failed before benchmark | 1.49 s | `function entry constant is unavailable` near `files/math.lisp:44` |
