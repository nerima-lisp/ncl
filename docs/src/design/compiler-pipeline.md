# Compiler pipeline

## 決定

front end は reader output を macroexpand、parse、declaration/type propagation、compiler-macro expansion、`ncl-ir` lowering の順に処理する。back end は `ncl-ir` を `ncl-codegen` の `MachineFunction`、target encoder、`CodeBlob`、`SafepointMap`、FASL sections へ lower する。front/back 境界は `ncl-ir` のみで、reader Form を越境させない。

`ncl-ir` の型一覧は維持する。Function は id、name、params、return types、basic blocks、locals、handler regions、debug locations、BasicBlock は id、block parameters、Ops、Terminator を持つ。OpKind は Const、Move、Load、Store、LoadField、StoreField、Alloc、LoadArg、Call、CallIndirect、Builtin、Prim、Compare、Convert、SetMultipleValues、Safepoint、Terminator は Jump、Branch、Switch、CallReturn、TailCall、Return、Throw、Unreachable とする。

段階は 1a 固定テンプレート展開、1b 線形走査レジスタ割当と spill、1c self tail call と一般 tail transfer および `&rest`/`&key` 専用プロローグ、2 fixnum/double の unbox、型推論接続、inline cache の順である。1a は値をスタックスロットに置き、scratch 2 本を使い、プロローグ、定数、return、呼び出し、分岐、割当、safepoint map、unwind を含む動く系とする。FASL の 64-byte header と section 構成は native backend の仕様を使い、対象不一致は load 前に拒否する。interpret evaluator は実装しない。

## 根拠

独立 IR に handler、GC metadata、debug location を含めると front と backend の並行実装が可能になる。`MachineFunction -> encoder -> CodeBlob` を一本化すると JIT、FASL、image が同じ machine contract を共有できる。

## 却下した代替案

- front end 内部 IR を backend の共有境界にする案は依存を増やすため却下する。
- stack bytecode を実行経路として残す案は native image 契約を二重化するため却下する。
- target ごとに別 IR を作る案は lane 間の意味を分岐させるため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- listed IR types、OpKind、Terminator、handler、debug location を変更しない。
- backend は `MachineFunction` と `CodeBlob` を通じて encoder を呼び、stack map を省略しない。
- 1a で unboxed representation、inline cache、独自 ABI を追加しない。Phase 1 は性能を主張しない。

## Complete IR contract

`ncl-ir` depends on no crate. `Function` contains id, name, parameters, return types, locals, blocks, handler regions, and debug locations. `BasicBlock` contains id, block parameters, operations, and one terminator. `Op` contains result type, source location, and `OpKind`.

`OpKind` variants are `Const`, `Move`, `Load`, `Store`, `LoadField`, `StoreField`, `Alloc`, `LoadArg`, `Call`, `CallIndirect`, `Builtin`, `Prim`, `Compare`, `Convert`, `SetMultipleValues`, and `Safepoint`. Terminators are `Jump`, `Branch`, `Switch`, `CallReturn`, `TailCall`, `Return`, `Throw`, and `Unreachable`.

Types are `Word`, `I64`, `F64`, `Address`, `Bool`, and `Unit`. Constants are fixnum, character, single-float, double-float, string bytes, and descriptors for symbol or object references. A symbol descriptor is package name plus symbol name; an object descriptor is a constant-table index. Codegen resolves descriptors, so they are not raw Word constants in the IR. Handler regions contain protected, handler, and cleanup blocks, catch tag, and dynamic depth. Debug locations contain source file id, line, column, and form id.

Direct expansion includes `car`, `cdr`, `rplaca`, `rplacd`, `svref`, `aref`, `aset`, fixnum arithmetic and comparison, `eq`, `eql`, `typep`, character predicates, and structure slot accessors. The compiler-macro registry stores symbol descriptor, arity pattern, expansion callback, and feature bit.

Phase 1a expands fixed templates with values in stack slots and two scratch registers, including prologue, constants, return, calls, branches, allocation, safepoint maps, and unwind. Its native entry must execute `(+ 1 2)`, cons, a branch, a builtin call, a safepoint, and `fib(25)`; performance is not an acceptance criterion. Phase 1b adds linear-scan register allocation and spill, with representative benchmarks no slower than the template version. Phase 1c adds self tail call, general tail transfer, and specialized `&rest`/`&key` prologues; self recursion at depth 1,000,000 must not add frames. Phase 2 adds fixnum/double unboxing, type-inference connection, and inline caches; the first gate is `fib(25)` within 2x SBCL on the same machine, recording commit hash and conditions. Each stage is complete only when its tests execute. FASL stores native code, relocations, descriptors/constants, symbol bindings, maps, and debug records; incompatible architecture or feature bits signal a condition.

