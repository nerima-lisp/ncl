# NCL コーディング規約

この文書は Phase 1 の全レーンが参照する実装契約である。後方互換性は目標にしない。設計上の不一致は実装で吸収せず、この文書と関連する設計契約を先に更新する。機械的な項目は `python3 scripts/check_standards.py` で検査する。

## 1. DDD と境界づけられたコンテキスト

クレートを境界づけられたコンテキストとする。公開 API はコンテキスト間の契約であり、内部型や GC 管理値を隣のクレートへ漏らさない。

| コンテキスト | クレート | 中心語彙 |
| --- | --- | --- |
| object model / heap | `ncl-sys`, `ncl-object` | `word`, `widetag`, `cons`, `symbol`, `package` |
| reader / printer | `ncl-reader`, `ncl-printer` | `readtable`, `dispatch macro`, `print readably` |
| type / condition | `ncl-types`, `ncl-conditions` | `type specifier`, `condition`, `handler`, `restart` |
| CLOS | `ncl-clos` | `class`, `slot`, `generic function`, `method`, `method combination` |
| compiler | `ncl-compiler-front`, `ncl-ir`, `ncl-compiler-back` | `macroexpand`, `declaration`, `form`, `IR`, `instruction` |
| standard libraries | `ncl-lib-*` | CLHS の各関数、`pathname`、`stream`、`sequence` など |
| execution boundary | `ncl-threads`, `ncl-ffi`, `ncl-image`, `ncl-runtime`, `ncl-conformance` | `thread`, `alien`, `image`, `eval`, `compile`, `load` |

ユビキタス言語は CLHS の用語に限定する。型名、モジュール名、公開 doc コメントも同じ語彙を使い、略語や独自語を導入しない。`Symbol`、`Package`、`Readtable`、`GenericFunction` はよいが、`Sym`、`Pkg`、`ReaderConfig` は契約にない限り使わない。Rust の表記には `symbol-value` を `symbol_value`、`*print-base*` をローカル変数 `print_base` とする規則だけを適用する。

### エンティティと値オブジェクト

エンティティはライフサイクル中の同一性で識別する。`symbol`、`package`、`class`、`thread` はエンティティで、GC または Runtime の所有権境界を越える識別子を newtype にする。値オブジェクトは値で比較する。`Word`、`TypeSpecifier`、`Pathname` の各成分、文字は値オブジェクトである。

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Word(u64);

#[derive(Debug, Eq, PartialEq)]
pub struct SymbolId(Word);
```

`Word` はタグ付き Lisp 値であり `Copy` を許可するが、Rust メモリに GC 管理の `Word` を置くことは許可しない。`SymbolId` の `PartialEq` は同一性比較を表し、内容比較ではない。可変エンティティに `Clone` を実装して所有権の複製に見せかけない。

### 集約、サービス、リポジトリ

集約の外から不変条件を破るフィールドアクセスを許可しない。`Package` の symbol table は `intern`、`unintern`、`shadow` を通してのみ変更する。hash table の rehash は GC からの移動通知を受けた処理だけが行う。`Runtime` は package table、function registry、class table、GC settings を所有し、`ThreadContext` は TLAB、binding stack、roots、handlers、safepoint、multiple values を所有する。

`intern`、`compile`、`macroexpand` はドメインサービスである。package table、class table、function registry は `ncl-object` の `Runtime` 内に置く repository 相当の registry とする。library の `register(&Runtime)` は `ncl-object` の契約へ登録するだけで、中央の独自登録表を作らない。`ncl-sys` の生 API を `ncl-object` が型付きで包む層を腐敗防止層とする。

## 2. Rust の構成と API

1 ファイルは目安 300 行、上限 500 行とする。`mod.rs` は使わず、`foo.rs` と必要なら `foo/` を使う。既定の可視性は非公開で、クレート内共有には `pub(crate)`、外部契約に列挙された項目だけに `pub` を付ける。

```rust
#[must_use]
pub fn alloc(
    thread: &mut ThreadContext,
    runtime: &Runtime,
    kind: TypeTag,
    words: usize,
) -> Result<Word, StorageCondition> { /* ... */ }
```

値の意味が型で表せるときは newtype を使い、表現を ABI として固定するときだけ `#[repr(transparent)]` を使う。無視するとバグになる `Result`、token、builder は `#[must_use]` とする。将来の variant 追加を利用者に強制したくない公開 enum は `#[non_exhaustive]` とする。

エラーは各クレートの専用 `enum` とし、`std::error::Error`、`Display`、必要な `From` を手書き実装する。`thiserror` と `Box<dyn Error>` は使わない。失敗を呼び出し側へ返し、`?` で伝播する。

```rust
pub fn read_form(input: &mut Input) -> Result<Form, ReaderError> {
    let token = input.next_token()?;
    parse_token(token)
}
```

