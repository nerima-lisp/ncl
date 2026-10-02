#![allow(clippy::expect_used)]

use super::{Module, Sccp};
use crate::FunctionPass;
use std::fmt::Debug;

trait Fixture<T> {
    fn fixture(self) -> T;
}
impl<T, E: Debug> Fixture<T> for Result<T, E> {
    fn fixture(self) -> T {
        self.unwrap_or_else(|_| std::process::exit(1))
    }
}
use ncl_ir::{
    BlockId, Compare, Constant, Convert, Function, FunctionBuilder, FunctionId, OpKind, Prim,
    Terminator, Ty, ValueId,
};

fn const_value(function: &ncl_ir::Function, op_index: usize) -> &Constant {
    let OpKind::Const { result } = function.blocks[0].ops[op_index].kind else {
        panic!("operation {op_index} was not folded to a constant");
    };
    &function.constants[result.0 as usize]
}

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
    let result = Sccp.run(&mut function, &Module::default());
    assert!(result.is_ok(), "SCCP failed: {result:?}");
    assert!(result.unwrap_or(false));
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

fn folding_function() -> (Function, ValueId) {
    let mut builder = FunctionBuilder::new(FunctionId(3), "fold-all", vec![], vec![Ty::Bool]);
    let six = builder.add_constant(Constant::Fixnum(6));
    let two = builder.add_constant(Constant::Fixnum(2));
    let left = builder
        .push_op(OpKind::Const { result: six }, &[Ty::I64])
        .fixture()[0];
    let right = builder
        .push_op(OpKind::Const { result: two }, &[Ty::I64])
        .fixture()[0];
    let sub = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumSub,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let mul = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumMul,
                args: vec![sub, right],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let div = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumDiv,
                args: vec![mul, right],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let _lt = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumLt,
                args: vec![right, left],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let _le = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumLe,
                args: vec![left, left],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let _eq = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumEq,
                args: vec![left, right],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let compare = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Gt,
                left: div,
                right,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![compare],
        })
        .fixture();
    (builder.finish(), compare)
}

#[test]
fn folds_fixnum_operations_and_comparisons_to_their_values() {
    let (mut function, compare) = folding_function();
    let result = Sccp.run(&mut function, &Module::default());
    assert!(
        result.is_ok(),
        "SCCP failed: {result:?}; function: {function:?}"
    );
    assert!(result.unwrap_or(false));
    assert_eq!(const_value(&function, 2), &Constant::Fixnum(4));
    assert_eq!(const_value(&function, 3), &Constant::Fixnum(8));
    assert_eq!(const_value(&function, 4), &Constant::Fixnum(4));
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Return {
            values: vec![compare]
        }
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn folds_constant_branch_and_removes_unreachable_block() {
    let mut builder = FunctionBuilder::new(FunctionId(4), "branch", vec![], vec![Ty::I64]);
    let left_constant = builder.add_constant(Constant::Fixnum(7));
    let right_constant = builder.add_constant(Constant::Fixnum(7));
    let result = builder.add_constant(Constant::Fixnum(11));
    let left = builder
        .push_op(
            OpKind::Const {
                result: left_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
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
    let entry = BlockId(0);
    let then_block = builder.create_block(Vec::new());
    let then_value = builder
        .push_op(OpKind::Const { result }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![then_value],
        })
        .fixture();
    let else_block = builder.create_block(Vec::new());
    let else_value = builder
        .push_op(OpKind::Const { result }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![else_value],
        })
        .fixture();
    builder.position_at(entry).fixture();
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target: then_block,
            then_args: vec![],
            else_target: else_block,
            else_args: vec![],
        })
        .fixture();
    let mut function = builder.finish();

    let result = Sccp.run(&mut function, &Module::default());
    assert!(
        result.is_ok(),
        "SCCP failed: {result:?}; function: {function:?}"
    );
    assert!(result.unwrap_or(false));
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Jump {
            target: then_block,
            args: vec![]
        }
    );
    assert_eq!(function.blocks.len(), 2);
}

