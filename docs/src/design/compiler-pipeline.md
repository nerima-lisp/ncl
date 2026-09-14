# Compiler pipeline

## 決定

front end は source reader output を macroexpand、parse、type propagation、declaration、compiler-macro expansion、IR lowering の順に処理する。back end は typed NCL IR を Cranelift CLIF、stack-map metadata、inline-cache metadata、FASL writer へ lower する。front/back の唯一の境界型は `ncl_compiler::ir::Function` とし、reader の旧 `Form` は使用しない。

IR は arena-owned immutable nodes として、`Function { params, blocks, locals, handlers, debug_spans }`、`BasicBlock { params, ops, terminator }`、`Op { result_types, kind }`、`Terminator { jump, branch, call, tail_call, return, throw }` を持つ。Lisp value は `Word`、machine integer、double、address の型を明示する。所有権は compiler arena、runtime object は `Word` 参照である。

direct-expansion primitive の固定集合は `car`, `cdr`, `rplaca`, `rplacd`, `svref`, `aref`, `aset`, fixnum `+ - * / < <= =`, `eq`, `eql`, `typep`, character code/case predicates, structure slot accessors とする。各 primitive は type guard failure の condition edge を IR に出す。Rust-side compiler macro registry は name, arity pattern, expansion callback, feature bit を持つ。

back end は `cranelift-codegen`, `cranelift-frontend`, `cranelift-module` と同一 version の `cranelift-jit`, `cranelift-object`, `cranelift-native` を使う。`compile` と REPL は JIT module、`compile-file` は object module を使う。non-tail calls に user stack maps を付け、frame slots の live Word を登録する。inline cache は calling-convention 契約の 4-entry IC である。

FASL は native code section, relocation table, constant/object section, symbol table, header の順。header は magic `NCLFASL\0`, format version `1`, target architecture, pointer width `64`, endianness, feature bitmap, code/object offsets を持つ。x86-64 と AArch64 の FASL は相互非互換であり、loader は architecture と features を検査して mismatch condition を返す。

`*evaluator-mode*` の interpret は実装しない。`eval` は source form を front/back で compile して実行し、compile-time macro effects と runtime execution を別 phase とする。

## 根拠

An explicit IR prevents reader syntax objects from leaking into code generation and makes type/GC metadata available before machine lowering. Cranelift JIT and object modules match the two artifact lifetimes. Native Rust standard libraries require direct expansion for hot structural and numeric operations because Rust-builtins are not Lisp-level inline candidates.

## 却下した代替案

- old AST/Form as runtime value was rejected because `read` must return Lisp data.
- stack bytecode and an LLVM backend were rejected by the project decision.
- a portable FASL was rejected because native code and relocation are architecture-specific.
- evaluator mode was rejected; `eval` follows the compile-and-run path.

## Phase 1 レーンが前提にしてよいこと / してはいけないこと

- IR node ownership and terminator exhaustiveness are stable.
- New builtins must declare whether they are direct-expansion primitives or ordinary calls.
- Back ends may not introduce a second IR or bypass stack-map emission.
- FASL readers must reject version, architecture, pointer-width, endianness, or feature mismatches.
- No lane may add a Lisp implementation of standard library behavior; standard libraries are Rust crates.
