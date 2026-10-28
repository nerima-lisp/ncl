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
                                specialization: None,
                                specialization_disabled: false,
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
                            specialization: None,
                            specialization_disabled: false,
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
                    Constant::UninternedSymbol {
                        identity: 7,
                        name: "ALPHA".into(),
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

fn documented_value_ops() -> Vec<Op> {
    vec![
        op(&[(10, Ty::Word)], OpKind::Move { value: ValueId(0) }),
        op(
            &[(11, Ty::Word)],
            OpKind::Load {
                address: ValueId(1),
            },
        ),
        op(
            &[],
            OpKind::Store {
                address: ValueId(1),
                value: ValueId(2),
            },
        ),
        op(
            &[(12, Ty::Word)],
            OpKind::LoadField {
                object: ValueId(1),
                field: 2,
            },
        ),
        op(
            &[],
            OpKind::StoreField {
                object: ValueId(1),
                field: 3,
                value: ValueId(2),
            },
        ),
        op(&[(13, Ty::Address)], OpKind::Alloc { words: 4 }),
        op(&[(14, Ty::Word)], OpKind::LoadArg { index: 5 }),
        op(
            &[(15, Ty::Word)],
            OpKind::Call {
                function: ValueId(1),
                args: vec![ValueId(2), ValueId(3)],
            },
        ),
        op(
            &[(16, Ty::Word)],
            OpKind::CallIndirect {
                callee: ValueId(1),
                args: vec![ValueId(2)],
            },
        ),
        op(
            &[(17, Ty::Word)],
            OpKind::Builtin {
                name: "runtime-name".into(),
                args: vec![ValueId(2)],
                specialization: None,
                specialization_disabled: false,
            },
        ),
        op(
            &[],
            OpKind::SetMultipleValues {
                values: vec![ValueId(2), ValueId(3)],
            },
        ),
        op(&[], OpKind::Safepoint),
    ]
}

fn documented_primitive_ops() -> Vec<Op> {
    [
        Prim::Car,
        Prim::Cdr,
        Prim::Rplaca,
        Prim::Rplacd,
        Prim::Svref,
        Prim::Aref,
        Prim::Aset,
        Prim::FixnumAdd,
        Prim::FixnumSub,
        Prim::FixnumMul,
        Prim::FixnumDiv,
        Prim::FixnumLt,
        Prim::FixnumLe,
        Prim::FixnumEq,
        Prim::Eq,
        Prim::Eql,
        Prim::Typep,
    ]
    .into_iter()
    .map(|primitive| {
        op(
            &[],
            OpKind::Prim {
                op: primitive,
                args: vec![ValueId(1), ValueId(2)],
                condition: Some(BlockId(7)),
            },
        )
    })
    .collect()
}

fn documented_comparison_and_hint_ops() -> Vec<Op> {
    let mut ops: Vec<_> = [
        Compare::Eq,
        Compare::Ne,
        Compare::Lt,
        Compare::Le,
        Compare::Gt,
        Compare::Ge,
    ]
    .into_iter()
    .map(|comparison| {
        op(
            &[(18, Ty::Bool)],
            OpKind::Compare {
                op: comparison,
                left: ValueId(1),
                right: ValueId(2),
            },
        )
    })
    .collect();

    ops.extend(
        [
            (20, crate::ScalarTypeHint::Fixnum),
            (21, crate::ScalarTypeHint::Integer),
            (22, crate::ScalarTypeHint::Number),
            (23, crate::ScalarTypeHint::Boolean),
            (24, crate::ScalarTypeHint::NonNumber),
        ]
        .into_iter()
        .map(|(value_id, hint)| {
            op(
                &[(value_id, Ty::Word)],
                OpKind::TypeHint {
                    value: ValueId(1),
                    ty: hint,
                },
            )
        }),
    );
    ops
}

#[test]
fn text_round_trips_documented_op_payloads_and_types() {
    let types = [Ty::Word, Ty::I64, Ty::F64, Ty::Address, Ty::Bool, Ty::Unit];
    let params = types
        .iter()
        .enumerate()
        .map(|(index, ty)| Param {
            name: format!("arg{index}"),
            ty: *ty,
        })
        .collect();
    let mut ops = documented_value_ops();
    ops.extend(documented_primitive_ops());
    ops.extend(documented_comparison_and_hint_ops());
    let function = finish(
        "operations",
        params,
        types.to_vec(),
        Vec::new(),
        vec![block(0, ops, Terminator::Return { values: Vec::new() })],
        Vec::new(),
    );

    assert_eq!(parse(&function.to_string()), Ok(function));
}

#[test]
fn text_round_trips_extended_constant_descriptors() {
    let mut constants = vec![
        Constant::Fixnum(i64::MIN),
        Constant::Bignum {
            negative: true,
            limbs: vec![0x89ab_cdef, 0x0123_4567],
        },
        Constant::Structure {
            kind: StructureKind::Cons,
            elements: vec![ConstantIndex(0), ConstantIndex(1)],
        },
        Constant::Structure {
            kind: StructureKind::SimpleVector,
            elements: vec![ConstantIndex(2)],
        },
        Constant::Ratio {
            numerator: ConstantIndex(0),
            denominator: ConstantIndex(1),
        },
        Constant::Complex {
            real: ConstantIndex(4),
            imaginary: ConstantIndex(0),
        },
        Constant::Fixnum(1),
    ];
    constants.extend(
        [
            SpecializedArrayElementType::Bit,
            SpecializedArrayElementType::Character,
            SpecializedArrayElementType::BaseChar,
            SpecializedArrayElementType::Fixnum,
            SpecializedArrayElementType::Signed,
            SpecializedArrayElementType::Unsigned,
            SpecializedArrayElementType::SingleFloat,
            SpecializedArrayElementType::DoubleFloat,
        ]
        .into_iter()
        .map(|element_type| Constant::Structure {
            kind: StructureKind::SpecializedArray { element_type },
            elements: vec![ConstantIndex(6)],
        }),
    );
    constants.extend([
        Constant::Character(u32::from('A')),
        Constant::Character(u32::from('B')),
        Constant::Structure {
            kind: StructureKind::Array {
                dimensions: vec![2],
                element_type: ArrayElementType::Character,
            },
            elements: vec![ConstantIndex(15), ConstantIndex(16)],
        },
        Constant::LiteralObject {
            identity: 11,
            value: ConstantIndex(17),
        },
        Constant::Structure {
            kind: StructureKind::Array {
                dimensions: vec![2, 0],
                element_type: ArrayElementType::T,
            },
            elements: Vec::new(),
        },
    ]);

    let function = finish(
        "constant-descriptors",
        Vec::new(),
        Vec::new(),
        constants,
        vec![block(
            0,
            Vec::new(),
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );

    assert_eq!(parse(&function.to_string()), Ok(function));
}

#[test]
fn text_round_trips_specialized_array_element_tags() {
    let cases = [
        (
            SpecializedArrayElementType::Bit,
            0,
            [Constant::Fixnum(0), Constant::Fixnum(1)],
        ),
        (
            SpecializedArrayElementType::Character,
            1,
            [
                Constant::Character('A' as u32),
                Constant::Character('B' as u32),
            ],
        ),
        (
            SpecializedArrayElementType::BaseChar,
            2,
            [
                Constant::Character('A' as u32),
                Constant::Character('B' as u32),
            ],
        ),
        (
            SpecializedArrayElementType::Fixnum,
            3,
            [Constant::Fixnum(-7), Constant::Fixnum(29)],
        ),
        (
            SpecializedArrayElementType::Signed,
            4,
            [Constant::Fixnum(-7), Constant::Fixnum(29)],
        ),
        (
            SpecializedArrayElementType::Unsigned,
            5,
            [Constant::Fixnum(7), Constant::Fixnum(29)],
        ),
        (
            SpecializedArrayElementType::SingleFloat,
            6,
            [Constant::SingleFloat(1.25), Constant::SingleFloat(-3.5)],
        ),
        (
            SpecializedArrayElementType::DoubleFloat,
            7,
            [Constant::DoubleFloat(1.25), Constant::DoubleFloat(-3.5)],
        ),
    ];

    for (element_type, tag, values) in cases {
        let mut constants = values.to_vec();
        constants.push(Constant::Structure {
            kind: StructureKind::SpecializedArray { element_type },
            elements: vec![ConstantIndex(0), ConstantIndex(1)],
        });
        let function = finish(
            "specialized-array-tags",
            Vec::new(),
            Vec::new(),
            constants,
            vec![block(
                0,
                Vec::new(),
                Terminator::Return { values: Vec::new() },
            )],
            Vec::new(),
        );

        let dump = function.to_string();
        let rendered_tag = format!(",b,2,{tag:x},2,0,1,");
        assert!(
            dump.contains(&rendered_tag),
            "wrong serialized tag for {element_type:?}: {dump}"
        );

        let parsed = parse(&dump);
        assert_eq!(
            parsed.as_ref().map(|parsed| &parsed.constants[..2]),
            Ok(&values[..]),
            "element values changed for {element_type:?}"
        );
        assert_eq!(
            parsed.as_ref().map(|parsed| parsed.constants.get(2)),
            Ok(Some(&Constant::Structure {
                kind: StructureKind::SpecializedArray { element_type },
                elements: vec![ConstantIndex(0), ConstantIndex(1)],
            })),
            "element type or shape changed for {element_type:?}"
        );

        if element_type == SpecializedArrayElementType::Bit {
            let invalid_tag = dump.replacen(&rendered_tag, ",b,2,8,2,0,1,", 1);
            assert_eq!(
                parse(&invalid_tag),
                Err(ParseError("bad specialized array element type".into()))
            );
        }
    }
}

#[test]
fn text_round_trips_remaining_documented_values() {
    let constants = vec![
        Constant::Character('λ' as u32),
        Constant::SingleFloat(1.5),
        Constant::DoubleFloat(-2.25),
        Constant::Symbol {
            package: "PKG".into(),
            name: "symbol".into(),
        },
        Constant::UninternedSymbol {
            identity: 23,
            name: "gensym".into(),
        },
        Constant::Object(ConstantIndex(0)),
        Constant::StringBytes(vec![0x00, 0x2c, 0xff]),
        Constant::Nil,
        Constant::T,
        Constant::Unbound,
        Constant::FunctionEntry(FunctionId(9)),
        Constant::Bignum {
            negative: false,
            limbs: vec![3],
        },
    ];
    let ops = vec![
        op(&[(1, Ty::Word)], OpKind::LoadCapture { index: 6 }),
        op(&[(2, Ty::Word)], OpKind::LoadFunctionObject),
        op(
            &[(3, Ty::Word)],
            OpKind::MakeClosure {
                entry: ValueId(1),
                captures: vec![ValueId(2), ValueId(4)],
            },
        ),
        op(
            &[(4, Ty::Word)],
            OpKind::MakeValueCell { value: ValueId(3) },
        ),
        op(
            &[(5, Ty::Word)],
            OpKind::CallClosure {
                closure: ValueId(4),
                args: vec![ValueId(3)],
                named_symbol: Some(ValueId(6)),
            },
        ),
    ];
    let function = finish(
        "remaining-values",
        Vec::new(),
        Vec::new(),
        constants,
        vec![
            block(
                0,
                ops,
                Terminator::Jump {
                    target: BlockId(1),
                    args: vec![ValueId(5)],
                },
            ),
            block(1, Vec::new(), Terminator::Return { values: Vec::new() }),
        ],
        Vec::new(),
    );

    assert_eq!(parse(&function.to_string()), Ok(function));
}

#[test]
fn text_round_trips_documented_conversions() {
    let conversions = [
        Convert::WordToI64,
        Convert::I64ToWord,
        Convert::WordToF64,
        Convert::F64ToWord,
        Convert::AddressToWord,
        Convert::WordToAddress,
    ];
    for conversion in conversions {
        let function = finish(
            "conversion",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![block(
                0,
                vec![op(
                    &[(1, Ty::Word)],
                    OpKind::Convert {
                        op: conversion,
                        value: ValueId(0),
                    },
                )],
                Terminator::Return { values: Vec::new() },
            )],
            Vec::new(),
        );
        let dump = function.to_string();
        assert_eq!(parse(&dump), Ok(function), "{conversion:?} in {dump:?}");
    }
}

#[test]
fn text_round_trips_documented_terminators() {
    let block_param = BlockParam {
        value: ValueId(8),
        ty: Ty::Word,
    };
    let cases = [
        finish(
            "branch",
            vec![Param {
                name: "condition".into(),
                ty: Ty::Bool,
            }],
            Vec::new(),
            Vec::new(),
            vec![
                block(
                    0,
                    Vec::new(),
                    Terminator::Branch {
                        condition: ValueId(0),
                        then_target: BlockId(1),
                        then_args: vec![ValueId(0)],
                        else_target: BlockId(2),
                        else_args: vec![ValueId(1)],
                    },
                ),
                BasicBlock {
                    id: BlockId(1),
                    params: vec![block_param],
                    ops: Vec::new(),
                    terminator: Terminator::Return { values: Vec::new() },
                },
                block(2, Vec::new(), Terminator::Return { values: Vec::new() }),
            ],
            Vec::new(),
        ),
        finish(
            "call-return",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![block(
                0,
                Vec::new(),
                Terminator::CallReturn {
                    function: ValueId(1),
                    args: vec![ValueId(2), ValueId(3)],
                },
            )],
            Vec::new(),
        ),
        finish(
            "unreachable",
            Vec::new(),
            Vec::new(),
            Vec::new(),
            vec![block(0, Vec::new(), Terminator::Unreachable)],
            Vec::new(),
        ),
    ];

    for function in cases {
        assert_eq!(parse(&function.to_string()), Ok(function));
    }
}

#[test]
fn text_round_trips_documented_handler_metadata() {
    let mut function = finish(
        "handler-metadata",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![BasicBlock {
            id: BlockId(0),
            params: vec![BlockParam {
                value: ValueId(8),
                ty: Ty::Word,
            }],
            ops: Vec::new(),
            terminator: Terminator::Return { values: Vec::new() },
        }],
        vec![
            HandlerRegion {
                id: HandlerRegionId(0),
                kind: HandlerKind::Catch,
                protected: vec![BlockId(0)],
                handler: BlockId(1),
                cleanup: None,
                catch_tag: Some(ValueId(4)),
                binding_targets: Vec::new(),
                depth: 0,
                parent: None,
            },
            HandlerRegion {
                id: HandlerRegionId(1),
                kind: HandlerKind::UnwindProtect,
                protected: vec![BlockId(0), BlockId(1)],
                handler: BlockId(2),
                cleanup: Some(BlockId(3)),
                catch_tag: None,
                binding_targets: Vec::new(),
                depth: 1,
                parent: Some(HandlerRegionId(0)),
            },
            HandlerRegion {
                id: HandlerRegionId(2),
                kind: HandlerKind::Progv,
                protected: Vec::new(),
                handler: BlockId(4),
                cleanup: None,
                catch_tag: None,
                binding_targets: vec![ValueId(5), ValueId(6)],
                depth: 2,
                parent: Some(HandlerRegionId(1)),
            },
        ],
    );
    function.locals = vec![Local {
        id: LocalId(1),
        name: "temporary".into(),
        ty: Ty::F64,
    }];
    function.debug = vec![DebugLocation {
        file: FileId(2),
        line: 10,
        column: 4,
        form: FormId(7),
    }];

    assert_eq!(parse(&function.to_string()), Ok(function));
}

#[test]
fn text_round_trips_negative_switch_cases() {
    let function = finish(
        "negative-switch",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            Vec::new(),
            Terminator::Switch {
                value: ValueId(0),
                cases: vec![(-15, BlockId(1), vec![ValueId(2)])],
                default: BlockId(2),
                default_args: vec![ValueId(3)],
            },
        )],
        Vec::new(),
    );
    let dump = function.to_string();

    assert_eq!(parse(&dump), Ok(function), "serialized switch was {dump:?}");
}

#[test]
fn text_printer_emits_the_documented_ir3_layout() {
    let mut body = block(
        0,
        vec![op(
            &[(1, Ty::Word)],
            OpKind::Const {
                result: ConstantIndex(0),
            },
        )],
        Terminator::Return {
            values: vec![ValueId(1)],
        },
    );
    body.ops[0].loc = Some(DebugLocationId(2));
    let mut function = finish(
        "worker/entry",
        vec![Param {
            name: "input".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
        vec![Constant::Symbol {
            package: "MY PKG".into(),
            name: "ITEM".into(),
        }],
        vec![body],
        Vec::new(),
    );
    function.locals = vec![Local {
        id: LocalId(3),
        name: "slot/name".into(),
        ty: Ty::Bool,
    }];
    function.debug = vec![DebugLocation {
        file: FileId(7),
        line: 8,
        column: 9,
        form: FormId(10),
    }];

    assert_eq!(
        function.to_string(),
        concat!(
            "fn @worker_2fentry ir3 { ",
            "1,worker_2fentry,1,input,0,1,0,1,4,MY_20PKG,ITEM,",
            "1,0,0,1,1,1,0,2,0,0,5,1,1,",
            "1,3,slot_2fname,4,0,1,7,8,9,a, }\n",
        )
    );
}

#[test]
fn text_round_trips_names_with_literal_underscores() {
    let function = finish(
        "field_20name",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            Vec::new(),
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );

    assert_eq!(parse(&function.to_string()), Ok(function));
}

fn input_with_payload(payload: &str) -> String {
    format!("fn @bad ir3 {{ {payload} }}\n")
}

fn assert_payload_errors(cases: &[(&str, &str, &str)]) {
    for (field, payload, expected) in cases {
        assert_eq!(
            parse(&input_with_payload(payload)),
            Err(ParseError((*expected).into())),
            "incorrect ParseError for {field}"
        );
    }
}

#[test]
fn text_rejects_malformed_identity_and_constant_fields() {
    assert_payload_errors(&[
        (
            "function id is not hexadecimal",
            "q,bad,0,0,0,1,0,0,0,5,0,0,0,0,",
            "bad integer",
        ),
        (
            "function id exceeds the public u32 id range",
            "100000000,bad,0,0,0,1,0,0,0,5,0,0,0,0,",
            "integer out of range",
        ),
        (
            "parameter type tag",
            "0,bad,1,p,6,0,0,1,0,0,0,5,0,0,0,0,",
            "bad type",
        ),
        (
            "parameter text escape",
            "0,bad,1,_zz,0,0,0,1,0,0,0,5,0,0,0,0,",
            "bad escape",
        ),
        (
            "truncated parameter text escape",
            "0,bad,1,_,0,0,0,1,0,0,0,5,0,0,0,0,",
            "bad escape",
        ),
        (
            "function name UTF-8 bytes",
            "0,_ff,0,0,0,1,0,0,0,5,0,0,0,0,",
            "invalid utf8",
        ),
        (
            "constant tag",
            "0,bad,0,0,1,12,0,1,0,0,0,5,0,0,0,0,",
            "bad constant",
        ),
        (
            "structure constant kind",
            "0,bad,0,0,1,b,4,0,1,0,0,0,5,0,0,0,0,",
            "bad structure kind",
        ),
    ]);
}

#[test]
fn text_rejects_malformed_operation_fields() {
    assert_payload_errors(&[
        (
            "operation tag",
            "0,bad,0,0,0,1,0,0,1,0,ffffffffffffffff,ff,",
            "bad operation",
        ),
        (
            "primitive name",
            "0,bad,0,0,0,1,0,0,1,0,ffffffffffffffff,d,Unknown,0,ffffffffffffffff,",
            "bad primitive",
        ),
        (
            "comparison name",
            "0,bad,0,0,0,1,0,0,1,0,ffffffffffffffff,e,Unknown,",
            "bad comparison",
        ),
        (
            "conversion name",
            "0,bad,0,0,0,1,0,0,1,0,ffffffffffffffff,f,Unknown,",
            "bad conversion",
        ),
        (
            "version-3 scalar type hint tag",
            "0,bad,0,0,0,1,0,0,1,0,ffffffffffffffff,17,0,5,",
            "bad scalar type hint",
        ),
        (
            "switch signed-value prefix",
            "0,bad,0,0,0,1,0,0,0,2,0,1,1,",
            "bad signed integer",
        ),
        (
            "switch signed-value digits",
            "0,bad,0,0,0,1,0,0,0,2,0,1,iGG,",
            "bad integer",
        ),
        (
            "terminator tag",
            "0,bad,0,0,0,1,0,0,0,ff,",
            "bad terminator",
        ),
        (
            "handler kind tag",
            "0,bad,0,0,0,1,0,0,0,5,0,0,1,0,ff,",
            "bad handler kind",
        ),
    ]);
}

#[test]
fn text_rejects_missing_trailing_and_malformed_header_fields() {
    assert_payload_errors(&[
        (
            "required payload field is absent",
            "0,bad,",
            "unexpected end",
        ),
        (
            "extra payload atom",
            "0,bad,0,0,0,1,0,0,0,5,0,0,0,0,extra,",
            "trailing input",
        ),
    ]);
    assert_eq!(
        parse("not an IR function"),
        Err(ParseError("expected fn header".into()))
    );
    assert_eq!(
        parse("fn @bad ir4 { }\n"),
        Err(ParseError("unsupported IR text version".into()))
    );
    assert_eq!(
        parse("fn @bad ir3"),
        Err(ParseError("expected fn header".into()))
    );
}

#[test]
fn text_round_trips_fixnum_specialization_metadata() {
    let specialization = FixnumSpecialization::new(FixnumOp::Add, &[true, true]);
    assert!(specialization.is_some());
    let function = finish(
        "checked-add",
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
                    OpKind::TypeHint {
                        value: ValueId(0),
                        ty: crate::ScalarTypeHint::Fixnum,
                    },
                ),
                op(
                    &[(3, Ty::Word)],
                    OpKind::Builtin {
                        name: "+".into(),
                        args: vec![ValueId(2), ValueId(1)],
                        specialization,
                        specialization_disabled: true,
                    },
                ),
            ],
            Terminator::Return {
                values: vec![ValueId(3)],
            },
        )],
        Vec::new(),
    );

    let dump = function.to_string();
    assert!(dump.starts_with("fn @checked-add ir3 { "));
    assert_eq!(parse(&dump), Ok(function));
}

