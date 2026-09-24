# Wave plan

2026-09-24 の /define で確定した並列実装計画である。基準 ref は `main` の `ebdfcf86`
(Phase 0 と Phase 1a 着地済み)。凍結契約は `docs/src/design/` にあり、本書は
その上に「残り全体をどう並列レーンへ切るか」と「未決定だった点の決定」を置く。
各レーンはゼロ文脈で起動する前提で書かれている。

## 1. 決定表

### 2026-09-24 に確定した 5 決定

| # | 決定 | 内容 | 影響 |
| --- | --- | --- | --- |
| D-24-1 | 到達技術 | 凍結済み Phase 2(fixnum/double unbox、型推論接続、inline cache)の先に、Phase 3 SSA 最適化器 + SSA レジスタ割当、Phase 4 並列 GC(parallel marking / evacuation、STW 維持)、Phase 5 tier-1 投機的最適化 + deopt/OSR、Phase 6 特殊化配列の SIMD 化、の 4 項目すべてを目標到達点に含める | Phase 5 の deopt 着地先は tier-0 テンプレートコード(interpreter は作らない)。frame-state map は FASL header version 2 の新 section。Phase 6 はレジスタクラス追加の契約変更を要する |
| D-24-2 | 依存グラフの理想形 | 集約 crate `ncl-stdlib` を新設し `ncl-runtime -> ncl-stdlib` とする。`ncl-lib-*` の隣接は実依存に改訂する。`ncl-lib-loop` は廃止し LOOP は `ncl-lib-macros` 所有とする(所有表と一致) | §2 の隣接表 |
| D-24-3 | contrib の実装言語 | SBCL contrib 20 本(所有表 Phase 3、1,884 シンボル)は全て Rust。2026-09-15 の決定 7「builtin は全て Rust」を contrib にも拡張する。asdf/uiop を含む | 上流 ASDF との互換維持は NCL 側の責務。受入は NFR-004(主要ライブラリの `.asd` が NCL の Rust ASDF でロードできる) |
| D-24-4 | CI の範囲 | `scripts/check_standards.py` の CI 配線のみ許可。CI は x86-64(ubuntu)のみで macOS runner は追加しない | CI が生成コードを実行する唯一の経路は L14(x86-64 lowering)になるため、L14 を Wave 1 先頭優先とする |
| D-24-5 | defer | L24 の RT ハーネス依存確定と L30 の Phase 6 レジスタクラス契約は /execute へ defer。他は全て ready | §5 の blocked 欄 |

D-24-3 と D-24-4 は finalize 後に別セッションから再質問されたが未回答である。本書は
finalize の結論(全て Rust、check_standards 配線のみ・x86-64 のみ)をそのまま採用する。
操作者が覆す場合は本表を改訂し、影響欄のレーンを更新する。

### 2026-09-15 の決定(要約再掲)

| # | 決定 |
| --- | --- |
| 1 | 車両は NCL の Rust コア再構築。cl-cc は車両ではない |
| 2 | 自前ネイティブバックエンド(x86-64 + AArch64、レジスタ割当、Mach-O/ELF/FASL writer、自前 unwinder)。Cranelift を含む外部 crate は使わない |
| 3 | unsafe は `ncl-sys` のみ。他 crate は `unsafe_code = "forbid"` |
| 4 | 共有ヒープ + OS スレッドを最初から設計(safepoint、TLAB、per-thread binding stack) |
| 5 | SBCL 公開拡張パッケージ全部と contrib 20 本、内部パッケージは主要ライブラリが参照する 58 シンボル |
| 6 | 受入基準 4 つ: ansi-test 合格数 >= SBCL(21,768/21,942)、cl-bench 幾何平均 <= SBCL(0.0755 s)、主要ライブラリがロードできる、起動(17 ms)・実行ファイル(82.5 MB)・compile-file 速度 <= SBCL |
| 7 | 標準ライブラリは全て Rust builtin。compiler は直接展開 primitive と Rust 側 compiler macro を持つ |
| 8 | 旧 interpreter/VM/syntax crate は削除済み。後方互換なし。ansi-test / cl-bench / ライブラリロードだけがゲート |
| 9 | 外部 crate ゼロ(dev-dependencies 含む)。DDD 構造。工数は制約ではなく理想到達点が目標 |

## 2. 改訂後の crate 隣接表

`docs/src/design/crates.md` の `text` ブロックをこの内容で差し替える。矢印は依存方向。

```text
ncl-sys -> none
ncl-object -> ncl-sys
ncl-ir -> none
ncl-asm-x86-64 -> none
ncl-asm-aarch64 -> none
ncl-objfile -> none
ncl-codegen -> ncl-ir, ncl-asm-x86-64, ncl-asm-aarch64, ncl-objfile, ncl-object, ncl-sys
ncl-ownership -> ncl-object
ncl-types -> ncl-object
ncl-reader -> ncl-object
ncl-printer -> ncl-object
ncl-conditions -> ncl-object
ncl-clos -> ncl-object, ncl-types, ncl-conditions
ncl-compiler-front -> ncl-ir, ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-clos
ncl-lib-numbers -> ncl-object, ncl-types, ncl-conditions
ncl-lib-sequences -> ncl-object, ncl-types, ncl-conditions
ncl-lib-strings -> ncl-object, ncl-types, ncl-conditions
ncl-lib-hash-arrays -> ncl-object, ncl-types, ncl-conditions
ncl-lib-streams -> ncl-object, ncl-types, ncl-conditions, ncl-sys
ncl-lib-pathnames -> ncl-object, ncl-types, ncl-conditions, ncl-lib-strings, ncl-sys
ncl-lib-packages -> ncl-object, ncl-types, ncl-conditions
ncl-lib-format -> ncl-object, ncl-types, ncl-conditions, ncl-lib-streams, ncl-lib-strings, ncl-printer
ncl-lib-macros -> ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-lib-sequences
ncl-stdlib -> ncl-types, ncl-reader, ncl-printer, ncl-conditions, ncl-clos,
              ncl-lib-numbers, ncl-lib-sequences, ncl-lib-strings, ncl-lib-hash-arrays,
              ncl-lib-streams, ncl-lib-pathnames, ncl-lib-packages, ncl-lib-format, ncl-lib-macros
ncl-threads -> ncl-object, ncl-sys, ncl-conditions
ncl-ffi -> ncl-object, ncl-sys, ncl-conditions
ncl-image -> ncl-object, ncl-objfile, ncl-sys
ncl-runtime -> ncl-compiler-front, ncl-codegen, ncl-stdlib, ncl-image, ncl-threads, ncl-ffi
ncl-conformance -> ncl-runtime, ncl-ownership
ncl (bin) -> ncl-runtime
```

