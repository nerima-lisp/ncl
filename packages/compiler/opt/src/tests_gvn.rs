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
use ncl_ir::{
    BasicBlock, Function, FunctionBuilder, FunctionId, Op, OpKind, Param, Prim, Terminator, Ty,
    ValueId,
};

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

#[test]
fn rewrites_values_in_every_terminator_shape() {
    fn function_with(term: Terminator, ty: Ty) -> Function {
        let return_types = if matches!(term, Terminator::Return { .. }) {
            vec![ty]
        } else {
            Vec::new()
        };
        Function {
            id: FunctionId(5),
            name: "gvn-terminator".into(),
            params: vec![Param {
                name: "value".into(),
                ty,
            }],
            return_types,
            blocks: vec![BasicBlock {
                id: ncl_ir::BlockId(0),
                params: Vec::new(),
                ops: vec![
                    Op {
                        results: vec![(ValueId(1), ty)],
                        kind: OpKind::Move { value: ValueId(0) },
                        loc: None,
                    },
                    Op {
                        results: vec![(ValueId(2), ty)],
                        kind: OpKind::Move { value: ValueId(0) },
                        loc: None,
                    },
                ],
                terminator: term,
            }],
            locals: Vec::new(),
            constants: Vec::new(),
            handler_regions: Vec::new(),
            debug: Vec::new(),
        }
    }

    let cases = vec![
        (
            Terminator::Jump {
                target: ncl_ir::BlockId(1),
                args: vec![ValueId(2)],
            },
            Terminator::Jump {
                target: ncl_ir::BlockId(1),
                args: vec![ValueId(1)],
            },
            Ty::Word,
        ),
        (
            Terminator::Branch {
                condition: ValueId(2),
                then_target: ncl_ir::BlockId(1),
                then_args: vec![],
                else_target: ncl_ir::BlockId(2),
                else_args: vec![],
            },
            Terminator::Branch {
                condition: ValueId(1),
                then_target: ncl_ir::BlockId(1),
                then_args: vec![],
                else_target: ncl_ir::BlockId(2),
                else_args: vec![],
            },
            Ty::Bool,
        ),
        (
            Terminator::Switch {
                value: ValueId(2),
                cases: vec![(1, ncl_ir::BlockId(1), vec![ValueId(2)])],
                default: ncl_ir::BlockId(2),
                default_args: vec![ValueId(2)],
            },
            Terminator::Switch {
                value: ValueId(1),
                cases: vec![(1, ncl_ir::BlockId(1), vec![ValueId(1)])],
                default: ncl_ir::BlockId(2),
                default_args: vec![ValueId(1)],
            },
            Ty::I64,
        ),
        (
            Terminator::CallReturn {
                function: ValueId(2),
                args: vec![ValueId(2)],
            },
            Terminator::CallReturn {
                function: ValueId(1),
                args: vec![ValueId(1)],
            },
            Ty::Word,
        ),
        (
            Terminator::TailCall {
                function: ValueId(2),
                args: vec![ValueId(2)],
            },
            Terminator::TailCall {
                function: ValueId(1),
                args: vec![ValueId(1)],
            },
            Ty::Word,
        ),
        (
            Terminator::Return {
                values: vec![ValueId(2)],
            },
            Terminator::Return {
                values: vec![ValueId(1)],
            },
            Ty::Word,
        ),
        (
            Terminator::Throw {
                condition: ValueId(2),
            },
            Terminator::Throw {
                condition: ValueId(1),
            },
            Ty::Word,
        ),
        (Terminator::Unreachable, Terminator::Unreachable, Ty::Word),
    ];

    for (term, expected, ty) in cases {
        let mut function = function_with(term, ty);
        if matches!(expected, Terminator::Jump { .. } | Terminator::Branch { .. } | Terminator::Switch { .. }) {
            let has_target_params = matches!(expected, Terminator::Jump { .. } | Terminator::Switch { .. });
            let target_params = if has_target_params {
                vec![
                    ncl_ir::BlockParam {
                        value: ValueId(3),
                        ty,
                    },
                ]
            } else {
                Vec::new()
            };
            function.blocks.push(BasicBlock {
                id: ncl_ir::BlockId(1),
                params: target_params.clone(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            });
            function.blocks.push(BasicBlock {
                id: ncl_ir::BlockId(2),
                params: if has_target_params {
                    vec![ncl_ir::BlockParam {
                        value: ValueId(4),
                        ty,
                    }]
                } else {
                    Vec::new()
                },
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            });
        }
        let mut pass = GlobalValueNumbering;
        let result = pass.run(&mut function, &Module::default());
        if matches!(expected, Terminator::Unreachable) {
            assert!(result.is_err(), "Unreachable should fail IR verification");
            continue;
        }
        assert!(result.is_ok(), "GVN failed for {expected:?}: {result:?}");
        assert!(result.unwrap_or(false));
        assert_eq!(function.blocks[0].terminator, expected);
        assert_eq!(function.blocks[0].ops.len(), 1);
        assert!(ncl_ir::verify(&function).is_ok(), "invalid {function:?}");
    }
}

#[test]
fn keeps_prim_conditions_in_expression_keys() {
    let mut builder = FunctionBuilder::new(
        FunctionId(6),
        "gvn-prim-condition",
        vec![Param {
            name: "x".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    let first = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: Some(ncl_ir::BlockId(1)),
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let different_condition = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: Some(ncl_ir::BlockId(2)),
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let duplicate = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: Some(ncl_ir::BlockId(1)),
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![duplicate],
        })
        .fixture();
    let constant = builder.add_constant(ncl_ir::Constant::Fixnum(0));

    let mut function = builder.finish();
    function.blocks.extend([
        BasicBlock {
            id: ncl_ir::BlockId(1),
            params: Vec::new(),
            ops: vec![Op {
                results: vec![(ValueId(3), Ty::I64)],
                kind: OpKind::Const { result: constant },
                loc: None,
            }],
            terminator: Terminator::Return {
                values: vec![ValueId(3)],
            },
        },
        BasicBlock {
            id: ncl_ir::BlockId(2),
            params: Vec::new(),
            ops: vec![Op {
                results: vec![(ValueId(4), Ty::I64)],
                kind: OpKind::Const { result: constant },
                loc: None,
            }],
            terminator: Terminator::Return {
                values: vec![ValueId(4)],
            },
        },
    ]);
    let result = GlobalValueNumbering.run(&mut function, &Module::default());
    assert!(result.is_ok(), "GVN failed: {result:?}");
    assert!(result.unwrap_or(false));
    assert_eq!(function.blocks[0].ops.len(), 2);
    assert!(function.blocks[0].ops.iter().any(|op| {
        matches!(
            op.kind,
            OpKind::Prim {
                condition: Some(ncl_ir::BlockId(2)),
                ..
            }
        )
    }));
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Return {
            values: vec![first]
        }
    );
    assert_ne!(first, different_condition);
    ncl_ir::verify(&function).fixture();
}

#[test]
fn store_field_and_call_each_invalidate_loads() {
    let mut builder = FunctionBuilder::new(
        FunctionId(7),
        "gvn-load-invalidation",
        vec![
            Param {
                name: "object".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word, Ty::Word, Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 1,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 1,
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::StoreField {
                object: ValueId(0),
                field: 1,
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    let after_store = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 1,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::Call {
                function: ValueId(1),
                args: Vec::new(),
            },
            &[Ty::Word],
        )
        .fixture();
    let after_call = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 1,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![first, after_store, after_call],
        })
        .fixture();

    let mut function = builder.finish();
    let result = GlobalValueNumbering.run(&mut function, &Module::default());
    assert!(result.is_ok(), "GVN failed: {result:?}");
    assert!(result.unwrap_or(false));
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::LoadField { .. }))
            .count(),
        3
    );
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Return {
            values: vec![first, after_store, after_call]
        }
    );
    ncl_ir::verify(&function).fixture();
}
