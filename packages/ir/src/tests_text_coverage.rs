#![allow(clippy::expect_used)]

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
    assert_eq!(parse(&printed), Ok(minimum));
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

    let escaped = minimal().to_string().replace("fn @edge ", "fn @edge_ ");
    assert_eq!(parse_error(&escaped), "bad escape");
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
