use super::tests_support::Fixture;
use super::{Module, Sccp};
use crate::FunctionPass;
use ncl_ir::{
    Compare, Constant, Convert, FunctionBuilder, FunctionId, OpKind, Param, Prim, Terminator, Ty,
    ValueId,
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
fn leaves_conditional_primitive_unfolded() {
    let mut builder = FunctionBuilder::new(FunctionId(3), "conditional", vec![], vec![Ty::I64]);
    let condition_target = builder.create_block(Vec::new());
    let left = builder.add_constant(Constant::Fixnum(2));
    let right = builder.add_constant(Constant::Fixnum(3));
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let left_values = builder
        .push_op(OpKind::Const { result: left }, &[Ty::I64])
        .fixture();
    let Some(left) = left_values.first().copied() else {
        std::process::exit(1);
    };
    let right_values = builder
        .push_op(OpKind::Const { result: right }, &[Ty::I64])
        .fixture();
    let Some(right) = right_values.first().copied() else {
        std::process::exit(1);
    };
    let sum_values = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: Some(condition_target),
            },
            &[Ty::I64],
        )
        .fixture();
    let Some(sum) = sum_values.first().copied() else {
        std::process::exit(1);
    };
    builder
        .terminate(Terminator::Return { values: vec![sum] })
        .fixture();
    let mut function = builder.finish();
    if Sccp.run(&mut function, &Module::default()).fixture() {
        std::process::exit(1);
    }
    let Some(block) = function.blocks.first() else {
        std::process::exit(1);
    };
    let Some(op) = block.ops.get(2) else {
        std::process::exit(1);
    };
    if !matches!(
        op.kind,
        OpKind::Prim {
            condition: Some(_),
            ..
        }
    ) {
        std::process::exit(1);
    }
    ncl_ir::verify(&function).fixture();
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
fn does_not_fold_a_join_with_conflicting_lattice_values() {
    let mut builder = FunctionBuilder::new(
        FunctionId(4),
        "join",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        vec![Ty::I64],
    );
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    let join_value = ValueId(3);
    let join_block = builder.create_block(vec![(Ty::I64, join_value)]);
    let two = builder.add_constant(Constant::Fixnum(2));
    let three = builder.add_constant(Constant::Fixnum(3));

    builder.position_at(ncl_ir::BlockId(0)).fixture();
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(0),
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .fixture();
    builder.position_at(then_block).fixture();
    let then_value = builder
        .push_op(OpKind::Const { result: two }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Jump {
            target: join_block,
            args: vec![then_value],
        })
        .fixture();
    builder.position_at(else_block).fixture();
    let else_value = builder
        .push_op(OpKind::Const { result: three }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Jump {
            target: join_block,
            args: vec![else_value],
        })
        .fixture();
    builder.position_at(join_block).fixture();
    let result = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![join_value, join_value],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[3].ops[0].kind,
        OpKind::Prim {
            op: Prim::FixnumAdd,
            ..
        }
    ));
}

