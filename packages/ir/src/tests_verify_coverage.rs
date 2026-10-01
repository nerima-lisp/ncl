#![allow(clippy::expect_used, clippy::too_many_lines)]

use super::tests_text::{block, finish, op};
use crate::*;

fn errors(function: &Function) -> Vec<VerifyError> {
    verify(function).expect_err("coverage fixture must be rejected")
}

fn region(kind: HandlerKind) -> HandlerRegion {
    HandlerRegion {
        id: HandlerRegionId(0),
        kind,
        protected: vec![BlockId(0)],
        handler: BlockId(0),
        cleanup: None,
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    }
}

#[test]
fn verifier_covers_empty_and_unreachable_functions() {
    assert!(verify(&finish(
        "empty",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new()
    ))
    .is_ok());
    let function = finish(
        "unreachable",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            block(0, Vec::new(), Terminator::Return { values: Vec::new() }),
            block(1, Vec::new(), Terminator::Return { values: Vec::new() }),
        ],
        Vec::new(),
    );
    assert!(verify(&function).is_ok());
    assert_eq!(
        VerifyError::TypeMismatch(BlockId(4)).to_string(),
        "TypeMismatch(BlockId(4))"
    );
}

#[test]
fn verifier_reports_handler_definition_errors() {
    let mut unwind = region(HandlerKind::UnwindProtect);
    assert!(errors(&finish(
        "unwind",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            Vec::new(),
            Terminator::Return { values: Vec::new() }
        )],
        vec![unwind.clone()]
    ))
    .contains(&VerifyError::HandlerMismatch(BlockId(0))));

    unwind.cleanup = Some(BlockId(0));
    unwind.binding_targets = vec![ValueId(8)];
    assert!(errors(&finish(
        "unwind-bindings",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            Vec::new(),
            Terminator::Return { values: Vec::new() }
        )],
        vec![unwind]
    ))
    .contains(&VerifyError::HandlerMismatch(BlockId(0))));

    let mut catch = region(HandlerKind::Catch);
    catch.catch_tag = Some(ValueId(0));
    catch.binding_targets = vec![ValueId(1)];
    let function = finish(
        "catch-bindings",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            Vec::new(),
            Terminator::Return { values: Vec::new() },
        )],
        vec![catch.clone(), catch.clone()],
    );
    let reported = errors(&function);
    assert!(reported.contains(&VerifyError::DuplicateHandlerRegion(HandlerRegionId(0))));
    assert!(
        reported.contains(&VerifyError::MissingHandlerRegion(HandlerRegionId(0)))
            || reported.contains(&VerifyError::TypeMismatch(BlockId(0)))
    );

    catch.id = HandlerRegionId(1);
    catch.parent = Some(HandlerRegionId(9));
    assert!(errors(&finish(
        "missing-parent",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            Vec::new(),
            Terminator::Return { values: Vec::new() }
        )],
        vec![catch]
    ))
    .contains(&VerifyError::MissingHandlerRegion(HandlerRegionId(9))));
}

#[test]
fn verifier_reports_handler_flow_and_nesting_errors() {
    let missing = finish(
        "missing-flow-region",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            vec![op(
                &[],
                OpKind::EnterHandler {
                    region: HandlerRegionId(7),
                },
            )],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );
    assert!(errors(&missing).contains(&VerifyError::MissingHandlerRegion(HandlerRegionId(7))));

    let mismatch = finish(
        "leave-mismatch",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            vec![op(
                &[],
                OpKind::LeaveHandler {
                    region: HandlerRegionId(0),
                },
            )],
            Terminator::Return { values: Vec::new() },
        )],
        vec![region(HandlerKind::UnwindProtect)],
    );
    assert!(errors(&mismatch).contains(&VerifyError::HandlerMismatch(BlockId(0))));

    let mut child = region(HandlerKind::UnwindProtect);
    child.id = HandlerRegionId(1);
    child.parent = Some(HandlerRegionId(0));
    child.depth = 1;
    child.protected = vec![BlockId(1)];
    let parent = region(HandlerKind::UnwindProtect);
    let nested = finish(
        "bad-nesting",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            block(0, Vec::new(), Terminator::Return { values: Vec::new() }),
            block(1, Vec::new(), Terminator::Return { values: Vec::new() }),
        ],
        vec![parent, child],
    );
    assert!(errors(&nested).contains(&VerifyError::HandlerNesting(HandlerRegionId(1))));

    let join = finish(
        "handler-join",
        Vec::new(),
        Vec::new(),
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
                vec![op(
                    &[],
                    OpKind::EnterHandler {
                        region: HandlerRegionId(0),
                    },
                )],
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
            block(3, Vec::new(), Terminator::Return { values: Vec::new() }),
        ],
        vec![HandlerRegion {
            catch_tag: Some(ValueId(0)),
            handler: BlockId(3),
            protected: vec![BlockId(0), BlockId(1), BlockId(2), BlockId(3)],
            ..region(HandlerKind::Catch)
        }],
    );
    assert!(errors(&join).contains(&VerifyError::HandlerUnbalanced(BlockId(3))));
}

#[test]
fn verifier_reports_result_and_safepoint_errors() {
    let wrong_result = finish(
        "wrong-result",
        Vec::new(),
        Vec::new(),
        vec![Constant::Nil],
        vec![block(
            0,
            vec![op(
                &[(0, Ty::I64)],
                OpKind::Const {
                    result: ConstantIndex(0),
                },
            )],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );
    assert!(errors(&wrong_result).contains(&VerifyError::TypeMismatch(BlockId(0))));

    let alloc = finish(
        "missing-alloc-safepoint",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            vec![op(&[(0, Ty::Address)], OpKind::Alloc { words: 1 })],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );
    assert!(errors(&alloc).contains(&VerifyError::SafepointWarning(BlockId(0))));

    let looped = finish(
        "missing-loop-safepoint",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            Vec::new(),
            Terminator::Jump {
                target: BlockId(0),
                args: Vec::new(),
            },
        )],
        Vec::new(),
    );
    assert!(errors(&looped).contains(&VerifyError::SafepointWarning(BlockId(0))));
}
