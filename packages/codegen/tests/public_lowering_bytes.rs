#![allow(missing_docs, clippy::expect_used)]

use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg};
use ncl_codegen::{X86_64Abi, compile_function_x86_64};
use ncl_ir::{Compare, Constant, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};

fn encoded(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
    let mut assembler = Assembler::new();
    for instruction in instructions {
        assembler
            .emit(&instruction)
            .expect("expected instruction encoding");
    }
    assembler.bytes().to_vec()
}

fn contains(code: &[u8], needle: &[u8]) -> bool {
    code.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn public_x86_lowering_emits_exact_memory_primitive_and_compare_bytes() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(201),
        "public-byte-contract",
        vec![
            Param {
                name: "object".into(),
                ty: Ty::Address,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let object = ncl_ir::ValueId(0);
    let value = ncl_ir::ValueId(1);
    let field = builder
        .push_op(OpKind::LoadField { object, field: 1 }, &[Ty::Word])
        .expect("field load")[0];
    builder
        .push_op(
            OpKind::StoreField {
                object,
                field: 2,
                value,
            },
            &[],
        )
        .expect("field store");
    let cdr = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Cdr,
                args: vec![object, value],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("cdr")[0];
    let equal = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left: field,
                right: cdr,
            },
            &[Ty::Bool],
        )
        .expect("comparison")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![equal],
        })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64Abi).expect("lowering");

    assert!(contains(
        &compiled.code,
        &encoded([Inst::MovRM(Reg::R10, Mem::base(Reg::R10, 16))])
    ));
    assert!(contains(
        &compiled.code,
        &encoded([Inst::MovMR(Mem::base(Reg::R10, 24), Reg::R11)])
    ));
    assert!(contains(
        &compiled.code,
        &encoded([Inst::MovRM(Reg::R10, Mem::base(Reg::R10, 8))])
    ));
    assert!(contains(
        &compiled.code,
        &encoded([
            Inst::CmpRR(Reg::R10, Reg::R11),
            Inst::Setcc(Cond::E, Reg::R10),
        ])
    ));
}

#[test]
fn public_x86_lowering_emits_heap_constant_and_multiple_value_epilogue() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(202),
        "public-constant-and-values",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::StringBytes(vec![0x11, 0x22, 0x33]));
    let heap = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .expect("heap constant")[0];
    let fixnum = builder.add_constant(Constant::Fixnum(37));
    let fixnum = builder
        .push_op(OpKind::Const { result: fixnum }, &[Ty::Word])
        .expect("fixnum constant")[0];
    let result = builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![heap, fixnum],
            },
            &[Ty::Word],
        )
        .expect("multiple values")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64Abi).expect("lowering");

    assert!(!compiled.code.is_empty());
    assert!(contains(
        &compiled.code,
        &encoded([Inst::MovRI(
            Reg::R10,
            Imm::I32(i32::try_from(ncl_sys::Word::fixnum(37).bits()).expect("fixnum fits")),
        )])
    ));
    assert!(contains(
        &compiled.code,
        &encoded([Inst::MovRI(Reg::Rdx, Imm::I32(2))])
    ));
}

#[test]
fn public_x86_lowering_reaches_fixnum_sub_and_mul_instruction_bytes() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(203),
        "public-arithmetic-bytes",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let left = ncl_ir::ValueId(0);
    let right = ncl_ir::ValueId(1);
    let sub = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumSub,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("sub")[0];
    let mul = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumMul,
                args: vec![sub, right],
                condition: None,
            },
            &[Ty::Word],
        )
        .expect("mul")[0];
    builder
        .terminate(Terminator::Return { values: vec![mul] })
        .expect("return");

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64Abi).expect("lowering");

    assert!(contains(
        &compiled.code,
        &encoded([Inst::BinRR(BinOp::Sub, Reg::R10, Reg::R11)])
    ));
    assert!(contains(
        &compiled.code,
        &encoded([Inst::ImulRR(Reg::R10, Reg::R11)])
    ));
}
