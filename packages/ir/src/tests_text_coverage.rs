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
