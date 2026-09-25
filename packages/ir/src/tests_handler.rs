use super::tests_text::{block, finish, op};
use crate::*;

fn verify_has(function: &Function, expected: &VerifyError) {
    let errors = match verify(function) {
        Ok(()) => unreachable!("invalid fixture was accepted"),
        Err(errors) => errors,
    };
    assert!(
        errors.contains(expected),
        "missing {expected:?} in {errors:?}"
    );
}

fn catch_region(protected: Vec<BlockId>) -> HandlerRegion {
    HandlerRegion {
        id: HandlerRegionId(0),
        kind: HandlerKind::Catch,
        protected,
        handler: BlockId(3),
        cleanup: None,
        catch_tag: Some(ValueId(1)),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    }
}

#[test]
fn verifier_accepts_handler_region_across_branch_join() {
    let function = finish(
        "handler-branch",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        Vec::new(),
        vec![Constant::Nil],
        vec![
            block(
                0,
                vec![
                    op(
                        &[(1, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    ),
                    op(
                        &[],
                        OpKind::EnterHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                ],
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
                Vec::new(),
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
                vec![op(
                    &[],
                    OpKind::LeaveHandler {
                        region: HandlerRegionId(0),
                    },
                )],
                Terminator::Return { values: Vec::new() },
            ),
        ],
        vec![catch_region(vec![
            BlockId(0),
            BlockId(1),
            BlockId(2),
            BlockId(3),
        ])],
    );
    assert!(verify(&function).is_ok());
}

#[test]
fn verifier_accepts_handler_region_across_loop() {
    let function = finish(
        "handler-loop",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        Vec::new(),
        vec![Constant::Nil],
        vec![
            block(
                0,
                vec![
                    op(
                        &[(1, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    ),
                    op(
                        &[],
                        OpKind::EnterHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                ],
                Terminator::Jump {
                    target: BlockId(1),
                    args: Vec::new(),
                },
            ),
            block(
                1,
                vec![op(&[], OpKind::Safepoint)],
                Terminator::Branch {
                    condition: ValueId(0),
                    then_target: BlockId(1),
                    then_args: Vec::new(),
                    else_target: BlockId(2),
                    else_args: Vec::new(),
                },
            ),
            block(
                2,
                vec![op(
                    &[],
                    OpKind::LeaveHandler {
                        region: HandlerRegionId(0),
                    },
                )],
                Terminator::Return { values: Vec::new() },
            ),
        ],
        vec![HandlerRegion {
            handler: BlockId(2),
            protected: vec![BlockId(0), BlockId(1), BlockId(2)],
            ..catch_region(Vec::new())
        }],
    );
    assert!(verify(&function).is_ok());
}

#[test]
fn verifier_rejects_unclosed_handler_at_normal_exit() {
    let function = finish(
        "unclosed-handler",
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
                    &[],
                    OpKind::EnterHandler {
                        region: HandlerRegionId(0),
                    },
                ),
            ],
            Terminator::Return { values: Vec::new() },
        )],
        vec![HandlerRegion {
            handler: BlockId(0),
            protected: vec![BlockId(0)],
            ..catch_region(Vec::new())
        }],
    );
    verify_has(&function, &VerifyError::HandlerUnbalanced(BlockId(0)));
}

#[test]
fn verifier_rejects_kind_without_required_payload() {
    let function = finish(
        "invalid-handler-kind",
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
            kind: HandlerKind::Progv,
            protected: vec![BlockId(0)],
            handler: BlockId(0),
            cleanup: None,
            catch_tag: None,
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        }],
    );
    verify_has(&function, &VerifyError::HandlerMismatch(BlockId(0)));
}
