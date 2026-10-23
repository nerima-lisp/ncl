# cl-bench NCL driver

このドライバは upstream の `cl-bench.lisp` をロードしません。実行時に
`package.lisp`、ドライバ内の最小 `CL-BENCH` ハーネス、`files/*.lisp`、
`tests.lisp` の順にロードします。

## 置換した関数

ドライバが置換する upstream ハーネス要素は次のとおりです。

- `DEFBENCH`: `tests.lisp` の関数・実行回数・setup を記録します。
- `BENCH-RUN-1`: 各サンプルで setup と NCL GC を計測前に実行し、ベンチマーク関数の呼び出しだけを `GET-INTERNAL-REAL-TIME` で計測します。
- `BENCH-RUN`: 登録順に全ベンチを実行し、scoreboard 用のトップレベル `times` 配列と、ベンチ名ごとの status・サンプルを持つ `benchmarks` オブジェクトを出力します。
- 時間計測: `GET-INTERNAL-REAL-TIME` と `INTERNAL-TIME-UNITS-PER-SECOND` を使います。
- GC: `NCL-GC:GC` を動的に呼び出します。`asdf` や `trivial-garbage` の名前は NCL runtime / `ncl-object` に追加しません。
- 結果記録: `*RESULTS*` にベンチ名とサンプル列を保存します。

## 実行

固定 cl-bench checkout のルートで、最適化モードに応じて次のどちらかを実行します。

```sh
<repo>/conformance/cl-bench/ncl-driver.sh <ncl> default
<repo>/conformance/cl-bench/ncl-driver.sh <ncl> off
```

`<ncl>` の後ろには最適化モード `default` または `off` のどちらか一方を指定します。shell driver は
cl-bench checkout をカレントディレクトリとして Lisp driver を起動し、成功したベンチ
マークの実測サンプルをトップレベル `times` に出力します。各ベンチの status、部分的な
サンプル、エラーも `benchmarks` に記録します。ロードまたはベンチマークが失敗した場合は、
全ベンチの処理後に JSON を出力してから非ゼロ終了します。

`scripts/conformance_scoreboard.py` に渡す場合は、cl-bench checkout を current directory
として、NCLバイナリとモードを引数にした shell driver を指定します。出力 JSON は従来の
トップレベル `times` 配列を保つため、scoreboard runner の cl-bench 入力形式に合います。

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

この調査では、ベンチマーク本体に到達する前に停止したロード処理の実測プロセス経過時間も記録しました。これはベンチマークの実行時間ではなく、ドライバが出力する JSON にも含まれません。4件とも関数本体の実行時間と値は未取得です。

個別ロードのプロセス経過時間（`target/debug/ncl --eval '(load "...")'`）は次のとおりです。

| benchmark | status | elapsed | reason |
| --- | --- | ---: | --- |
| TAK | failed before benchmark | 1.87 s | `TypeError` in `files/gabriel.lisp` load |
| BOYER | failed before benchmark | 1.87 s | `TypeError` in `files/gabriel.lisp` load |
| FIB | failed before benchmark | 1.50 s | `function entry constant is unavailable` near `files/math.lisp:44` |
| ACKERMANN | failed before benchmark | 1.49 s | `function entry constant is unavailable` near `files/math.lisp:44` |
