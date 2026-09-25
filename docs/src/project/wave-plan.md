# Wave plan

## 概要

本書は、旧 `wave-plan.md` を置き換える NCL の並列実装計画書です。各レーンは
ゼロ文脈で起動する前提です。

- **問題**: SBCL を超える Common Lisp 処理系が必要です。現行 NCL は SBCL 互換
  (SB-* パッケージ表面と SBCL contrib 20 本) を前提に計画されており、受入
  ゲートの所有表もその前提で作られています。
- **成果**: SBCL 互換を捨てた NCL 独自の処理系にします。対象は ANSI CL と
  NCL 独自の拡張 API で、実装は既存の Rust コアを引き継ぎます。前回の
  Wave 1 は 9 レーン同時でしたが、今回は 16 レーンを常時埋める計画にします。
- **受入**: 前回決定 6 の 4 基準 (ansi-test、cl-bench、ライブラリロード、
  起動・サイズ・compile-file) をそのまま使います。比較対象は SBCL 2.6.0 の
  実測値で、SBCL との互換性ではなく SBCL の数値を上回るかで判定します。

## 今回の決定

| # | 決定 | 置き換える既存決定 |
|---|---|---|
| D-1 | 既存コアを継続する。sys/object/ir/codegen/asm/objfile/reader/printer/types などの SBCL 非依存部は残し、SB-* 表面と SBCL 前提の設計・所有表だけを作り直す | なし |
| D-2 | SB-* パッケージとシンボルは作らない。機能は NCL 独自パッケージと独自 API で持つ。互換 shim や別名パッケージも置かない | 2026-09-15 決定 5 |
| D-3 | 移植範囲は「実行基盤 + 開発ツール」(FR-003, FR-004)。gmp/mpfr/simple-streams/aclrepl/cltl2/rt/md5/rotate-byte は移植しない | D-24-3 (contrib 20 本すべて Rust) |
| D-4 | ASDF/UIOP は Rust で再実装する。パッケージ名は上流 ASDF のもの (ASDF/INTERFACE, UIOP/DRIVER) を使う。これは SBCL の表面ではない | D-24-3 を ASDF/UIOP に限って維持 |
| D-5 | 可搬層ライブラリの `#+ncl` バックエンドは NCL リポジトリ内に置く。上流への PR は受入後に別途判断する | なし |
| D-6 | 同時に走らせるレーンは最大 16。うち 1 本は統合専任 | なし |
| D-7 | CI は x86-64 のみ。AArch64 の回帰は、統合レーンが merge のたびに macOS arm64 で全テストを流して検出する | D-24-4 を再確認 |
| D-8 | 最適化の到達点は D-24-1 の 4 項目 (SSA 最適化 + SSA レジスタ割当、並列 GC、tier-1 投機 + deopt/OSR、SIMD) に、e-graph (等価飽和) 最適化を加えたもの | D-24-1 を拡張 |

## 現状

Gate 0 の N01 が契約と骨格を着地させています。次の事実は着地先の main で確認したものです。

- `src/main.rs` は 23 行の stub です。動くのは `--version` / `-V` のみで、
  `--eval` は "not implemented during the native rewrite" を表示して exit 1、
  それ以外の引数は exit 2、引数なしは書き換えの案内を表示して exit 2 に
  なります。`--file`、`--repl`、`--compiled`、`--load`、`--script` はありません。
- 旧 interpreter、VM、syntax crate は削除済みで、`packages/core/*` はもう
  存在しません。
- 着地済みまたは一部着地の crate: sys、object、ir、codegen、objfile、
  asm/x86-64、asm/aarch64。言語・ライブラリ・runtime・conformance の各 crate は
  骨格または部分実装の段階です。
- N01 が所有表の分割 (`packages/<crate>/ownership.tsv`)、拡張 crate 7 本
  (ncl-os、ncl-asdf、ncl-uiop、ncl-profiler、ncl-coverage、ncl-debug、
  ncl-disasm) の骨格、`docs/src/design/extension-api.md` を着地させました。
  グローバルな `conformance/ownership/symbols.tsv` には、先頭フィールドが `SB-`
  で始まる行がまだ 1,925 行残っており、FR-001 がこれを削除します。
- SBCL 互換は意図的に廃止しています (D-2)。拡張機能は NCL 独自パッケージが
  提供します (FR-003)。
- 開発環境は `nix develop` で整います。Rust 1.98.0、clippy、rustfmt、
  cargo-llvm-cov、mkdocs が揃います。

## 機能要件

### FR-001 SBCL 表面の撤去 (必須。D-2 の直接の帰結)

