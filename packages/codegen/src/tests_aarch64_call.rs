#![expect(
    clippy::expect_used,
    reason = "AArch64 call fixtures are constructed to be valid, so expect failures identify broken test setup without adding uncovered fallback branches."
)]

use super::*;

#[test]
fn aarch64_call_with_five_arguments_reserves_and_roots_rest_area() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(33),
        "aarch64-five-argument-call",
        vec![
            ncl_ir::Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "first".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "second".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "third".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "fourth".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "fifth".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let argc = builder.add_constant(Constant::Fixnum(5));
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::I64])
        .expect("argc constant lowers")
        .first()
        .copied()
        .expect("argc constant produces a value");
    let argc = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: argc,
            },
            &[Ty::Word],
        )
        .expect("argc conversion lowers")
        .first()
        .copied()
        .expect("argc conversion produces a value");
    let result = builder
        .push_op(
            OpKind::Call {
                function: ncl_ir::ValueId(0),
                args: vec![
                    argc,
                    ncl_ir::ValueId(1),
                    ncl_ir::ValueId(2),
                    ncl_ir::ValueId(3),
                    ncl_ir::ValueId(4),
                    ncl_ir::ValueId(5),
                ],
            },
            &[Ty::Word],
        )
        .expect("call lowers")
        .first()
        .copied()
        .expect("call produces a value");
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return terminator lowers");

    let compiled = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
        .expect("five-argument call compiles");
    assert!(compiled.frame_size >= 48, "rest area was not reserved");
    let map = compiled
        .safepoint_maps
        .first()
        .expect("call safepoint map was emitted");
    let rest_roots = map.bitmap.get(1).expect("outgoing roots were represented");
    assert_eq!(*rest_roots & 0b1100, 0b1100, "rest roots were marked");
}

#[test]
fn aarch64_closure_call_acquire_loads_function_entry() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(34),
        "aarch64-acquire-function-entry",
        vec![
            ncl_ir::Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "argc".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let result = builder
        .push_op(
            OpKind::CallClosure {
                closure: ncl_ir::ValueId(0),
                args: vec![ncl_ir::ValueId(1)],
                named_symbol: None,
            },
            &[Ty::Word],
        )
        .expect("closure call lowers")
        .first()
        .copied()
        .expect("closure call produces a value");
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .expect("return terminator lowers");

    let compiled = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
        .expect("closure call compiles");
    let expected = encode_aarch64([
        ncl_asm_aarch64::Inst::AddImm {
            rd: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(17)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(17)),
            imm: 8,
            shift: false,
        },
        ncl_asm_aarch64::Inst::Ldar {
            rt: ncl_asm_aarch64::Reg(17),
            rn: ncl_asm_aarch64::Reg(17),
        },
        ncl_asm_aarch64::Inst::AsrImm {
            rd: ncl_asm_aarch64::Reg(17),
            rn: ncl_asm_aarch64::Reg(17),
            amount: 1,
        },
    ]);
    assert_code_contains_aarch64(
        &compiled.code,
        &expected,
        "closure call acquire-loads the function entry before decoding it",
    );
}

#[test]
fn aarch64_function_cell_field_load_uses_acquire() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(35),
        "aarch64-function-cell-load",
        vec![ncl_ir::Param {
            name: "symbol".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let field = u32::try_from(ncl_object::symbol_offset::FUNCTION)
        .expect("function-cell field offset fits in IR");
    let value = builder
        .push_op(
            OpKind::LoadField {
                object: ncl_ir::ValueId(0),
                field,
            },
            &[Ty::Word],
        )
        .expect("function-cell field load lowers")
        .first()
        .copied()
        .expect("function-cell field load produces a value");
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .expect("return terminator lowers");

    let compiled = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
        .expect("function-cell field load compiles");
    let offset = u16::try_from((ncl_object::symbol_offset::FUNCTION + 1) * 8)
        .expect("function-cell byte offset fits in an AArch64 instruction");
    let expected = encode_aarch64([
        ncl_asm_aarch64::Inst::AndImm {
            rd: ncl_asm_aarch64::Reg(16),
            rn: ncl_asm_aarch64::Reg(16),
            imm: !ncl_sys::LOWTAG_MASK,
        },
        ncl_asm_aarch64::Inst::AddImm {
            rd: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(16)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(16)),
            imm: offset,
            shift: false,
        },
        ncl_asm_aarch64::Inst::Ldar {
            rt: ncl_asm_aarch64::Reg(16),
            rn: ncl_asm_aarch64::Reg(16),
        },
    ]);
    assert_code_contains_aarch64(
        &compiled.code,
        &expected,
        "function-cell LoadField forms its address and uses LDAR",
    );
}