旧表からの差分は次のとおり。

- `ncl-lib-loop` を削除する。所有表 `conformance/ownership/symbols.tsv` は LOOP と
  LOOP-FINISH を `ncl-lib-macros` に割り当てており、`ncl-lib-loop` の行は存在しない。
- `ncl-stdlib` を新設し、`ncl-runtime` の辺に加える。旧表には 10 個の `ncl-lib-*` の
  `register(&Runtime)` を呼ぶ crate が存在しなかった。
- `ncl-ownership` を新設する(§4 の W0-3)。所有表を読み、登録済みシンボル集合と突合する
  テスト支援 crate で、`ncl-object` にしか依存しない。
- `ncl-lib-*` の辺を実依存に改訂する。`ncl-conditions` は全ライブラリが condition を
  signal するために必要。fd stream と pathname の OS 呼出は `ncl-sys` 経由。format は
  stream と printer に、macros(LOOP 展開)は sequences に依存する。
- `ncl-clos` に `ncl-conditions` を加える(所有表の lane summary と一致)。
- `ncl-compiler-front` に `ncl-object` と `ncl-types` を加える(form は `Word`、
  declaration の型は `ncl-types` の型指定子)。

グラフは非循環である。`ncl-stdlib` は runtime より下、全 `ncl-lib-*` より上に置き、
登録順序という関心を単独で所有する。

### Public API summary に加える行

| crate | public API and responsibility |
| --- | --- |
| ncl-ownership | `symbols.tsv` の行型、`rows_for_crate`、`assert_crate_coverage(&Runtime, &mut ThreadContext, crate)`。テスト支援専用で Lisp 値を公開しない |
| ncl-stdlib | `register_all(&Runtime)`: 全 `ncl-lib-*` と types/reader/printer/conditions/clos の `register` を依存順に呼ぶ唯一の入口。順序表を持つ |
| ncl-lib-numbers | 数値塔の builtin 登録(所有表 164 行)。bignum/ratio/float/complex の演算、`ash`、`random`、`boole` |
| ncl-lib-sequences | list/sequence/tree の builtin 登録(150 行)。`equal`/`equalp`、`sort`、`map` 系 |
| ncl-lib-strings | 文字・文字列の builtin 登録(108 行)。文字述語、`char=` 系、名前と符号 |
| ncl-lib-hash-arrays | 配列と hash table の builtin 登録(55 行)。`aref` 系、`adjust-array`、synchronized hash table |
| ncl-lib-streams | stream の builtin 登録(121 行)。fd-stream、standard stream 変数、`read-byte`/`write` 系、external format |
| ncl-lib-pathnames | pathname と file system の builtin 登録(43 行)。`native-namestring`、directory 操作 |
| ncl-lib-packages | package と symbol の builtin 登録(58 行)。package-local nickname、`gensym`/`gentemp` |
| ncl-lib-format | `format` の全 directive(所有表 1 行、directive 集合は CLHS 22.3 全部) |
| ncl-lib-macros | 標準マクロの Rust expander 登録(97 行)。`defun`/`when`/`setf`/`loop` など、`macroexpand`、`funcall`/`apply` |

### Registration and ownership rules に加える文

`ncl-stdlib::register_all(&Runtime)` が標準ライブラリ登録の唯一の入口である。各
`ncl-lib-*` の `register(&Runtime)` は他 crate の `register` を呼ばない。`ncl-runtime` は
起動時に `register_all` を一度だけ呼び、二度目の呼出は `ObjectError` を返す。

## 3. 22 crate の骨格一覧

W0-2 で作る空 crate の一覧。既存 7 crate(sys, object, ir, codegen, objfile, asm/x86-64,
asm/aarch64)と bin は対象外。`Cargo.toml` は既存 crate と同じ形式(下記)で、
`[dependencies]` に隣接表の path 依存のみを書く。`[lints] workspace = true` により
`unsafe_code = "forbid"` が全 crate に効くので `#![forbid(unsafe_code)]` の再宣言は
不要である。`src/lib.rs` は crate doc コメント 1 行のみとし、`register` を持つ crate も
W0-2 では空のままにする(実装はレーンの責務。空の `register` を置くとレーン着地前に
`register_all` が偽の成功を返す)。README は W0-2 では作らず、各レーンが自 crate の
README を書く。

```toml
[package]
name = "<name>"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
<隣接表の行を path 依存で列挙>

[lints]
workspace = true
```