- 実行時に `SB-` で始まるパッケージを作りません。
- `git grep -n '"SB-' -- packages src` が 0 件になります。
- 削除: `conformance/sbcl/symbols/`、`conformance/sbcl/extract-surface.lisp`、グローバルの `symbols.tsv`。
- `conformance/baselines/` は比較用の数値なので残します。

### FR-002 ANSI CL (必須。受入基準 1)

- COMMON-LISP パッケージの外部シンボル 978 個を全て所有・登録します。
- ansi-test (`ca06bd91…`) の合格数が SBCL の 21,768/21,942 以上になります。

### FR-003 NCL 拡張 API: 実行基盤 (必須。D-3。ライブラリロードの前提になります)

どの crate が何を持つか:

| 機能 | 担当 crate |
|---|---|
| スレッド、同期、タイマ、並行キュー | ncl-threads |
| FFI | ncl-ffi |
| MOP | ncl-clos |
| Gray stream | ncl-lib-streams |
| weak pointer、finalizer、GC 制御 | ncl-object |
| image 保存と単体実行ファイル | ncl-image |
| PLN、package lock | ncl-lib-packages |
| Unicode | ncl-lib-strings |
| POSIX、プロセス、ソケット | ncl-os (新設) |

- パッケージは文脈ごとに `NCL-` 接頭辞を付けて 1 つずつ作ります。
- 既定の名前は次のとおりで、N01 が凍結します: NCL-THREADS, NCL-FFI,
  NCL-MOP, NCL-GRAY, NCL-GC, NCL-IMAGE, NCL-UNICODE, NCL-OS, NCL-EXT
  (`truly-the` など言語拡張), NCL-SYS (内部 primitive)。
- AMOP や Gray の既存名は、意味が同一の場合に限って流用して構いません。

### FR-004 開発ツール (必須。D-3)

- 統計プロファイラ (ncl-profiler) を作ります。
- カバレッジ (ncl-coverage) を作ります。
- デバッガと introspection (ncl-debug) を作ります。
- x86-64/AArch64 の逆アセンブラ (ncl-disasm。capstone は使わず自前で書きます) を作ります。

### FR-005 ASDF/UIOP (必須。D-4)

- ncl-uiop と ncl-asdf を Rust で実装します。旧所有表の ASDF/INTERFACE 230 行と UIOP/DRIVER 427 行を移します。
- 固定コミットの上流 `.asd` がそのままロードできます。

### FR-006 ライブラリロード (必須。受入基準 3)

- 対象は `conformance/sbcl/symbols/README.md:39` に記録された 14 ライブラリ
  (固定コミット) です: cffi, bordeaux-threads, closer-mop, trivial-features,
  trivial-garbage, trivial-gray-streams, trivial-backtrace, alexandria,
  cl-ppcre, babel, usocket, flexi-streams, static-vectors, cl-fad。
- `libraries/<name>/` に置いた `#+ncl` バックエンドを重ねて、NCL の ASDF で
  ロードできます。
- どのライブラリにバックエンドが要るかは inferred です。N77 が `#+sbcl`
  分岐を grep して確定します。

### FR-007 性能 (必須。受入基準 2 と 4。同じマシン・同じ日に SBCL と対で計測し、commit hash を記録します)

- cl-bench の幾何平均 ≤ 0.0755 s
- 起動 ≤ 17.331 ms (中央値)
- 実行ファイル ≤ 82,501,760 bytes
- compile-file: cl-bench `files/*.lisp` を 0.08 s 以下で処理
- SBCL 側の値の出典は `conformance/baselines/`。

### FR-008 コンパイラと GC の段階 (必須。D-8)

| Phase | 内容 |
|---|---|
| 1b | 線形走査レジスタ割当。GC がフレームチェーン全体を走査するようにする |
| 1c | tail call、`&rest`/`&key` 専用のプロローグ |
| 2 | unbox、型推論、inline cache |
| 3 | inlining、escape analysis + スタック割当、GVN、SCCP、LICM、e-graph |
| 3 | SSA ベースのレジスタ割当 |
| 4 | 並列 GC |
| 5 | tier-1 投機最適化 + deopt/OSR |
| 6 | SIMD |

各 Phase の受入は `compiler-pipeline.md` の既存定義に従います。e-graph pass の
受入は次の 2 点です:

- 規則ごとに IR の往復テストがあり、`verify()` を通ります。
- cl-bench の幾何平均が e-graph 導入前を上回りません (遅くなりません)。

### FR-009 安定性 (必須。「安定性と速度の両立」を観測できる形にしたものです)

- pass を 1 つ通すごとに IR `verify()` を実行します。
- 差分テスト: 同じプログラムを tier-0、最適化あり、tier-1 で実行し、結果が一致します。
- GC stress モード: safepoint ごとに collect を走らせても全テストが通ります。
- 投機 1 種ごとに deopt テストを置きます。
- 並行テストには watchdog を付けます。
- coverage は regions 95% 以上を維持します。ゲートは緩めません。

