# Compiler pipeline

## 決定

front end は reader output を macroexpand、parse、declaration/type propagation、compiler-macro expansion、ncl-ir lowering の順に処理する。back end は ncl-ir を Cranelift CLIF、user stack-map metadata、inline-cache metadata、FASL writer へ lower する。front/back の境界型は独立 crate ncl-ir のみとし、reader の Form を境界に出さない。

ncl-ir は Phase 0 で todo!() なしにデータ型を完全定義する。Function は id、name、params、return types、basic blocks、locals、handler regions、debug locations を持つ。BasicBlock は id、block parameters、Vec<Op>、Terminator を持つ。OpKind は Const、Move、Load、Store、LoadField、StoreField、Alloc、LoadArg、Call、CallIndirect、Builtin、Prim、Compare、Convert、SetMultipleValues、Safepoint とする。Terminator は Jump、Branch、Switch、CallReturn、TailCall、Return、Throw、Unreachable とする。型注釈は Word、I64、F64、Address、Bool、Unit、定数は fixnum、character、single/double-float、symbol reference、object reference、string bytes とする。handler region は protected blocks、handler blocks、cleanup blocks、catch tag、dynamic depth を持ち、debug location は source file id、line、column、form id を持つ。

direct-expansion primitive は car、cdr、rplaca、rplacd、svref、aref、aset、fixnum arithmetic/comparison、eq、eql、typep、character predicates、structure slot accessors とする。type guard failure は condition edge にする。Rust-side compiler macro registry は name、arity pattern、expansion callback、feature bit を持つ。

Cranelift family は Phase 0 で 0.134.3 に固定する。compile と REPL は cranelift-jit、compile-file は cranelift-object を使い、codegen、frontend、module、jit、object、native も同じ版に揃える。non-tail call の user stack map は ncl-ir の Safepoint metadata から生成する。inline cache は call-site、generic function identity、class layout generation の 3 tuple を key とし最大 4 entry とする。

FASL は native code、relocation、constant/object、symbol table、header の順で、header は magic NCLFASL、format version 1、target architecture、pointer width 64、endianness、feature bitmap、section offsets を持つ。architecture と feature が一致しない image は condition にする。evaluator-mode の interpret は実装せず、eval は front/back で compile して実行する。

## 根拠

独立 IR は front/back を同時並行で実装可能にし、型、GC metadata、handler region、debug location を machine lowering 前に固定する。JIT/object module は compile と compile-file の成果物の寿命に対応する。Phase 0 の型定義を完全にすれば lane が todo!() の実装詳細に依存しない。

## 却下した代替案

- ncl_compiler::ir の内部 module は共有契約にならないため却下した。
- old AST/Form、stack bytecode、LLVM、自前 backend はプロジェクト決定に反するため却下した。
- portable FASL と interpret evaluator は native code 契約および決定済み実行経路と両立しないため却下した。

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- IR の型、OpKind、Terminator、定数、handler region、debug location を使い、第二の IR を作らない。
- backend は ncl-ir を検証してから Cranelift に lower し、stack-map metadata を省略しない。
- compile/REPL と compile-file の module 選択、FASL header 検証、interpret 非対応を変更しない。
