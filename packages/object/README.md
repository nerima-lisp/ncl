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
make_array(&mut ThreadContext, &Runtime, &[usize], ArrayOptions) -> Result<Word, ObjectError>
array_dimensions/array_row_major_ref/array_row_major_set
```

`Runtime::new` と `Runtime::with_config` は `Result<Runtime, ObjectError>` を返します。`Runtime::register_layouts`、`Runtime::define_function(&mut ThreadContext, ...)`、`Runtime::function(&mut ThreadContext, ...)`、`Runtime::ensure_package(&mut ThreadContext, ...)`、`Runtime::find_package(&ThreadContext, ...)`、`Runtime::define_class(&mut ThreadContext, ...)`、`Runtime::class(&mut ThreadContext, ...)`、`Runtime::add_feature`、`Runtime::features`、`Runtime::gc_config`、`Runtime::set_strict_forwarding` が runtime の登録・照会・GC API です。registry 操作は呼び出し側 context を使います。`ThreadContext::register` は heap への登録、`bind`/`unbind` は special 束縛、`set_values`/`values` は多値領域、`collect` は GC を提供します。

`ThreadContext::register` 後もコンテキストの move は安全です。`Thread` は Box で固定され、heap が保持するアドレスは変わりません。生成コードへは `thread_mut()` で得た `&mut Thread` から `*mut Thread` を渡します。Drop 時に登録解除されます。未登録 context は `collect`、allocation、`make_cons` を拒否します。

`HashTable::new`、`insert`、`get`、`remove`、`for_each_entry`、`capacity` と `sxhash` が hash table API です。`Eq` は identity、`Eql` は数値値、`Equal` は文字列内容と cons、`Equalp` はそれらに ASCII case folding を加えた比較です。`Package::new`、`find_symbol`、`intern`、`unintern`、`export`、`unexport`、`import`、`shadow`、`use_package`、`unuse_package`、`add_nickname`、`gensym` が package API です。`package::nil()` と `package::truth()` は静的 NIL/T です。

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

確保関数が値で受け取る managed 引数は、その関数自身が確保を跨いで root 化します。managed Word を含む slice/Vec は確保を跨いで保持せず、必要なら各要素を root 化してから確保します。`ThreadContext::set_gc_stress(true)` を使うと、確保ごとの GC でこの契約をテストできます。stale-word 検査は `ThreadContext::set_strict_forwarding(true)` または heap 全体に効く `Runtime::set_strict_forwarding(true)` で有効化します。

constructor は値で受け取った managed 引数と slice の各要素を、内部の確保より前に root 化し、確保後は更新済みの root slot から読み取ります。

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

double-float は binary64 の生ビット 1 語、bignum limb は little-endian の u32 2 個を 1 語に詰める。`ThreadContext` のレイアウトを下流 ABI に公開せず、生成コードへは `thread_mut()` で得た `Thread` のポインタを渡す。下流は payload offset を raw heap index と混同せず、GC を跨ぐ参照を root 化する。

`ThreadContext` の Drop は登録解除に `ncl_sys::unregister_thread` を使います。`unregister_thread` は `Thread` が保持する heap 参照から heap を引くため、`Runtime`（heap の所有者）は登録済みの全 `ThreadContext` より長生きしなければなりません。登録済み context が生きているうちに `Runtime` を drop すると解放済み heap を参照します。

hash table と PACKAGE の payload はスカラー metadata を先頭、参照語を末尾に置き、`boxed_from` は最初の参照語（header 込み index）です。HASH_TABLE はスカラー payload 0..7、参照 payload 8..10、PACKAGE はスカラー payload 0..1、参照 payload 2..9 の順序で、fixnum metadata を boxed reference として走査しません。HASH_TABLE の 5..7 は順にフリーリスト先頭、高水位、occupied（live と tombstone の合計）です。KV の空き key slot は予約 tagged word、対応する value slot は次の空き position です。新規 position は高水位から切り出し、削除 position はフリーリストから O(1) で再利用します。insert/remove/get は平均 O(1)、rehash は O(n)、resize は O(n) です。全参照 store は write barrier 経由です。

hash table の削除は tombstone を使います。空 slot はプローブ連鎖の終端、tombstone は連鎖を維持したまま insert が再利用できる slot です。lookup は tombstone を越えて続行し、load factor は tombstone を含めて計算します。resize は KV を高水位順に読み、旧エントリの Vec を保持せず新しい KV/INDEX に再配置して tombstone を掃除します。

registry は専用 context を保持せず、各操作が呼び出し側の登録済み context を受け取ります。registry が保持する Word は root slot から allocation 後に再読します。

`ncl-sys` の conservative `find_raw` は lowtag を検証しないため、object 側は payload 語順と `boxed_from` を使ってこの段階の誤走査を回避しています。lowtag 検証そのものは sys 側の残課題です。weakness enum/API は登録済みですが、weak table の key/value clearing の完全な CL semantics は下流実装で補完します。

| 種別 | layout | accessor | ctor | GC test |
| --- | --- | --- | --- | --- |
| structure / instance | 済 | 済 | 済 | 部分 |
| simple-fun / closure | 済 | 済 | 済 | closure 1件 |
| bignum / ratio / double / complex | 済 | 済 | 済 | 部分 |
| stream / readtable / code | 済 | 部分 | 済 | 未実施 |

下流レーンは `Word` の lowtag を直接判定せず `classify` または typed accessor を使い、allocation を跨ぐ引数は `RootToken` で保護してください。

残課題: `Runtime::function` と `class` の照会では検索キー文字列を毎回 heap allocation しています。キー文字列を一時 allocation なしで照会する仕組みは未実施です。`find_package` は registry の package 名と nickname を既存の heap 値から照合します。lowtag 検証と weak table の完全な Common Lisp semantics も sys/下流実装の課題です。
残課題: CLHS が要求する `unintern` 時の name-conflict 検出は未対応です。
