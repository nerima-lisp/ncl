再現コマンド: `sbcl --noinform --disable-debugger --userinit /dev/null --load quicklisp/setup.lisp --eval '(push #p"/tmp/ncl-conformance/cl-bench/" ql:*local-project-directories*)' --eval '(ql:quickload :cl-bench)' --eval '(cl-bench:bench-run)' --quit`

# cl-bench on SBCL 2.6.0

- 測定日: 2026-09-15
- リポジトリ: cl-bench `553fbcdf88d2ca4e340a0cdf02679055f3279c8f`
- warmup: 2 回
- 記録サンプル: 3 回
- 実時間の幾何平均: 0.0955 秒（正の値 60 ベンチマーク）
- ベンチマーク数: 64

各ベンチマークの `real` は、3 サンプルの最小 / 中央値 / 最大です。

| benchmark | min | median | max |
| --- | ---: | ---: | ---: |
| 1D-ARRAYS | 0.03 | 0.03 | 0.03 |
| 2D-ARRAYS | 0.11 | 0.11 | 0.12 |
| 3D-ARRAYS | 0.14 | 0.15 | 0.15 |
| ACKERMANN | 1.14 | 1.15 | 1.19 |
| BENCH-STRINGS | 1.11 | 1.12 | 1.13 |
| BIGNUM/ELEM-100-1000 | 0.09 | 0.10 | 0.10 |
| BIGNUM/ELEM-1000-100 | 0.14 | 0.14 | 0.14 |
| BIGNUM/ELEM-10000-1 | 0.11 | 0.12 | 0.12 |
| BIGNUM/PARI-100-10 | 0.02 | 0.02 | 0.02 |
| BIGNUM/PARI-200-5 | 0.07 | 0.08 | 0.08 |
| BITVECTORS | 0.05 | 0.05 | 0.05 |
| BOEHM-GC | 0.15 | 0.17 | 0.17 |
| BOYER | 0.06 | 0.06 | 0.06 |
| BROWSE | 0.03 | 0.03 | 0.03 |
| CLOS/complex-methods | 0.10 | 0.11 | 0.13 |
| CLOS/defclass | 0.16 | 0.17 | 0.19 |
| CLOS/defmethod | 2.20 | 2.26 | 2.26 |
| CLOS/instantiate | 0.15 | 0.15 | 0.16 |
| CLOS/method+after | 0.27 | 0.27 | 0.28 |
| CLOS/methodcalls | 0.80 | 0.82 | 0.83 |
| CLOS/simple-instantiate | 0.01 | 0.01 | 0.01 |
| COMPILER | 1.01 | 1.04 | 1.05 |
| CRC40 | 0.15 | 0.15 | 0.15 |
| CTAK | 0.02 | 0.02 | 0.02 |
| DDERIV | 0.03 | 0.03 | 0.03 |
| DEFLATE-FILE | 0.02 | 0.02 | 0.03 |
| DERIV | 0.03 | 0.03 | 0.03 |
| DESTRUCTIVE | 0.05 | 0.05 | 0.05 |
| DIV2-TEST-1 | 0.06 | 0.06 | 0.07 |
| DIV2-TEST-2 | 0.18 | 0.19 | 0.19 |
| EQL-SPECIALIZED-FIB | 0.11 | 0.11 | 0.12 |
| FACTORIAL | 0.02 | 0.02 | 0.02 |
| FFT | 0.00 | 0.00 | 0.00 |
| FIB | 0.04 | 0.05 | 0.05 |
| FIB-RATIO | 0.00 | 0.00 | 0.00 |
| FPRINT/PRETTY | 0.15 | 0.15 | 0.15 |
| FPRINT/UGLY | 0.09 | 0.10 | 0.10 |
| FRPOLY/BIGNUM | 0.03 | 0.04 | 0.04 |
| FRPOLY/FIXNUM | 0.06 | 0.06 | 0.06 |
| FRPOLY/FLOAT | 0.06 | 0.06 | 0.07 |
| HASH-INTEGERS | 0.04 | 0.04 | 0.04 |
| HASH-STRINGS | 0.02 | 0.02 | 0.02 |
| LOAD-FASL | 0.02 | 0.02 | 0.02 |
| MANDELBROT/COMPLEX | 0.04 | 0.04 | 0.04 |
| MANDELBROT/DFLOAT | 0.00 | 0.00 | 0.00 |
| MRG32K3A | 0.13 | 0.14 | 0.14 |
| PI-ATAN | 0.09 | 0.09 | 0.10 |
| PI-DECIMAL/BIG | 0.04 | 0.04 | 0.05 |
| PI-DECIMAL/SMALL | 0.07 | 0.07 | 0.08 |
| PI-RATIOS | 0.22 | 0.22 | 0.23 |
| PUZZLE | 0.11 | 0.11 | 0.14 |
| RICHARDS | 0.10 | 0.11 | 0.11 |
| SEARCH-SEQUENCE | 0.14 | 0.14 | 0.14 |
| SLURP-LINES | 0.24 | 0.24 | 0.24 |
| STAK | 0.05 | 0.05 | 0.05 |
| STRING-CONCAT | 3.13 | 3.18 | 3.21 |
| SUM-PERMUTATIONS | 0.12 | 0.12 | 0.13 |
| TAK | 0.03 | 0.03 | 0.04 |
| TAKL | 0.07 | 0.07 | 0.09 |
| TRAVERSE | 0.22 | 0.22 | 0.22 |
| TRIANGLE | 0.15 | 0.15 | 0.17 |
| TRTAK | 0.03 | 0.03 | 0.03 |
| WALK-LIST/SEQ | 0.00 | 0.00 | 0.00 |
| fill-strings/adjustable | 0.28 | 0.29 | 0.30 |

`WALK-LIST/MESS` は 3 回とも NIL で、実時間を取得できなかったため幾何平均から除外しました。0.00 表示の項目も丸め前は測定値が分解能未満です。
