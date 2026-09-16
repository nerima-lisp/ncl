# ncl-object

`ncl-object` は `ncl-sys` の `Word` と heap API を、型検査と `ObjectError` を備えた安全な object 層として公開します。

## 公開 API

主な型は `Word`、`ObjectRef`、`ObjectError`、`Runtime`、`ThreadContext`、`ArrayElementType`、`Builtin`、`MultipleValues`、`NclStatus`、`HashTable`、`HashTest`、`Weakness`、`Package`、`FindStatus` です。

主な関数は次のとおりです。

```text
classify(Word) -> ObjectRef
register(&Runtime)
make_cons(&mut ThreadContext, &Runtime, Word, Word) -> Result<Word, ObjectError>
allocate(&mut ThreadContext, &Runtime, u8, usize) -> Result<Word, ObjectError>
make_symbol(&mut ThreadContext, &Runtime, Word) -> Result<Word, ObjectError>
car/cdr(&mut ThreadContext, Word) -> Result<Word, ObjectError>
rplaca/rplacd(&mut ThreadContext, Word, Word) -> Result<Word, ObjectError>
symbol_value/symbol_function/symbol_plist/symbol_name(&ThreadContext, Word) -> Result<Word, ObjectError>
set_symbol_value(&mut ThreadContext, Word, Word) -> Result<(), ObjectError>
push_root(&mut ThreadContext, &mut Word) -> RootToken
pop_root(&mut ThreadContext, RootToken) -> bool
write_object_slot(&mut ThreadContext, Word, usize, Word) -> Result<(), ObjectError>
make_string(&mut ThreadContext, &Runtime, &[char]) -> Result<Word, ObjectError>
string_length/string_ref/string_set
make_simple_vector(&mut ThreadContext, &Runtime, &[Word]) -> Result<Word, ObjectError>
simple_vector_length/simple_vector_ref/simple_vector_set
make_specialized_array(&mut ThreadContext, &Runtime, ArrayElementType, &[Word]) -> Result<Word, ObjectError>
specialized_array_element_type/specialized_array_ref/specialized_array_set
make_array(&mut ThreadContext, &Runtime, &[usize], ArrayElementType, Word, bool, Option<usize>, Option<Word>, usize) -> Result<Word, ObjectError>
array_dimensions/array_row_major_ref/array_row_major_set
```

`Runtime::new` と `Runtime::with_config` は `Result<Runtime, ObjectError>` を返します。`Runtime::register_layouts`、`Runtime::define_function`、`Runtime::function`、`Runtime::ensure_package`、`Runtime::find_package`、`Runtime::define_class`、`Runtime::class`、`Runtime::add_feature`、`Runtime::features`、`Runtime::gc_config` が runtime の登録・照会 API です。`define_function` と `define_class` も登録失敗を `Result` で返します。`ThreadContext::register` は heap への登録、`bind`/`unbind` は special 束縛、`set_values`/`values` は多値領域、`collect` は GC を提供します。

`ThreadContext::register` 後はコンテキストを move してはいけません。登録前に `Box<ThreadContext>` に入れるか、スタック上の同じ場所で使い続けてください。

`HashTable::new`、`insert`、`get`、`remove`、`for_each_entry`、`capacity` と `sxhash` が hash table API です。`Eq` は identity、`Eql` は数値値、`Equal` は文字列内容と cons、`Equalp` はそれらに ASCII case folding を加えた比較です。`Package::new`、`find_symbol`、`intern`、`unintern`、`export`、`unexport`、`import`、`shadow`、`use_package`、`gensym` が package API です。`package::nil()` と `package::truth()` は静的 NIL/T です。

## builtin!

固定 arity の metadata だけを登録する形式:

```rust
builtin!(CAR_BUILTIN, 1, "object");
```

直接 ABI と可変長 ABI を同時に生成する形式:

```rust
builtin!(CAR_BUILTIN, 1, "object", car_direct, car_variadic);
```

この形式は `extern "C" fn(*mut ThreadContext, Word) -> Word` と、`extern "C" fn(*mut ThreadContext, usize, *const Word, *mut MultipleValues) -> NclStatus` を生成します。arity は 0〜4 です。実装本体を接続する場合も、生成された ABI の引数型と `MultipleValues` の所有期間を変えないでください。

## register

登録関数は `RegisterFn = fn(&Runtime)` です。

```rust
fn register_my_functions(runtime: &Runtime) -> Result<(), ObjectError> {
    runtime.define_function("NCL", "MY-FUNCTION", function_object)?;
    Ok(())
}
```

`register(&runtime)` は object 層が所有する SB-EXT の GC、weak pointer、finalizer 関数名を登録します。関数 object は `FunctionObject = Word` です。

## GC と RootToken

GC を跨いで生存させる `Word` は Rust のローカル変数、`Vec`、hash table、package map に置くだけでは不十分です。必ず live な slot を root として登録し、割り当てや collection の後は更新された slot を使います。

