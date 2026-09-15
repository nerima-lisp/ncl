# NCL

NCL is being rewritten as a native Common Lisp implementation. The design
contracts for the rewrite are in `docs/src/design/`. The former interpreter,
VM, and syntax crates are retired at commit `d9bbb4ec` and retained only as
legacy tests for later migration.

## 契約との差分

`ncl-sys` は設計契約の実行時境界を次の形で具体化しています。

- `alloc_code` と `publish_code` は OS の mapping/protection 失敗を返す
  `Result` API です。公開後の entry は release store、読み取りは acquire
  load です。
- `ReferenceLayout` は header-inclusive の固定 `reference_words` に加え、
  `boxed_from` からオブジェクト末尾までを参照として扱えます。payload `N`
  は生添字 `N + 1` です。
- `Thread` は `repr(C)` で、生成コードが使う `thread_layout()` の offset を
  ABI として固定します。上位の `ThreadContext` は `Thread` を先頭に置き、
  `*mut ThreadContext` を `*mut Thread` として渡す契約です。
- code object metadata は frame size、function name、source locations を保持し、
  `CodeRegistry::release` は lookup table から除去してから mapping を解放します。
- `Heap::collect` は登録された code registry と frame snapshot を使い、return PC
  ごとの safepoint map、function object、live slots、callee-saved registers を更新します。

契約文書側への正式な反映は、run ブランチでまとめて行います。この worktree では
`docs/src/design/` を編集していません。

## Object kinds

| kind | layout | accessor | ctor | GC test |
| --- | --- | --- | --- | --- |
| symbol | registered fields | value/function/plist/name | `make_symbol` | GC accessor test |
| string / vector | length and data | string/vector accessors | `make_string`, `make_simple_vector` | GC kind suite |
| specialized array | element type and data | `specialized_array_*` | `make_specialized_array` | GC kind suite |
| non-simple array | rank-variable metadata and boxed payload tail | `array_*` | `make_array` | `non_simple_array_references_survive_minor_and_full_gc` |
| structure / instance | layout or class and slots | `structure_*`, `slot_*` | `make_structure`, `make_instance` | `remaining_object_kinds_round_trip` |
| simple-fun / closure | entry, code, inline captures | `function_*`, `closure_ref` | `make_simple_fun`, `make_closure` | `remaining_object_kinds_round_trip` |
| bignum / ratio | packed limbs or references | numeric accessors | numeric constructors | `remaining_object_kinds_round_trip` |
| double-float / complex | binary64 or two references | numeric accessors | numeric constructors | `remaining_object_kinds_round_trip` |
| stream / readtable | named descriptor slots | named accessors | `make_stream`, `make_readtable` | `remaining_object_kinds_round_trip` |
| code | entry, size, tables | `code_*` | `make_code_object` | `remaining_object_kinds_round_trip` |

The GC tests allocate each object, allocate additional objects, run minor and
full collection, and check payload values and reference identity.

下流レーンは、非 simple 配列の rank 可変 metadata が `boxed_from` から全語走査されること、kind 境界型が `Word` の transparent newtype であること、`classify_object` が Structure/Instance と SimpleFun/Closure を別 variant に返すことを前提にしてください。
