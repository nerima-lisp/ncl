use super::*;
use ncl_ir::{BasicBlock, FunctionId, Param};

fn function(constants: Vec<Constant>) -> Function {
    Function {
        id: FunctionId(10),
        name: "sccp-private".into(),
        params: vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        return_types: vec![],
        blocks: vec![],
        locals: vec![],
        constants,
        handler_regions: vec![],
        debug: vec![],
    }
}

fn op(kind: OpKind) -> Op {
    Op {
        results: vec![],
        kind,
        loc: None,
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "table-driven coverage fixture exercises private lattice cases"
)]
fn evaluates_private_lattice_and_successor_paths() {
    assert_eq!(
        State::Unknown.merge(State::Constant(ConstantIndex(0))),
        State::Constant(ConstantIndex(0))
    );
    assert_eq!(
        State::Constant(ConstantIndex(0)).merge(State::Constant(ConstantIndex(1))),
        State::Overdefined
    );
    let mut states = HashMap::from([
        (ValueId(0), State::Constant(ConstantIndex(0))),
        (ValueId(1), State::Constant(ConstantIndex(1))),
        (ValueId(4), State::Overdefined),
    ]);
    assert!(Sccp::update(
        &mut states,
        ValueId(2),
        State::Constant(ConstantIndex(0))
    ));
    assert!(!Sccp::update(
        &mut states,
        ValueId(2),
        State::Constant(ConstantIndex(0))
    ));
    assert_eq!(
        Sccp::binary_unknown(&states, ValueId(4), ValueId(9)),
        State::Overdefined
    );
    assert_eq!(
        Sccp::binary_unknown(&states, ValueId(9), ValueId(8)),
        State::Unknown
    );

    let mut current = function(vec![
        Constant::Fixnum(2),
        Constant::Fixnum(3),
        Constant::T,
        Constant::Nil,
    ]);
    let fixnums = [
        Prim::FixnumAdd,
        Prim::FixnumSub,
        Prim::FixnumMul,
        Prim::FixnumDiv,
    ];
    for primitive in fixnums {
        let result = Sccp::eval_op(
            &mut current,
            &states,
            &op(OpKind::Prim {
                op: primitive,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            }),
        );
        assert!(matches!(result, State::Constant(_)));
    }
    let mut exceptional = function(vec![
        Constant::Fixnum(i64::MAX),
        Constant::Fixnum(1),
        Constant::Fixnum(0),
    ]);
    let exceptional_states = HashMap::from([
        (ValueId(0), State::Constant(ConstantIndex(0))),
        (ValueId(1), State::Constant(ConstantIndex(1))),
        (ValueId(2), State::Constant(ConstantIndex(2))),
    ]);
    assert_eq!(
        Sccp::eval_op(
            &mut exceptional,
            &exceptional_states,
            &op(OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(1)],
                condition: None
            })
        ),
        State::Overdefined
    );
    assert_eq!(
        Sccp::eval_op(
            &mut exceptional,
            &exceptional_states,
            &op(OpKind::Prim {
                op: Prim::FixnumDiv,
                args: vec![ValueId(0), ValueId(2)],
                condition: None
            })
        ),
        State::Overdefined
    );

    for primitive in [Prim::FixnumLt, Prim::FixnumLe, Prim::FixnumEq] {
        assert!(matches!(
            Sccp::eval_op(
                &mut current,
                &states,
                &op(OpKind::Prim {
                    op: primitive,
                    args: vec![ValueId(0), ValueId(1)],
                    condition: None
                })
            ),
            State::Constant(_)
        ));
    }
    assert_eq!(
        Sccp::eval_op(
            &mut current,
            &HashMap::new(),
            &op(OpKind::Prim {
                op: Prim::FixnumEq,
                args: vec![ValueId(7), ValueId(8)],
                condition: None
            })
        ),
        State::Unknown
    );
    assert!(matches!(
        Sccp::eval_op(
            &mut current,
            &states,
            &op(OpKind::Prim {
                op: Prim::Eq,
                args: vec![ValueId(0), ValueId(0)],
                condition: None
            })
        ),
        State::Constant(_)
    ));
    assert_eq!(
        Sccp::eval_op(
            &mut current,
            &states,
            &op(OpKind::Prim {
                op: Prim::Eq,
                args: vec![ValueId(0), ValueId(1)],
                condition: None
            })
        ),
        State::Unknown
    );
    assert_eq!(
        Sccp::eval_op(
            &mut current,
            &states,
            &op(OpKind::Prim {
                op: Prim::Car,
                args: vec![ValueId(0)],
                condition: None
            })
        ),
        State::Overdefined
    );
    assert_eq!(
        Sccp::eval_op(&mut current, &states, &op(OpKind::Safepoint)),
        State::Unknown
    );
    assert_eq!(
        Sccp::eval_op(
            &mut current,
            &states,
            &op(OpKind::Const {
                result: ConstantIndex(99)
            })
        ),
        State::Unknown
    );
    assert_eq!(
        Sccp::eval_op(
            &mut current,
            &states,
            &op(OpKind::Move { value: ValueId(9) })
        ),
        State::Unknown
    );

    for compare in [
        Compare::Eq,
        Compare::Ne,
        Compare::Lt,
        Compare::Le,
        Compare::Gt,
        Compare::Ge,
    ] {
        assert!(matches!(
            Sccp::eval_op(
                &mut current,
                &states,
                &op(OpKind::Compare {
                    op: compare,
                    left: ValueId(0),
                    right: ValueId(1)
                })
            ),
            State::Constant(_)
        ));
    }
    for compare in [
        Compare::Eq,
        Compare::Ne,
        Compare::Lt,
        Compare::Le,
        Compare::Gt,
        Compare::Ge,
    ] {
        assert!(matches!(
            Sccp::eval_op(
                &mut current,
                &states,
                &op(OpKind::Compare {
                    op: compare,
                    left: ValueId(0),
                    right: ValueId(0)
                })
            ),
            State::Constant(_)
        ));
    }
    assert_eq!(
        Sccp::eval_op(
            &mut current,
            &HashMap::new(),
            &op(OpKind::Compare {
                op: Compare::Eq,
                left: ValueId(7),
                right: ValueId(8)
            })
        ),
        State::Unknown
    );

    let branch = BasicBlock {
        id: BlockId(0),
        params: vec![],
        ops: vec![],
        terminator: Terminator::Branch {
            condition: ValueId(2),
            then_target: BlockId(1),
            then_args: vec![],
            else_target: BlockId(2),
            else_args: vec![],
        },
    };
    let true_states = HashMap::from([(ValueId(2), State::Constant(ConstantIndex(2)))]);
    assert_eq!(Sccp::successors(&current, &true_states, &branch).len(), 1);
    let false_states = HashMap::from([(ValueId(2), State::Constant(ConstantIndex(3)))]);
    assert_eq!(Sccp::successors(&current, &false_states, &branch).len(), 1);
    assert_eq!(
        Sccp::successors(&current, &HashMap::new(), &branch).len(),
        2
    );
    let switch = BasicBlock {
        id: BlockId(0),
        params: vec![],
        ops: vec![],
        terminator: Terminator::Switch {
            value: ValueId(0),
            cases: vec![(2, BlockId(1), vec![])],
            default: BlockId(2),
            default_args: vec![],
        },
    };
    assert_eq!(Sccp::successors(&current, &states, &switch).len(), 1);
    assert_eq!(
        Sccp::successors(&current, &HashMap::new(), &switch).len(),
        2
    );
    let non_fixnum = HashMap::from([(ValueId(0), State::Constant(ConstantIndex(2)))]);
    assert_eq!(Sccp::successors(&current, &non_fixnum, &switch).len(), 1);
    let miss = HashMap::from([(ValueId(0), State::Constant(ConstantIndex(1)))]);
    assert_eq!(Sccp::successors(&current, &miss, &switch).len(), 1);
}