#[test]
fn text_parses_legacy_builtin_dump_without_specialization_metadata() {
    let legacy = concat!(
        "fn @legacy { ",
        "1,legacy,1,x,0,1,0,0,1,0,0,1,1,1,0,ffffffffffffffff,c,_2b,1,0,",
        "5,1,1,0,0,0,",
        " }\n",
    );
    let expected = finish(
        "legacy",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
        Vec::new(),
        vec![block(
            0,
            vec![op(
                &[(1, Ty::Word)],
                OpKind::Builtin {
                    name: "+".into(),
                    args: vec![ValueId(0)],
                    specialization: None,
                    specialization_disabled: false,
                },
            )],
            Terminator::Return {
                values: vec![ValueId(1)],
            },
        )],
        Vec::new(),
    );

    assert_eq!(parse(legacy), Ok(expected));
}

#[test]
fn text_parses_stage_a_v2_builtin_with_default_specialization_policy() {
    let v2 = concat!(
        "fn @legacy ir2 { ",
        "1,legacy,1,x,0,1,0,0,1,0,0,1,1,1,0,ffffffffffffffff,c,_2b,1,0,0,",
        "5,1,1,0,0,0,",
        " }\n",
    );
    let expected = finish(
        "legacy",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
        Vec::new(),
        vec![block(
            0,
            vec![op(
                &[(1, Ty::Word)],
                OpKind::Builtin {
                    name: "+".into(),
                    args: vec![ValueId(0)],
                    specialization: None,
                    specialization_disabled: false,
                },
            )],
            Terminator::Return {
                values: vec![ValueId(1)],
            },
        )],
        Vec::new(),
    );

    assert_eq!(parse(v2), Ok(expected));
}
