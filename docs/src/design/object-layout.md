# Object layout

## 決定

NCL の Lisp 値は常に 64-bit の `Word` で表す。タグは下位 3 bit とし、値は次の通り固定する。

| tag | 種別 | 表現 |
| --- | --- | --- |
| `000` | fixnum | `n << 1`。符号付き 62-bit、範囲 `-2^62`..`2^62-1` |
| `001` | character | bit 3..23 に Unicode scalar value、bit 24..31 に character flags |
| `010` | single-float | bit 3..34 に IEEE-754 binary32 の bit pattern |
| `011` | heap pointer | 8-byte aligned address に `011` を加えた値 |
| `100` | immediate true | `+0x4` |
| `101` | immediate nil | `+0x5` |
| `110` | immediate unbound | `+0x6` |
| `111` | reserved | 常に invalid-value condition |

従って `most-positive-fixnum` は `4611686018427387903` である。pointer の復号は `word - 3` の一通りにし、GC は tag `011` だけを heap reference として扱う。

全ヒープオブジェクトは 16-byte header と 8-byte word 配列を持つ。header は low-to-high の順に、`type: u16`、`flags: u16`、`size_words: u32`、`hash: u64` とする。flags は bit 0 young、1 marked、2 pinned、3 finalizable、4 weak、5 forwarded、6..7 generation (0..3)、8..15 reserved。forwarding 中は payload word 0 に新アドレスを置く。

オブジェクト payload は以下で固定する。offset は header 直後を 0 とし、参照フィールドは必ず `Word` である。

| object | payload |
| --- | --- |
| cons | `car`, `cdr` |
| symbol | `value`, `function`, `plist`, `package`, `name`, `special_tls_index: u32`, flags |
| string | `length: u64`, `element_type`, packed code units。base-char は u8、character は UTF-32 |
| simple-vector | `length`, followed by `Word[length]` |
| specialized array | rank, dimensions, element type, data offset, data。`bit`, `u8`, `u16`, `u32`, `u64`, `i8`, `i16`, `i32`, `i64`, `single-float`, `double-float`, `character` を許す |
| array | rank, dimensions vector, data vector, fill-pointer (or unbound), displaced-to, displaced offset, adjustable |
| hash-table | test id, count, capacity, rehash size, threshold, weakness (none/key/value/key-and-value), synchronized flag, key/value arrays |
| structure | layout id, class, slot words |
| CLOS instance | class, indirect slot-vector, layout generation, slot words。再定義は indirect vector を差し替える |
| function | entry pointer, closure vector, name, lambda-list metadata |
| bignum | sign, limb count, little-endian base `2^32` `u32` limbs |
| ratio | normalized numerator and positive denominator bignum references |
| double-float | IEEE-754 binary64 bits |
| complex | real and imag references |
| package | name, nicknames, use-list, used-by list, symbol table, shadowing set |
| readtable | syntax-type table, dispatch table, case mode |
| stream | direction, element type, external format, state, implementation payload |

Hash-table buckets use open addressing with power-of-two capacity and a 7/8 maximum load factor. Weak tables are processed by GC before sweep; synchronized tables use a per-table mutex. Structure layout ids and CLOS layout generations are never reused.

## 根拠

The 64-bit word leaves 62 signed fixnum bits while preserving an 8-byte aligned pointer tag. It gives compiled arithmetic and identity checks a single-word fast path. A header-owned hash field avoids a side table and permits identity hashing before an object becomes immutable. The symbol has separate value and function cells because lexical function lookup and special bindings must not share a string-keyed environment. The TLS index is an index, not a pointer, so a symbol remains movable.

The fixnum limit and word size were observed on SBCL 2.6.0 with `(format t "~D ~D" most-positive-fixnum sb-vm:n-word-bits)` on the target machine. The representation itself is NCL-owned and is not required to match SBCL's internal headers.

## 却下した代替案

- Three low tag bits for every immediate was rejected because it leaves only 61 signed payload bits.
- `Rc` graphs and string-keyed binding maps were rejected because they leak cycles and make symbol lookup allocate.
- `ibig` was rejected; bignums are GC objects with an owned limb representation.
- A side table for symbol cells was rejected because it obscures object identity and makes moving GC harder.

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- `Word`, `HeapPtr`, `ObjectHeader`, and each payload layout are stable contracts.
- A heap pointer is valid only with tag `011`; `Word` values must be checked before dereference.
- Accessors may assume 16-byte headers and word-aligned payloads, but may not assume an object address is stable across allocation.
- No lane may add an object type, tag, header bit, or pointer side table without changing this document first.
- No lane may use `Rc`, `Arc` cycles, string-keyed symbol bindings, or `unsafe` outside `ncl-sys`.
