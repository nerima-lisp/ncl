#![allow(clippy::unwrap_used)]

use super::{GlobalValueNumbering, Module};
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
use ncl_ir::{FunctionBuilder, FunctionId, OpKind, Param, Prim, Terminator, Ty, ValueId};

fn binary_duplicate() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "duplicate",
        vec![Param {
            name: "x".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    builder.finish()
}

#[test]
fn removes_redundant_pure_computation() {
    let mut function = binary_duplicate();
    let mut pass = GlobalValueNumbering;
    let result = pass.run(&mut function, &Module::default());
    assert!(result.is_ok(), "GVN failed: {result:?}");
    assert!(result.unwrap_or(false));
    assert_eq!(function.blocks[0].ops.len(), 1);
    assert!(
        matches!(&function.blocks[0].terminator, Terminator::Return { values } if values.len() == 1)
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn store_invalidates_load_value() {
    let mut builder = FunctionBuilder::new(
        FunctionId(2),
        "memory",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let duplicate = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(
            OpKind::Store {
                address: ValueId(0),
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::Load { .. }))
            .count(),
        2
    );
    assert_ne!(first, duplicate);
    assert_ne!(first, second);
}

#[test]
fn removes_duplicate_expression_in_a_dominated_branch_and_rewrites_return() {
    let mut builder = FunctionBuilder::new(
        FunctionId(3),
        "dominated-duplicate",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        vec![Ty::Bool],
    );
    let entry_value = builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Bool])
        .fixture()[0];
    let then_block = builder.create_block(Vec::new());
    let then_value = builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Bool])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![then_value],
        })
        .fixture();
    let else_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return {
            values: vec![entry_value],
        })
        .fixture();
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(0),
            then_target: then_block,
            then_args: vec![],
            else_target: else_block,
            else_args: vec![],
        })
        .fixture();
    let mut function = builder.finish();

    let mut pass = GlobalValueNumbering;
    let result = pass.run(&mut function, &Module::default());
    assert!(result.is_ok(), "GVN failed: {result:?}");
    assert!(result.unwrap_or(false));
    assert!(function.blocks[1].ops.is_empty());
    assert_eq!(
        function.blocks[1].terminator,
        Terminator::Return {
            values: vec![entry_value]
        }
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn merges_duplicate_comparisons_and_preserves_boolean_result() {
    let mut builder = FunctionBuilder::new(
        FunctionId(4),
        "duplicate-compare",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::I64,
            },
            Param {
                name: "right".into(),
                ty: Ty::I64,
            },
        ],
        vec![Ty::Bool],
    );
    builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: ValueId(0),
                right: ValueId(1),
            },
            &[Ty::Bool],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: ValueId(0),
                right: ValueId(1),
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    let mut function = builder.finish();
    let expected_result = function.blocks[0].ops[0].results[0].0;
    assert_eq!(function.blocks[0].ops.len(), 2);

    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());

    assert_eq!(function.blocks[0].ops.len(), 1);
    assert!(matches!(
        function.blocks[0].ops[0].kind,
        OpKind::Compare {
            op: ncl_ir::Compare::Eq,
            left: ValueId(0),
            right: ValueId(1)
        }
    ));
    assert_eq!(function.blocks[0].ops[0].results[0].0, expected_result);
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Return {
            values: vec![expected_result]
        }
    );
    ncl_ir::verify(&function).fixture();
}