本番コードで `unwrap`、`expect`、`panic!` を使わない。`todo!()` は Phase 0 の骨格に限り許可し、検査スクリプトが件数を報告する。`unsafe` は `ncl-sys` のみで許可し、各ブロックに `// SAFETY:` とアラインメント、寿命、所有権、FFI の不変条件を書く。`Send`/`Sync` は実際の共有・排他条件を確認して明示する。自動導出の `Clone` は安価な値だけに限定する。

iterator を優先し、`impl Trait` は戻り値で抽象的な iterator を返す用途を基本とする。ISA や allocator の差し替えのような戦略は trait、閉じたデータ集合は enum で表す。

全ての `pub` 項目に doc コメントを付ける。公開関数のコメントには該当する `# Errors`、`# Panics`、`# Safety` 節を記載する。panic しない場合も `# Panics` に `Panics: Never` と明記する。

## 3. Lisp 値、GC、OS 境界

`Word` は `#[repr(transparent)] struct Word(u64)` とし、heap に触る API の第一引数を `&mut ThreadContext`、共有状態を `&Runtime` とする。Rust の `Vec<Word>`、`HashMap<_, Word>`、`static`、`Box` に GC managed value を置かない。例外は `push_root` 済み slot、または Runtime に root 集合として登録した構造体だけである。field store は write barrier を通し、blocking I/O、sleep、外部呼出しは `enter_native` と `leave_native` で対にする。

OS 機能は `ncl-sys` 内の手書き `extern "C"` 宣言で提供する。対象は `mmap`、`munmap`、`mprotect`、`pthread_create`、`pthread_join`、`pthread_self`、pthread attribute の stack API、mutex、condition variable、`dlopen`、`dlsym`、`dlerror`、`clock_gettime`、`read`、`write`、`open`、`close`、`stat`、`opendir`、signal 関連、macOS の `pthread_jit_write_protect_np` と `sys_icache_invalidate` である。`cfg(target_os)` と ABI の違いは `ncl-sys` に閉じ込める。

外部 crate はゼロとする。標準ライブラリの `std::collections::HashMap` は許可するが、ハッシュの安全性や再現性が契約に関わる箇所は SipHash 等を自前実装する。乱数は OS entropy API を `ncl-sys` から包む。`sb-unicode` の47シンボルに必要な UnicodeData は生成スクリプトから Rust の静的な表へ変換し、実行時ファイルに依存しない。bignum は little-endian の `u32` limb 演算を自前実装する。正規表現 crate は導入しない。

## 4. 良い例と悪い例

```rust
// 良い: domain error を契約の型で返す
pub fn unintern(package: &mut Package, name: SymbolName) -> Result<bool, PackageError> {
    package.remove_symbol(name)
}
```

```rust
// 悪い: panic と曖昧な動的エラーで不変条件を隠す
pub fn unintern(package: &mut Package, name: &str) -> Box<dyn std::error::Error> {
    package.table.remove(name).expect("symbol exists");
}
```

```rust
// 良い: trait は差し替え可能な戦略を表す
pub trait InstructionSelector {
    fn select(&self, form: &Form) -> Result<Instruction, CodegenError>;
}
```

```rust
// 悪い: closed set を文字列と分岐漏れで表す
pub fn select(isa: &str) -> Result<Vec<String>, String> { /* ... */ }
```

## 5. テスト、bench、lane の作法

テストは標準の `#[test]` だけを使う。`rstest` は禁止し、パラメータ化は入力と期待値の table を走査するヘルパで書く。bench は `criterion` を使わず、`ncl-conformance` の runner として測る。テストでも本番契約を無効化する `unsafe`、skip、弱い assertion を追加しない。

各 lane は自クレートとその tests だけを編集し、他クレート、`conformance/`、設定ファイルを変更しない。依存を追加せず、crate manifest は `[lints] workspace = true` を持つ。検証は `nix develop path:. --command cargo ...` の形で実行し、報告には変更ファイル、コマンド、終了状態、選択されたテスト数を記載する。コミットが必要な場合は対象ファイルを pathspec に明記し、push と PR は行わない。

## 6. 機械検査の契約

`python3 scripts/check_standards.py` は `packages/*` 以下を走査し、依存が path/workspace 以外でないこと、`packages/sys` 外の `unsafe`、500 行超の `.rs`、`mod.rs`、テスト外の `unwrap(`/`expect(`/`panic!(` を検出する。テスト外の `todo!()` 件数は報告する。違反が一つでもあれば非 0 で終了する。これは完全な Rust parser ではないため、cargo、clippy、レビューの代替ではない。

骨格段階の違反は隠さず、その時点の lane 状態として報告する。検査結果には対象ファイル数と違反の全出力を含める。
