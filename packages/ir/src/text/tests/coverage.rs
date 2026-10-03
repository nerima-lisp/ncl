#![allow(
    clippy::too_many_lines,
    clippy::unwrap_used,
    reason = "coverage fixtures exercise parser and printer boundaries"
)]

use super::parse;
use crate::{
    BasicBlock, BlockId, BlockParam, Compare, Constant, ConstantIndex, Convert, DebugLocation,
    DebugLocationId, FileId, FormId, Function, FunctionId, HandlerKind, HandlerRegion,
    HandlerRegionId, Local, LocalId, Op, OpKind, Param, Prim, StructureKind, Terminator, Ty,
    ValueId,
};

fn op(kind: OpKind, result: u32, ty: Ty) -> Op {
    Op {
        results: vec![(ValueId(result), ty)],
        kind,
        loc: Some(DebugLocationId(0)),
    }
}

fn all_variants() -> Function {
    let v = ValueId(1);
    let ops = vec![
        op(
            OpKind::Const {
                result: ConstantIndex(0),
            },
            1,
            Ty::Word,
        ),
        op(OpKind::Move { value: v }, 2, Ty::Word),
        op(OpKind::Load { address: v }, 3, Ty::Word),
        op(
            OpKind::Store {
                address: v,
                value: v,
            },
            4,
            Ty::Unit,
        ),
        op(
            OpKind::LoadField {
                object: v,
                field: 2,
            },
            5,
            Ty::Word,
        ),
        op(
            OpKind::StoreField {
                object: v,
                field: 3,
                value: v,
            },
            6,
            Ty::Unit,
        ),
        op(OpKind::Alloc { words: 4 }, 7, Ty::Address),
        op(OpKind::LoadArg { index: 5 }, 8, Ty::Word),
        op(OpKind::LoadCapture { index: 6 }, 9, Ty::Word),
        op(OpKind::LoadFunctionObject, 10, Ty::Word),
        op(
            OpKind::Call {
                function: v,
                args: vec![v],
            },
            11,
            Ty::Word,
        ),
        op(
            OpKind::CallIndirect {
                callee: v,
                args: vec![v],
            },
            12,
            Ty::Word,
        ),
        op(
            OpKind::MakeClosure {
                entry: v,
                captures: vec![v],
            },
            13,
            Ty::Word,
        ),
        op(OpKind::MakeValueCell { value: v }, 14, Ty::Word),
        op(
            OpKind::CallClosure {
                closure: v,
                args: vec![v],
                named_symbol: Some(v),
            },
            15,
            Ty::Word,
        ),
        op(
            OpKind::Builtin {
                name: "x y".into(),
                args: vec![v],
            },
            16,
            Ty::Word,
        ),
        op(
            OpKind::Prim {
                op: Prim::Typep,
                args: vec![v],
                condition: Some(BlockId(2)),
            },
            17,
            Ty::Bool,
        ),
        op(
            OpKind::Compare {
                op: Compare::Ge,
                left: v,
                right: v,
            },
            18,
            Ty::Bool,
        ),
        op(
            OpKind::Convert {
                op: Convert::WordToAddress,
                value: v,
            },
            19,
            Ty::Address,
        ),
        op(OpKind::SetMultipleValues { values: vec![v] }, 20, Ty::Unit),
        op(OpKind::Safepoint, 21, Ty::Unit),
        op(
            OpKind::EnterHandler {
                region: HandlerRegionId(0),
            },
            22,
            Ty::Unit,
        ),
        op(
            OpKind::LeaveHandler {
                region: HandlerRegionId(0),
            },
            23,
            Ty::Unit,
        ),
    ];
    Function {
        id: FunctionId(7),
        name: "name with spaces/ümlaut".into(),
        params: vec![Param {
            name: "arg-name".into(),
            ty: Ty::F64,
        }],
        return_types: vec![Ty::Word, Ty::I64, Ty::F64, Ty::Address, Ty::Bool, Ty::Unit],
        constants: vec![
            Constant::Fixnum(9),
            Constant::Character(0x0010_ffff),
            Constant::SingleFloat(-1.5),
            Constant::DoubleFloat(f64::INFINITY),
            Constant::Symbol {
                package: "PKG name".into(),
                name: "a/b".into(),
            },
            Constant::Object(ConstantIndex(0)),
            Constant::StringBytes(vec![0, b',', 0xff]),
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
                limbs: vec![0, u32::MAX],
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
                    value: ValueId(0),
                    ty: Ty::Word,
                }],
                ops,
                terminator: Terminator::Branch {
                    condition: v,
                    then_target: BlockId(1),
                    then_args: vec![v],
                    else_target: BlockId(2),
                    else_args: vec![],
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: vec![],
                ops: vec![],
                terminator: Terminator::Switch {
                    value: v,
                    cases: vec![(2, BlockId(2), vec![v]), (3, BlockId(3), vec![])],
                    default: BlockId(3),
                    default_args: vec![v],
                },
            },
            BasicBlock {
                id: BlockId(2),
                params: vec![],
                ops: vec![],
                terminator: Terminator::CallReturn {
                    function: v,
                    args: vec![v],
                },
            },
            BasicBlock {
                id: BlockId(3),
                params: vec![],
                ops: vec![],
                terminator: Terminator::Jump {
                    target: BlockId(4),
                    args: vec![v],
                },
            },
            BasicBlock {
                id: BlockId(4),
                params: vec![],
                ops: vec![],
                terminator: Terminator::TailCall {
                    function: v,
                    args: vec![v],
                },
            },
            BasicBlock {
                id: BlockId(5),
                params: vec![],
                ops: vec![],
                terminator: Terminator::Throw { condition: v },
            },
            BasicBlock {
                id: BlockId(6),
                params: vec![],
                ops: vec![],
                terminator: Terminator::Unreachable,
            },
        ],
        locals: vec![Local {
            id: LocalId(3),
            name: "local name".into(),
            ty: Ty::Bool,
        }],
        handler_regions: vec![
            HandlerRegion {
                id: HandlerRegionId(0),
                kind: HandlerKind::Progv,
                protected: vec![BlockId(0), BlockId(1)],
                handler: BlockId(2),
                cleanup: Some(BlockId(3)),
                catch_tag: Some(v),
                binding_targets: vec![v],
                depth: 2,
                parent: Some(HandlerRegionId(1)),
            },
            HandlerRegion {
                id: HandlerRegionId(1),
                kind: HandlerKind::UnwindProtect,
                protected: vec![],
                handler: BlockId(4),
                cleanup: None,
                catch_tag: None,
                binding_targets: vec![],
                depth: 0,
                parent: None,
            },
        ],
        debug: vec![DebugLocation {
            file: FileId(1),
            line: 2,
            column: 3,
            form: FormId(4),
        }],
    }
}