| # | name | path | `[dependencies]` | `register` |
| --- | --- | --- | --- | --- |
| 1 | ncl-ownership | packages/ownership | ncl-object | なし |
| 2 | ncl-types | packages/types | ncl-object | あり |
| 3 | ncl-reader | packages/reader | ncl-object | あり |
| 4 | ncl-printer | packages/printer | ncl-object | あり |
| 5 | ncl-conditions | packages/conditions | ncl-object | あり |
| 6 | ncl-clos | packages/clos | ncl-object, ncl-types, ncl-conditions | あり |
| 7 | ncl-compiler-front | packages/compiler/front | ncl-ir, ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-clos | あり(特殊形式表と compiler macro) |
| 8 | ncl-lib-numbers | packages/lib/numbers | ncl-object, ncl-types, ncl-conditions | あり |
| 9 | ncl-lib-sequences | packages/lib/sequences | ncl-object, ncl-types, ncl-conditions | あり |
| 10 | ncl-lib-strings | packages/lib/strings | ncl-object, ncl-types, ncl-conditions | あり |
| 11 | ncl-lib-hash-arrays | packages/lib/hash-arrays | ncl-object, ncl-types, ncl-conditions | あり |
| 12 | ncl-lib-streams | packages/lib/streams | ncl-object, ncl-types, ncl-conditions, ncl-sys | あり |
| 13 | ncl-lib-pathnames | packages/lib/pathnames | ncl-object, ncl-types, ncl-conditions, ncl-lib-strings, ncl-sys | あり |
| 14 | ncl-lib-packages | packages/lib/packages | ncl-object, ncl-types, ncl-conditions | あり |
| 15 | ncl-lib-format | packages/lib/format | ncl-object, ncl-types, ncl-conditions, ncl-lib-streams, ncl-lib-strings, ncl-printer | あり |
| 16 | ncl-lib-macros | packages/lib/macros | ncl-object, ncl-types, ncl-reader, ncl-conditions, ncl-lib-sequences | あり |
| 17 | ncl-stdlib | packages/stdlib | 2〜6、8〜16 | `register_all` |
| 18 | ncl-threads | packages/threads | ncl-object, ncl-sys, ncl-conditions | あり |
| 19 | ncl-ffi | packages/ffi | ncl-object, ncl-sys, ncl-conditions | あり |
| 20 | ncl-image | packages/image | ncl-object, ncl-objfile, ncl-sys | あり |
| 21 | ncl-runtime | packages/runtime | ncl-compiler-front, ncl-codegen, ncl-stdlib, ncl-image, ncl-threads, ncl-ffi | なし(呼ぶ側) |
| 22 | ncl-conformance | packages/conformance | ncl-runtime, ncl-ownership | なし |

path 依存の相対パスは `packages/lib/<name>` からは `../../object` のように 2 段、
`packages/compiler/front` も 2 段、それ以外は 1 段である。

root `Cargo.toml` の `members` と `default-members` には既存 8 項目の後ろに上表の順
(1 → 22)で追加する。両リストは同一内容にする。contrib 20 crate(`ncl-contrib-*`)は
Wave 4 の kickoff commit で同じ規則により一括登録し、contrib レーンも root `Cargo.toml`
に触れない。

W0-2 の受入: `cargo check --workspace` が全 30 crate を通し、`python3
scripts/check_standards.py` の `manifests checked` が 29 になり violations none、
`cargo test --workspace` の結果が W0-2 前と同じ passed 数(現在 147)である。

## 4. W0-1〜W0-4 の実装仕様

Wave 0 は他の全レーンの前提で、W0-1 と W0-2 は同一 worktree で直列に行う。W0-3 と
W0-4 は別 worktree で並列に行えるが、ファイル集合の重なりを下表に示す。

| タスク | 触るファイル | 重なり |
| --- | --- | --- |
| W0-1 | `docs/src/design/crates.md`, `docs/src/design/compiler-pipeline.md`, `docs/src/design/native-backend.md`, `docs/src/project/roadmap.md` | なし |
| W0-2 | root `Cargo.toml`, `Cargo.lock`, `packages/**`(22 crate の `Cargo.toml` と `src/lib.rs`) | W0-3 と root `Cargo.toml` の members 行 |
| W0-3 | `packages/ownership/**` | W0-2 と root `Cargo.toml`。W0-2 の commit を起点に始めれば重ならない。先に始める場合、衝突する hunk は members/default-members の 1 行ずつだけ |
| W0-4 | `.github/workflows/ci.yml` | なし |

### W0-1 契約改訂

**`docs/src/design/crates.md`**

- 「決定」節の `text` ブロックを §2 の表で差し替える。
- 「Paths are ...」の文に `packages/stdlib`、`packages/ownership` を加える。
- 「Public API summary」表の `ncl-lib-*` 1 行を §2 の 11 行に置き換える。
- 「Registration and ownership rules」に §2 末尾の 3 文を加える。
- 「却下した代替案」に「`ncl-runtime` が `ncl-lib-*` へ直接依存する案は、登録順序という
  独立した関心を runtime に混ぜるため却下する」を加える。

**`docs/src/design/compiler-pipeline.md`**

「決定」の後に「front end の lowering 規則」節を新設し、次を書く。

- 可変キャプチャ変数: 2 つ以上のクロージャから代入される変数は 1 語 cell object に
  box し、クロージャは cell への参照を inline capture する。読み取り専用キャプチャは
  値を直接 inline する。
- クロージャ越えの `return-from`/`go`: 脱出しないと証明できない block/tagbody は catch
  record(HandlerRegion + `Throw`)で実装する。同一関数内で完結するものは `Jump`。
