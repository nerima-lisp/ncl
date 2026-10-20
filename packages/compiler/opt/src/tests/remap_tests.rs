use super::tests_support::leaf;
use crate::remap::{next_value, remap_kind, remap_op_values, remap_term_values};
use ncl_ir::{Constant, ConstantIndex, Op, OpKind, StructureKind, Terminator, ValueId};
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
            named_symbol: None,
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

#[test]
fn remap_kind_rewrites_all_value_bearing_call_shapes() {
    let values = HashMap::from([
        (ValueId(0), ValueId(9)),
        (ValueId(1), ValueId(10)),
        (ValueId(2), ValueId(11)),
    ]);
    let mut constants = HashMap::new();
    let mut caller_constants = Vec::new();
    let callee = leaf();
    let mut remap = |kind| {
        remap_kind(
            &kind,
            &values,
            &mut constants,
            &mut caller_constants,
            &callee,
        )
    };

    assert_eq!(
        remap(OpKind::Call {
            function: ValueId(0),
            args: vec![ValueId(1)],
        }),
        OpKind::Call {
            function: ValueId(9),
            args: vec![ValueId(10)],
        }
    );
    assert_eq!(
        remap(OpKind::CallIndirect {
            callee: ValueId(0),
            args: vec![ValueId(1)],
        }),
        OpKind::CallIndirect {
            callee: ValueId(9),
            args: vec![ValueId(10)],
        }
    );
    assert_eq!(
        remap(OpKind::MakeClosure {
            entry: ValueId(0),
            captures: vec![ValueId(1), ValueId(2)],
        }),
        OpKind::MakeClosure {
            entry: ValueId(9),
            captures: vec![ValueId(10), ValueId(11)],
        }
    );
    assert_eq!(
        remap(OpKind::CallClosure {
            closure: ValueId(0),
            args: vec![ValueId(1)],
            named_symbol: Some(ValueId(2)),
        }),
        OpKind::CallClosure {
            closure: ValueId(9),
            args: vec![ValueId(10)],
            named_symbol: Some(ValueId(11)),
        }
    );
    assert_eq!(
        remap(OpKind::SetMultipleValues {
            values: vec![ValueId(0), ValueId(1)],
        }),
        OpKind::SetMultipleValues {
            values: vec![ValueId(9), ValueId(10)],
        }
    );
}

#[test]
fn remap_kind_copies_nested_constants_once_and_preserves_invalid_indices() {
    let mut callee = leaf();
    callee.constants = vec![
        Constant::Object(ConstantIndex(1)),
        Constant::Structure {
            kind: StructureKind::Cons,
            elements: vec![ConstantIndex(2)],
        },
        Constant::Ratio {
            numerator: ConstantIndex(3),
            denominator: ConstantIndex(4),
        },
        Constant::Complex {
            real: ConstantIndex(4),
            imaginary: ConstantIndex(4),
        },
        Constant::Fixnum(7),
        Constant::Object(ConstantIndex(99)),
    ];
    let mut constants = HashMap::new();
    let mut caller_constants = Vec::new();

    let first = remap_kind(
        &OpKind::Const {
            result: ConstantIndex(0),
        },
        &HashMap::new(),
        &mut constants,
        &mut caller_constants,
        &callee,
    );
    assert_eq!(
        first,
        OpKind::Const {
            result: ConstantIndex(0)
        }
    );
    assert_eq!(
        caller_constants,
        vec![
            Constant::Object(ConstantIndex(1)),
            Constant::Structure {
                kind: StructureKind::Cons,
                elements: vec![ConstantIndex(2)],
            },
            Constant::Ratio {
                numerator: ConstantIndex(3),
                denominator: ConstantIndex(4),
            },
            Constant::Complex {
                real: ConstantIndex(4),
                imaginary: ConstantIndex(4),
            },
            Constant::Fixnum(7),
        ]
    );
    assert_eq!(
        remap_kind(
            &OpKind::Const {
                result: ConstantIndex(0),
            },
            &HashMap::new(),
            &mut constants,
            &mut caller_constants,
            &callee,
        ),
        first
    );

    assert_eq!(
        remap_kind(
            &OpKind::Const {
                result: ConstantIndex(5),
            },
            &HashMap::new(),
            &mut constants,
            &mut caller_constants,
            &callee,
        ),
        OpKind::Const {
            result: ConstantIndex(5)
        }
    );
    assert_eq!(
        remap_kind(
            &OpKind::Const {
                result: ConstantIndex(99),
            },
            &HashMap::new(),
            &mut constants,
            &mut caller_constants,
            &callee,
        ),
        OpKind::Const {
            result: ConstantIndex(99)
        }
    );
}

#[test]
fn remap_op_values_rewrites_named_symbols_and_captured_values() {
    let replacements = HashMap::from([
        (ValueId(1), ValueId(11)),
        (ValueId(2), ValueId(12)),
        (ValueId(3), ValueId(13)),
    ]);
    let mut op = Op {
        results: vec![],
        kind: OpKind::CallClosure {
            closure: ValueId(1),
            args: vec![ValueId(2)],
            named_symbol: Some(ValueId(3)),
        },
        loc: None,
    };

    remap_op_values(&mut op, &replacements);

    assert_eq!(
        op.kind,
        OpKind::CallClosure {
            closure: ValueId(11),
            args: vec![ValueId(12)],
            named_symbol: Some(ValueId(13)),
        }
    );
}
