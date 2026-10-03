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
            "load-capture",
            finish(
                "capture",
                Vec::new(),
                vec![Ty::Word],
                Vec::new(),
                vec![block(
                    0,
                    vec![op(&[(0, Ty::Word)], OpKind::LoadCapture { index: 2 })],
                    Terminator::Return {
                        values: vec![ValueId(0)],
                    },
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
                                named_symbol: None,
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

#[test]
#[allow(clippy::too_many_lines)]
fn text_round_trips_all_serialized_forms() {
    let function = Function {
        id: FunctionId(7),
        name: "name with spaces/日本語".into(),
        params: vec![
            Param {
                name: "word-param".into(),
                ty: Ty::Word,
            },
            Param {
                name: "i64-param".into(),
                ty: Ty::I64,
            },
        ],
        return_types: vec![Ty::Word],
        constants: vec![
            Constant::Fixnum(-2),
            Constant::Character(0x41),
            Constant::SingleFloat(1.5),
            Constant::DoubleFloat(-2.5),
            Constant::Symbol {
                package: "PKG/NAME".into(),
                name: "symbol name".into(),
            },
            Constant::Object(ConstantIndex(0)),
            Constant::StringBytes(vec![0, 1, 0xff]),
            Constant::Structure {
                kind: StructureKind::Cons,
                elements: vec![ConstantIndex(0), ConstantIndex(1)],
            },
            Constant::Structure {
                kind: StructureKind::SimpleVector,
                elements: vec![ConstantIndex(2)],
            },
            Constant::Nil,
            Constant::T,
            Constant::Unbound,
            Constant::FunctionEntry(FunctionId(8)),
            Constant::Bignum {
                negative: true,
                limbs: vec![1, 0xffff_ffff],
            },
            Constant::Ratio {
                numerator: ConstantIndex(0),
                denominator: ConstantIndex(1),
            },
            Constant::Complex {
                real: ConstantIndex(0),
                imaginary: ConstantIndex(1),
            },
        ],
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: vec![BlockParam {
                    value: ValueId(20),
                    ty: Ty::F64,
                }],
                ops: vec![
                    op(
                        &[(21, Ty::Word)],
                        OpKind::Const {
                            result: ConstantIndex(0),
                        },
                    ),
                    op(&[(22, Ty::Word)], OpKind::Move { value: ValueId(21) }),
                    op(
                        &[(23, Ty::Word)],
                        OpKind::Load {
                            address: ValueId(22),
                        },
                    ),
                    op(
                        &[],
                        OpKind::Store {
                            address: ValueId(22),
                            value: ValueId(23),
                        },
                    ),
                    op(
                        &[(24, Ty::Word)],
                        OpKind::LoadField {
                            object: ValueId(23),
                            field: 3,
                        },
                    ),
                    op(
                        &[],
                        OpKind::StoreField {
                            object: ValueId(23),
                            field: 4,
                            value: ValueId(24),
                        },
                    ),
                    op(&[(25, Ty::Address)], OpKind::Alloc { words: 5 }),
                    op(&[(26, Ty::Word)], OpKind::LoadArg { index: 1 }),
                    op(&[(27, Ty::Word)], OpKind::LoadCapture { index: 2 }),
                    op(&[(28, Ty::Word)], OpKind::LoadFunctionObject),
                    op(
                        &[(29, Ty::Word)],
                        OpKind::Call {
                            function: ValueId(21),
                            args: vec![ValueId(22)],
                        },
                    ),
                    op(
                        &[(30, Ty::Word)],
                        OpKind::CallIndirect {
                            callee: ValueId(21),
                            args: vec![ValueId(22)],
                        },
                    ),
                    op(
                        &[(31, Ty::Word)],
                        OpKind::MakeClosure {
                            entry: ValueId(21),
                            captures: vec![ValueId(22)],
                        },
                    ),
                    op(
                        &[(32, Ty::Word)],
                        OpKind::MakeValueCell { value: ValueId(22) },
                    ),
                    op(
                        &[(33, Ty::Word)],
                        OpKind::CallClosure {
                            closure: ValueId(31),
                            args: vec![ValueId(22)],
                            named_symbol: Some(ValueId(21)),
                        },
                    ),
                    op(
                        &[(34, Ty::Word)],
                        OpKind::Builtin {
                            name: "built-in name".into(),
                            args: vec![ValueId(22)],
                        },
                    ),
                    op(
                        &[(35, Ty::Word)],
                        OpKind::Prim {
                            op: Prim::Car,
                            args: vec![ValueId(22)],
                            condition: Some(BlockId(1)),
                        },
                    ),
                    op(
                        &[(36, Ty::Bool)],
                        OpKind::Compare {
                            op: Compare::Ge,
                            left: ValueId(21),
                            right: ValueId(22),
                        },
                    ),
                    op(
                        &[(37, Ty::I64)],
                        OpKind::Convert {
                            op: Convert::WordToI64,
                            value: ValueId(22),
                        },
                    ),
                    op(
                        &[],
                        OpKind::SetMultipleValues {
                            values: vec![ValueId(21), ValueId(22)],
                        },
                    ),
                    op(&[], OpKind::Safepoint),
                    op(
                        &[],
                        OpKind::EnterHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                    op(
                        &[],
                        OpKind::LeaveHandler {
                            region: HandlerRegionId(0),
                        },
                    ),
                ],
                terminator: Terminator::Branch {
                    condition: ValueId(36),
                    then_target: BlockId(1),
                    then_args: vec![ValueId(21)],
                    else_target: BlockId(2),
                    else_args: vec![ValueId(22)],
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: vec![BlockParam {
                    value: ValueId(40),
                    ty: Ty::Word,
                }],
                ops: Vec::new(),
                terminator: Terminator::Switch {
                    value: ValueId(37),
                    cases: vec![(i64::MIN, BlockId(2), vec![ValueId(40)])],
                    default: BlockId(3),
                    default_args: vec![ValueId(40)],
                },
            },
            BasicBlock {
                id: BlockId(2),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::CallReturn {
                    function: ValueId(21),
                    args: vec![ValueId(22)],
                },
            },
            BasicBlock {
                id: BlockId(3),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::TailCall {
                    function: ValueId(21),
                    args: vec![ValueId(22)],
                },
            },
            BasicBlock {
                id: BlockId(4),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Throw {
                    condition: ValueId(21),
                },
            },
            BasicBlock {
                id: BlockId(5),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Unreachable,
            },
        ],
        locals: vec![Local {
            id: LocalId(0),
            name: "local name".into(),
            ty: Ty::Unit,
        }],
        handler_regions: vec![
            HandlerRegion {
                id: HandlerRegionId(0),
                kind: HandlerKind::Catch,
                protected: vec![BlockId(0)],
                handler: BlockId(1),
                cleanup: Some(BlockId(2)),
                catch_tag: Some(ValueId(21)),
                binding_targets: vec![ValueId(22)],
                depth: 2,
                parent: Some(HandlerRegionId(1)),
            },
            HandlerRegion {
                id: HandlerRegionId(1),
                kind: HandlerKind::UnwindProtect,
                protected: vec![BlockId(1)],
                handler: BlockId(2),
                cleanup: None,
                catch_tag: None,
                binding_targets: Vec::new(),
                depth: 3,
                parent: None,
            },
        ],
        debug: vec![DebugLocation {
            file: FileId(1),
            line: 2,
            column: 3,
            form: FormId(4),
        }],
    };

    let dump = function.to_string();
    assert_eq!(parse(&dump), Ok(function));
}

#[test]
#[allow(clippy::too_many_lines)]
fn text_round_trips_all_descriptor_variants() {
    let function = Function {
        id: FunctionId(7),
        name: "name with spaces/日本語".into(),
        params: vec![Param {
            name: "arg-name".into(),
            ty: Ty::F64,
        }],
        return_types: vec![Ty::Address, Ty::Unit],
        blocks: vec![BasicBlock {
            id: BlockId(2),
            params: vec![BlockParam {
                value: ValueId(4),
                ty: Ty::Bool,
            }],
            ops: vec![
                op(
                    &[(5, Ty::I64)],
                    OpKind::Const {
                        result: ConstantIndex(0),
                    },
                ),
                op(&[(6, Ty::Word)], OpKind::LoadCapture { index: 3 }),
                op(&[(7, Ty::Word)], OpKind::LoadFunctionObject),
                op(&[(8, Ty::Address)], OpKind::Alloc { words: 2 }),
                op(
                    &[],
                    OpKind::Store {
                        address: ValueId(8),
                        value: ValueId(7),
                    },
                ),
                op(
                    &[(9, Ty::Word)],
                    OpKind::Load {
                        address: ValueId(8),
                    },
                ),
                op(
                    &[(10, Ty::Word)],
                    OpKind::LoadField {
                        object: ValueId(9),
                        field: 4,
                    },
                ),
                op(
                    &[],
                    OpKind::StoreField {
                        object: ValueId(9),
                        field: 4,
                        value: ValueId(7),
                    },
                ),
                op(
                    &[(11, Ty::Word)],
                    OpKind::CallIndirect {
                        callee: ValueId(7),
                        args: vec![ValueId(9)],
                    },
                ),
                op(
                    &[(12, Ty::Word)],
                    OpKind::MakeValueCell { value: ValueId(7) },
                ),
                op(
                    &[(13, Ty::Bool)],
                    OpKind::Compare {
                        op: Compare::Ge,
                        left: ValueId(5),
                        right: ValueId(5),
                    },
                ),
                op(
                    &[(14, Ty::F64)],
                    OpKind::Convert {
                        op: Convert::WordToF64,
                        value: ValueId(7),
                    },
                ),
                op(
                    &[],
                    OpKind::SetMultipleValues {
                        values: vec![ValueId(7), ValueId(9)],
                    },
                ),
                op(&[], OpKind::Safepoint),
            ],
            terminator: Terminator::Switch {
                value: ValueId(5),
                cases: vec![(-2, BlockId(3), vec![ValueId(4)])],
                default: BlockId(4),
                default_args: vec![],
            },
        }],
        locals: vec![Local {
            id: LocalId(1),
            name: "local name".into(),
            ty: Ty::Unit,
        }],
        constants: vec![
            Constant::Fixnum(-3),
            Constant::Character(0x1f600),
            Constant::SingleFloat(1.5),
            Constant::DoubleFloat(-2.5),
            Constant::Symbol {
                package: "PKG/".into(),
                name: "sym name".into(),
            },
            Constant::Object(ConstantIndex(0)),
            Constant::StringBytes(vec![0, 1, 255]),
            Constant::Nil,
            Constant::T,
            Constant::Unbound,
            Constant::FunctionEntry(FunctionId(8)),
            Constant::Structure {
                kind: StructureKind::SimpleVector,
                elements: vec![ConstantIndex(1)],
            },
            Constant::Bignum {
                negative: true,
                limbs: vec![0, u32::MAX],
            },
            Constant::Ratio {
                numerator: ConstantIndex(0),
                denominator: ConstantIndex(1),
            },
            Constant::Complex {
                real: ConstantIndex(2),
                imaginary: ConstantIndex(3),
            },
        ],
        handler_regions: vec![HandlerRegion {
            id: HandlerRegionId(2),
            kind: HandlerKind::Progv,
            protected: vec![BlockId(2)],
            handler: BlockId(3),
            cleanup: None,
            catch_tag: None,
            binding_targets: vec![ValueId(7)],
            depth: 1,
            parent: Some(HandlerRegionId(1)),
        }],
        debug: vec![DebugLocation {
            file: FileId(3),
            line: 12,
            column: 8,
            form: FormId(9),
        }],
    };
    let dump = function.to_string();
    assert!(
        dump.contains("name_20with_20spaces_2f"),
        "escaped name missing: {dump}"
    );
    assert_eq!(parse(&dump), Ok(function));
}

#[test]
fn text_parser_rejects_malformed_headers_and_payloads() {
    for input in ["", "fn @x", "fn @x { 0, }"] {
        assert!(parse(input).is_err(), "accepted malformed input: {input:?}");
    }
    let base = finish("x", Vec::new(), Vec::new(), Vec::new(), vec![], Vec::new()).to_string();
    for suffix in ["ffff,", "0,0,0,0,0,0,0,0,0,0,0,0,0,0,99,"] {
        let malformed = base.trim_end_matches(" }\n").to_owned() + suffix + " }\n";
        assert!(
            parse(&malformed).is_err(),
            "accepted malformed payload: {malformed}"
        );
    }
}