fn evaluate_fixnum_return(function: &Function) -> i64 {
    let mut block_id = function.blocks[0].id;
    loop {
        let block = function
            .blocks
            .iter()
            .find(|block| block.id == block_id)
            .expect("evaluation reached a missing block");
        match &block.terminator {
            Terminator::Jump { target, .. } => block_id = *target,
            Terminator::Branch {
                condition,
                then_target,
                else_target,
                ..
            } => {
                let condition_is_true = block
                    .ops
                    .iter()
                    .find_map(|op| match op.kind {
                        OpKind::Compare {
                            op: Compare::Eq,
                            left,
                            right,
                        } if op.results.first().map(|(id, _)| id) == Some(condition)
                            && left == right =>
                        {
                            Some(true)
                        }
                        _ => None,
                    })
                    .expect("branch condition has no constant definition");
                block_id = if condition_is_true {
                    *then_target
                } else {
                    *else_target
                };
            }
            Terminator::Return { values } => {
                let value = values.first().expect("expected one return value");
                let constant = block
                    .ops
                    .iter()
                    .find_map(|op| match op.kind {
                        OpKind::Const { result }
                            if op.results.first().map(|(id, _)| id) == Some(value) =>
                        {
                            Some(&function.constants[result.0 as usize])
                        }
                        _ => None,
                    })
                    .expect("return value has no constant definition");
                let Constant::Fixnum(value) = constant else {
                    panic!("expected a fixnum return value, got {constant:?}");
                };
                return *value;
            }
            terminator => panic!("unsupported terminator in test evaluator: {terminator:?}"),
        }
    }
}

#[test]
fn normalizes_unreachable_block_chain_without_changing_result() {
    let mut builder = FunctionBuilder::new(FunctionId(6), "dead-chain", vec![], vec![Ty::I64]);
    let condition_constant = builder.add_constant(Constant::Fixnum(7));
    let live_constant = builder.add_constant(Constant::Fixnum(11));
    let dead_constant = builder.add_constant(Constant::Fixnum(99));
    let condition_value = builder
        .push_op(
            OpKind::Const {
                result: condition_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let condition = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Eq,
                left: condition_value,
                right: condition_value,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let entry = BlockId(0);
    let live_block = builder.create_block(Vec::new());
    let live_value = builder
        .push_op(
            OpKind::Const {
                result: live_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![live_value],
        })
        .fixture();
    let dead_block = builder.create_block(Vec::new());
    let dead_tail = builder.create_block(Vec::new());
    let dead_value = builder
        .push_op(
            OpKind::Const {
                result: dead_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![dead_value],
        })
        .fixture();
    builder.position_at(dead_block).fixture();
    builder
        .terminate(Terminator::Jump {
            target: dead_tail,
            args: vec![],
        })
        .fixture();
    builder.position_at(entry).fixture();
    builder
        .terminate(Terminator::Branch {
            condition,
            then_target: live_block,
            then_args: vec![],
            else_target: dead_block,
            else_args: vec![],
        })
        .fixture();
    let mut function = builder.finish();
    let before = function.clone();

    assert_eq!(evaluate_fixnum_return(&before), 11);
    let result = Sccp.run(&mut function, &Module::default());
    assert!(
        result.is_ok(),
        "SCCP failed: {result:?}; function: {function:?}"
    );
    assert!(result.unwrap_or(false));
    assert_eq!(evaluate_fixnum_return(&function), 11);
    assert_eq!(
        function
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![entry, live_block]
    );
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Jump {
            target: live_block,
            args: vec![]
        }
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn folds_constant_switch_to_default_and_removes_other_cases() {
    let mut builder = FunctionBuilder::new(FunctionId(5), "switch", vec![], vec![Ty::I64]);
    let selector = builder.add_constant(Constant::Fixnum(9));
    let case_value = builder.add_constant(Constant::Fixnum(10));
    let default_value = builder.add_constant(Constant::Fixnum(20));
    let selector_value = builder
        .push_op(OpKind::Const { result: selector }, &[Ty::I64])
        .fixture()[0];
    let case_block = builder.create_block(Vec::new());
    let case_result = builder
        .push_op(OpKind::Const { result: case_value }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![case_result],
        })
        .fixture();
    let default_block = builder.create_block(Vec::new());
    let default_result = builder
        .push_op(
            OpKind::Const {
                result: default_value,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![default_result],
        })
        .fixture();
    builder.position_at(BlockId(0)).fixture();
    builder
        .terminate(Terminator::Switch {
            value: selector_value,
            cases: vec![(4, case_block, vec![])],
            default: default_block,
            default_args: vec![],
        })
        .fixture();
    let mut function = builder.finish();

    let result = Sccp.run(&mut function, &Module::default());
    assert!(
        result.is_ok(),
        "SCCP failed: {result:?}; function: {function:?}"
    );
    assert!(result.unwrap_or(false));
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Jump {
            target: default_block,
            args: vec![]
        }
    );
    assert_eq!(function.blocks.len(), 2);
    ncl_ir::verify(&function).fixture();
}