```rust
let mut value = make_cons(ctx, runtime, Word::NIL, Word::NIL)?;
let token = push_root(ctx, &mut value);
ctx.collect(false);
let _ = car(ctx, value)?;
assert!(pop_root(ctx, token));
```

`RootToken` は LIFO です。トークンを逆順に pop し、token が有効な間は参照先の slot を move、resize、drop しないでください。Runtime の package、class、function registry は heap hash table で、Runtime が保持する managed Word は移動しない `Box<Word>` root slot とその `RootToken` だけです。未登録の Rust `Vec`、`HashMap`、package registry に managed Word を保存しないでください。

## 実装状況

| 範囲 | 状態 | 下流への注意 |
| --- | --- | --- |
| widetag、固定 layout、cons/symbol accessor | 済 | layout の payload offset は header を含まない |
| Runtime、ThreadContext、RootToken、builtin metadata | 済 | special binding と多値の高水準 ABI は利用可能 |
| hash table、package intern、GC symbol registration | 済 | test 別の hash/equality、package の internal/external/use-list を heap object で保持する |
| generational GC を跨ぐ object root 更新 | 済 | Runtime registry は安定 root slot、hash table/package payload は layout 登録済み |
| string、simple-vector、特殊化配列、非 simple 配列の typed accessor/constructor | 済 | `ArrayElementType` と row-major accessor を使用する |
| structure、CLOS instance、simple-fun、closure、bignum、ratio、double、complex、stream、readtable、code object | 部分 | payload accessor と layout は揃っている。可変長 closure は sys の `boxed_from` 接続後に GC 検証する |

## 契約との差分

Object payload ordering, `ReferenceLayout`, runtime roots, hash tests, and write-barrier requirements are specified in [Object layout](../../docs/src/design/object-layout.md) and [GC interface](../../docs/src/design/gc-interface.md). The following notes retain Phase 1 implementation details and known gaps.

double-float は binary64 の生ビット 1 語、bignum limb は little-endian の u32 2 個を 1 語に詰める。`ThreadContext` は `repr(C)` で `Thread` を先頭に持つ。下流は payload offset を raw heap index と混同せず、GC を跨ぐ参照を root 化する。

hash table と PACKAGE の payload はスカラー metadata を先頭、参照語を末尾に置き、`boxed_from` は最初の参照語（header 込み index）です。HASH_TABLE はスカラー payload 0..7、参照 payload 8..9、PACKAGE はスカラー payload 0..1、参照 payload 2..9 の順序で、fixnum metadata を boxed reference として走査しません。HASH_TABLE の 5..7 は順にフリーリスト先頭、高水位、occupied（live と tombstone の合計）です。KV の空き key slot は予約 tagged word、対応する value slot は次の空き position です。新規 position は高水位から切り出し、削除 position はフリーリストから O(1) で再利用します。insert/remove/get は平均 O(1)、rehash は O(n)、resize は O(n) です。全参照 store は write barrier 経由です。

hash table の削除は tombstone を使います。空 slot はプローブ連鎖の終端、tombstone は連鎖を維持したまま insert が再利用できる slot です。lookup は tombstone を越えて続行し、load factor は tombstone を含めて計算します。resize は KV を高水位順に読み、旧エントリの Vec を保持せず新しい KV/INDEX に再配置して tombstone を掃除します。

registry は専用の `registry_context` を Native 状態で保持します。sys の STW 判定では Native mutator は `active_mutators` から除外され、現在の `allocate` は GC を起動せず容量超過を返すため、現行実装では STW と整合します。将来 allocation が GC を起動する場合は、Native 状態を一時的に Lisp 状態へ戻す公開 sys API、または allocation 中も STW 対象にする sys 側変更が必要です。registry が保持する Word は root slot から allocation 後に再読します。

`ncl-sys` の conservative `find_raw` は lowtag を検証しないため、object 側は payload 語順と `boxed_from` を使ってこの段階の誤走査を回避しています。lowtag 検証そのものは sys 側の残課題です。weakness enum/API は登録済みですが、weak table の key/value clearing の完全な CL semantics は下流実装で補完します。

| 種別 | layout | accessor | ctor | GC test |
| --- | --- | --- | --- | --- |
| structure / instance | 済 | 済 | 済 | 部分 |
| simple-fun / closure | 済 | 済 | 済 | closure 1件 |
| bignum / ratio / double / complex | 済 | 済 | 済 | 部分 |
| stream / readtable / code | 済 | 部分 | 済 | 未実施 |

下流レーンは `Word` の lowtag を直接判定せず `classify` または typed accessor を使い、allocation を跨ぐ引数は `RootToken` で保護してください。

残課題: `Runtime::function`、`class`、`find_package` などの照会で検索キー文字列を毎回 heap allocation しています。キー文字列を一時 allocation なしで照会する仕組みは未実施です。lowtag 検証と weak table の完全な Common Lisp semantics も sys/下流実装の課題です。
