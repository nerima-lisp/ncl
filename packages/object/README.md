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

`Runtime::new`, `Runtime::with_config`, `Runtime::register_layouts`、`Runtime::define_function`、`Runtime::function`、`Runtime::ensure_package`、`Runtime::define_class`、`Runtime::class`、`Runtime::add_feature`、`Runtime::features`、`Runtime::gc_config` が runtime の登録・照会 API です。`ThreadContext::register` は heap への登録、`bind`/`unbind` は special 束縛、`set_values`/`values` は多値領域、`collect` は GC を提供します。

`HashTable::new`、`insert`、`get`、`remove`、`gc_cleared`、`rehash_after_gc` と `sxhash` が hash table API です。`Package::new`、`find_symbol`、`intern`、`unintern`、`export`、`import`、`shadow`、`use_package`、`gensym` が package API です。`package::nil()` と `package::truth()` は静的 NIL/T です。

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
fn register_my_functions(runtime: &Runtime) {
    runtime.define_function("NCL", "MY-FUNCTION", function_object);
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

`RootToken` は LIFO です。トークンを逆順に pop し、token が有効な間は参照先の slot を move、resize、drop しないでください。`Word` を Rust メモリに保存したまま GC 後も使う必要がある場合は、その所有構造を heap object として設計し、参照 layout を `Runtime::register_layouts` 相当の登録経路に追加します。

## 実装状況

| 範囲 | 状態 | 下流への注意 |
| --- | --- | --- |
| widetag、固定 layout、cons/symbol accessor | 済 | layout の payload offset は header を含まない |
| Runtime、ThreadContext、RootToken、builtin metadata | 済 | special binding と多値の高水準 ABI は利用可能 |
| hash table、package intern、GC symbol registration | 済 | Rust 内 hash table の Word は caller が root を管理する |
| generational GC を跨ぐ object root 更新 | 部分 | package/hash table の移動参照更新統合は下流で要確認 |
| string、simple-vector、特殊化配列、非 simple 配列の typed accessor/constructor | 済 | `ArrayElementType` と row-major accessor を使用する |
| structure、CLOS instance、simple-fun、closure、bignum、ratio、double、complex、stream、readtable、code object | 部分 | payload accessor と layout は揃っている。可変長 closure は sys の `boxed_from` 接続後に GC 検証する |

## 契約との差分

double-float は binary64 の生ビット 1 語、bignum limb は little-endian の u32 2 個を 1 語に詰める。`ThreadContext` は `repr(C)` で `Thread` を先頭に持つ。下流は payload offset を raw heap index と混同せず、GC を跨ぐ参照を root 化する。

下流レーンは `Word` の lowtag を直接判定せず `classify` または typed accessor を使い、allocation を跨ぐ引数は `RootToken` で保護してください。
