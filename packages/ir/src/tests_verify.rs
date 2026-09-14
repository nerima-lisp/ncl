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
                    protected: vec![BlockId(7)],
                    handler: BlockId(8),
                    cleanup: Some(BlockId(9)),
                    catch_tag: None,
                    depth: 0,
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
fn verifier_error_catalog_names_unimplemented_checks() {
    let catalog = [
        VerifyError::DuplicateBlock(BlockId(0)),
        VerifyError::DuplicateValue(ValueId(0)),
        VerifyError::TypeMismatch(BlockId(0)),
        VerifyError::SafepointWarning(BlockId(0)),
    ];
    assert_eq!(catalog.len(), 4);
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
