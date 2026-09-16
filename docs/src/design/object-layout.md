# Object layout

## 決定

`Word` は 64-bit tagged value で、NIL と T は静的に固定配置する。cons は header なしの `(car, cdr)` 2 語、header object は 1 語 header の後ろに payload を置く。bit 0..7 は widetag、bit 8..15 は GC flags、bit 16..63 は size/length とする。generation と pin は page metadata に置く。

lowtag の契約は次のとおりである。`listp` は list lowtag の検査だけ、`consp` は list lowtag かつ NIL でない値、`symbolp` は NIL または other-pointer と symbol widetag の組み合わせを検査する。つまり symbolp は「NIL または other-pointer + symbol widetag」である。character、single-float、function は対応する immediate/function lowtag を使う。unbound marker は予約済み other-immediate である。

symbol は value、function、plist、package、name、`tls_index: u32`、identity-hash slot、flags word を持つ。flags word は bit 0 special、bit 1 constant、bit 2 macro、bit 3 package-lock、残りを予約とする。cons 専用 page と header-object page は混在させず、pin は page attribute とする。

所有境界は [GC interface](gc-interface.md) と [Native backend](native-backend.md) に従う。heap、code space、per-thread roots は `ncl-sys` が所有し、`ncl-object` は widetag、accessor、symbol/package/intern、`Runtime`、`ThreadContext` wrapper、register 型を提供する。weak pointer は強 root ではなく、到達不能後の finalizer queue だけを強く保持する。

## 根拠

NIL を list lowtag として扱うことで list predicate を高速にし、symbol predicate だけは NIL の言語仕様を明示できる。固定 header と page 種別は collector が payload の解釈を推測せずに済む。移動対象と code space を分けることで return PC の再配置を不要にする。

## 却下した代替案

- 全 object に hash word を追加する案は payload と GC scan を膨らませるため却下する。
- cons と header object を同一 page に置く案は scan mode を曖昧にするため却下する。
- pin を generation として表す案は collector の状態を誤分類するため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- lowtag、widetag、header bit 割当、cons 2 語、symbol flags の値を変更しない。
- Rust の未登録領域に heap `Word` を保持せず、移動 object の address を code bytes に埋め込まない。
- accessor は object ownership を越えて raw OS API を直接呼ばない。

## Concrete representation tables

| bit 1..3 | kind | payload |
| --- | --- | --- |
| 000 | character | Unicode scalar in bit 4..24 |
| 001 | list pointer | aligned cons address, two words |
| 010 | single-float | IEEE-754 binary32 in bit 4..35 |
| 011 | function pointer | simple-fun or closure |
| 100 | other-immediate | unbound marker and reserved immediates |
| 101 | instance pointer | structure or CLOS instance |
| 110 | reserved | must not be allocated |
| 111 | other pointer | symbol, string, vector, number, package |

Fixnum uses bit 0 = 0, a signed 63-bit payload, and `most-positive-fixnum = 4611686018427387903`. `consp` checks the list lowtag and excludes NIL; `listp` checks NIL or that lowtag. `fixnump` checks bit 0. Character, single-float, and function predicates inspect their lowtag. `symbolp`, `stringp`, and `simple-vector-p` inspect the widetag after the other-pointer lowtag.

| header bits | meaning |
| --- | --- |
| 0..7 | widetag |
| 8..15 | GC flags: young, marked, forwarded, finalizable, weak, hashed |
| 16..63 | size or length |

Cons pages contain `(car, cdr)` with no header. Header-object pages contain the one-word header followed by payload. Page metadata stores generation and pin state.

