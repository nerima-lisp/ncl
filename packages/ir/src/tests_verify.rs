use super::tests_text::{block, finish, op};
use crate::*;

fn verify_has(function: &Function, expected: &VerifyError) {
    let errors = match verify(function) {
        Ok(()) => {
            assert!(false, "invalid fixture was accepted");
            return;
        }
        Err(errors) => errors,
    };
    assert!(
        errors.contains(expected),
        "missing {expected:?} in {errors:?}"
    );
}

#[allow(
    clippy::too_many_lines,
    reason = "the table keeps the required verifier fixtures together"
)]
fn invalid_cases() -> Vec<(&'static str, Function, VerifyError)> {
    vec![
        (
            "missing terminator",
            finish(
                "x",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![block(0, Vec::new(), Terminator::Unreachable)],
                Vec::new(),
            ),
            VerifyError::MissingTerminator(BlockId(0)),
        ),
        (
            "missing block",
            finish(
                "x",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![block(
                    0,
                    Vec::new(),
                    Terminator::Jump {
                        target: BlockId(9),
                        args: Vec::new(),
                    },
                )],
                Vec::new(),
            ),
            VerifyError::MissingBlock(BlockId(9)),
        ),
        (
            "successor arity",
            finish(
                "x",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![
                    block(
                        0,
                        Vec::new(),
                        Terminator::Jump {
                            target: BlockId(1),
                            args: Vec::new(),
                        },
                    ),
                    BasicBlock {
                        id: BlockId(1),
                        params: vec![BlockParam {
                            value: ValueId(0),
                            ty: Ty::Word,
                        }],
                        ops: Vec::new(),
                        terminator: Terminator::Return { values: Vec::new() },
                    },
                ],
                Vec::new(),
            ),
            VerifyError::SuccessorArity(BlockId(0)),
        ),
        (
            "successor type",
            finish(
                "x",
                Vec::new(),
                Vec::new(),
                vec![Constant::Nil],
                vec![
                    block(
                        0,
                        vec![op(
                            &[(0, Ty::Word)],
                            OpKind::Const {
                                result: ConstantIndex(0),
                            },
                        )],
                        Terminator::Jump {
                            target: BlockId(1),
                            args: vec![ValueId(0)],
                        },
                    ),
                    BasicBlock {
                        id: BlockId(1),
                        params: vec![BlockParam {
                            value: ValueId(1),
                            ty: Ty::I64,
                        }],
                        ops: Vec::new(),
                        terminator: Terminator::Return { values: Vec::new() },
                    },
                ],
                Vec::new(),
            ),
            VerifyError::SuccessorType(BlockId(0)),
        ),
        (
            "undefined value",
            finish(
                "x",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![block(
                    0,
                    Vec::new(),
                    Terminator::Return {
                        values: vec![ValueId(99)],
                    },
                )],
                Vec::new(),
            ),
            VerifyError::UndefinedValue(ValueId(99)),
        ),
        (
            "constant bounds",
            finish(
                "x",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![block(
                    0,
                    vec![op(
                        &[(0, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(9),
                        },
                    )],
                    Terminator::Return { values: Vec::new() },
                )],
                Vec::new(),
            ),
            VerifyError::ConstantOutOfBounds(BlockId(0)),
        ),
        (
            "return arity",
            finish(
                "x",
                Vec::new(),
                vec![Ty::Word],
                Vec::new(),
                vec![block(
                    0,
                    Vec::new(),
                    Terminator::Return { values: Vec::new() },
                )],
                Vec::new(),
            ),
            VerifyError::ReturnArity(BlockId(0)),
        ),
        (
            "handler target",
            finish(
                "x",
                Vec::new(),
                Vec::new(),
                Vec::new(),
                vec![block(
                    0,
                    Vec::new(),
                    Terminator::Return { values: Vec::new() },
                )],
                vec![HandlerRegion {
                    id: HandlerRegionId(0),
                    kind: HandlerKind::Catch,
                    protected: vec![BlockId(7)],
                    handler: BlockId(8),
                    cleanup: Some(BlockId(9)),
                    catch_tag: None,
                    binding_targets: Vec::new(),
                    depth: 0,
                    parent: None,
                }],
            ),
            VerifyError::HandlerTarget(BlockId(7)),
        ),
        (
            "prim condition target",
            finish(
                "x",
                vec![Param {
                    name: "x".into(),
                    ty: Ty::Word,
                }],
                Vec::new(),
                Vec::new(),
                vec![block(
                    0,
                    vec![op(
                        &[(1, Ty::Word)],
                        OpKind::Prim {
                            op: Prim::Car,
                            args: vec![ValueId(0)],
                            condition: Some(BlockId(4)),
                        },
                    )],
                    Terminator::Return {
                        values: vec![ValueId(1)],
                    },
                )],
                Vec::new(),
            ),
            VerifyError::MissingBlock(BlockId(4)),
        ),
    ]
}

#[test]
fn verifier_reports_each_reachable_error_fixture() {
    let cases = invalid_cases();
    assert_eq!(cases.len(), 9);
    for (name, function, expected) in cases {
        verify_has(&function, &expected);
        assert!(!name.is_empty());
    }
}

#[test]
fn verifier_reports_duplicate_block() {
    let function = finish(
        "duplicate-block",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            block(0, Vec::new(), Terminator::Return { values: Vec::new() }),
            block(0, Vec::new(), Terminator::Return { values: Vec::new() }),
        ],
        Vec::new(),
    );
    verify_has(&function, &VerifyError::DuplicateBlock(BlockId(0)));
}