Inline-cache keys are the triple `(call-site, generic-function identity, class-layout generation)`. Each site has at most four entries; a miss falls back to runtime dispatch. A failed type guard is a condition edge in the IR, not an unchecked branch.

## `ncl-ir` テキスト形式の補足

### 公開される IR 型

`ncl-ir` は依存 crate を持たず、`lib.rs` から次の型と関数を公開する。ID 型 (`FunctionId`、`BlockId`、`ValueId`、`LocalId`、`ConstantIndex`、`FileId`、`FormId`、`DebugLocationId`) は `u32` の newtype で、`Display` は `%<decimal-value>` 形式である。構造体は次のフィールドを持つ。

- `Function`: `id: FunctionId`、`name: String`、`params: Vec<Param>`、`return_types: Vec<Ty>`、`blocks: Vec<BasicBlock>`、`locals: Vec<Local>`、`constants: Vec<Constant>`、`handler_regions: Vec<HandlerRegion>`、`debug: Vec<DebugLocation>`。
- `Param`: `name: String`、`ty: Ty`。
- `Local`: `id: LocalId`、`name: String`、`ty: Ty`。
- `BlockParam`: `value: ValueId`、`ty: Ty`。SSA の phi 値に相当する。
- `BasicBlock`: `id: BlockId`、`params: Vec<BlockParam>`、`ops: Vec<Op>`、`terminator: Terminator`。
- `Op`: `results: Vec<(ValueId, Ty)>`、`kind: OpKind`、`loc: Option<DebugLocationId>`。
- `HandlerRegion`: `protected: Vec<BlockId>`、`handler: BlockId`、`cleanup: Option<BlockId>`、`catch_tag: Option<ValueId>`、`depth: u32`。
- `DebugLocation`: `file: FileId`、`line: u32`、`column: u32`、`form: FormId`。

`Ty` の variant は `Word` (処理系の汎用値)、`I64`、`F64`、`Address`、`Bool`、`Unit` である。

`Constant` の variant は `Fixnum(i64)`、`Character(u32)`、`SingleFloat(f32)`、`DoubleFloat(f64)`、`Symbol { package: String, name: String }`、`Object(ConstantIndex)`、`StringBytes(Vec<u8>)`、`Nil`、`T`、`Unbound` である。`Symbol` と `Object` は descriptor であり、codegen/runtime が解決する。`Object` は同じ `Function` の constant table index を参照し、raw `Word` を表さない。

### OpKind と Terminator

`OpKind` の全 variant と役割は次の通りである。

- `Const { result }`: constant table の `ConstantIndex` を値にする。
- `Move { value }`: SSA 値を移送する。
- `Load { address }` / `Store { address, value }`: メモリを読み書きする。
- `LoadField { object, field }` / `StoreField { object, field, value }`: オブジェクトの数値フィールドを読み書きする。
- `Alloc { words }`: 指定ワード数のオブジェクト領域を確保する。
- `LoadArg { index }`: 引数を読み込む。
- `Call { function, args }`: 関数値を直接呼び出す。
- `CallIndirect { callee, args }`: callee 値を間接呼び出しする。
- `Builtin { name, args }`: runtime builtin を名前で呼び出す。
- `Prim { op, args, condition }`: `Prim` の組み込みプリミティブを実行する。`condition` は型ガード等が失敗した場合の condition edge である。
- `Compare { op, left, right }`: `Compare` (`Eq`、`Ne`、`Lt`、`Le`、`Gt`、`Ge`) で比較する。
- `Convert { op, value }`: `Convert` (`WordToI64`、`I64ToWord`、`WordToF64`、`F64ToWord`、`AddressToWord`、`WordToAddress`) で表現を変換する。
- `SetMultipleValues { values }`: 複数戻り値の集合を設定する。
- `Safepoint`: GC safepoint を置く。

`Prim` の variant は `Car`、`Cdr`、`Rplaca`、`Rplacd`、`Svref`、`Aref`、`Aset`、`FixnumAdd`、`FixnumSub`、`FixnumMul`、`FixnumDiv`、`FixnumLt`、`FixnumLe`、`FixnumEq`、`Eq`、`Eql`、`Typep`、`CharacterPredicate(String)`、`StructureSlot(String)` である。

`Terminator` は各 basic block の末尾に一つ置く。