| object | payload layout |
| --- | --- |
| cons | car, cdr |
| symbol | value, function, plist, package, name, tls_index, hash, flags |
| string | base-char u8; character UTF-32 scalar units |
| simple-vector | length and contiguous Word elements |
| specialized array | bit, unsigned/signed integer, single-float, double-float, or character elements |
| non-simple array | dimensions, fill-pointer, displaced-to, offset, adjustable flag |
| hash-table | open addressing, power-of-two capacity, 7/8 load threshold, four weakness modes, synchronized flag |
| structure | layout descriptor and slots |
| CLOS instance | class pointer, indirect slot vector, layout generation |
| simple-fun / closure | entry and code object; closure captured values inline |
| bignum / ratio | sign and little-endian u32 limbs; numerator and denominator |
| double-float / complex | binary64; real and imaginary values |
| package / readtable / stream | names and tables; syntax and dispatch tables; direction, element type, buffer and state |
| code object | entry, size, constant table, stack-map index, debug table |

Symbol flags are bit 0 special, bit 1 constant, bit 2 macro, bit 3 package-lock, with the rest reserved. NIL and T are statically allocated, scanned but never moved. NIL is list-lowtag compatible: its car and cdr positions point to NIL and align with symbol value/function cells. Symbols and instances keep identity hashes in slots. Other `eq` keys use address hashing and the hashed flag, with rehash notification after movement.

## Phase 1 payload and registration contract

Header-object payload references are registered from payload-relative offsets by adding one header word. The collector consumes the resulting header-inclusive `reference_words` and optional `boxed_from`; scalar metadata must never be registered as a reference. References are placed after scalar metadata (scalars-first), and every reference store goes through the object access path and its write barrier. (`packages/object/src/layout.rs`, `reference_words`; `packages/object/src/gc.rs`, `register_layouts`; `packages/object/src/object_access.rs`.)

At 340464b0, the observed HASH_TABLE payload has scalar slots 0..7 (`TEST`, `WEAKNESS`, `COUNT`, `CAPACITY`, `EPOCH`, `FREE_HEAD`, `HIGH_WATER`, `OCCUPIED`) followed by reference slots 8..10 (`MARKER`, `KV`, `INDEX`). The PACKAGE payload has scalar slots 0..1 (`LOCK`, `GENSYM`) followed by reference slots 2..9 (`NAME`, `NICKNAMES`, `USE_LIST`, `USED_BY`, `INTERNAL`, `EXTERNAL`, `SHADOWING`, `LOCAL_NICKNAMES`). These slot numbers are a snapshot, not the cross-lane contract: the invariant is scalars-first, registration through `ReferenceLayout`, and barrier-protected reference stores. (`packages/object/src/hash_table.rs`, `packages/object/src/package.rs`, `packages/object/src/gc.rs`.)

Package `import`, `unintern`, `export`, `use`, `shadow`, and `nickname` operations follow CLHS semantics and maintain the `USED_BY` and `NICKNAMES` reference slots. (`packages/object/src/package.rs`, `import`, `unintern`, `export`; `packages/object/src/package/lists.rs`, `use_package`, `shadow`, `add_nickname`.)

Hash tables use open addressing with `EMPTY` as a probe terminator and `TOMBSTONE` as a reusable deletion marker. Lookup passes tombstones, insertion may reuse them, and resize/rehash repacks live entries. `Eq` compares identity and uses identity hashing; `Eql` adds numeric equality and numeric hashing; `Equal` hashes string/cons and numeric contents; `Equalp` applies ASCII case folding to content hashing and comparison. When identity or numeric semantics require it, the table epoch is compared with the current heap epoch and the table is rehashed before lookup/update. (`packages/object/src/hash_table.rs`, `rehash_if_needed`; `packages/object/src/hash_table/equality.rs`, `equal`, `hash_key`.)

`Runtime::new` and `Runtime::with_config` return `Result`. Runtime registries are heap hash tables retained through boxed `Word` slots and heap `RootToken`s; `Runtime` does not keep managed words in an unregistered Rust `HashMap` or `Vec`. `register_layouts` is idempotent after the first complete registration. (`packages/object/src/lib.rs`, `Runtime`, `RootedTable`, `register_layouts`.)
