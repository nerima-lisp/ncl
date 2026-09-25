use crate::*;

pub(super) fn finish(
    name: &str,
    params: Vec<Param>,
    returns: Vec<Ty>,
    constants: Vec<Constant>,
    blocks: Vec<BasicBlock>,
    handlers: Vec<HandlerRegion>,
) -> Function {
    Function {
        id: FunctionId(1),
        name: name.into(),
        params,
        return_types: returns,
        blocks,
        locals: Vec::new(),
        constants,
        handler_regions: handlers,
        debug: Vec::new(),
    }
}

pub(super) fn block(id: u32, ops: Vec<Op>, terminator: Terminator) -> BasicBlock {
    BasicBlock {
        id: BlockId(id),
        params: Vec::new(),
        ops,
        terminator,
    }
}

pub(super) fn op(results: &[(u32, Ty)], kind: OpKind) -> Op {
    Op {
        results: results.iter().map(|(id, ty)| (ValueId(*id), *ty)).collect(),
        kind,
        loc: None,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the table keeps the required round-trip fixtures together"
)]
fn round_trip_cases() -> Vec<(&'static str, Function)> {
    vec![
        (
            "fib",
            finish(
                "fib",
                vec![Param {
                    name: "n".into(),
                    ty: Ty::I64,
                }],
                vec![Ty::I64],
                vec![Constant::Fixnum(1)],
                vec![block(
                    0,
                    vec![op(
                        &[(0, Ty::I64)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    )],
                    Terminator::Return {
                        values: vec![ValueId(0)],
                    },
                )],
                Vec::new(),
            ),
        ),
        (
            "cons-car-cdr",
            finish(
                "cons",
                vec![
                    Param {
                        name: "cell".into(),
                        ty: Ty::Word,
                    },
                    Param {
                        name: "value".into(),
                        ty: Ty::Word,
                    },
                ],
                vec![Ty::Word],
                Vec::new(),
                vec![block(
                    0,
                    vec![
                        op(
                            &[(1, Ty::Word)],
                            OpKind::Builtin {
                                name: "cons".into(),
                                args: vec![ValueId(0), ValueId(1)],
                            },
                        ),
                        op(
                            &[(2, Ty::Word)],
                            OpKind::Prim {
                                op: Prim::Car,
                                args: vec![ValueId(1)],
                                condition: None,
                            },
                        ),
                        op(
                            &[(3, Ty::Word)],
                            OpKind::Prim {
                                op: Prim::Cdr,
                                args: vec![ValueId(1)],
                                condition: None,
                            },
                        ),
                    ],
                    Terminator::Return {
                        values: vec![ValueId(3)],
                    },
                )],
                Vec::new(),
            ),
        ),
        (
            "multiple-values",
            finish(
                "values",
                Vec::new(),
                vec![Ty::Word, Ty::I64],
                vec![Constant::Nil, Constant::Fixnum(2)],
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
                            &[(1, Ty::I64)],
                            OpKind::Const {
                                result: ConstantIndex(1),
                            },
                        ),
                        op(
                            &[],
                            OpKind::SetMultipleValues {
                                values: vec![ValueId(0), ValueId(1)],
                            },
                        ),
                    ],
                    Terminator::Return {
                        values: vec![ValueId(0), ValueId(1)],
                    },
                )],
                Vec::new(),
            ),
        ),
        (
            "handler-region",
            finish(
                "catch-unwind-protect",
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
                    block(
                        1,
                        Vec::new(),
                        Terminator::Jump {
                            target: BlockId(2),
                            args: Vec::new(),
                        },
                    ),
                    block(2, Vec::new(), Terminator::Return { values: Vec::new() }),
                ],
                vec![HandlerRegion {
                    id: HandlerRegionId(0),
                    kind: HandlerKind::Catch,
                    protected: vec![BlockId(0)],
                    handler: BlockId(1),
                    cleanup: Some(BlockId(2)),
                    catch_tag: None,
                    binding_targets: Vec::new(),
                    depth: 1,
                    parent: None,
                }],
            ),
        ),
        (
            "self-tail-call",
            finish(
                "loop",
                vec![
                    Param {
                        name: "self".into(),
                        ty: Ty::Word,
                    },
                    Param {
                        name: "n".into(),
                        ty: Ty::I64,
                    },
                ],
                Vec::new(),
                Vec::new(),
                vec![block(
                    0,
                    Vec::new(),
                    Terminator::TailCall {
                        function: ValueId(0),
                        args: vec![ValueId(1)],
                    },
                )],
                Vec::new(),
            ),
        ),
        (
            "switch",
            finish(
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
                            cases: vec![(0, BlockId(1), Vec::new()), (1, BlockId(2), Vec::new())],
                            default: BlockId(3),
                            default_args: Vec::new(),
                        },
                    ),
                    block(1, Vec::new(), Terminator::Return { values: Vec::new() }),
                    block(2, Vec::new(), Terminator::Return { values: Vec::new() }),
                    block(3, Vec::new(), Terminator::Return { values: Vec::new() }),
                ],
                Vec::new(),
            ),
        ),
        (
            "builtin",
            finish(
                "length",
                vec![Param {
                    name: "object".into(),
                    ty: Ty::Word,
                }],
                vec![Ty::Word],
                Vec::new(),
                vec![block(
                    0,
                    vec![op(
                        &[(1, Ty::Word)],
                        OpKind::Builtin {
                            name: "length".into(),
                            args: vec![ValueId(0)],
                        },
                    )],
                    Terminator::Return {
                        values: vec![ValueId(1)],
                    },
                )],
                Vec::new(),
            ),
        ),
        (
            "call-indirect",
            finish(
                "invoke",
                vec![
                    Param {
                        name: "callee".into(),
                        ty: Ty::Word,
                    },
                    Param {
                        name: "arg".into(),
                        ty: Ty::I64,
                    },
                ],
                vec![Ty::Word],
                Vec::new(),
                vec![block(
                    0,
                    vec![op(
                        &[(2, Ty::Word)],
                        OpKind::CallIndirect {
                            callee: ValueId(0),
                            args: vec![ValueId(1)],
                        },
                    )],
                    Terminator::Return {
                        values: vec![ValueId(2)],
                    },
                )],
                Vec::new(),
            ),
        ),
        (
            "prim-condition",
            finish(
                "guarded-car",
                vec![Param {
                    name: "cell".into(),
                    ty: Ty::Word,
                }],
                vec![Ty::Word],
                Vec::new(),
                vec![
                    block(
                        0,
                        vec![op(
                            &[(1, Ty::Word)],
                            OpKind::Prim {
                                op: Prim::Car,
                                args: vec![ValueId(0)],
                                condition: Some(BlockId(1)),
                            },
                        )],
                        Terminator::Return {
                            values: vec![ValueId(1)],
                        },
                    ),
                    block(
                        1,
                        Vec::new(),
                        Terminator::Throw {
                            condition: ValueId(0),
                        },
                    ),
                ],
                Vec::new(),
            ),
        ),
        (
            "alloc-safepoint",
            finish(
                "allocate",
                Vec::new(),
                vec![Ty::Address],
                Vec::new(),
                vec![block(
                    0,
                    vec![
                        op(&[(0, Ty::Address)], OpKind::Alloc { words: 3 }),
                        op(&[], OpKind::Safepoint),
                    ],
                    Terminator::Return {
                        values: vec![ValueId(0)],
                    },
                )],
                Vec::new(),
            ),
        ),
        (
            "all-constant-variants",
            finish(
                "constants",
                Vec::new(),
                Vec::new(),
                vec![
                    Constant::Fixnum(1),
                    Constant::Character(65),
                    Constant::SingleFloat(1.5),
                    Constant::DoubleFloat(2.5),
                    Constant::Symbol {
                        package: "CL".into(),
                        name: "T".into(),
                    },
                    Constant::Object(ConstantIndex(0)),
                    Constant::StringBytes(vec![0, 1, 255]),
                    Constant::Nil,
                    Constant::T,
                    Constant::Unbound,
                ],
                vec![block(
                    0,
                    Vec::new(),
                    Terminator::Return { values: Vec::new() },
                )],
                Vec::new(),
            ),
        ),
        (
            "closure-and-handler-ops",
            finish(
                "closure",
                Vec::new(),
                vec![Ty::Word],
                vec![Constant::FunctionEntry(FunctionId(9))],
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
                        op(
                            &[(1, Ty::Word)],
                            OpKind::MakeClosure {
                                entry: ValueId(0),
                                captures: Vec::new(),
                            },
                        ),
                        op(
                            &[(2, Ty::Word)],
                            OpKind::CallClosure {
                                closure: ValueId(1),
                                args: Vec::new(),
                            },
                        ),
                        op(
                            &[],
                            OpKind::LeaveHandler {
                                region: HandlerRegionId(0),
                            },
                        ),
                    ],
                    Terminator::Return {
                        values: vec![ValueId(2)],
                    },
                )],
                vec![HandlerRegion {
                    id: HandlerRegionId(0),
                    kind: HandlerKind::Catch,
                    protected: vec![BlockId(0)],
                    handler: BlockId(0),
                    cleanup: None,
                    catch_tag: None,
                    binding_targets: Vec::new(),
                    depth: 0,
                    parent: None,
                }],
            ),
        ),
    ]
}

#[test]
fn text_round_trips_requested_forms() {
    let cases = round_trip_cases();
    assert!(cases.len() >= 10);
    for (name, function) in cases {
        let dump = function.to_string();
        assert_eq!(parse(&dump), Ok(function), "round trip failed for {name}");
    }
}