- `multiple-value-call`/`multiple-value-prog1`: variadic adapter ABI(`(ctx, argc, args,
  mv) -> NclStatus`)経由。固定 arity の `Call` には使わない。
- `progv` と special 変数の bind/unbind: `Builtin`。binding depth は record chain が持ち、
  非局所脱出時の復元は runtime unwinder の責務。
- `&optional`: front end が `LoadArg` + argc 比較 + default block を IR で生成する。codegen
  プロローグの責務は `&rest`/`&key` のみ(calling-convention と一致)。
- `Terminator::Throw` の operand は任意の `Word`。`(throw tag value)` の value も condition
  object も同じ経路で、catch_tag が区別する。
- `dynamic-extent` 宣言: Phase 3 の escape analysis が入るまで無視する(CLHS が許容)。
  実スタック割当は Phase 3 で `Alloc` のスタック版と SafepointMap の拡張を伴う契約変更として
  扱う。
- コンパイル時マクロ展開: 標準マクロは Rust expander を Runtime registry に登録する。
  ユーザー `defmacro` は native compile 後に `ncl-compiler-front` が定義する
  `MacroCaller` trait(`ncl-runtime` が実装)経由で呼ぶ。front end は runtime に逆依存しない。

「段階は 1a ... 2 ...」の段落の後に Phase 3〜6 を追記する。

- Phase 3: `ncl-ir` 上の pass 群(inlining、escape analysis + stack allocation、GVN、SCCP、
  LICM)と、`ncl-codegen` の SSA ベースレジスタ割当。IR 型と ABI は不変。受入は cl-bench
  幾何平均が Phase 2 着地時の同機測定以下。
- Phase 4: `ncl-sys` collector の parallel marking / evacuation。STW は維持。frame header、
  SafepointMap、ABI は不変。remembered set と forwarding の並行安全化を伴う。受入は同一
  ヒープ負荷で停止時間中央値が直列版未満。
- Phase 5: tier-1 投機的最適化。deopt 着地先は tier-0(Phase 1a テンプレート層、全 SSA 値が
  スタックスロットにある)。frame-state map は最適化フレームの各値から tier-0 スロットへの
  写像で、FASL header version 2 の新 section に置く。OSR は tier-0 → tier-1 のループ入口
  のみ。受入は全投機に対応する deopt テストがあり、deopt 後の結果が tier-0 と一致すること。
- Phase 6: 特殊化配列ループの SIMD 命令選択。ベクトルレジスタを第一級値にするため
  `RegisterId` と SafepointMap `register_mask` にレジスタクラスを加える契約変更を、Phase 3
  着地後に別途凍結する。受入は対象カーネルの実行時間が非 SIMD 版以下。

**`docs/src/design/native-backend.md`**

- 「Known limitations at the Phase 1 landing point」の (a)(c)(d) を削除し、(b) のみを
  残す。冒頭の「corrections are in progress and will be folded in by integration 14」を
  「(b) は Phase 1b(L13)で解消する」に改める。(a)(c)(d) が修正済みである根拠は §6。
- 「metadata と object」節の FASL header の記述に「version 2 は frame-state section を
  加える(Phase 5)。version 1 の loader は version 2 を拒否する」を加える。
- phase 列挙(1a、1b、1c、2)の後に「3〜6 は compiler-pipeline を参照」を加える。

**`docs/src/project/roadmap.md`**

現行 3 段落を、Wave 0〜4 の一覧(本書 §5 の要約)と本書へのリンクに置き換える。
「Phase 1a now includes ...」の段落は「Phase 1a は AArch64 で着地済み。x86-64 lowering は
Wave 1 の L14」に改める。

検証: `nix develop --command mkdocs build --strict --config-file docs/mkdocs.yml` が
exit 0。受入: 上記の各編集が差分に含まれ、`docs/src/design/` に `ncl-lib-loop` と
「integration 14」の文字列が残らない(`grep -rn "ncl-lib-loop\|integration 14"
docs/src/design/` が空)。

### W0-2 crate 骨格

§3 のとおり。検証: `nix develop --command cargo check --workspace` exit 0、
`python3 scripts/check_standards.py` exit 0、`nix develop --command cargo test
--workspace` exit 0 で passed 数が W0-2 前と同じ。`cargo clippy --workspace
--all-targets -- -D warnings` exit 0。

### W0-3 symbol-coverage ゲート

**目的**: 各レーンが自 crate の所有シンボルを漏れなく登録したことを機械検査する。
所有表は `conformance/ownership/symbols.tsv`(列: package, symbol, kind, crate, phase,
direct-expansion, notes)。`docs/src/notes/symbol-ownership.md` はその要約で、検査の
入力ではない。既存の `conformance/ownership/check.py` は所有表自体の完全性(SBCL 表面の
全行に割当があるか)を検査するもので、実装の検査はしない。

**設計**: `packages/ownership`(crate `ncl-ownership`、依存は `ncl-object` のみ)。

- `include_str!` で `symbols.tsv` を取り込み、`Row { package, symbol, kind: Vec<Kind>,
  crate_name, phase, direct_expansion }` に parse する。`kind` 列は `class+function` のように
  `+` で複数種を持つため `Vec<Kind>` とする。壊れた行(列数不足、不正 kind、不正 phase、
  不正 direct-expansion)は `OwnershipError::BadRow` を返す。
- `rows() -> Result<Vec<Row>, OwnershipError>`、`rows_for_crate(crate_name: &str, phase: u8)
  -> Result<Vec<Row>, OwnershipError>`。
