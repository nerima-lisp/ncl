use super::{DeadCodeElimination, Module, Sccp};
use crate::FunctionPass;
use crate::tests_support::Fixture;
use ncl_ir::{
    Compare, Constant, Convert, FunctionBuilder, FunctionId, OpKind, Prim, Terminator, Ty,
};

#[test]
fn folds_checked_fixnum_arithmetic() {
    let mut builder = FunctionBuilder::new(FunctionId(1), "fold", vec![], vec![Ty::I64]);
    let left = builder.add_constant(Constant::Fixnum(2));
    let right = builder.add_constant(Constant::Fixnum(3));
    let l = builder
        .push_op(OpKind::Const { result: left }, &[Ty::I64])
        .fixture()[0];
    let r = builder
        .push_op(OpKind::Const { result: right }, &[Ty::I64])
        .fixture()[0];
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![l, r],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .fixture();
    let mut function = builder.finish();
    assert!(Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].ops[2].kind,
        OpKind::Const { .. }
    ));
}

#[test]
fn leaves_fixnum_overflow_unfolded() {
    let mut builder = FunctionBuilder::new(FunctionId(1), "safe", vec![], vec![Ty::I64]);
    let max = builder.add_constant(Constant::Fixnum(i64::MAX));
    let one = builder.add_constant(Constant::Fixnum(1));
    let left = builder
        .push_op(OpKind::Const { result: max }, &[Ty::I64])
        .fixture()[0];
    let right = builder
        .push_op(OpKind::Const { result: one }, &[Ty::I64])
        .fixture()[0];
    let sum = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .fixture();
    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].ops[2].kind,
        OpKind::Prim { .. }
    ));
}

#[test]
fn does_not_replace_word_conversion_with_i64_constant() {
    let mut builder = FunctionBuilder::new(FunctionId(2), "word-convert", vec![], vec![Ty::Word]);
    let source = builder.add_constant(Constant::Fixnum(7));
    let raw = builder
        .push_op(OpKind::Const { result: source }, &[Ty::I64])
        .fixture()[0];
    let converted = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: raw,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![converted],
        })
        .fixture();
    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].ops[1].kind,
        OpKind::Convert { .. }
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn leaves_unreachable_blocks_valid_for_dead_code_elimination() {
    let mut builder = FunctionBuilder::new(FunctionId(3), "branch", vec![], vec![Ty::I64]);
    let entry = ncl_ir::BlockId(0);
    let then_block = builder.create_block(Vec::new());
    let then_value = builder.add_constant(Constant::Fixnum(1));
    let then_result = builder
        .push_op(OpKind::Const { result: then_value }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![then_result],
        })
        .fixture();

    let else_block = builder.create_block(Vec::new());
    let else_value = builder.add_constant(Constant::Fixnum(2));
    let else_result = builder
        .push_op(OpKind::Const { result: else_value }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![else_result],
        })
        .fixture();

    builder.position_at(entry).fixture();
    let left_constant = builder.add_constant(Constant::Fixnum(1));
    let left = builder
        .push_op(
            OpKind::Const {
                result: left_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let right_constant = builder.add_constant(Constant::Fixnum(1));
    let right = builder
        .push_op(
            OpKind::Const {
                result: right_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let condition = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left,
                right,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .fixture();

    let mut function = builder.finish();
    assert!(Sccp.run(&mut function, &Module::default()).fixture());
    ncl_ir::verify(&function).fixture();
    assert!(
        matches!(function.blocks[0].terminator, Terminator::Jump { target, .. } if target == then_block)
    );
    assert_eq!(function.blocks.len(), 3);

    assert!(
        DeadCodeElimination
            .run(&mut function, &Module::default())
            .fixture()
    );
    assert_eq!(function.blocks.len(), 2);
    ncl_ir::verify(&function).fixture();
}
