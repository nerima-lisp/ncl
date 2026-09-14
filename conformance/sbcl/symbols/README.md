# SBCL surface tables

再生成コマンド:

```sh
sbcl --script conformance/sbcl/extract-surface.lisp
```

測定対象は SBCL 2.6.0 (`This is SBCL 2.6.0, an implementation of ANSI Common Lisp.`)、Darwin arm64 です。features の要約は `:ARM64 :DARWIN :SB-THREAD :SB-UNICODE :SBCL :UNIX :COMMON-LISP :ANSI-CL :64-BIT` です。

行数はヘッダーを除く外部シンボル数です。

| 表 | 件数 |
| --- | ---: |
| COMMON-LISP | 977 |
| SB-EXT | 236 |
| SB-THREAD | 82 |
| SB-ALIEN | 61 |
| SB-SYS | 110 |
| SB-MOP | 101 |
| SB-GRAY | 31 |
| SB-UNICODE | 46 |
| SB-DEBUG | 23 |

`COMMON-LISP.tsv` はヘッダーを含め 978 行で、期待された 978 個と一致します。contrib の成否と件数は `contribs.tsv` にあります。内部参照表は Quicklisp に存在した 10 ライブラリのソースを走査した結果で、`sb-ext` の公開参照は対象外です。Quicklisp にソースがなかった usocket、flexi-streams、static-vectors、cl-fad、uiop は、ASDF 組み込みの UIOP を除き追加 clone を行っていないため、内部参照表の調査対象外です。