### FR-010 AI Slop 除去 (必須。ユーザー指示)

- 対象: root README の書き直し、em dash の置換、`SymbolKind`/`SymbolRow` の
  ncl-ownership への一本化、SBCL 前提の文書記述、旧 `wave-plan.md` のこの
  計画での置き換え。
- 再発防止として `check_standards.py` に em dash 検査を加えます。

### FR-011 並列実行規約 (必須。D-6)

- 1 レーン = 1 worktree = 1 ブランチ。
- 所有表は crate ごとに分け、`packages/<crate>/ownership.tsv` に置きます。
  こうすると共有 TSV での衝突が起きません。
- 各レーンは最初のコミットで、自 crate の表と公開 API を凍結します。
- root `Cargo.toml`/`Cargo.lock` と凍結契約文書を変更してよいのは契約レーン (N01/N02) だけです。

## 技術仕様

### 所有表の分割 (D-1, FR-011)

決定内容:

- `ncl-ownership` は、テストが渡した表テキスト (`include_str!` で読む crate ごとの表) で検査する形に変えます。
- `check.py` は全 `packages/**/ownership.tsv` を集め、3 点を検査します: CL の
  978 シンボルが網羅されていること、行が重複していないこと、`SB-` が 0 件
  であること。

理由: 表が 1 ファイルのままだと、16 レーンが同じファイルを編集して衝突するためです。

却下した案: 単一 TSV を維持し、統合者がまとめて編集する案。統合者が律速点になるため却下しました。

### 登録の入口

- `ncl-stdlib::register_all` を、シンボルを所有する全 crate (threads/ffi/image と新設 crate を含む) の唯一の入口にします。
- まだ着地していない crate は何も登録しません。conformance runner はそれを失敗ではなく「未着地」と表示します。

### 新設 crate の依存辺

N01 が `crates.md` に反映します。非循環であることを確認済みです (inferred)。

```text
ncl-os -> ncl-object, ncl-sys, ncl-conditions, ncl-lib-streams
ncl-uiop -> ncl-object, ncl-conditions, ncl-lib-streams, ncl-lib-pathnames, ncl-lib-strings, ncl-os
ncl-asdf -> ncl-object, ncl-conditions, ncl-clos, ncl-uiop
ncl-profiler -> ncl-object, ncl-sys, ncl-threads, ncl-conditions
ncl-coverage -> ncl-object, ncl-compiler-front
ncl-debug -> ncl-object, ncl-sys, ncl-conditions, ncl-codegen, ncl-lib-streams
ncl-disasm -> ncl-object
ncl-stdlib -> (既存の辺) + ncl-threads, ncl-ffi, ncl-image, ncl-compiler-front, 上記 7 crate
```

### IR 契約 v2 (N02)

- 加えるもの: 関数エントリ定数、クロージャ生成、`catch`/`unwind-protect`/`progv` の handler-region lowering。
- 理由: クロージャ (ANSI の必須機能) と Phase 3 の pass が、安定した IR を前提にするためです。
- 範囲: 両アーキテクチャの codegen テンプレートまでを 1 レーンで atomic に着地させます。
- レジスタクラス (Phase 6) の契約は、N49 の着地後に凍結します。これはユーザー判断ではなく順序の問題です。

## 制約

- 外部 crate はゼロ。`unsafe` は `ncl-sys` だけに置きます。DDD 構造を守ります
  (2026-09-15 決定 3 と 9。変更なし)。
- 後方互換はありません。旧 NCL の API や FASL v1 のための別名や移行層も作りません。
- 検証は `nix develop --command cargo test --workspace` で行います。bare cargo は bin のリンクに失敗します。
- main の実体は `git ls-remote origin main` で確認します。fetch refspec が空なので `origin/main` は古い値のままになります。

## テスト要件

### 全レーン共通

- 自 crate の `registers_every_owned_symbol` が緑。
- 自 crate の regions coverage が 95% 以上。
- `cargo test --workspace`、`cargo clippy -D warnings`、`check_standards.py` がすべて exit 0。
- 自 crate に `"SB-` の文字列が無い。

### 失敗時の挙動

- 所有表が空なら `NoRows` で失敗します (偽の合格を防ぎます)。
- 登録が 1 件でも欠けていれば `package::symbol (kind): reason` の形で列挙して失敗します。

### マイルストーン