- `Jump { target, args }`: `target` へ block 引数を渡す。
- `Branch { condition, then_target, then_args, else_target, else_args }`: condition により二つの successor を選ぶ。
- `Switch { value, cases, default, default_args }`: signed integer case と default successor を選ぶ。
- `CallReturn { function, args }`: 呼び出し後に通常復帰する。
- `TailCall { function, args }`: 現在の frame を継続せず末尾呼び出しする。
- `Return { values }`: 関数から値を返す。
- `Throw { condition }`: condition を送出する。
- `Unreachable`: 到達不能な未完成 block を表す内部初期値であり、検証時には `MissingTerminator` になる。

### テキスト形式の文法

以下は `Function` の `Display` 出力と `parse` が扱う payload の EBNF 相当表記である。空白は header の固定位置にのみ現れ、payload の atom はカンマで区切られる。`hex` は小文字 hexadecimal、`signed` は `i` に続く hexadecimal 表現、`id` は unsigned atom、`text` は escaped string atom である。`n * X` は X を n 回繰り返す。

```text
function       = "fn @", escaped-name, " { ", function-payload, " }", newline ;
function-payload = id, text, params, types, constants, blocks, locals,
                   handlers, debug ;
params         = count, count * (text, type) ;
types          = count, count * type ;
constants     = count, count * constant ;
blocks         = count, count * block ;
locals         = count, count * (id, text, type) ;
handlers       = count, count * handler ;
debug          = count, count * (id, id, id, id) ;
block          = id, count, count * (id, type), count, count * op, terminator ;
op             = count, count * (id, type), optional-location, op-kind ;
optional-location = absent-location | id ;
handler        = count, count * id, id, optional-block, optional-value, id ;
optional-block = flag | flag, id ;
optional-value = flag | flag, id ;
constant      = fixnum | character | single-float | double-float
               | symbol | object | string-bytes | nil | true | unbound ;
fixnum        = tag, signed ;
character     = tag, id ;
single-float  = tag, id ;
double-float  = tag, id ;
symbol        = tag, text, text ;
object        = tag, id ;
string-bytes  = tag, count, count * id ;
nil           = tag ; true = tag ; unbound = tag ;
```

`op-kind` は variant tag に続けて、`Const` は `id`、単一値 operand の `Move`/`Load`/`Convert` は `id`、二値 operand の `Store`/`Compare` は `id, id`、field 操作は object/value と field、`Alloc` と `LoadArg` は数値を記録する。`Call`/`CallIndirect` は callee と `count, count * id`、`Builtin` は `text, count, count * id`、`Prim` は primitive 名、`count, count * id`、condition block の optional id、`SetMultipleValues` は `count, count * id`、`Safepoint` は追加 atom なしである。

terminator の tag と payload は `Jump` が target と values、`Branch` が condition・then successor・else successor、`Switch` が value・case 数・各 signed case と successor・default successor、`CallReturn`/`TailCall` が callee と values、`Return` が values、`Throw` が condition、`Unreachable` が追加 payload なしである。全 successor の形式は `id, count, count * id` である。

文字列 atom は ASCII の英数字、`_`、`-` をそのまま使い、それ以外の byte を `_hh` として escape する。payload の各数値は hexadecimal atom である。`parse` はこの形式を `Result<Function, ParseError>` として読み、余分な atom、未知の tag、壊れた escape を拒否する。front と codegen は `Function` を直接構築するか `parse` で読み、`verify(&function)` で構造・SSA 可視性・型・戻り値契約を検査してから利用する。現実装の parser は `Prim::CharacterPredicate` と `Prim::StructureSlot` の文字列表現を読み戻さないため、これらを含む関数では無条件の往復保証を置かない。

### front/codegen が利用してよい API

- 構築: `FunctionBuilder::new(FunctionId, name, params, return_types)`、`add_local`、`add_constant`、`create_block`、`position_at`、`fresh_value`、`push_op`、`terminate`、`add_handler_region`、`finish`。
- 入出力: `Function` の `Display` 実装、`parse(&str) -> Result<Function, ParseError>`。
- 検証: `verify(&Function) -> Result<(), Vec<VerifyError>>`。エラー分類は `DuplicateBlock`、`MissingBlock`、`MissingTerminator`、`SuccessorArity`、`SuccessorType`、`UndefinedValue`、`DuplicateValue`、`ConstantOutOfBounds`、`ReturnArity`、`TypeMismatch`、`HandlerTarget`、`SafepointWarning` である。

builder は ID を単調増加で割り当て、生成直後に空の entry block を選択する。`push_op` は指定した result type ごとに SSA 値を割り当て、`finish` が所有権を持つ `Function` を返す。backend は descriptor constant を解決し、`Op` の `results` と `loc`、handler region、safepoint を保持したまま machine IR へ lower する。
