再現コマンド: `sbcl --noinform --userinit /dev/null --load quicklisp/setup.lisp --eval '(push #p"/tmp/ncl-conformance/cl-bench/" ql:*local-project-directories*)' --eval '(ql:quickload :cl-bench)' --eval '(cl-bench:bench-run)' --eval '(sb-ext:exit)'`

# cl-bench on SBCL 2.6.0

- 測定日: 2026-09-15
- リポジトリ: cl-bench `553fbcdf88d2ca4e340a0cdf02679055f3279c8f`
- warmup: なし
- 記録サンプル: 3 回（各サンプルは同一コミットの実行ログ）
- 実時間の幾何平均: 0.0755 秒（正の値 64 ベンチマーク）
- ベンチマーク数: 65（`WALK-LIST/MESS` を含む）

各ベンチマークの `real` は、3 サンプルの最小 / 中央値 / 最大です。

| benchmark | min | median | max |
| --- | ---: | ---: | ---: |
| 1D-ARRAYS | 0.0337 | 0.0348 | 0.0349 |
| 2D-ARRAYS | 0.1065 | 0.1075 | 0.1196 |
| 3D-ARRAYS | 0.1429 | 0.1468 | 0.1509 |
| ACKERMANN | 1.1359 | 1.1508 | 1.1894 |
| BENCH-STRINGS | 1.1068 | 1.1208 | 1.1259 |
| BIGNUM/ELEM-100-1000 | 0.0948 | 0.0964 | 0.0972 |
| BIGNUM/ELEM-1000-100 | 0.1422 | 0.1427 | 0.1437 |
| BIGNUM/ELEM-10000-1 | 0.1092 | 0.1175 | 0.1219 |
| BIGNUM/PARI-100-10 | 0.0188 | 0.0195 | 0.0203 |
| BIGNUM/PARI-200-5 | 0.0686 | 0.0765 | 0.0769 |
| BITVECTORS | 0.0480 | 0.0486 | 0.0498 |
| BOEHM-GC | 0.1484 | 0.1684 | 0.1697 |
| BOYER | 0.0557 | 0.0562 | 0.0601 |
| BROWSE | 0.0280 | 0.0282 | 0.0291 |
| CLOS/complex-methods | 0.1035 | 0.1084 | 0.1259 |
| CLOS/defclass | 0.1647 | 0.1695 | 0.1891 |
| CLOS/defmethod | 2.1965 | 2.2558 | 2.2620 |
| CLOS/instantiate | 0.1511 | 0.1535 | 0.1573 |
| CLOS/method+after | 0.2704 | 0.2737 | 0.2847 |
| CLOS/methodcalls | 0.7956 | 0.8192 | 0.8286 |
| CLOS/simple-instantiate | 0.0088 | 0.0091 | 0.0091 |
| COMPILER | 1.0144 | 1.0390 | 1.0538 |
| CRC40 | 0.1538 | 0.1538 | 0.1550 |
| CTAK | 0.0163 | 0.0163 | 0.0166 |
| DDERIV | 0.0279 | 0.0281 | 0.0299 |
| DEFLATE-FILE | 0.0244 | 0.0245 | 0.0255 |
| DERIV | 0.0280 | 0.0284 | 0.0285 |
| DESTRUCTIVE | 0.0511 | 0.0520 | 0.0523 |
| DIV2-TEST-1 | 0.0567 | 0.0568 | 0.0650 |
| DIV2-TEST-2 | 0.1846 | 0.1855 | 0.1894 |
| EQL-SPECIALIZED-FIB | 0.1070 | 0.1078 | 0.1197 |
| FACTORIAL | 0.0157 | 0.0163 | 0.0171 |
| FFT | 0.0038 | 0.0038 | 0.0039 |
| FIB | 0.0448 | 0.0456 | 0.0465 |
| FIB-RATIO | 0.0033 | 0.0033 | 0.0040 |
| FPRINT/PRETTY | 0.1514 | 0.1524 | 0.1526 |
| FPRINT/UGLY | 0.0901 | 0.0950 | 0.0966 |
| FRPOLY/BIGNUM | 0.0346 | 0.0352 | 0.0353 |
| FRPOLY/FIXNUM | 0.0572 | 0.0572 | 0.0573 |
| FRPOLY/FLOAT | 0.0632 | 0.0636 | 0.0710 |
| HASH-INTEGERS | 0.0431 | 0.0432 | 0.0432 |
| HASH-STRINGS | 0.0236 | 0.0237 | 0.0248 |
| LOAD-FASL | 0.0151 | 0.0161 | 0.0178 |
| MANDELBROT/COMPLEX | 0.0366 | 0.0367 | 0.0382 |
| MANDELBROT/DFLOAT | 0.0013 | 0.0013 | 0.0014 |
| MRG32K3A | 0.1332 | 0.1353 | 0.1372 |
| PI-ATAN | 0.0900 | 0.0913 | 0.0964 |
| PI-DECIMAL/BIG | 0.0351 | 0.0354 | 0.0476 |
| PI-DECIMAL/SMALL | 0.0704 | 0.0733 | 0.0763 |
| PI-RATIOS | 0.2241 | 0.2246 | 0.2298 |
| PUZZLE | 0.1083 | 0.1133 | 0.1351 |
| RICHARDS | 0.1023 | 0.1057 | 0.1078 |
| SEARCH-SEQUENCE | 0.1356 | 0.1386 | 0.1391 |
| SLURP-LINES | 0.2363 | 0.2368 | 0.2376 |
| STAK | 0.0533 | 0.0533 | 0.0536 |
| STRING-CONCAT | 3.1298 | 3.1808 | 3.2092 |
| SUM-PERMUTATIONS | 0.1168 | 0.1169 | 0.1309 |
| TAK | 0.0327 | 0.0339 | 0.0350 |
| TAKL | 0.0725 | 0.0738 | 0.0932 |
| TRAVERSE | 0.2179 | 0.2213 | 0.2217 |
| TRIANGLE | 0.1521 | 0.1535 | 0.1684 |
| TRTAK | 0.0326 | 0.0333 | 0.0342 |
| WALK-LIST/SEQ | 0.0023 | 0.0027 | 0.0029 |
| fill-strings/adjustable | 0.2823 | 0.2853 | 0.2981 |
`WALK-LIST/MESS` は 3 回とも NIL でした。cl-bench の定義で SBCL が `disabled-for` に指定されているため実行されず、幾何平均から除外しました。全 64 件の実測値を内部時間単位から秒へ換算し、4 桁で記録しました。
