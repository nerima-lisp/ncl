# SBCL surface tables

再生成コマンド:

```sh
sbcl --script conformance/sbcl/extract-surface.lisp
```

測定対象は SBCL 2.6.0 (`This is SBCL 2.6.0, an implementation of ANSI Common Lisp.`)、Darwin arm64 です。features の要約は `:ARM64 :DARWIN :SB-THREAD :SB-UNICODE :SBCL :UNIX :COMMON-LISP :ANSI-CL :64-BIT` です。

行数はヘッダーを除く外部シンボル数です。

| 表 | 件数 |
| --- | ---: |
| COMMON-LISP | 978 |
| SB-EXT | 236 |
| SB-THREAD | 82 |
| SB-ALIEN | 61 |
| SB-SYS | 110 |
| SB-MOP | 101 |
| SB-GRAY | 31 |
| SB-UNICODE | 46 |
| SB-DEBUG | 23 |

`COMMON-LISP.tsv` はヘッダーを含め 979 行で、外部シンボル 978 個と一致します。抽出時に行の計算でエラーが起きても列数を保った行を出力するよう `extract-surface.lisp` のエラー分岐を修正しました。contrib の成否と件数は `contribs.tsv` にあります。

内部参照表は、指定した 14 ライブラリと ASDF 同梱 UIOP のソースを走査した結果です。対象パッケージは `SB-KERNEL`、`SB-INT`、`SB-IMPL`、`SB-C`、`SB-VM`、`SB-PCL`、`SB-UNIX`、`SB-DI`、`SB-FASL`、`SB-ALIEN-INTERNALS`、`SB-WALKER`、`SB-FORMAT` です。`sb-ext` の公開参照は対象外です。

| ライブラリ | 内部参照件数 |
| --- | ---: |
| cffi | 7 |
| alexandria | 2 |
| babel | 1 |
| uiop | 36 |
| usocket | 12 |
| その他 10 ライブラリ | 0 |
| 合計 | 58 |

追加 clone の測定コミットは usocket `4951d575b8f73270802a03cc5812b8310409caa9`、flexi-streams `3d9d89b4950b72e0e5bdacfcdfd366bde72386d2`、static-vectors `d492f746d33fed32283143394c7ead1cce59ba37`、cl-fad `714257f064cbe326855701be1aa5ef1199f3c676` です。Quicklisp のコミットは cffi `20260101-git`、bordeaux-threads `v0.9.4`、closer-mop `20260101-git`、trivial-features `20250622-git`、trivial-garbage `20231021-git`、trivial-gray-streams `20241012-git`、trivial-backtrace `20230214-git`、alexandria `20241012-git`、cl-ppcre `20250622-git`、babel `20260101-git` です。
UIOP は SBCL 2.6.0 に同梱された ASDF の `uiop.lisp` を走査しました。Quicklisp のソースは git 管理外の配布ディレクトリだったため、各ディストリビューション版識別子を記録しています。
