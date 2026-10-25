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

#[test]
fn verifier_accepts_address_dead_before_safepoint() {
    let function = finish(
        "address-dead-before-safepoint",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
        vec![Constant::Nil],
        vec![block(
            0,
            vec![
                op(
                    &[(1, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(0),
                    },
                ),
                op(&[], OpKind::Safepoint),
                op(
                    &[(2, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(2)],
            },
        )],
        Vec::new(),
    );

    assert!(verify(&function).is_ok());
}

#[test]
fn verifier_rejects_address_provenance_through_conversions_at_safepoint() {
    let function = finish(
        "address-conversion-chain",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
        vec![Constant::Nil],
        vec![block(
            0,
            vec![
                op(
                    &[(2, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                ),
                op(&[(3, Ty::Word)], OpKind::Move { value: ValueId(2) }),
                op(
                    &[(4, Ty::I64)],
                    OpKind::Convert {
                        op: Convert::WordToI64,
                        value: ValueId(3),
                    },
                ),
                op(
                    &[(5, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::I64ToWord,
                        value: ValueId(4),
                    },
                ),
                op(
                    &[(6, Ty::F64)],
                    OpKind::Convert {
                        op: Convert::WordToF64,
                        value: ValueId(5),
                    },
                ),
                op(
                    &[(7, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::F64ToWord,
                        value: ValueId(6),
                    },
                ),
                op(&[], OpKind::Safepoint),
                op(
                    &[(8, Ty::Address)],
                    OpKind::Convert {
                        op: Convert::WordToAddress,
                        value: ValueId(7),
                    },
                ),
                op(&[(9, Ty::Address)], OpKind::Move { value: ValueId(8) }),
                op(
                    &[(10, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(9),
                    },
                ),
                op(
                    &[(11, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(11)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(0),
            value: ValueId(7),
            boundary: AddressLivenessBoundary::Safepoint,
        },
    );
}

#[test]
fn verifier_rejects_address_provenance_stored_across_safepoint() {
    let function = finish(
        "address-through-raw-storage",
        vec![
            Param {
                name: "owner".into(),
                ty: Ty::Address,
            },
            Param {
                name: "slot".into(),
                ty: Ty::Address,
            },
        ],
        vec![Ty::Word],
        Vec::new(),
        vec![block(
            0,
            vec![
                op(
                    &[(2, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                ),
                op(
                    &[],
                    OpKind::Store {
                        address: ValueId(1),
                        value: ValueId(2),
                    },
                ),
                op(&[], OpKind::Safepoint),
                op(
                    &[(3, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(1),
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(3)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(0),
            value: ValueId(2),
            boundary: AddressLivenessBoundary::Safepoint,
        },
    );
}

#[test]
fn verifier_rejects_call_results_spilled_across_safepoint() {
    let make_function = |name: &str, call: OpKind| {
        finish(
            name,
            vec![
                Param {
                    name: "callee".into(),
                    ty: Ty::Word,
                },
                Param {
                    name: "slot".into(),
                    ty: Ty::Address,
                },
            ],
            vec![Ty::Word],
            Vec::new(),
            vec![block(
                0,
                vec![
                    op(&[], OpKind::Safepoint),
                    op(&[(2, Ty::Word)], call),
                    op(&[(3, Ty::Address)], OpKind::LoadArg { index: 1 }),
                    op(
                        &[],
                        OpKind::Store {
                            address: ValueId(3),
                            value: ValueId(2),
                        },
                    ),
                    op(&[], OpKind::Safepoint),
                    op(&[(4, Ty::Address)], OpKind::LoadArg { index: 1 }),
                    op(
                        &[(5, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(4),
                        },
                    ),
                    op(
                        &[(6, Ty::Address)],
                        OpKind::Convert {
                            op: Convert::WordToAddress,
                            value: ValueId(5),
                        },
                    ),
                    op(
                        &[(7, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(6),
                        },
                    ),
                ],
                Terminator::Return {
                    values: vec![ValueId(7)],
                },
            )],
            Vec::new(),
        )
    };
    let direct_call = make_function(
        "direct-call-result-spilled-across-safepoint",
        OpKind::Call {
            function: ValueId(0),
            args: Vec::new(),
        },
    );
    let indirect_call = make_function(
        "indirect-call-result-spilled-across-safepoint",
        OpKind::CallIndirect {
            callee: ValueId(0),
            args: Vec::new(),
        },
    );
    let expected = VerifyError::UnprovenAddressProvenance {
        block: BlockId(0),
        value: ValueId(5),
    };
    let direct_errors = verify(&direct_call).err().unwrap_or_default();
    let indirect_errors = verify(&indirect_call).err().unwrap_or_default();
    assert!(
        direct_errors.contains(&expected) && indirect_errors.contains(&expected),
        "expected rejection for both call result spills, got direct={direct_errors:?}, indirect={indirect_errors:?}"
    );
}

#[test]
fn verifier_rejects_unproven_call_result_word_to_address() {
    for call in [
        OpKind::Call {
            function: ValueId(0),
            args: Vec::new(),
        },
        OpKind::CallIndirect {
            callee: ValueId(0),
            args: Vec::new(),
        },
    ] {
        let function = finish(
            "unproven-call-result-word-to-address",
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
                    op(&[(1, Ty::Word)], call),
                    op(&[(2, Ty::Word)], OpKind::Move { value: ValueId(1) }),
                    op(
                        &[(3, Ty::Address)],
                        OpKind::Convert {
                            op: Convert::WordToAddress,
                            value: ValueId(2),
                        },
                    ),
                    op(
                        &[(4, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(3),
                        },
                    ),
                ],
                Terminator::Return {
                    values: vec![ValueId(4)],
                },
            )],
            Vec::new(),
        );

        let errors = verify(&function).err().unwrap_or_default();
        assert!(
            errors.contains(&VerifyError::UnprovenAddressProvenance {
                block: BlockId(0),
                value: ValueId(2),
            }),
            "call result must not prove WordToAddress provenance, got {errors:?}"
        );
    }
}

#[test]
fn verifier_rejects_word_to_address_after_mixed_block_parameter() {
    let function = finish(
        "mixed-address-provenance-block-parameter",
        vec![
            Param {
                name: "condition".into(),
                ty: Ty::Bool,
            },
            Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
        ],
        vec![Ty::Word],
        Vec::new(),
        vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops: vec![op(
                    &[(3, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(2),
                    },
                )],
                terminator: Terminator::Branch {
                    condition: ValueId(0),
                    then_target: BlockId(1),
                    then_args: Vec::new(),
                    else_target: BlockId(2),
                    else_args: Vec::new(),
                },
            },
            block(
                1,
                Vec::new(),
                Terminator::Jump {
                    target: BlockId(3),
                    args: vec![ValueId(3)],
                },
            ),
            block(
                2,
                vec![
                    op(&[], OpKind::Safepoint),
                    op(
                        &[(4, Ty::Word)],
                        OpKind::Call {
                            function: ValueId(1),
                            args: Vec::new(),
                        },
                    ),
                ],
                Terminator::Jump {
                    target: BlockId(3),
                    args: vec![ValueId(4)],
                },
            ),
            BasicBlock {
                id: BlockId(3),
                params: vec![BlockParam {
                    value: ValueId(5),
                    ty: Ty::Word,
                }],
                ops: vec![
                    op(
                        &[(6, Ty::Address)],
                        OpKind::Convert {
                            op: Convert::WordToAddress,
                            value: ValueId(5),
                        },
                    ),
                    op(
                        &[(7, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(6),
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    values: vec![ValueId(7)],
                },
            },
        ],
        Vec::new(),
    );

    let errors = verify(&function).err().unwrap_or_default();
    assert!(
        errors.contains(&VerifyError::UnprovenAddressProvenance {
            block: BlockId(3),
            value: ValueId(5),
        }),
        "mixed proven and unknown inputs must not prove provenance, got {errors:?}"
    );
}

#[test]
fn verifier_rejects_word_to_address_from_unrooted_cfg_cycle() {
    let function = finish(
        "unrooted-word-cycle",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![
            block(0, Vec::new(), Terminator::Return { values: Vec::new() }),
            BasicBlock {
                id: BlockId(1),
                params: vec![BlockParam {
                    value: ValueId(0),
                    ty: Ty::Word,
                }],
                ops: vec![
                    op(
                        &[(1, Ty::Address)],
                        OpKind::Convert {
                            op: Convert::WordToAddress,
                            value: ValueId(0),
                        },
                    ),
                    op(
                        &[(2, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(1),
                        },
                    ),
                    op(&[], OpKind::Safepoint),
                ],
                terminator: Terminator::Jump {
                    target: BlockId(1),
                    args: vec![ValueId(0)],
                },
            },
        ],
        Vec::new(),
    );

    assert_eq!(
        verify(&function),
        Err(vec![VerifyError::UnprovenAddressProvenance {
            block: BlockId(1),
            value: ValueId(0),
        }]),
    );
}

#[test]
fn verifier_rejects_address_saved_in_multiple_values_across_safepoint() {
    let function = finish(
        "address-saved-in-multiple-values-across-safepoint",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            vec![
                op(
                    &[(1, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                ),
                op(
                    &[],
                    OpKind::SetMultipleValues {
                        values: vec![ValueId(1)],
                    },
                ),
                op(&[], OpKind::Safepoint),
            ],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );

    let errors = verify(&function).err().unwrap_or_default();
    assert!(
        errors.contains(&VerifyError::AddressLiveAcross {
            block: BlockId(0),
            value: ValueId(1),
            boundary: AddressLivenessBoundary::Safepoint,
        }),
        "expected rejection for the address saved in multiple values, got {errors:?}"
    );
}

#[test]
fn verifier_rejects_address_provenance_through_arithmetic_at_call() {
    let function = finish(
        "address-arithmetic-chain",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "offset".into(),
                ty: Ty::I64,
            },
        ],
        vec![Ty::I64],
        Vec::new(),
        vec![block(
            0,
            vec![
                op(
                    &[(2, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                ),
                op(&[(3, Ty::Word)], OpKind::Move { value: ValueId(2) }),
                op(
                    &[(4, Ty::I64)],
                    OpKind::Convert {
                        op: Convert::WordToI64,
                        value: ValueId(3),
                    },
                ),
                op(
                    &[(5, Ty::I64)],
                    OpKind::Prim {
                        op: Prim::FixnumAdd,
                        args: vec![ValueId(4), ValueId(1)],
                        condition: None,
                    },
                ),
                op(
                    &[(6, Ty::I64)],
                    OpKind::Prim {
                        op: Prim::FixnumMul,
                        args: vec![ValueId(5), ValueId(1)],
                        condition: None,
                    },
                ),
                op(
                    &[(7, Ty::I64)],
                    OpKind::Prim {
                        op: Prim::FixnumDiv,
                        args: vec![ValueId(6), ValueId(1)],
                        condition: None,
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(7)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(0),
            value: ValueId(6),
            boundary: AddressLivenessBoundary::Call,
        },
    );
}

#[test]
fn verifier_tracks_address_provenance_through_block_parameters() {
    let function = finish(
        "address-block-parameter",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
        vec![Constant::Nil],
        vec![
            block(
                0,
                vec![op(
                    &[(1, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                )],
                Terminator::Jump {
                    target: BlockId(1),
                    args: vec![ValueId(1)],
                },
            ),
            BasicBlock {
                id: BlockId(1),
                params: vec![BlockParam {
                    value: ValueId(2),
                    ty: Ty::Word,
                }],
                ops: vec![
                    op(&[], OpKind::Safepoint),
                    op(
                        &[(3, Ty::Address)],
                        OpKind::Convert {
                            op: Convert::WordToAddress,
                            value: ValueId(2),
                        },
                    ),
                    op(
                        &[(4, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(3),
                        },
                    ),
                    op(
                        &[(5, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    ),
                ],
                terminator: Terminator::Return {
                    values: vec![ValueId(5)],
                },
            },
        ],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(1),
            value: ValueId(2),
            boundary: AddressLivenessBoundary::Safepoint,
        },
    );
}

#[test]
fn verifier_rejects_address_live_across_allocation() {
    let function = finish(
        "address-live-across-allocation",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
        vec![Constant::Nil],
        vec![block(
            0,
            vec![
                op(
                    &[(1, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                ),
                op(&[(2, Ty::Address)], OpKind::Alloc { words: 1 }),
                op(&[], OpKind::Safepoint),
                op(
                    &[(3, Ty::Address)],
                    OpKind::Convert {
                        op: Convert::WordToAddress,
                        value: ValueId(1),
                    },
                ),
                op(
                    &[(4, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(3),
                    },
                ),
                op(
                    &[(5, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(5)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(0),
            value: ValueId(1),
            boundary: AddressLivenessBoundary::Alloc,
        },
    );
}

#[test]
fn verifier_rejects_address_live_across_call_operation() {
    let function = finish(
        "address-live-across-call",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
        vec![Constant::Nil],
        vec![block(
            0,
            vec![
                op(
                    &[(1, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
                op(
                    &[(2, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                ),
                op(&[], OpKind::Safepoint),
                op(
                    &[(3, Ty::Word)],
                    OpKind::Call {
                        function: ValueId(1),
                        args: vec![ValueId(2)],
                    },
                ),
                op(
                    &[(4, Ty::Address)],
                    OpKind::Convert {
                        op: Convert::WordToAddress,
                        value: ValueId(2),
                    },
                ),
                op(
                    &[(5, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(4),
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(3)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(0),
            value: ValueId(2),
            boundary: AddressLivenessBoundary::Call,
        },
    );
}

#[test]
fn verifier_treats_unknown_call_primitive_as_call_boundary() {
    let function = finish(
        "address-live-across-typep",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
        vec![Constant::Nil],
        vec![block(
            0,
            vec![
                op(
                    &[(1, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
                op(
                    &[(2, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                ),
                op(
                    &[(3, Ty::Bool)],
                    OpKind::Prim {
                        op: Prim::Typep,
                        args: vec![ValueId(1), ValueId(2)],
                        condition: None,
                    },
                ),
                op(
                    &[(4, Ty::Address)],
                    OpKind::Convert {
                        op: Convert::WordToAddress,
                        value: ValueId(2),
                    },
                ),
                op(
                    &[(5, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(4),
                    },
                ),
                op(
                    &[(6, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(6)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(0),
            value: ValueId(2),
            boundary: AddressLivenessBoundary::Call,
        },
    );
}

#[test]
fn verifier_rejects_address_passed_to_call_terminators() {
    for terminator in [
        Terminator::CallReturn {
            function: ValueId(1),
            args: vec![ValueId(2)],
        },
        Terminator::TailCall {
            function: ValueId(1),
            args: vec![ValueId(2)],
        },
    ] {
        let function = finish(
            "address-live-across-call-terminator",
            vec![Param {
                name: "address".into(),
                ty: Ty::Address,
            }],
            Vec::new(),
            vec![Constant::Nil],
            vec![block(
                0,
                vec![
                    op(
                        &[(1, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    ),
                    op(
                        &[(2, Ty::Word)],
                        OpKind::Convert {
                            op: Convert::AddressToWord,
                            value: ValueId(0),
                        },
                    ),
                ],
                terminator,
            )],
            Vec::new(),
        );

        verify_has(
            &function,
            &VerifyError::AddressLiveAcross {
                block: BlockId(0),
                value: ValueId(2),
                boundary: AddressLivenessBoundary::Call,
            },
        );
    }
}

#[test]
fn verifier_rejects_address_live_across_loop_backedge() {
    let function = finish(
        "address-live-across-loop-backedge",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "again".into(),
                ty: Ty::Bool,
            },
        ],
        vec![Ty::Word],
        Vec::new(),
        vec![
            block(
                0,
                vec![op(
                    &[(2, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::AddressToWord,
                        value: ValueId(0),
                    },
                )],
                Terminator::Jump {
                    target: BlockId(1),
                    args: vec![ValueId(2)],
                },
            ),
            BasicBlock {
                id: BlockId(1),
                params: vec![BlockParam {
                    value: ValueId(3),
                    ty: Ty::Word,
                }],
                ops: Vec::new(),
                terminator: Terminator::Branch {
                    condition: ValueId(1),
                    then_target: BlockId(2),
                    then_args: Vec::new(),
                    else_target: BlockId(3),
                    else_args: Vec::new(),
                },
            },
            block(
                2,
                vec![op(&[], OpKind::Safepoint)],
                Terminator::Jump {
                    target: BlockId(1),
                    args: vec![ValueId(3)],
                },
            ),
            block(
                3,
                vec![
                    op(
                        &[(4, Ty::Address)],
                        OpKind::Convert {
                            op: Convert::WordToAddress,
                            value: ValueId(3),
                        },
                    ),
                    op(
                        &[(5, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(4),
                        },
                    ),
                ],
                Terminator::Return {
                    values: vec![ValueId(5)],
                },
            ),
        ],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::AddressLiveAcross {
            block: BlockId(2),
            value: ValueId(3),
            boundary: AddressLivenessBoundary::LoopBackedge { target: BlockId(1) },
        },
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
fn verifier_uses_handler_edges_for_dominance() {
    let function = finish(
        "handler-dominance",
        Vec::new(),
        vec![Ty::Word],
        vec![Constant::Nil],
        vec![
            block(
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
                Terminator::Jump {
                    target: BlockId(1),
                    args: Vec::new(),
                },
            ),
            block(
                1,
                Vec::new(),
                Terminator::Throw {
                    condition: ValueId(0),
                },
            ),
            BasicBlock {
                id: BlockId(2),
                params: vec![BlockParam {
                    value: ValueId(1),
                    ty: Ty::Word,
                }],
                ops: vec![op(
                    &[],
                    OpKind::LeaveHandler {
                        region: HandlerRegionId(0),
                    },
                )],
                terminator: Terminator::Jump {
                    target: BlockId(3),
                    args: Vec::new(),
                },
            },
            block(
                3,
                Vec::new(),
                Terminator::Return {
                    values: vec![ValueId(0)],
                },
            ),
        ],
        vec![HandlerRegion {
            id: HandlerRegionId(0),
            kind: HandlerKind::Catch,
            protected: vec![BlockId(1)],
            handler: BlockId(2),
            cleanup: None,
            catch_tag: Some(ValueId(0)),
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        }],
    );

    assert!(verify(&function).is_ok());
}

#[test]
fn unreachable_predecessor_does_not_break_reachable_dominance() {
    let function = finish(
        "unreachable-predecessor",
        Vec::new(),
        vec![Ty::Word],
        vec![Constant::Nil, Constant::T],
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
                    target: BlockId(2),
                    args: Vec::new(),
                },
            ),
            block(
                1,
                vec![op(
                    &[(1, Ty::Word)],
                    OpKind::Const {
                        result: ConstantIndex(1),
                    },
                )],
                Terminator::Jump {
                    target: BlockId(2),
                    args: Vec::new(),
                },
            ),
            block(
                2,
                Vec::new(),
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
fn verifier_accepts_outer_values_used_by_unwind_cleanup() {
    let function = finish(
        "unwind-cleanup-outer-value",
        Vec::new(),
        vec![Ty::Word],
        vec![Constant::Nil, Constant::T],
        vec![
            block(
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
                        OpKind::MakeValueCell { value: ValueId(0) },
                    ),
                    op(
                        &[],
                        OpKind::EnterHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                ],
                Terminator::Jump {
                    target: BlockId(2),
                    args: Vec::new(),
                },
            ),
            block(
                1,
                vec![
                    op(
                        &[],
                        OpKind::LeaveHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                    op(
                        &[],
                        OpKind::StoreField {
                            object: ValueId(1),
                            field: 0,
                            value: ValueId(0),
                        },
                    ),
                    op(
                        &[],
                        OpKind::LeaveHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                    op(
                        &[(2, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(1),
                        },
                    ),
                ],
                Terminator::Return {
                    values: vec![ValueId(2)],
                },
            ),
            block(
                2,
                vec![
                    op(
                        &[],
                        OpKind::LeaveHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                    op(
                        &[(3, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    ),
                ],
                Terminator::Return {
                    values: vec![ValueId(3)],
                },
            ),
        ],
        vec![HandlerRegion {
            id: HandlerRegionId(0),
            kind: HandlerKind::UnwindProtect,
            protected: vec![BlockId(0)],
            handler: BlockId(1),
            cleanup: Some(BlockId(1)),
            catch_tag: None,
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        }],
    );

    assert!(verify(&function).is_ok());
}

#[test]
fn proven_fixnum_values_flow_through_all_fixnum_branch_inputs() {
    let function = finish(
        "fixnum-join",
        Vec::new(),
        vec![Ty::Word],
        vec![Constant::Fixnum(2), Constant::Fixnum(3)],
        vec![
            block(
                0,
                vec![
                    op(
                        &[(0, Ty::I64)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    ),
                    op(
                        &[(1, Ty::Word)],
                        OpKind::Convert {
                            op: Convert::I64ToWord,
                            value: ValueId(0),
                        },
                    ),
                    op(
                        &[(2, Ty::I64)],
                        OpKind::Const {
                            result: ConstantIndex(1),
                        },
                    ),
                    op(
                        &[(3, Ty::Word)],
                        OpKind::Convert {
                            op: Convert::I64ToWord,
                            value: ValueId(2),
                        },
                    ),
                    op(
                        &[(4, Ty::Bool)],
                        OpKind::Compare {
                            op: Compare::Eq,
                            left: ValueId(0),
                            right: ValueId(2),
                        },
                    ),
                ],
                Terminator::Branch {
                    condition: ValueId(4),
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
                    args: vec![ValueId(1)],
                },
            ),
            block(
                2,
                Vec::new(),
                Terminator::Jump {
                    target: BlockId(3),
                    args: vec![ValueId(3)],
                },
            ),
            BasicBlock {
                id: BlockId(3),
                params: vec![BlockParam {
                    value: ValueId(5),
                    ty: Ty::Word,
                }],
                ops: Vec::new(),
                terminator: Terminator::Return {
                    values: vec![ValueId(5)],
                },
            },
        ],
        Vec::new(),
    );

    assert!(proven_fixnum_values(&function).contains(&ValueId(5)));
    assert!(verify(&function).is_ok());
}

fn handler_fixnum_entry() -> BasicBlock {
    block(
        0,
        vec![
            op(
                &[(0, Ty::I64)],
                OpKind::Const {
                    result: ConstantIndex(0),
                },
            ),
            op(
                &[(1, Ty::Word)],
                OpKind::Convert {
                    op: Convert::I64ToWord,
                    value: ValueId(0),
                },
            ),
            op(
                &[(2, Ty::Bool)],
                OpKind::Compare {
                    op: Compare::Eq,
                    left: ValueId(0),
                    right: ValueId(0),
                },
            ),
            op(
                &[(3, Ty::Word)],
                OpKind::Const {
                    result: ConstantIndex(1),
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
    )
}

fn handler_fixnum_condition() -> BasicBlock {
    block(
        1,
        Vec::new(),
        Terminator::Branch {
            condition: ValueId(2),
            then_target: BlockId(2),
            then_args: vec![ValueId(1), ValueId(1)],
            else_target: BlockId(3),
            else_args: Vec::new(),
        },
    )
}

fn handler_fixnum_body(args: Vec<ValueId>, specialization: FixnumSpecialization) -> BasicBlock {
    BasicBlock {
        id: BlockId(2),
        params: vec![
            BlockParam {
                value: ValueId(4),
                ty: Ty::Word,
            },
            BlockParam {
                value: ValueId(5),
                ty: Ty::Word,
            },
        ],
        ops: vec![
            op(
                &[],
                OpKind::LeaveHandler {
                    region: HandlerRegionId(0),
                },
            ),
            op(&[], OpKind::Safepoint),
            op(
                &[(6, Ty::Word)],
                OpKind::Builtin {
                    name: "+".into(),
                    args,
                    specialization: Some(specialization),
                    specialization_disabled: false,
                },
            ),
        ],
        terminator: Terminator::Return {
            values: vec![ValueId(6)],
        },
    }
}

fn handler_fixnum_region(kind: HandlerKind) -> HandlerRegion {
    HandlerRegion {
        id: HandlerRegionId(0),
        kind,
        protected: vec![BlockId(1), BlockId(3)],
        handler: BlockId(2),
        cleanup: None,
        catch_tag: (kind == HandlerKind::Catch).then_some(ValueId(3)),
        binding_targets: vec![ValueId(1)],
        depth: 0,
        parent: None,
    }
}

fn handler_fixnum_function(kind: HandlerKind, builtin_args: Vec<ValueId>) -> Function {
    let specialization = FixnumSpecialization::new(FixnumOp::Add, &[false, false])
        .unwrap_or_else(|| panic!("two-operand specialization"));
    let name = match kind {
        HandlerKind::Catch => "catch-fixnum-join",
        HandlerKind::Progv => "progv-fixnum-join",
        HandlerKind::UnwindProtect => "unwind-fixnum-join",
    };
    finish(
        name,
        Vec::new(),
        vec![Ty::Word],
        vec![Constant::Fixnum(2), Constant::Nil],
        vec![
            handler_fixnum_entry(),
            handler_fixnum_condition(),
            handler_fixnum_body(builtin_args, specialization),
            block(
                3,
                Vec::new(),
                Terminator::Throw {
                    condition: ValueId(3),
                },
            ),
        ],
        vec![handler_fixnum_region(kind)],
    )
}

#[test]
fn verifier_treats_catch_payload_as_an_unproven_fixnum_input() {
    let function = handler_fixnum_function(HandlerKind::Catch, vec![ValueId(4), ValueId(5)]);

    assert!(!proven_fixnum_values(&function).contains(&ValueId(4)));
    assert!(proven_fixnum_values(&function).contains(&ValueId(5)));
    verify_has(
        &function,
        &VerifyError::InvalidFixnumSpecialization(BlockId(2)),
    );

    let mut binding_without_payload = function;
    binding_without_payload.blocks[2].params.clear();
    verify_has(
        &binding_without_payload,
        &VerifyError::HandlerMismatch(BlockId(2)),
    );
}

#[test]
fn verifier_does_not_use_progv_bindings_as_handler_parameters() {
    let function = handler_fixnum_function(HandlerKind::Progv, vec![ValueId(5), ValueId(1)]);

    assert!(!proven_fixnum_values(&function).contains(&ValueId(5)));
    verify_has(
        &function,
        &VerifyError::InvalidFixnumSpecialization(BlockId(2)),
    );
    verify_has(&function, &VerifyError::HandlerMismatch(BlockId(2)));

    let mut unwind_protect = function;
    unwind_protect.handler_regions[0].kind = HandlerKind::UnwindProtect;
    unwind_protect.handler_regions[0].cleanup = Some(BlockId(2));
    unwind_protect.handler_regions[0].binding_targets.clear();
    verify_has(&unwind_protect, &VerifyError::HandlerMismatch(BlockId(2)));
}

#[test]
fn verifier_rejects_fixnum_specialization_without_a_tag_guard() {
    let specialization = FixnumSpecialization::new(FixnumOp::Add, &[false, false])
        .unwrap_or_else(|| panic!("two-operand specialization"));
    let function = finish(
        "unguarded-parameter",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
        vec![Constant::Fixnum(1)],
        vec![block(
            0,
            vec![
                op(
                    &[(1, Ty::I64)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
                op(
                    &[(2, Ty::Word)],
                    OpKind::Convert {
                        op: Convert::I64ToWord,
                        value: ValueId(1),
                    },
                ),
                op(&[], OpKind::Safepoint),
                op(
                    &[(3, Ty::Word)],
                    OpKind::Builtin {
                        name: "+".into(),
                        args: vec![ValueId(0), ValueId(2)],
                        specialization: Some(specialization),
                        specialization_disabled: false,
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(3)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::InvalidFixnumSpecialization(BlockId(0)),
    );
}

#[test]
fn verifier_rejects_fixnum_metadata_with_the_wrong_operation() {
    assert!(FixnumSpecialization::new(FixnumOp::Add, &[true]).is_none());
    let specialization = FixnumSpecialization::new(FixnumOp::Add, &[true, true])
        .unwrap_or_else(|| panic!("two-operand specialization"));
    let function = finish(
        "mismatched-operation",
        vec![
            Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
        Vec::new(),
        vec![block(
            0,
            vec![
                op(&[], OpKind::Safepoint),
                op(
                    &[(2, Ty::Word)],
                    OpKind::Builtin {
                        name: "-".into(),
                        args: vec![ValueId(0), ValueId(1)],
                        specialization: Some(specialization),
                        specialization_disabled: false,
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(2)],
            },
        )],
        Vec::new(),
    );

    verify_has(
        &function,
        &VerifyError::InvalidFixnumSpecialization(BlockId(0)),
    );
}