#[test]
fn round_trip_all_printer_and_parser_variants() {
    let mut function = all_variants();
    function.blocks[0]
        .ops
        .retain(|operation| !matches!(operation.kind, OpKind::Convert { .. }));
    let dump = function.to_string();
    assert!(dump.contains("_20"), "escaped punctuation must be encoded");
    assert_eq!(parse(&dump), Ok(function), "dump={dump}");
}

fn minimal(payload: &str) -> String {
    format!("fn @x {{ {payload} }}\n")
}

#[test]
fn parser_reports_structural_and_encoding_errors() {
    let cases = [
        ("", "expected fn header"),
        ("fn @x { 0,x,0,0,0,0,0,0,0,1, }\n", "trailing input"),
        (&minimal("zz"), "bad integer"),
        (&minimal("0,x,0,0,0,0,0,0,0,"), ""),
    ];
    assert_eq!(parse(cases[0].0).unwrap_err().0, cases[0].1);
    assert_eq!(parse(cases[1].0).unwrap_err().0, cases[1].1);
    assert_eq!(parse(cases[2].0).unwrap_err().0, cases[2].1);
    assert_eq!(
        parse(&minimal("0,x,0,0,0,0,0,0,0,")),
        Ok(Function {
            id: FunctionId(0),
            name: "x".into(),
            params: vec![],
            return_types: vec![],
            blocks: vec![],
            locals: vec![],
            constants: vec![],
            handler_regions: vec![],
            debug: vec![],
        })
    );
}

#[test]
fn parser_rejects_unknown_tags_and_bad_escapes() {
    let base = "0,x,0,0,1,";
    for constant in ["f,", "b,", "1,10000000000000000,"] {
        let input = minimal(&format!("{base}{constant}0,0,0,0,"));
        assert!(
            parse(&input).is_err(),
            "constant fixture should fail: {input}"
        );
    }
    let bad_op = minimal("0,x,0,0,0,1,0,0,1,0,0,ff,7,");
    assert_eq!(parse(&bad_op).unwrap_err().0, "bad operation");
    let bad_term = minimal("0,x,0,0,0,1,0,0,0,ff,");
    assert_eq!(parse(&bad_term).unwrap_err().0, "bad terminator");
    let bad_name = minimal("0,_zz,0,0,0,0,0,0,0,");
    assert_eq!(parse(&bad_name).unwrap_err().0, "bad escape");
}