#[test]
fn folds_constant_branch_and_rewrites_the_terminator() {
    let mut builder = FunctionBuilder::new(FunctionId(5), "branch", vec![], vec![Ty::I64]);
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    let one = builder.add_constant(Constant::Fixnum(1));
    let one_again = builder.add_constant(Constant::Fixnum(1));
    let seven = builder.add_constant(Constant::Fixnum(7));
    let eight = builder.add_constant(Constant::Fixnum(8));

    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let left = builder
        .push_op(OpKind::Const { result: one }, &[Ty::I64])
        .fixture()[0];
    let right = builder
        .push_op(OpKind::Const { result: one_again }, &[Ty::I64])
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
    builder.position_at(then_block).fixture();
    let then_value = builder
        .push_op(OpKind::Const { result: seven }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![then_value],
        })
        .fixture();
    builder.position_at(else_block).fixture();
    let else_value = builder
        .push_op(OpKind::Const { result: eight }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![else_value],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Jump { target, .. } if target == then_block
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn preserves_conditional_prim_failure_edge_without_folding_it() {
    let mut builder = FunctionBuilder::new(FunctionId(6), "failure-edge", vec![], vec![Ty::I64]);
    let failure_block = builder.create_block(Vec::new());
    let left = builder.add_constant(Constant::Fixnum(2));
    let right = builder.add_constant(Constant::Fixnum(3));
    let one = builder.add_constant(Constant::Fixnum(1));

    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let left = builder
        .push_op(OpKind::Const { result: left }, &[Ty::I64])
        .fixture()[0];
    let right = builder
        .push_op(OpKind::Const { result: right }, &[Ty::I64])
        .fixture()[0];
    let result = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![left, right],
                condition: Some(failure_block),
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .fixture();
    builder.position_at(failure_block).fixture();
    let one = builder
        .push_op(OpKind::Const { result: one }, &[Ty::I64])
        .fixture()[0];
    let failure_result = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![one, one],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![failure_result],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].ops[2].kind,
        OpKind::Prim {
            condition: Some(target),
            ..
        } if target == failure_block
    ));
    assert!(matches!(
        function.blocks[1].ops[1].kind,
        OpKind::Prim {
            op: Prim::FixnumAdd,
            condition: None,
            ..
        }
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn leaves_unknown_primitive_as_an_exact_no_op() {
    let mut builder = FunctionBuilder::new(
        FunctionId(7),
        "unknown",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let result = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Car,
                args: vec![ValueId(0)],
                condition: None,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .fixture();

    let mut function = builder.finish();
    let before = function.clone();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert_eq!(function, before);
}

#[test]
fn folds_fixnum_primitives_and_comparisons() {
    let mut builder = FunctionBuilder::new(FunctionId(8), "primitives", vec![], vec![Ty::Bool]);
    let two = builder.add_constant(Constant::Fixnum(2));
    let three = builder.add_constant(Constant::Fixnum(3));
    let left = builder
        .push_op(OpKind::Const { result: two }, &[Ty::I64])
        .fixture()[0];
    let right = builder
        .push_op(OpKind::Const { result: three }, &[Ty::I64])
        .fixture()[0];
    let t = builder.add_constant(Constant::T);
    let nil = builder.add_constant(Constant::Nil);
    let word_left = builder
        .push_op(OpKind::Const { result: t }, &[Ty::Word])
        .fixture()[0];
    let word_right = builder
        .push_op(OpKind::Const { result: nil }, &[Ty::Word])
        .fixture()[0];
    let operations = [
        Prim::FixnumSub,
        Prim::FixnumMul,
        Prim::FixnumDiv,
        Prim::FixnumLt,
        Prim::FixnumLe,
        Prim::FixnumEq,
    ];
    let mut results = Vec::new();
    for op in operations {
        let result_ty = if matches!(op, Prim::FixnumLt | Prim::FixnumLe | Prim::FixnumEq) {
            Ty::Bool
        } else {
            Ty::I64
        };
        results.push(
            builder
                .push_op(
                    OpKind::Prim {
                        op,
                        args: vec![left, right],
                        condition: None,
                    },
                    &[result_ty],
                )
                .fixture()[0],
        );
    }
    let same_eq = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Eq,
                args: vec![word_left, word_left],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let same_eql = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Eql,
                args: vec![word_right, word_right],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    results.extend([same_eq, same_eql]);
    builder
        .terminate(Terminator::Return {
            values: vec![results[7]],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(Sccp.run(&mut function, &Module::default()).fixture());
    for (index, result) in results.into_iter().enumerate() {
        let expected_const = index < 3;
        let kind = function.blocks[0]
            .ops
            .iter()
            .find(|op| op.results.iter().any(|(value, _)| *value == result))
            .map(|op| &op.kind);
        if expected_const {
            assert!(matches!(kind, Some(OpKind::Const { .. })));
        } else {
            assert!(matches!(kind, Some(OpKind::Prim { .. })));
        }
    }
}

#[test]
fn folds_all_compare_operators_and_false_branch() {
    let mut builder = FunctionBuilder::new(FunctionId(9), "compare", vec![], vec![Ty::I64]);
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let two = builder.add_constant(Constant::Fixnum(2));
    let three = builder.add_constant(Constant::Fixnum(3));
    let left = builder
        .push_op(OpKind::Const { result: two }, &[Ty::I64])
        .fixture()[0];
    let right = builder
        .push_op(OpKind::Const { result: three }, &[Ty::I64])
        .fixture()[0];
    for op in [
        Compare::Eq,
        Compare::Ne,
        Compare::Lt,
        Compare::Le,
        Compare::Gt,
        Compare::Ge,
    ] {
        builder
            .push_op(OpKind::Compare { op, left, right }, &[Ty::Bool])
            .fixture();
    }
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
    builder.position_at(then_block).fixture();
    let then_value = builder
        .push_op(OpKind::Const { result: two }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![then_value],
        })
        .fixture();
    builder.position_at(else_block).fixture();
    let else_value = builder
        .push_op(OpKind::Const { result: three }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![else_value],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Jump { target, .. } if target == else_block
    ));
}

#[test]
fn folds_switch_match_and_default_edges() {
    for (value, matches_case) in [(2, true), (7, false)] {
        let mut builder = FunctionBuilder::new(
            FunctionId(u32::try_from(value).fixture() + 10),
            "switch",
            vec![],
            vec![],
        );
        let case_block = builder.create_block(Vec::new());
        let default_block = builder.create_block(Vec::new());
        builder.position_at(ncl_ir::BlockId(0)).fixture();
        let constant = builder.add_constant(Constant::Fixnum(value));
        let selector = builder
            .push_op(OpKind::Const { result: constant }, &[Ty::I64])
            .fixture()[0];
        builder
            .terminate(Terminator::Switch {
                value: selector,
                cases: vec![(2, case_block, Vec::new())],
                default: default_block,
                default_args: Vec::new(),
            })
            .fixture();
        builder.position_at(case_block).fixture();
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .fixture();
        builder.position_at(default_block).fixture();
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .fixture();

        let mut function = builder.finish();
        assert!(Sccp.run(&mut function, &Module::default()).fixture());
        let expected_target = if matches_case {
            case_block
        } else {
            default_block
        };
        assert!(matches!(
            function.blocks[0].terminator,
            Terminator::Jump { target, .. } if target == expected_target
        ));
    }
}

#[test]
fn keeps_unknown_switch_reachable_and_rewrites_typed_moves() {
    let mut builder = FunctionBuilder::new(
        FunctionId(12),
        "unknown-switch",
        vec![Param {
            name: "selector".into(),
            ty: Ty::I64,
        }],
        vec![Ty::Word],
    );
    let case_block = builder.create_block(Vec::new());
    let default_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let word_constant = builder.add_constant(Constant::T);
    let float_constant = builder.add_constant(Constant::DoubleFloat(1.5));
    let word = builder
        .push_op(
            OpKind::Const {
                result: word_constant,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let moved_word = builder
        .push_op(OpKind::Move { value: word }, &[Ty::Word])
        .fixture()[0];
    let float = builder
        .push_op(
            OpKind::Const {
                result: float_constant,
            },
            &[Ty::F64],
        )
        .fixture()[0];
    let moved_float = builder
        .push_op(OpKind::Move { value: float }, &[Ty::F64])
        .fixture()[0];
    builder
        .terminate(Terminator::Switch {
            value: ValueId(0),
            cases: vec![(1, case_block, Vec::new())],
            default: default_block,
            default_args: Vec::new(),
        })
        .fixture();
    builder.position_at(case_block).fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![moved_word],
        })
        .fixture();
    builder.position_at(default_block).fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![moved_word],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Switch { .. }
    ));
    assert!(matches!(
        function.blocks[0].ops[1].kind,
        OpKind::Const { .. }
    ));
    assert!(matches!(
        function.blocks[0].ops[3].kind,
        OpKind::Const { .. }
    ));
    assert!(matches!(
        function.blocks[0]
            .ops
            .iter()
            .find(|op| op.results.iter().any(|(value, _)| *value == moved_float))
            .map(|op| &op.kind),
        Some(OpKind::Const { .. })
    ));
}

#[test]
fn ignores_valid_but_unreachable_blocks() {
    let mut builder = FunctionBuilder::new(FunctionId(13), "unreachable", vec![], vec![Ty::I64]);
    let dead_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let live_constant = builder.add_constant(Constant::Fixnum(1));
    let live = builder
        .push_op(
            OpKind::Const {
                result: live_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return { values: vec![live] })
        .fixture();
    builder.position_at(dead_block).fixture();
    let dead_constant = builder.add_constant(Constant::Fixnum(2));
    let dead = builder
        .push_op(
            OpKind::Const {
                result: dead_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let dead_result = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![dead, dead],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![dead_result],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[1].ops[1].kind,
        OpKind::Prim { .. }
    ));
}

fn assert_preserved_op(function: &ncl_ir::Function, value: ValueId) {
    assert!(matches!(
        function.blocks[0]
            .ops
            .iter()
            .find(|op| op.results.iter().any(|(result, _)| *result == value))
            .map(|op| &op.kind),
        Some(OpKind::Prim { .. } | OpKind::Compare { .. } | OpKind::Load { .. })
    ));
}

#[test]
fn keeps_overdefined_and_unknown_operations_conservative() {
    let mut builder = FunctionBuilder::new(
        FunctionId(14),
        "lattice-edges",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::I64,
            },
            Param {
                name: "right".into(),
                ty: Ty::I64,
            },
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "word_left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "word_right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let add = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let less_than = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumLt,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let eq = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Eq,
                args: vec![ValueId(3), ValueId(4)],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let same_compare = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Ge,
                left: ValueId(0),
                right: ValueId(0),
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let unknown_compare = builder
        .push_op(
            OpKind::Compare {
                op: Compare::Ne,
                left: ValueId(0),
                right: ValueId(1),
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let loaded = builder
        .push_op(
            OpKind::Load {
                address: ValueId(2),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![loaded],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    for value in [add, less_than, eq, same_compare, unknown_compare, loaded] {
        assert_preserved_op(&function, value);
    }
}

#[test]
fn keeps_all_conversions_conservative() {
    let mut builder = FunctionBuilder::new(
        FunctionId(15),
        "conversions",
        vec![
            Param {
                name: "word".into(),
                ty: Ty::Word,
            },
            Param {
                name: "integer".into(),
                ty: Ty::I64,
            },
            Param {
                name: "float".into(),
                ty: Ty::F64,
            },
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
        ],
        vec![Ty::Word],
    );
    let conversions = [
        (Convert::WordToI64, ValueId(0), Ty::I64),
        (Convert::I64ToWord, ValueId(1), Ty::Word),
        (Convert::WordToF64, ValueId(0), Ty::F64),
        (Convert::F64ToWord, ValueId(2), Ty::Word),
        (Convert::AddressToWord, ValueId(3), Ty::Word),
        (Convert::WordToAddress, ValueId(0), Ty::Address),
    ];
    let mut results = Vec::new();
    for (op, value, ty) in conversions {
        results.push(
            builder
                .push_op(OpKind::Convert { op, value }, &[ty])
                .fixture()[0],
        );
    }
    builder
        .terminate(Terminator::Return {
            values: vec![results[1]],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    for value in results {
        assert!(matches!(
            function.blocks[0]
                .ops
                .iter()
                .find(|op| op.results.iter().any(|(result, _)| *result == value))
                .map(|op| &op.kind),
            Some(OpKind::Convert { .. })
        ));
    }
    ncl_ir::verify(&function).fixture();
}

#[test]
fn keeps_an_unknown_switch_conservative() {
    let mut builder = FunctionBuilder::new(
        FunctionId(16),
        "unknown-switch-again",
        vec![Param {
            name: "selector".into(),
            ty: Ty::I64,
        }],
        vec![],
    );
    let case_block = builder.create_block(Vec::new());
    let default_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    builder
        .terminate(Terminator::Switch {
            value: ValueId(0),
            cases: vec![(1, case_block, Vec::new())],
            default: default_block,
            default_args: Vec::new(),
        })
        .fixture();
    builder.position_at(case_block).fixture();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    builder.position_at(default_block).fixture();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();

    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Switch { .. }
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn ignores_resultless_operations() {
    let mut builder = FunctionBuilder::new(FunctionId(17), "safepoint", vec![], vec![Ty::I64]);
    let constant = builder.add_constant(Constant::Fixnum(4));
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .fixture();

    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert!(matches!(function.blocks[0].ops[0].kind, OpKind::Safepoint));
    ncl_ir::verify(&function).fixture();
}
