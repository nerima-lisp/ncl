# Compiler pipeline

## 決定

front end は reader output を macroexpand、parse、declaration/type propagation、compiler-macro expansion、`ncl-ir` lowering の順に処理する。back end は `ncl-ir` を `ncl-codegen` の `MachineFunction`、target encoder、`CodeBlob`、`SafepointMap`、FASL sections へ lower する。front/back 境界は `ncl-ir` のみで、reader Form を越境させない。

`ncl-ir` の型一覧は維持する。Function は id、name、params、return types、basic blocks、locals、handler regions、debug locations、BasicBlock は id、block parameters、Ops、Terminator を持つ。OpKind は Const、Move、Load、Store、LoadField、StoreField、Alloc、LoadArg、Call、CallIndirect、Builtin、Prim、Compare、Convert、SetMultipleValues、Safepoint、Terminator は Jump、Branch、Switch、CallReturn、TailCall、Return、Throw、Unreachable とする。

段階は 1a 固定テンプレート、1b linear scan、1c tail transfer、2 unbox/inline cache の順である。FASL の 64-byte header と section 構成は native backend の仕様を使い、対象不一致は load 前に拒否する。interpret evaluator は実装しない。

## 根拠

独立 IR に handler、GC metadata、debug location を含めると front と backend の並行実装が可能になる。`MachineFunction -> encoder -> CodeBlob` を一本化すると JIT、FASL、image が同じ machine contract を共有できる。

## 却下した代替案

- front end 内部 IR を backend の共有境界にする案は依存を増やすため却下する。
- stack bytecode を実行経路として残す案は native image 契約を二重化するため却下する。
- target ごとに別 IR を作る案は lane 間の意味を分岐させるため却下する。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- listed IR types、OpKind、Terminator、handler、debug location を変更しない。
- backend は `MachineFunction` と `CodeBlob` を通じて encoder を呼び、stack map を省略しない。
- 1a で unboxed representation、inline cache、独自 ABI を追加しない。

## `ncl-ir` 最終契約

`ncl-ir` は依存 crate を持たない。公開される値は `Function`、`BasicBlock`、`Op`、`OpKind`、`Terminator`、`Ty`、`Constant`、`HandlerRegion`、`DebugLocation` と、それらの ID 型である。`OpKind` は `Const`、`Move`、`Load`、`Store`、`LoadField`、`StoreField`、`Alloc`、`LoadArg`、`Call`、`CallIndirect`、`Builtin`、`Prim`、`Compare`、`Convert`、`SetMultipleValues`、`Safepoint` を持つ。`Terminator` は `Jump`、`Branch`、`Switch`、`CallReturn`、`TailCall`、`Return`、`Throw`、`Unreachable` を持つ。

### テキスト形式

`Function` の `Display` は `fn @name { payload }` の形式で、payload は長さと variant tag を含むカンマ区切りの hexadecimal atom 列である。文字列は ASCII の安全な文字以外を `_hh` として escape する。フィールドを失わないため、`parse(function.to_string()) == function` を保証する。