- `assert_crate_coverage(runtime: &Runtime, ctx: &mut ThreadContext, crate_name: &str)
  -> Result<(), OwnershipError>`。各 Phase 1 行について次を検査する。
  - package が `Runtime::find_package` で見つかること。
  - symbol が `Package::find_symbol` で intern 済みであること(`&mut ThreadContext` を取る)。
  - kind `function` → `Runtime::function` に登録済みであること。`ncl-object` は symbol の
    function cell を書く公開 API を持たない(`set_symbol_function` は無い)ため、
    `Runtime::define_function` の registry を検査する。
  - kind `class`/`condition` → `Runtime::class` が見つかること。
  - kind `macro` → `symbol_is_macro` が真、kind `variable` → `symbol_is_special` が真、
    kind `constant` → `symbol_is_constant` が真であること。
  - kind `type`/`special-operator`/`other` → intern 済みであることのみ。
- `Missing { package, symbol, kind, reason }` を欠落ごとに 1 件返す。package/symbol 単位の
  欠落は行の全 kind を持つ 1 件、kind 単位の欠落は当該 kind を持つ 1 件とする。

**symbol flags**: `ncl-object` が `symbol_flags` / `symbol_is_*` / `set_symbol_*` を公開し、
`packages/object/src/layout.rs` の `symbol_flag`(bit 0 special、bit 1 constant、bit 2 macro、
bit 3 package-lock)を読み書きする。macro/variable/constant の検査はこれらの accessor を使う。

**使い方**: 各レーンは `tests/coverage.rs` に次の 1 テストを置く。

```rust
#[test]
fn registers_every_owned_symbol() {
    let runtime = ncl_object::Runtime::new().unwrap(); // tests 配下では unwrap 可
    let mut ctx = ncl_object::ThreadContext::new();
    ctx.register(&runtime).unwrap();
    ncl_<crate>::register(&runtime).unwrap(); // 各レーンが自 crate に実装
    ncl_ownership::assert_crate_coverage(&runtime, &mut ctx, "ncl-<crate>").unwrap();
}
```

`ncl_<crate>::register(&Runtime) -> Result<(), ObjectError>` は各レーンが自 crate に実装する
登録関数で、`ncl-ownership` 側はこれを呼ばない。

**失敗条件**: 欠落が 1 件でもあればテスト失敗。エラー表示は `package::symbol (kind):
reason` を 1 行 1 件で並べる(`OwnershipError` の `Display`/`Debug` が同じ形式を返す)。
**入力が空なら失敗**: 所有表に当該 crate の Phase 1 行が 0 件のときは
`OwnershipError::NoRows` を返し、空集合に対する偽の合格を防ぐ。

**CI**: 通常の `cargo test --workspace` に含まれる。追加ジョブは不要。

**W0-3 自身の検証**: `ncl-ownership` のテストで、(1) 所有表の全行が parse できる、
(2) `rows_for_crate("ncl-types", 1)` の件数が `grep -c $'\tncl-types\t1\t'
conformance/ownership/symbols.tsv` と一致、(3) 空 Runtime に対する `assert_crate_coverage`
が全行を Missing として返す、(4) 存在しない crate 名で `NoRows` が返る。加えて
`src/table.rs` の単体テストが parse のエラー分岐(列数不足、不正 kind、不正 phase、不正
direct-expansion)を踏む。`nix develop --command cargo test -p ncl-ownership` exit 0。受入:
上記 4 テストが選択・実行され(`tests/coverage.rs` の `test result:` 行の passed が 4)、
`check_standards.py` が violations none。

### W0-4 CI 配線

`.github/workflows/ci.yml` の `check` ジョブで、`- run: python3 scripts/reachability.py`
(現在 52 行)の直後に `- run: python3 scripts/check_standards.py` を 1 行加える。
Rust toolchain 導入より前に置けるのは、このスクリプトが Python のみで動くためである。
ベースライン(`ebdfcf86`)での `python3 scripts/check_standards.py` は manifests 7、
Rust files 103、violations none、exit 0 を確認済みで、配線で CI が赤になることはない。
macOS runner は加えない(D-24-4)。検証: ローカルで同スクリプトが exit 0。受入: CI の
`check` ジョブに当該 step が現れ緑になる。

## 5. レーン表

### 並列化の原則

- 1 レーン = 1 crate = 1 worktree = 1 ブランチ(`takeokunn-ncl-<crate>`)。
- レーン間の共有物は凍結契約と W0-2 の空 crate だけ。root `Cargo.toml` と `Cargo.lock` に
  レーンは触らない。
- 統合は wave 単位で run branch に merge し、統合者が全体 `cargo test --workspace` と
  `check_standards.py` を回す。
- 依存先 crate の API は「最初の commit で凍結」する規約とし、依存元レーンはその commit
  以降に開始する(下表の依存欄)。
- 各レーンの受入には自 crate の `registers_every_owned_symbol` テストを含める(所有
  シンボルを持つ crate のみ)。

### マイルストーン

| ID | 定義 | 観測 |
| --- | --- | --- |
| M1 | `ncl --eval "(defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) (fib 25)"` がソーステキストから native 実行され `75025` を print する | AArch64(macOS arm64)と x86-64(Linux)の両方で `packages/conformance` の統合テスト |
| M2 | ansi-test(`ca06bd919661af162c67407c9d994e881870bdb3`)が完走し合格数を出す | conformance runner の出力に合格/失敗/未実行の 3 数値と commit hash |
| M3 | cl-bench(`553fbcdf88d2ca4e340a0cdf02679055f3279c8f`)65 本が完走する | runner の出力に各 3 サンプルの最小/中央/最大と幾何平均 |

