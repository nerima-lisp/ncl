#![allow(clippy::expect_used, clippy::too_many_lines)]

use super::tests_text::{block, finish, op};
use crate::*;

fn parse_error(input: &str) -> String {
    parse(input)
        .expect_err("coverage fixture must be rejected")
        .to_string()
}

fn minimal() -> Function {
    finish(
        "edge",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![block(0, Vec::new(), Terminator::Unreachable)],
        Vec::new(),
    )
}

#[test]
fn parser_rejects_signed_integer_and_escape_edges() {
    let minimum = finish(
        "edge",
        Vec::new(),
        Vec::new(),
        vec![Constant::Fixnum(i64::MIN)],
        vec![block(0, Vec::new(), Terminator::Unreachable)],
        Vec::new(),
    );
    let printed = minimum.to_string();
    assert_eq!(parse(&printed), Ok(minimum.clone()));
    let explicit_negative = printed.replace("i8000000000000000", "i-8000000000000000");
    assert_eq!(parse(&explicit_negative), Ok(minimum));
    assert_eq!(
        parse_error(&printed.replace("i8000000000000000", "i-8000000000000001")),
        "integer out of range"
    );
    assert_eq!(
        parse(&printed.replace("i8000000000000000", "i-1"))
            .expect("negative signed integer should parse")
            .constants,
        vec![Constant::Fixnum(-1)]
    );

    let escaped = minimal().to_string().replace(",edge,", ",edge_,");
    assert_eq!(parse_error(&escaped), "bad escape");
}

#[test]
fn parser_reports_signed_integer_format_errors() {
    let function = finish(
        "signed",
        Vec::new(),
        Vec::new(),
        vec![Constant::Fixnum(1)],
        vec![block(0, Vec::new(), Terminator::Unreachable)],
        Vec::new(),
    );
    let printed = function.to_string();

    assert_eq!(
        parse(&printed.replace(",i1,", ",1,")),
        Err(ParseError("bad signed integer".into()))
    );
    assert_eq!(
        parse(&printed.replace(",i1,", ",i-zz,")),
        Err(ParseError("bad integer".into()))
    );
}

#[test]
fn parser_rejects_descriptor_and_debug_edges() {
    let comparison = finish(
        "compare",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            vec![op(
                &[],
                OpKind::Compare {
                    op: Compare::Eq,
                    left: ValueId(0),
                    right: ValueId(0),
                },
            )],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );
    assert_eq!(
        parse_error(&comparison.to_string().replace(",Eq,", ",Nope,")),
        "bad comparison"
    );

    let conversion = finish(
        "convert",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
        Vec::new(),
        vec![block(
            0,
            vec![op(
                &[],
                OpKind::Convert {
                    op: Convert::WordToI64,
                    value: ValueId(0),
                },
            )],
            Terminator::Return { values: Vec::new() },
        )],
        Vec::new(),
    );
    assert_eq!(
        parse_error(&conversion.to_string().replace(",WordToI64,", ",Nope,")),
        "bad conversion"
    );

    let debug = Function {
        debug: vec![DebugLocation {
            file: FileId(0),
            line: 1,
            column: 2,
            form: FormId(3),
        }],
        ..minimal()
    };
    assert_eq!(parse(&debug.to_string()), Ok(debug.clone()));
    assert_eq!(
        parse_error(&debug.to_string().replace(",0,1,2,3,", ",0,100000000,2,3,")),
        "integer out of range"
    );

    let located = Function {
        blocks: vec![block(
            0,
            vec![Op {
                results: Vec::new(),
                loc: Some(DebugLocationId(0)),
                kind: OpKind::Safepoint,
            }],
            Terminator::Unreachable,
        )],
        ..minimal()
    };
    assert_eq!(
        parse_error(&located.to_string().replace(",0,11,", ",100000000,11,")),
        "integer out of range"
    );

    let handler = Function {
        handler_regions: vec![HandlerRegion {
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
        ..minimal()
    };
    assert_eq!(
        parse_error(
            &handler
                .to_string()
                .replace(",0,0,1,0,0,0,0,", ",0,9,1,0,0,0,0,0,")
        ),
        "bad handler kind"
    );
}

#[test]
fn parser_reports_structural_and_descriptor_errors() {
    assert_eq!(parse_error("not an ir function"), "expected fn header");
    let minimal_text = minimal().to_string();
    assert_eq!(
        parse_error(&format!("{minimal_text}trailing")),
        "trailing input"
    );

    let typed = finish(
        "typed",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
        Vec::new(),
        vec![block(0, Vec::new(), Terminator::Unreachable)],
        Vec::new(),
    );
    let typed_text = typed.to_string();
    assert_eq!(
        parse_error(&typed_text.replacen(",0,", ",9,", 1)),
        "bad type"
    );

    let op = finish(
        "op",
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
                OpKind::Builtin {
                    name: "builtin".into(),
                    args: vec![ValueId(0)],
                },
            )],
            Terminator::Return {
                values: vec![ValueId(1)],
            },
        )],
        Vec::new(),
    );
    let op_text = op.to_string();
    assert_eq!(
        parse_error(&op_text.replacen(",c,builtin,", ",ff,builtin,", 1)),
        "bad operation"
    );
    assert_eq!(
        parse_error(&op_text.replacen(",c,builtin,", ",d,NoSuchPrimitive,", 1)),
        "bad primitive"
    );
}

