# DEFSTRUCT

## 決定

`defstruct` は CLOS registration layer の adapted macro として展開する。
構造体本体は既存の `widetag::STRUCTURE` と `StructureLayout` を使い、payload
は `[layout-id, slot0, ...]` とする。layout の slot 数と名前付き metadata は
`Runtime` の登録表で管理し、GC をまたぐ一時的な Lisp 値は `Scope`、`Handle`、
`HandleVec` で root する。

macro は構造体定義の登録と、keyword constructor、predicate、reader、writer、
copier を生成する。accessor の writer は `(setf accessor)` の function cell
に登録し、constructor と copier は `make_structure`、`structure_ref`、
`structure_set` を経由する。`:include` は親の slot metadata を先に並べ、子の
同名 slot で上書きする。structure の `class-of` は対応する structure class
を返し、`typep` と `defmethod` の specializer は class identity と継承関係を
使う。既定の印字は `#S(name slot value ...)` とする。

初回実装の `:type list`、`:type vector` の unnamed/named 表現と高度な BOA
lambda-list は未実装として明示的な error にする。`print-function` と
`print-object` は既定の `#S` 表示を優先し、専用 callback は後段に残す。

## 根拠

`packages/object/src/structure.rs` の allocation、slot access、write barrier と
`packages/object/src/gc.rs` の structure slot scan を再利用できる。CLOS の class
descriptor と macro registration は既存の `DEFCLASS`、`DEFGENERIC`、`DEFMETHOD`
と同じ経路に置けるため、structure を CLOS instance に変換せずに dispatch の
class-of/typep 契約へ接続できる。

## 却下した代替案

- structure を CLOS instance と同じ payload にする案は専用 widetag と既存 GC
  layout を捨てるため採用しない。
- layout id や slot metadata を展開形へ raw `Word` として埋め込む案は、GC と
  registry lifecycle をまたぐ値を root できないため採用しない。
- metadata を未登録 Rust の `HashMap` に保持する案は Runtime の所有境界に
 反するため採用しない。

## 検証対象

constructor/accessor/setf/predicate/copy、`:include` の多段継承、BOA の対応範囲、
structure specializer の `defmethod`、`#S` print、gc_stress と strict_forwarding
を組み合わせた生成・更新・copy を専用 E2E と単体テストで確認する。