### Wave 1(17 レーン、W0 着地後)

| L | crate | path | 依存 | 所有シンボル | 前提契約 | 受入 | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| L1 | ncl-types | packages/types | object | symbol-ownership.md lane summary `ncl-types`(64) | object-layout.md 全節、crates.md API shape | 型指定子 parse、`typep`/`subtypep` の単体テスト、coverage テスト。最初の commit で `TypeSpecifier`/`typep`/`subtypep`/`TypeError` の API を凍結し、L5〜L11 と L18 はその後に開始 | ready |
| L2 | ncl-reader | packages/reader | object | `ncl-reader`(23) | object-layout.md(symbol/package)、compiler-pipeline.md 「front end は reader output を...」 | 標準 readtable の全 macro char、`#.` `#+` `#-` `#'` `#\` `#(` `#*` `#:` `#|`、`*read-base*`、`read-from-string` 往復テスト、coverage | ready |
| L3 | ncl-printer | packages/printer | object | `ncl-printer`(24) | object-layout.md | `*print-*` 変数群、`prin1`/`princ`/`pprint`、`write-to-string` が reader と往復、coverage | ready |
| L4 | ncl-conditions | packages/conditions | object | `ncl-conditions`(129) | native-backend.md 「handler、cleanup、catch は ThreadContext の 3 本の現在ポインタで record chain」、calling-convention.md | handler/restart/cleanup record が契約の chain 形式、`handler-bind`/`handler-case`/`restart-case` の runtime 側、標準 condition 階層、coverage | ready |
| L5 | ncl-lib-numbers | packages/lib/numbers | object, types(API), conditions(API) | `ncl-lib-numbers`(164) | object-layout.md(bignum/ratio/float/complex)、compiler-pipeline.md 直接展開 primitive | 数値塔の全演算、bignum 演算の往復、`ash`/`boole`/`random`、coverage | ready(L1 API 凍結後、L4 後) |
| L6 | ncl-lib-sequences | packages/lib/sequences | object, types(API), conditions(API) | `ncl-lib-sequences`(150) | object-layout.md | list/sequence/tree 関数、`equal`/`equalp`、`sort` 安定性、coverage | ready(L1、L4 後) |
| L7 | ncl-lib-strings | packages/lib/strings | object, types(API), conditions(API) | `ncl-lib-strings`(108) | object-layout.md(character、string) | 文字述語と変換、Unicode 名前表、coverage | ready(L1、L4 後) |
| L8 | ncl-lib-hash-arrays | packages/lib/hash-arrays | object, types(API), conditions(API) | `ncl-lib-hash-arrays`(55) | object-layout.md(array、hash table) | `aref`/`adjust-array`/fill pointer、hash table の 4 test、synchronized、coverage | ready(L1、L4 後) |
| L9 | ncl-lib-streams | packages/lib/streams | object, types(API), conditions(API), sys | `ncl-lib-streams`(121) | crates.md 「ncl-sys の OS surface」 | fd-stream の read/write、string stream、broadcast/concatenated/two-way、external format(utf-8/latin-1)、coverage | ready(L1、L4 後) |
| L10 | ncl-lib-pathnames | packages/lib/pathnames | object, types(API), conditions(API), strings(API), sys | `ncl-lib-pathnames`(43) | crates.md | pathname parse/merge/namestring、directory/probe-file、coverage | ready(L1、L4、L7 後) |
| L11 | ncl-lib-packages | packages/lib/packages | object, types(API), conditions(API) | `ncl-lib-packages`(58) | object-layout.md(symbol flags、package) | `defpackage` 相当の runtime 側、package-local nickname、package lock condition、coverage | ready(L1、L4 後) |
| L12a | ncl-compiler-front 前半 | packages/compiler/front(`env`、`expand`、`special` モジュール) | ir, object, types(API), reader(API), conditions(API), clos | `ncl-compiler-front`(151)のうち特殊形式、宣言、lambda list keyword | compiler-pipeline.md 「front end の lowering 規則」(W0-1 追記) | 全 25 特殊形式 + lambda list を内部 AST に解析、`MacroCaller` trait 定義、declaration/type propagation、最初の commit で内部 AST 契約を凍結 | ready |
| L12b | ncl-compiler-front 後半 | packages/compiler/front(`lower` モジュール) | L12a の AST 契約 | 同上の残り(compiler macro registry、`compile-file-line` など) | compiler-pipeline.md 「Complete IR contract」、ncl-ir の `verify` | AST → `ncl-ir` lowering。D4 の各規則(cell boxing、脱出 block、MV call、`&optional`)に 1 テストずつ、生成 IR が `verify()` を通る、coverage | ready(L12a の契約 commit 後) |
| L13 | Phase 1b | packages/codegen(`frame.rs`、`machine.rs`、regalloc 新規モジュール)、packages/sys(`thread.rs`、`code.rs`、`heap/collect.rs`) | なし | なし | native-backend.md phase 1b、calling-convention.md | 線形走査割当 + spill、`fib(25)` フィクスチャが template 版より遅くない、深さ 10,000 再帰中の safepoint GC で全フレームの function object が forward される(制限 (b) 解消)。L14 の merge 後に rebase | ready |
| L14 | x86-64 lowering | packages/codegen(`target_x86_64.rs`、`target_x86_64_lowering.rs`、`target_x86_64_lowering/ops.rs` 新規、`isa_x86_64.rs`)、`tests/exec_x86_64.rs` | なし | なし | native-backend.md 物理呼び出し規約表(x86-64 SysV 列)、calling-convention.md | AArch64 の Phase 1a 7 フィクスチャ((a)〜(f) と e2)が x86-64 Linux で通る。**Wave 1 先頭優先**: CI は x86-64 のみで、現状生成コードを 1 バイトも実行していない。このレーンが着地するまで CI 緑は生成コードの正しさを意味しない。共有ファイルは `codegen/src/lib.rs` の module 宣言のみで、L13 より先に merge する | ready |
| L15 | ncl-threads | packages/threads | object, sys, conditions(API) | `ncl-threads`(130) | threads.md 全節、gc-interface.md(safepoint) | thread 生成/join、mutex/condvar/semaphore、deadline、`with-interrupts`、timer、2 スレッド並行 allocation 中の GC、coverage | ready |
| L16 | ncl-ffi | packages/ffi | object, sys, conditions(API) | `ncl-ffi`(106) | crates.md 「ncl-ffi reaches dynamic loading only through ncl-sys」 | alien 型、`dlopen`/`dlsym` 経由の foreign call(libc `strlen` 相当を自前宣言で呼ぶ)、SAP、coverage | ready |
| L17 | ncl-image | packages/image | object, objfile, sys | `ncl-image`(7) | native-backend.md(code space、FASL)、gc-interface.md | ヒープ + code space + symbol table の save/load 往復、load 後に GC が動く、coverage | ready |

