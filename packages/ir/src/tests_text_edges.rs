#![allow(clippy::expect_used)]

use super::*;

fn input(payload: &str) -> String {
    format!("fn @edge {{ {payload} }}\n")
}

fn error(payload: &str) -> String {
    parse(&input(payload))
        .expect_err("the hand-written payload must be rejected")
        .to_string()
}

fn minimal_function(with_op: bool) -> String {
    let ops = if with_op {
        vec![Op {
            results: Vec::new(),
            kind: OpKind::Const {
                result: ConstantIndex(0),
            },
            loc: None,
        }]
    } else {
        Vec::new()
    };
    Function {
        id: FunctionId(0),
        name: "edge".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        constants: vec![Constant::Nil],
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            ops,
            terminator: Terminator::Unreachable,
        }],
        locals: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
    .to_string()
}

#[test]
fn parser_reports_bad_tags_and_descriptors() {
    assert_eq!(error("0,edge,1,arg,9,0,0,0,0,"), "bad type");
    assert_eq!(error("0,edge,0,0,1,99,0,0,0,"), "bad constant");
    let bad_operation = minimal_function(true).replace("0,0,7,", "99,7,");
    assert_eq!(
        parse(&bad_operation).expect_err("bad op tag").to_string(),
        "bad operation"
    );
    let mut bad_terminator = minimal_function(false);
    let terminator = bad_terminator.rfind("7,").expect("unreachable terminator");
    bad_terminator.replace_range(terminator..terminator + 2, "99,");
    assert_eq!(
        parse(&bad_terminator)
            .expect_err("bad terminator tag")
            .to_string(),
        "bad terminator"
    );
    assert_eq!(error("0,edge,0,0,1,b,99,"), "bad structure kind");
}

#[test]
fn parser_rejects_bad_names_and_integer_ranges() {
    assert_eq!(error("0,edge_zz,0,0,0,0,0,0,0,"), "bad escape");
    assert_eq!(error("0,edge_ff,0,0,0,0,0,0,0,"), "invalid utf8");
    assert_eq!(
        error("100000000,edge,0,0,0,0,0,0,0,"),
        "integer out of range"
    );
    assert_eq!(
        error("0,edge,0,0,0,1,0,0,0,0,0,7,100,ffffffffffffffff,7,0,1,100,"),
        "integer out of range"
    );
}

#[test]
fn parser_accepts_signed_extremes_and_printer_escapes_output() {
    let function = Function {
        id: FunctionId(0),
        name: "name with spaces".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        constants: vec![Constant::Fixnum(i64::MIN), Constant::Fixnum(i64::MAX)],
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            ops: Vec::new(),
            terminator: Terminator::Unreachable,
        }],
        locals: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let printed = function.to_string();
    assert!(printed.starts_with("fn @name_20with_20spaces { "));
    assert_eq!(parse(&printed), Ok(function));
}
