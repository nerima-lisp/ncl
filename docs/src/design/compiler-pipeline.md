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

`Function` の `Display` は `fn @name { payload }` の形式で、payload は長さと variant tag を含むカンマ区切りの hexadecimal atom 列である。文字列は ASCII の安全な文字以外を `_hh` として escape する。フィールドを失わないため、`parse(function.to_string()) == function` を保証する。