L12a/L12b 注: `ncl-compiler-front -> ncl-clos` は隣接表にあるが、`ncl-clos` は Wave 2 の
レーン(L18)である。Wave 1 の compiler-front は W0-2 の空 `ncl-clos` スケルトンに対して
コンパイルし、CLOS 依存機能の完成は L18 の着地後になる。

### Wave 2(5 レーン、依存先着地後)

| L | crate | path | 依存 | 所有シンボル | 前提契約 | 受入 | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| L18 | ncl-clos | packages/clos | types, conditions | `ncl-clos`(147) | object-layout.md(instance、layout generation)、compiler-pipeline.md(inline cache key) | `defclass`/`defmethod`/`defgeneric` の runtime 側、標準 method combination、MOP、class redefinition と instance update、`change-class`、coverage | ready(L1、L4 後) |
| L19 | ncl-lib-macros | packages/lib/macros | reader, conditions, types, sequences | `ncl-lib-macros`(97、LOOP 含む) | compiler-pipeline.md 「コンパイル時マクロ展開」(W0-1 追記) | 全標準マクロの Rust expander、`setf` expander、LOOP の全節(ANSI 6.1)、`macroexpand`、coverage | ready(L2、L4、L6 後) |
| L20 | ncl-lib-format | packages/lib/format | streams, strings, printer | `ncl-lib-format`(1) | なし(CLHS 22.3) | 全 directive、`~{`/`~<`/`~^` の入れ子、pretty printing directive、coverage | ready(L3、L7、L9 後) |
| L21 | ncl-stdlib | packages/stdlib | 全 lib-*, clos, types, reader, printer, conditions | なし | crates.md 「Registration and ownership rules」(W0-1 追記) | `register_all` が依存順に全 `register` を呼び、二度目は `ObjectError`、呼出後に全 crate の coverage テストが通る | ready(L1〜L11、L18〜L20 後) |
| L22 | Phase 4 並列 GC | packages/sys(`heap/collect.rs`、`stw.rs`、`heap_state.rs`、`heap.rs` の write barrier) | なし | なし | gc-interface.md、threads.md | 並列 marking / evacuation、sharded remembered set、forwarding の CAS 化。同一ヒープ負荷で停止時間中央値が直列版未満、既存 GC テスト全通過 | ready(L13 の merge 後。`collect.rs` の走査ディスパッチが重なるため) |

### Wave 3(統合)

| L | crate | path | 依存 | 所有シンボル | 受入 | 状態 |
| --- | --- | --- | --- | --- | --- | --- |
| L23 | ncl-runtime + ncl bin | packages/runtime、src/main.rs | compiler-front, codegen, stdlib, image, threads, ffi | `ncl-runtime`(Phase 2、59) | `eval`/`compile`/`compile-file`/`load`、`MacroCaller` 実装、REPL、`--eval`/`--load`/`--script`、**M1** を両アーキテクチャで | ready(L12b、L13、L14、L21 後) |
| L24 | ncl-conformance | packages/conformance | runtime, ownership | なし | ansi-test / cl-bench / 起動 / サイズ / compile-file の runner。SBCL と同日同機で対計測し commit hash を記録。**M2**、**M3** の初回数値 | **blocked**(下記) |

L24 の blocked 解除手順: Wave 3 開始時に ansi-test の `rt.lsp`、`rt-package.lsp`、
`doit.lsp`(リポジトリ外)を読み、RT が使う CL シンボルを列挙して所有 crate に写像する。
Wave 1/2 で全 lib crate を作るため、写像結果でレーン構成は変わらない。確定するのは
M2 の前提 crate 集合と、その集合の coverage テストが全て緑であることの確認手順である。

### Wave 4 以降(性能 Phase、contrib)

