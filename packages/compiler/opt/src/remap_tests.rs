use super::tests_support::leaf;
use crate::remap::{next_value, remap_kind, remap_op_values, remap_term_values};
use ncl_ir::{Constant, ConstantIndex, Op, OpKind, Terminator, ValueId};
use std::collections::HashMap;

#[test]
#[allow(clippy::too_many_lines)]
fn remap_helpers_cover_ir_shapes() {
    let values = HashMap::from([(ValueId(0), ValueId(9))]);
    let mut constants = HashMap::new();
    let mut caller_constants = Vec::new();
    let mut callee = leaf();
    callee.constants.push(Constant::Nil);
    let kinds = vec![
        OpKind::Const {
            result: ConstantIndex(0),
        },
        OpKind::Move { value: ValueId(0) },
        OpKind::Load {
            address: ValueId(0),
        },
        OpKind::Store {
            address: ValueId(0),
            value: ValueId(0),
        },
        OpKind::LoadField {
            object: ValueId(0),
            field: 1,
        },
        OpKind::StoreField {
            object: ValueId(0),
            field: 1,
            value: ValueId(0),
        },
        OpKind::Alloc { words: 1 },
        OpKind::LoadArg { index: 0 },
        OpKind::Builtin {
            name: "x".into(),
            args: vec![ValueId(0)],
        },
        OpKind::Prim {
            op: ncl_ir::Prim::Car,
            args: vec![ValueId(0)],
            condition: None,
        },
        OpKind::Compare {
            op: ncl_ir::Compare::Eq,
            left: ValueId(0),
            right: ValueId(0),
        },
        OpKind::Convert {
            op: ncl_ir::Convert::I64ToWord,
            value: ValueId(0),
        },
        OpKind::Call {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        OpKind::CallIndirect {
            callee: ValueId(0),
            args: vec![ValueId(0)],
        },
        OpKind::MakeClosure {
            entry: ValueId(0),
            captures: vec![ValueId(0)],
        },
        OpKind::CallClosure {
            closure: ValueId(0),
            args: vec![ValueId(0)],
        },
        OpKind::SetMultipleValues {
            values: vec![ValueId(0)],
        },
        OpKind::Safepoint,
        OpKind::EnterHandler {
            region: ncl_ir::HandlerRegionId(0),
        },
        OpKind::LeaveHandler {
            region: ncl_ir::HandlerRegionId(0),
        },
    ];
    for kind in kinds {
        let _ = remap_kind(
            &kind,
            &values,
            &mut constants,
            &mut caller_constants,
            &callee,
        );
        remap_op_values(
            &mut Op {
                results: vec![],
                kind,
                loc: None,
            },
            &values,
        );
    }
    let mut terms = vec![
        Terminator::Jump {
            target: ncl_ir::BlockId(1),
            args: vec![ValueId(0)],
        },
        Terminator::Branch {
            condition: ValueId(0),
            then_target: ncl_ir::BlockId(1),
            then_args: vec![ValueId(0)],
            else_target: ncl_ir::BlockId(2),
            else_args: vec![ValueId(0)],
        },
        Terminator::Switch {
            value: ValueId(0),
            cases: vec![(1, ncl_ir::BlockId(1), vec![ValueId(0)])],
            default: ncl_ir::BlockId(2),
            default_args: vec![ValueId(0)],
        },
        Terminator::CallReturn {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        Terminator::TailCall {
            function: ValueId(0),
            args: vec![ValueId(0)],
        },
        Terminator::Return {
            values: vec![ValueId(0)],
        },
        Terminator::Throw {
            condition: ValueId(0),
        },
        Terminator::Unreachable,
    ];
    for term in &mut terms {
        remap_term_values(term, &values);
    }
    assert_eq!(next_value(&callee), 2);
}