#[test]
fn verifier_reports_duplicate_value() {
    let function = finish(
        "duplicate-value",
        Vec::new(),
        Vec::new(),
        vec![Constant::Nil],
        vec![block(
            0,
            vec![
                op(
                    &[(0, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
                op(
                    &[(0, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
            ],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );
    verify_has(&function, &VerifyError::DuplicateValue(ValueId(0)));
}

#[test]
fn verifier_reports_type_mismatch() {
    let function = finish(
        "wrong-branch-condition",
        Vec::new(),
        Vec::new(),
        vec![Constant::Fixnum(1)],
        vec![
            block(
                0,
                vec![op(
                    &[(0, Ty::I64)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                )],
                Terminator::Branch {
                    condition: ValueId(0),
                    then_target: BlockId(1),
                    then_args: Vec::new(),
                    else_target: BlockId(2),
                    else_args: Vec::new(),
                },
            ),
            block(1, Vec::new(), Terminator::Return { values: Vec::new() }),
            block(2, Vec::new(), Terminator::Return { values: Vec::new() }),
        ],
        Vec::new(),
    );
    verify_has(&function, &VerifyError::TypeMismatch(BlockId(0)));
}

#[test]
fn verifier_accepts_unique_ids_and_matching_types() {
    let function = finish(
        "valid-structure",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        vec![Ty::Bool],
        Vec::new(),
        vec![
            block(
                0,
                Vec::new(),
                Terminator::Jump {
                    target: BlockId(1),
                    args: Vec::new(),
                },
            ),
            block(
                1,
                vec![op(&[], OpKind::Safepoint)],
                Terminator::Return {
                    values: vec![ValueId(0)],
                },
            ),
        ],
        Vec::new(),
    );
    assert!(verify(&function).is_ok());
}

#[test]
fn verifier_rejects_value_not_dominated_by_use_block() {
    let function = finish(
        "non-dominating-value",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        vec![Ty::Bool],
        Vec::new(),
        vec![
            block(
                0,
                Vec::new(),
                Terminator::Branch {
                    condition: ValueId(0),
                    then_target: BlockId(1),
                    then_args: Vec::new(),
                    else_target: BlockId(2),
                    else_args: Vec::new(),
                },
            ),
            block(
                1,
                vec![op(&[(1, Ty::Bool)], OpKind::Move { value: ValueId(0) })],
                Terminator::Jump {
                    target: BlockId(3),
                    args: Vec::new(),
                },
            ),
            block(
                2,
                Vec::new(),
                Terminator::Jump {
                    target: BlockId(3),
                    args: Vec::new(),
                },
            ),
            block(
                3,
                Vec::new(),
                Terminator::Return {
                    values: vec![ValueId(1)],
                },
            ),
        ],
        Vec::new(),
    );
    verify_has(&function, &VerifyError::UndefinedValue(ValueId(1)));
}

#[test]
fn verifier_reports_missing_call_safepoint_and_accepts_present_one() {
    let without = finish(
        "missing-safepoint",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
        Vec::new(),
        vec![block(
            0,
            vec![op(
                &[(1, Ty::Word)],
                OpKind::Call {
                    function: ValueId(0),
                    args: Vec::new(),
                },
            )],
            Terminator::Return {
                values: vec![ValueId(1)],
            },
        )],
        Vec::new(),
    );
    verify_has(&without, &VerifyError::SafepointWarning(BlockId(0)));

    let with = finish(
        "present-safepoint",
        vec![Param {
            name: "callee".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
        Vec::new(),
        vec![block(
            0,
            vec![
                op(&[], OpKind::Safepoint),
                op(
                    &[(1, Ty::Word)],
                    OpKind::Call {
                        function: ValueId(0),
                        args: Vec::new(),
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(1)],
            },
        )],
        Vec::new(),
    );
    assert!(verify(&with).is_ok());
}

#[test]
fn function_builder_usage_example_builds_and_verifies() {
    let mut builder = FunctionBuilder::new(
        FunctionId(7),
        "builder-example",
        vec![Param {
            name: "n".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    let one = builder.add_constant(Constant::Fixnum(1));
    let values = builder.push_op(OpKind::Const { result: one }, &[Ty::I64]);
    assert!(values.is_ok());
    let value = values.map_or(ValueId(0), |mut ids| ids.remove(0));
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![value]
            })
            .is_ok()
    );
    assert!(verify(&builder.finish()).is_ok());
}

#[test]
#[allow(clippy::too_many_lines)]
fn verifier_accepts_each_operation_type() {
    let function = finish(
        "all-operations",
        vec![
            Param {
                name: "word".into(),
                ty: Ty::Word,
            },
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "i64".into(),
                ty: Ty::I64,
            },
            Param {
                name: "f64".into(),
                ty: Ty::F64,
            },
            Param {
                name: "bool".into(),
                ty: Ty::Bool,
            },
        ],
        Vec::new(),
        vec![Constant::Nil],
        vec![block(
            0,
            vec![
                op(
                    &[(10, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
                op(&[(11, Ty::Word)], OpKind::Move { value: ValueId(0) }),
                op(
                    &[(12, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(1),
                    },
                ),
                op(
                    &[],
                    OpKind::Store {
                        address: ValueId(1),
                        value: ValueId(0),
                    },
                ),
                op(
                    &[(13, Ty::Word)],
                    OpKind::LoadField {
                        object: ValueId(0),
                        field: 2,
                    },
                ),
                op(
                    &[],
                    OpKind::StoreField {
                        object: ValueId(0),
                        field: 3,
                        value: ValueId(10),
                    },
                ),
                op(&[(14, Ty::Address)], OpKind::Alloc { words: 4 }),
                op(&[], OpKind::Safepoint),
                op(&[(15, Ty::Address)], OpKind::LoadArg { index: 1 }),
                op(&[(16, Ty::Word)], OpKind::LoadCapture { index: 0 }),
                op(&[(17, Ty::Word)], OpKind::LoadFunctionObject),
                op(&[], OpKind::Safepoint),
                op(
                    &[(18, Ty::Word)],
                    OpKind::Call {
                        function: ValueId(0),
                        args: vec![ValueId(10)],
                    },
                ),
                op(&[], OpKind::Safepoint),
                op(&[], OpKind::Safepoint),
                op(
                    &[(19, Ty::Word)],
                    OpKind::CallIndirect {
                        callee: ValueId(0),
                        args: vec![ValueId(10)],
                    },
                ),
                op(
                    &[(20, Ty::Word)],
                    OpKind::MakeClosure {
                        entry: ValueId(0),
                        captures: vec![ValueId(10)],
                    },
                ),
                op(
                    &[(21, Ty::Word)],
                    OpKind::MakeValueCell { value: ValueId(10) },
                ),
                op(&[], OpKind::Safepoint),
                op(
                    &[(22, Ty::Word)],
                    OpKind::CallClosure {
                        closure: ValueId(20),
                        args: vec![ValueId(10)],
                        named_symbol: Some(ValueId(0)),
                    },
                ),
                op(&[], OpKind::Safepoint),
                op(
                    &[(23, Ty::Word)],
                    OpKind::Builtin {
                        name: "identity".into(),
                        args: vec![ValueId(10)],
                    },
                ),
                op(
                    &[(24, Ty::Word)],
                    OpKind::Prim {
                        op: Prim::Car,
                        args: vec![ValueId(10)],
                        condition: None,
                    },
                ),
                op(
                    &[(25, Ty::Word)],
                    OpKind::Prim {
                        op: Prim::Cdr,
                        args: vec![ValueId(10)],
                        condition: None,
                    },
                ),
                op(
                    &[(48, Ty::Unit)],
                    OpKind::Prim {
                        op: Prim::Rplaca,
                        args: vec![ValueId(10), ValueId(0)],
                        condition: None,
                    },
                ),
                op(
                    &[(49, Ty::Unit)],
                    OpKind::Prim {
                        op: Prim::Rplacd,
                        args: vec![ValueId(10), ValueId(0)],
                        condition: None,
                    },
                ),
                op(
                    &[(26, Ty::Word)],
                    OpKind::Prim {
                        op: Prim::Svref,
                        args: vec![ValueId(10), ValueId(0)],
                        condition: None,
                    },
                ),
                op(
                    &[(27, Ty::Word)],
                    OpKind::Prim {
                        op: Prim::Aref,
                        args: vec![ValueId(10), ValueId(0)],
                        condition: None,
                    },
                ),
                op(
                    &[(50, Ty::Unit)],
                    OpKind::Prim {
                        op: Prim::Aset,
                        args: vec![ValueId(10), ValueId(0), ValueId(10)],
                        condition: None,
                    },
                ),
                op(
                    &[(28, Ty::I64)],
                    OpKind::Prim {
                        op: Prim::FixnumAdd,
                        args: vec![ValueId(2), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(29, Ty::I64)],
                    OpKind::Prim {
                        op: Prim::FixnumSub,
                        args: vec![ValueId(2), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(30, Ty::I64)],
                    OpKind::Prim {
                        op: Prim::FixnumMul,
                        args: vec![ValueId(2), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(31, Ty::I64)],
                    OpKind::Prim {
                        op: Prim::FixnumDiv,
                        args: vec![ValueId(2), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(32, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::FixnumLt,
                        args: vec![ValueId(2), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(33, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::FixnumLe,
                        args: vec![ValueId(2), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(34, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::FixnumEq,
                        args: vec![ValueId(2), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(35, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::Eq,
                        args: vec![ValueId(10), ValueId(0)],
                        condition: None,
                    },
                ),
                op(
                    &[(36, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::Eql,
                        args: vec![ValueId(10), ValueId(0)],
                        condition: None,
                    },
                ),
                op(
                    &[(37, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::Typep,
                        args: vec![ValueId(10), ValueId(0)],
                        condition: None,
                    },
                ),
                op(
                    &[(38, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::CharacterPredicate("alpha".into()),
                        args: vec![ValueId(10)],
                        condition: None,
                    },
                ),
                op(
                    &[(39, Ty::Word)],
                    OpKind::Prim {
                        op: Prim::StructureSlot("car".into()),
                        args: vec![ValueId(10)],
                        condition: None,
                    },
                ),
                op(
                    &[(40, Ty::Bool)],
                    OpKind::Compare {
                        op: Compare::Eq,
                        left: ValueId(10),
                        right: ValueId(0),
                    },
                ),
                op(
                    &[(41, Ty::I64)],
                    OpKind::Convert {
                        op: Convert::WordToI64,
                        value: ValueId(0),
                    },
                ),
                op(
                    &[(42, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::I64ToWord,
                        value: ValueId(2),
                    },
                ),
                op(
                    &[(43, Ty::F64)],
                    OpKind::Convert {
                        op: Convert::WordToF64,
                        value: ValueId(0),
                    },
                ),
                op(
                    &[(44, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::F64ToWord,
                        value: ValueId(3),
                    },
                ),
                op(
                    &[(45, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(1),
                    },
                ),
                op(
                    &[(46, Ty::Address)],
                    OpKind::Convert {
                        op: Convert::WordToAddress,
                        value: ValueId(0),
                    },
                ),
                op(
                    &[],
                    OpKind::SetMultipleValues {
                        values: vec![ValueId(10), ValueId(0)],
                    },
                ),
            ],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );

    assert_eq!(verify(&function), Ok(()));
}

#[test]
fn verifier_accepts_switch_call_and_throw_terminators() {
    let switch = finish(
        "switch",
        vec![Param {
            name: "key".into(),
            ty: Ty::I64,
        }],
        Vec::new(),
        Vec::new(),
        vec![
            block(
                0,
                Vec::new(),
                Terminator::Switch {
                    value: ValueId(0),
                    cases: vec![(1, BlockId(1), Vec::new())],
                    default: BlockId(2),
                    default_args: Vec::new(),
                },
            ),
            block(1, Vec::new(), Terminator::Return { values: Vec::new() }),
            block(2, Vec::new(), Terminator::Return { values: Vec::new() }),
        ],
        Vec::new(),
    );
    assert_eq!(verify(&switch), Ok(()));

    for terminator in [
        Terminator::CallReturn {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        Terminator::TailCall {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        Terminator::Throw {
            condition: ValueId(0),
        },
    ] {
        let function = finish(
            "terminator",
            vec![Param {
                name: "callee-or-condition".into(),
                ty: Ty::Word,
            }],
            Vec::new(),
            Vec::new(),
            vec![block(0, Vec::new(), terminator)],
            Vec::new(),
        );
        assert_eq!(verify(&function), Ok(()));
    }
}

#[test]
#[allow(clippy::too_many_lines)]
fn verifier_accepts_operation_type_matrix() {
    let params = vec![
        Param {
            name: "address".into(),
            ty: Ty::Address,
        },
        Param {
            name: "word".into(),
            ty: Ty::Word,
        },
        Param {
            name: "integer".into(),
            ty: Ty::I64,
        },
        Param {
            name: "flag".into(),
            ty: Ty::Bool,
        },
    ];
    let mut ops = vec![
        op(
            &[(4, Ty::Word)],
            OpKind::Load {
                address: ValueId(0),
            },
        ),
        op(
            &[],
            OpKind::Store {
                address: ValueId(0),
                value: ValueId(1),
            },
        ),
        op(
            &[(5, Ty::Word)],
            OpKind::LoadField {
                object: ValueId(1),
                field: 2,
            },
        ),
        op(
            &[],
            OpKind::StoreField {
                object: ValueId(1),
                field: 2,
                value: ValueId(1),
            },
        ),
        op(&[(6, Ty::Address)], OpKind::Alloc { words: 3 }),
        op(&[], OpKind::Safepoint),
        op(&[(7, Ty::I64)], OpKind::LoadArg { index: 2 }),
        op(&[(8, Ty::Word)], OpKind::LoadCapture { index: 1 }),
        op(&[(9, Ty::Word)], OpKind::LoadFunctionObject),
        op(
            &[(10, Ty::Word)],
            OpKind::Prim {
                op: Prim::Car,
                args: vec![ValueId(1)],
                condition: None,
            },
        ),
        op(
            &[(11, Ty::Unit)],
            OpKind::Prim {
                op: Prim::Rplaca,
                args: vec![ValueId(1), ValueId(1)],
                condition: None,
            },
        ),
        op(
            &[(12, Ty::Word)],
            OpKind::Prim {
                op: Prim::Svref,
                args: vec![ValueId(1), ValueId(1)],
                condition: None,
            },
        ),
        op(
            &[(13, Ty::Unit)],
            OpKind::Prim {
                op: Prim::Aset,
                args: vec![ValueId(1), ValueId(1), ValueId(1)],
                condition: None,
            },
        ),
        op(
            &[(14, Ty::I64)],
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(2), ValueId(2)],
                condition: None,
            },
        ),
        op(
            &[(15, Ty::Bool)],
            OpKind::Prim {
                op: Prim::FixnumLt,
                args: vec![ValueId(2), ValueId(2)],
                condition: None,
            },
        ),
        op(
            &[(16, Ty::Bool)],
            OpKind::Prim {
                op: Prim::Eq,
                args: vec![ValueId(1), ValueId(4)],
                condition: None,
            },
        ),
        op(
            &[(17, Ty::Bool)],
            OpKind::Prim {
                op: Prim::CharacterPredicate("digitp".into()),
                args: vec![ValueId(1)],
                condition: None,
            },
        ),
        op(
            &[(18, Ty::Word)],
            OpKind::Prim {
                op: Prim::StructureSlot("car".into()),
                args: vec![ValueId(1)],
                condition: None,
            },
        ),
        op(
            &[(19, Ty::Bool)],
            OpKind::Compare {
                op: Compare::Eq,
                left: ValueId(2),
                right: ValueId(2),
            },
        ),
        op(
            &[(20, Ty::I64)],
            OpKind::Convert {
                op: Convert::WordToI64,
                value: ValueId(1),
            },
        ),
        op(
            &[],
            OpKind::SetMultipleValues {
                values: vec![ValueId(1), ValueId(2)],
            },
        ),
    ];
    for (value, call) in [
        (
            21,
            OpKind::Call {
                function: ValueId(1),
                args: vec![ValueId(1)],
            },
        ),
        (
            22,
            OpKind::MakeClosure {
                entry: ValueId(1),
                captures: vec![ValueId(1)],
            },
        ),
        (23, OpKind::MakeValueCell { value: ValueId(1) }),
        (
            24,
            OpKind::CallClosure {
                closure: ValueId(1),
                args: vec![ValueId(1)],
                named_symbol: Some(ValueId(1)),
            },
        ),
        (
            25,
            OpKind::Builtin {
                name: "length".into(),
                args: vec![ValueId(1)],
            },
        ),
    ] {
        ops.push(op(&[], OpKind::Safepoint));
        ops.push(op(&[(value, Ty::Word)], call));
    }
    let function = finish(
        "operation-matrix",
        params,
        Vec::new(),
        Vec::new(),
        vec![block(0, ops, Terminator::Return { values: Vec::new() })],
        Vec::new(),
    );
    assert!(
        verify(&function).is_ok(),
        "operation matrix rejected: {:?}",
        verify(&function)
    );
}

#[test]
fn verifier_checks_nested_constant_references() {
    let function = finish(
        "constant-references",
        Vec::new(),
        Vec::new(),
        vec![
            Constant::Structure {
                kind: StructureKind::Cons,
                elements: vec![ConstantIndex(9)],
            },
            Constant::Ratio {
                numerator: ConstantIndex(0),
                denominator: ConstantIndex(9),
            },
            Constant::Complex {
                real: ConstantIndex(9),
                imaginary: ConstantIndex(1),
            },
        ],
        vec![block(
            0,
            vec![
                op(
                    &[(0, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
                op(
                    &[(1, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(1),
                    },
                ),
                op(
                    &[(2, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(2),
                    },
                ),
            ],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );
    let result = verify(&function);
    assert!(result.is_err());
    let Some(errors) = result.err() else {
        return;
    };
    assert_eq!(
        errors
            .iter()
            .filter(|error| **error == VerifyError::ConstantOutOfBounds(BlockId(0)))
            .count(),
        3
    );
}
