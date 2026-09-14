# Crates

## 決定

workspace は packages/<name>/ に置き、crate name は ncl-<name> とする。Runtime、ThreadContext、register の型、builtin! は全 library より下の ncl-object に置く。Runtime は package table、function registry、class table、GC settings を共有する Sync 状態とし、ThreadContext は TLAB、binding stack、roots、handlers、safepoint、MV を持つ。ncl-runtime は eval/compile/load と登録集約だけを担う。

### crate と隣接リスト

| crate | responsibility | dependencies |
| --- | --- | --- |
| ncl-sys | unsafe boundary、OS/FFI primitives | libc |
| ncl-object | Word、heap、GC interface、Runtime、ThreadContext、object、package/intern、builtin ABI | ncl-sys |
| ncl-ir | compiler IR data types only | ncl-sys |
| ncl-types | type specifier/predicate/error | ncl-object |
| ncl-reader | reader | ncl-object |
| ncl-printer | printer | ncl-object |
| ncl-conditions | conditions、handlers、restarts | ncl-object |
| ncl-clos | classes、slots、generic functions、MOP | ncl-object、ncl-types、ncl-conditions |
| ncl-compiler-front | macroexpand、declarations、compiler macros、front lowering | ncl-object、ncl-types、ncl-reader、ncl-conditions、ncl-ir |
| ncl-compiler-back | Cranelift lowering、JIT/object、stack maps、FASL | ncl-object、ncl-ir、cranelift-codegen、cranelift-frontend、cranelift-module、cranelift-jit、cranelift-object、cranelift-native |
| ncl-lib-* | standard-library builtins | ncl-object および各機能の下層 crate |
| ncl-threads | OS-thread Lisp API | ncl-object、ncl-conditions、ncl-sys |
| ncl-ffi | dynamic loading/alien calls | ncl-object、ncl-conditions、libloading |
| ncl-image | FASL image load/relocation | ncl-object、ncl-compiler-back |
| ncl-runtime | eval/compile/load、registration aggregation | library crates、ncl-compiler-front、ncl-compiler-back、ncl-image |
| ncl-conformance | ansi-test/cl-bench runners | ncl-runtime |
| root ncl | CLI/REPL | ncl-runtime |

ncl-compiler-front は ncl-compiler-back に依存せず、ncl-compiler-back は front に依存しない。ncl-conditions -> ncl-clos、ncl-reader -> ncl-object、全 lib -> ncl-object の向きを固定するため循環を含まない。ncl-sys だけが unsafe_code = allow、他は unsafe_code = forbid とする。外部依存は Cranelift 0.134.3 の 6 crate、libc、libloading だけに固定する。

### Phase 1 signature surface

    #[repr(transparent)] pub struct Word(u64);
    pub struct Runtime;
    pub struct ThreadContext;
    pub fn alloc(thread: &mut ThreadContext, rt: &Runtime, ty: TypeTag, words: usize) -> Result<Word, StorageCondition>;
    pub fn register_thread(rt: &Runtime, thread: &mut ThreadContext);
    pub fn poll_safepoint(thread: &mut ThreadContext);
    pub fn push_root(thread: &mut ThreadContext, slot: &mut Word) -> RootToken;
    pub fn write_barrier(thread: &mut ThreadContext, object: Word, slot: Slot);
    pub fn enter_native(thread: &mut ThreadContext);
    pub fn leave_native(thread: &mut ThreadContext);
    pub fn register(rt: &Runtime);

各 library crate の register は &Runtime を受ける。builtin! は固定 arity の直接 signature と可変長/keyword の配列 signature を生成し、pending flag と予約戻り値で condition/non-local exit を伝える。標準 library は Rust crate とし、package intern は ncl-object API を使う。

## 根拠

低層の Runtime/ThreadContext/register を ncl-object に置くことで library が最上位 runtime に依存する循環を除く。ncl-ir を独立させることで front と back が同時に安定した型へ compile できる。隣接リストを明示すると非循環を review できる。

## 却下した代替案

- register(&mut Runtime) を ncl-runtime に置く設計は library -> runtime の循環になるため却下した。
- pub type Word = u64 はタグ操作と GC 値の混同を許すため newtype に置換した。
- central registration table、cl-cc、LLVM、自前 backend、bytecode、Phase 1 の追加依存はプロジェクト決定に反するため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- 隣接リスト、ncl-ir の独立性、Runtime/ThreadContext の所在、Word newtype、Cranelift 0.134.3 固定を変更しない。
- 各 lane は自身の crate と tests だけを編集し、central registration table や conformance/ を作らない。
- unsafe、alternate backend、alternate builtin ABI、未登録の Rust Word storage を追加しない。