| L | Phase | crate | 依存レーン | 受入 | 状態 |
| --- | --- | --- | --- | --- | --- |
| L25 | 1c | codegen | L13 | self tail call、一般 tail transfer、`&rest`/`&key` 専用プロローグ。深さ 1,000,000 の自己再帰でフレーム不増 | ready |
| L26 | 2 | compiler-front, codegen | L12b, L25 | fixnum/double unbox、型推論接続、4 エントリ inline cache。`fib(25)` が SBCL の 2 倍以内(同機、commit hash 記録) | ready |
| L27 | 3 | ir(新 pass モジュール) | L12b | inlining、escape analysis + stack allocation、GVN、SCCP、LICM。各 pass に IR 往復テストと `verify()` 通過 | ready |
| L28 | 3 | codegen | L13 | SSA ベースレジスタ割当。cl-bench 幾何平均が Phase 2 着地時以下 | ready |
| L29 | 5 | codegen, objfile, sys | L27, L28 | tier-1 投機 + frame-state map(FASL v2)+ deopt/OSR。全投機に deopt テスト、deopt 後の結果が tier-0 と一致 | ready |
| L30 | 6 | codegen, asm-x86-64, asm-aarch64 | L28 | SIMD 命令選択。対象カーネルが非 SIMD 版以下 | **blocked**: `RegisterId`/SafepointMap `register_mask` へのレジスタクラス追加を L28 着地後に契約として凍結してから開始 |
| L31〜L50 | contrib | `ncl-contrib-<name>`(20 crate、Wave 4 kickoff commit で一括登録) | L23 | 所有表 Phase 3 行の coverage、NFR-004 のロードマトリクス | ready(D-24-3 により全て Rust) |

## 6. 調査で判明した verified 事実

いずれも `ebdfcf86` で行を読んで確認したもの。行番号は移動するので、再確認は同名の
関数を検索すること。

| 事実 | 根拠 | 帰結 |
| --- | --- | --- |
| `native-backend.md` の既知制限 (a)(c)(d) は修正済み。(b) のみ残る | (a) `packages/sys/src/code.rs` `scan_frame` が `frame_words` 全体を走査、`thread.rs` `set_native_frame` が `frame_words` 長の snapshot を取る。(c) `packages/codegen/src/target_aarch64.rs` の `spill_arguments` がプロローグで op ループ前に全引数を spill し、`LoadArg` は `[x29 + offset]` から読む。(d) `thread.rs` `write_back_frame_snapshot` が `frame_address`/`frame_chain`/`frame_registers` を消去し回帰テスト `frame_snapshot_write_back_consumes_snapshot` がある。(b) `thread.rs` `scan_native_frame` は `frame_chain.get(1)` の return PC 1 つだけを走査し、`heap/collect.rs` の本番経路はこの関数を呼ぶ。チェーン走査 `scan_frame_chain_with_registry` はテスト専用の `set_frame_snapshot` 経路からしか呼ばれない | W0-1 で文書を直す。(b) は L13 |
| CI は ubuntu x86-64 のみで、生成コードを実行していない | `packages/codegen/tests/exec_aarch64.rs` 冒頭の `#![cfg(target_arch = "aarch64")]`、`.github/workflows/ci.yml` の全ジョブが `ubuntu-latest`。`packages/codegen/src` に `target_x86_64*` は無く、`isa_x86_64.rs` はテンプレート分類と単独 `prologue` のみ | L14 を先頭優先 |
| `scripts/check_standards.py` は CI から呼ばれていない | `grep -rn check_standards` のヒットは `docs/src/design/coding-standards.md` のみ。CI は `scripts/reachability.py` を呼ぶ | W0-4 |
| 所有表に `ncl-lib-loop` 行が無い | `conformance/ownership/symbols.tsv` で LOOP と LOOP-FINISH は `ncl-lib-macros`。`grep -c ncl-lib-loop` は 0 | D-24-2 |
| `crates.md` に `ncl-runtime -> ncl-lib-*` の辺が無く、`register(&Runtime)` の呼び手が不在 | `crates.md` の `ncl-runtime -> ncl-compiler-front, ncl-codegen, ncl-image, ncl-threads, ncl-ffi` | D-24-2、`ncl-stdlib` |
| `&optional` が設計文書に未記載 | `docs/src/design/*.md` を `&optional` で grep して 0 件。`&rest`/`&key` はプロローグの記述がある | W0-1 の lowering 規則 |
| 凍結 IR は ANSI CL の全機能を variant 追加なしで表現できる。例外は `dynamic-extent` の実スタック割当 | `packages/ir/src/types.rs` の `OpKind`/`Terminator`/`HandlerRegion`、`Terminator::Throw`、`SetMultipleValues`、`catch_tag`。クロージャ、special bind、unwind-protect、progv、MV call は `Builtin`/`Alloc`+`StoreField`/HandlerRegion で lowering できる | W0-1 の lowering 規則 |
| reader、macroexpander、evaluator、printer はツリーに存在しない。`src/main.rs` は `--version` のみの stub | `packages/`、`src/` を `readtable|macroexpand|fn read|Form|eval|print_object|lambda_list` で grep し、IR 自身のテキスト形式以外にヒット無し | Wave 1〜3 |
| object 層は bignum/ratio/double/complex/stream/readtable/closure/instance を含む 18 の `make_*` を持つ | `packages/object/src/number.rs`、`stream.rs`、`readtable.rs`、`function.rs`、`instance.rs` | lib レーンは object 拡張を待たない |
| GC は単一ロック STW、`HashSet` remembered set、単一スレッド collector | `packages/sys/src/stw.rs`、`heap_state.rs`、`heap/collect.rs` | L22 の書換範囲 |
| `cargo test --workspace` は exit 0、147 passed / 0 failed / 1 ignored | `nix develop --command cargo test --workspace`(2026-09-24、macOS arm64) | W0-2 の受入基準 |
| `check_standards.py` は manifests 7、Rust files 103、violations none、exit 0 | `python3 scripts/check_standards.py`(2026-09-24) | W0-4 で CI が赤にならない |
| SBCL 2.6.0 基準値は macOS arm64 で取得 | `conformance/baselines/environment.md` | 受入測定は同環境 |