| ID | 観測できる状態 |
|---|---|
| M0 | FR-001 の grep が 0 件、`check.py` が CL 978 を網羅、旧 `symbols.tsv` が消えている |
| M1 | `ncl --eval` に fib の定義と `(fib 25)` を渡すと、ソースから native 実行されて `75025` を出力する。x86-64 は CI、AArch64 は統合レーンのローカル実行で確認 |
| M2 | ansi-test が完走し、合格・失敗・未実行の 3 つの数値と commit hash を出す |
| M3 | cl-bench 65 本が完走する |
| M4 | 14 ライブラリのロードマトリクスが全て成功する |
| M5 | FR-002 と FR-007 の数値が SBCL の値以上 (時間とサイズは以下) |

## タスク分解

レーン番号は旧 L 番号と区別するため N を付けています。「開始条件」の番号は、
そのレーンの API 凍結コミットを指します。

### Gate 0 (この 6 レーンを最初に同時起動します)

| N | 範囲 | 開始条件 | 受入 |
|---|---|---|---|
| N01 | 拡張 API の契約 (`docs/src/design/extension-api.md`)、`crates.md`、所有表の分割、`ncl-ownership` の API 変更と型の一本化、`check.py` の書き直し、SBCL 抽出物の削除、新設 7 crate の骨格と root `Cargo.toml`、`register_all` の順序表 | なし | M0 の `check.py` 部分。最初のコミットで表の形式と ownership API を凍結する |
| N02 | IR 契約 v2 (ir、`compiler-pipeline.md`、両アーキテクチャの codegen テンプレート) | なし | 新しい op が両アーキテクチャで実行テストを通る |
| N03 | Slop 除去と文書 (README、`docs/src/**`、ただし N01/N02 が持つファイルは除く。新計画書) | なし | FR-010。`mkdocs build --strict` が exit 0 |
| N04 | 統合 (常駐)。merge、全体テスト、arm64 でのローカル全テスト | なし | merge ごとに両アーキテクチャが緑 |
| N05 | sys/object/objfile/asm の coverage 回復 | なし | 各 crate の regions が 95% 以上 |
| N76 | ncl-disasm のデコーダ | なし (登録は N01 の後) | 自前 assembler の出力を逆アセンブルして往復一致する |

### Wave A (N01/N02 の凍結後、空いた枠に優先順で投入します)

| 優先 | N | 範囲 | 開始条件 |
|---|---|---|---|
| 1 | N60 | runtime + bin (eval/compile/compile-file/load、`MacroCaller`、REPL、`--eval`/`--load`/`--script`)。M1 を担当 | N01, N02 |
| 2 | N27 | lib-macros A (定義、制御、setf) | N01 |
| 3 | N20 | lib-numbers | N01 |
| 4 | N13 | compiler-front の表面移行と IR v2 の lowering (クロージャ、catch/unwind-protect/progv、macrolet) | N01, N02 |
| 5 | N40 | Phase 1b | N02 |
| 6〜14 | N10 threads、N11 ffi、N12 conditions、N14 types/reader/printer/image/object の表面移行、N21 sequences、N22 strings、N24 streams、N25 packages、N26 clos | N01 |
| 15 | N43 | pass manager + inlining | N02 |

### Wave B (依存先の凍結後に投入します)

| N | 範囲 | 開始条件 |
|---|---|---|
| N23 | hash-arrays | N01 |
| N28 | LOOP | N27 |
| N29 | pathnames | N22 |
| N30 | format | N22, N24, printer |
| N44〜N47 | escape analysis、GVN+SCCP、LICM、e-graph | N43 |
| N41 | Phase 1c | N40 |
| N49 | SSA レジスタ割当 | N40 |
| N50 | Phase 4 並列 GC | N40 (`collect.rs` が重なるため) |
| N62 | conformance runner (M2/M3/M5) | N60 |
| N63 | 差分テスト + GC stress | N60 |
| N70 | ncl-os | N24 |
| N73 | ncl-profiler | N10 |

### Wave C

| N | 範囲 | 開始条件 |
|---|---|---|
| N42 | Phase 2 | N41, N13 |
| N52 | Phase 5 | N42, N44〜N47, N49 |
| N53 | Phase 6 SIMD | N49 の後、レジスタクラス契約を凍結してから |
| N71 | ncl-uiop | N24, N29, N70 |
| N72 | ncl-asdf | N71, N26 |
| N74 | ncl-coverage | N13, N60 |
| N75 | ncl-debug | N12, N60 |
| N77 | ライブラリバックエンドとロードマトリクス (M4) | N72, N10, N11, N26 |

### 引き継ぎの前提と禁止事項

- 基準は main の `5bf6506a` 以降です。printer の未 merge 2 コミットの扱いは N04 が決めます。
- 使ってはいけないもの: 旧 `wave-plan.md` の D-24-3 決定、所有表の SB-* 行、coverage ゲートの引き下げ。
- 前回の Wave 1 は opencode の利用上限で全停止しました。16 本を同時に起動する際は、上限に達したときの再開手順を kickoff に含めます。