#[test]
fn rewrites_constant_results_and_retains_referenced_blocks() {
    let mut function = function(vec![Constant::Fixnum(7), Constant::T, Constant::Nil]);
    function.blocks = vec![
        BasicBlock {
            id: BlockId(0),
            params: vec![],
            ops: vec![Op {
                results: vec![(ValueId(5), Ty::I64)],
                kind: OpKind::Move { value: ValueId(0) },
                loc: None,
            }],
            terminator: Terminator::Branch {
                condition: ValueId(1),
                then_target: BlockId(1),
                then_args: vec![],
                else_target: BlockId(2),
                else_args: vec![],
            },
        },
        BasicBlock {
            id: BlockId(1),
            params: vec![],
            ops: vec![],
            terminator: Terminator::Return { values: vec![] },
        },
        BasicBlock {
            id: BlockId(2),
            params: vec![],
            ops: vec![],
            terminator: Terminator::Return { values: vec![] },
        },
        BasicBlock {
            id: BlockId(3),
            params: vec![],
            ops: vec![],
            terminator: Terminator::Return { values: vec![] },
        },
    ];
    let states = HashMap::from([
        (ValueId(5), State::Constant(ConstantIndex(0))),
        (ValueId(1), State::Constant(ConstantIndex(1))),
    ]);
    assert!(Sccp::rewrite(
        &mut function,
        &HashSet::from([BlockId(0), BlockId(1), BlockId(2)]),
        &states
    ));
    assert!(matches!(
        function.blocks[0].ops[0].kind,
        OpKind::Const {
            result: ConstantIndex(0)
        }
    ));
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Jump {
            target: BlockId(1),
            ..
        }
    ));
    assert_eq!(function.blocks.len(), 2);
    assert!(!Sccp::retain_referenced_blocks(&mut Function {
        blocks: vec![],
        ..function.clone()
    }));
    assert!(
        Sccp::constant_value_from(&function.constants, State::Constant(ConstantIndex(99)))
            .is_none()
    );
}