#[test]
fn parser_round_trips_all_string_dispatch_variants() {
    let mut ops = [
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
                args: vec![ValueId(0)],
                condition: None,
            },
        )
    })
    .collect::<Vec<_>>();
    ops.extend(
        [
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
                &[],
                OpKind::Compare {
                    op: comparison,
                    left: ValueId(0),
                    right: ValueId(0),
                },
            )
        }),
    );
    ops.extend(
        [
            Convert::WordToI64,
            Convert::I64ToWord,
            Convert::WordToF64,
            Convert::F64ToWord,
            Convert::AddressToWord,
            Convert::WordToAddress,
        ]
        .into_iter()
        .map(|conversion| {
            op(
                &[],
                OpKind::Convert {
                    op: conversion,
                    value: ValueId(0),
                },
            )
        }),
    );

    let function = finish(
        "dispatch-variants",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
        Vec::new(),
        vec![block(0, ops, Terminator::Return { values: Vec::new() })],
        Vec::new(),
    );
    assert_eq!(parse(&function.to_string()), Ok(function));
}

#[test]
fn parser_round_trips_constant_and_terminator_variants() {
    let constants = vec![
        Constant::Fixnum(-7),
        Constant::Character('x' as u32),
        Constant::SingleFloat(1.5),
        Constant::DoubleFloat(-2.5),
        Constant::Symbol {
            package: "CL".into(),
            name: "VALUE".into(),
        },
        Constant::Object(ConstantIndex(0)),
        Constant::StringBytes(vec![0, 1, 255]),
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
        Constant::FunctionEntry(FunctionId(9)),
        Constant::Bignum {
            negative: true,
            limbs: vec![1, 2, 3],
        },
        Constant::Ratio {
            numerator: ConstantIndex(0),
            denominator: ConstantIndex(1),
        },
        Constant::Complex {
            real: ConstantIndex(2),
            imaginary: ConstantIndex(3),
        },
    ];
    let blocks = vec![
        block(
            0,
            Vec::new(),
            Terminator::Jump {
                target: BlockId(1),
                args: vec![ValueId(0)],
            },
        ),
        block(
            1,
            Vec::new(),
            Terminator::Branch {
                condition: ValueId(0),
                then_target: BlockId(2),
                then_args: vec![ValueId(1)],
                else_target: BlockId(3),
                else_args: Vec::new(),
            },
        ),
        block(
            2,
            Vec::new(),
            Terminator::Switch {
                value: ValueId(0),
                cases: vec![(1, BlockId(3), vec![ValueId(2)])],
                default: BlockId(4),
                default_args: Vec::new(),
            },
        ),
        block(
            3,
            Vec::new(),
            Terminator::CallReturn {
                function: ValueId(0),
                args: vec![ValueId(1)],
            },
        ),
        block(
            4,
            Vec::new(),
            Terminator::TailCall {
                function: ValueId(0),
                args: Vec::new(),
            },
        ),
        block(
            5,
            Vec::new(),
            Terminator::Throw {
                condition: ValueId(0),
            },
        ),
        block(6, Vec::new(), Terminator::Unreachable),
    ];
    let function = finish(
        "constants-and-terms",
        Vec::new(),
        Vec::new(),
        constants,
        blocks,
        vec![],
    );
    assert_eq!(parse(&function.to_string()), Ok(function));
}

#[test]
fn parser_preserves_optional_fields_and_boundary_values() {
    let function = Function {
        id: FunctionId(u32::MAX),
        name: "name,with space".into(),
        params: vec![Param {
            name: "parameter".into(),
            ty: Ty::Unit,
        }],
        return_types: vec![Ty::Bool],
        constants: vec![Constant::StringBytes(vec![0, 0xff])],
        blocks: vec![BasicBlock {
            id: BlockId(u32::MAX),
            params: vec![BlockParam {
                value: ValueId(u32::MAX),
                ty: Ty::Address,
            }],
            ops: vec![Op {
                results: vec![(ValueId(u32::MAX), Ty::Word)],
                loc: Some(DebugLocationId(u32::MAX)),
                kind: OpKind::CallClosure {
                    closure: ValueId(u32::MAX),
                    named_symbol: Some(ValueId(u32::MAX)),
                    args: vec![ValueId(u32::MAX)],
                },
            }],
            terminator: Terminator::Return {
                values: vec![ValueId(u32::MAX)],
            },
        }],
        locals: vec![Local {
            id: LocalId(u32::MAX),
            name: "local".into(),
            ty: Ty::F64,
        }],
        handler_regions: vec![HandlerRegion {
            id: HandlerRegionId(u32::MAX),
            kind: HandlerKind::Catch,
            protected: vec![BlockId(u32::MAX)],
            handler: BlockId(u32::MAX),
            cleanup: Some(BlockId(u32::MAX)),
            catch_tag: Some(ValueId(u32::MAX)),
            binding_targets: vec![ValueId(u32::MAX)],
            depth: u32::MAX,
            parent: Some(HandlerRegionId(u32::MAX)),
        }],
        debug: vec![DebugLocation {
            file: FileId(u32::MAX),
            line: u32::MAX,
            column: u32::MAX,
            form: FormId(u32::MAX),
        }],
    };

    let parsed = parse(&function.to_string()).expect("boundary fixture should parse");
    assert_eq!(parsed, function);
}

#[test]
fn parser_reports_capture_index_overflow_and_invalid_utf8() {
    let capture_overflow = "fn @edge { 0,edge,0,0,0,1,0,0,1,0,ffffffffffffffff,14,100,7,0,0,0, }\n";
    assert_eq!(parse_error(capture_overflow), "integer out of range");

    let invalid_utf8 = minimal().to_string().replace(",edge,", ",edge_ff,");
    assert_eq!(parse_error(&invalid_utf8), "invalid utf8");
}
