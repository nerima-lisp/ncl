use super::*;
use ncl_ir::{BasicBlock, Compare, FunctionId, Op, Prim, Ty};

fn op(kind: OpKind) -> Op {
    Op {
        results: vec![(ValueId(2), Ty::Word)],
        kind,
        loc: None,
    }
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "table-driven coverage fixture exercises private expression cases"
)]
fn covers_tables_expression_classification_and_control_flow_shapes() {
    let mut tables = Tables::default();
    tables.replacements.insert(ValueId(1), ValueId(2));
    tables.replacements.insert(ValueId(2), ValueId(1));
    assert_eq!(tables.resolve(ValueId(1)), ValueId(1));
    tables.invalidate_loads();
    assert!(tables.loads.is_empty());

    let kinds = vec![
        OpKind::Const {
            result: ncl_ir::ConstantIndex(0),
        },
        OpKind::Move { value: ValueId(0) },
        OpKind::Load {
            address: ValueId(0),
        },
        OpKind::LoadField {
            object: ValueId(0),
            field: 1,
        },
        OpKind::Prim {
            op: Prim::FixnumAdd,
            args: vec![ValueId(0)],
            condition: None,
        },
        OpKind::Prim {
            op: Prim::Car,
            args: vec![ValueId(0)],
            condition: Some(BlockId(1)),
        },
        OpKind::Compare {
            op: Compare::Eq,
            left: ValueId(0),
            right: ValueId(1),
        },
        OpKind::Convert {
            op: ncl_ir::Convert::I64ToWord,
            value: ValueId(0),
        },
        OpKind::Store {
            address: ValueId(0),
            value: ValueId(1),
        },
        OpKind::Safepoint,
    ];
    let keys = kinds
        .iter()
        .map(|kind| GlobalValueNumbering::expression(&op(kind.clone())).is_some())
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![true, true, true, true, true, true, true, true, false, false]
    );
    assert!(GlobalValueNumbering::is_load(&OpKind::Load {
        address: ValueId(0)
    }));
    assert!(GlobalValueNumbering::is_load(&OpKind::Prim {
        op: Prim::Car,
        args: vec![],
        condition: None
    }));
    assert!(!GlobalValueNumbering::is_load(&OpKind::Prim {
        op: Prim::FixnumAdd,
        args: vec![],
        condition: None
    }));
    assert!(GlobalValueNumbering::invalidates_memory(&OpKind::Store {
        address: ValueId(0),
        value: ValueId(1)
    }));
    assert!(GlobalValueNumbering::invalidates_memory(&OpKind::Prim {
        op: Prim::Rplaca,
        args: vec![],
        condition: None
    }));
    assert!(!GlobalValueNumbering::invalidates_memory(
        &OpKind::Safepoint
    ));

    let terms = [
        Terminator::Jump {
            target: BlockId(1),
            args: vec![],
        },
        Terminator::Branch {
            condition: ValueId(0),
            then_target: BlockId(1),
            then_args: vec![],
            else_target: BlockId(2),
            else_args: vec![],
        },
        Terminator::Switch {
            value: ValueId(0),
            cases: vec![(1, BlockId(1), vec![])],
            default: BlockId(2),
            default_args: vec![],
        },
        Terminator::CallReturn {
            function: ValueId(0),
            args: vec![],
        },
        Terminator::TailCall {
            function: ValueId(0),
            args: vec![],
        },
        Terminator::Return { values: vec![] },
        Terminator::Throw {
            condition: ValueId(0),
        },
        Terminator::Unreachable,
    ];
    assert_eq!(
        terms
            .iter()
            .map(GlobalValueNumbering::successors)
            .map(|successors| successors.len())
            .collect::<Vec<_>>(),
        vec![1, 2, 2, 0, 0, 0, 0, 0]
    );

    let empty = Function {
        id: FunctionId(1),
        name: "empty".into(),
        params: vec![],
        return_types: vec![],
        blocks: vec![],
        locals: vec![],
        constants: vec![],
        handler_regions: vec![],
        debug: vec![],
    };
    assert!(GlobalValueNumbering::dominator_tree(&empty).is_empty());
    let disconnected = BasicBlock {
        id: BlockId(4),
        params: vec![],
        ops: vec![],
        terminator: Terminator::Return { values: vec![] },
    };
    let mut disconnected_fn = empty;
    disconnected_fn.blocks = vec![
        BasicBlock {
            id: BlockId(0),
            params: vec![],
            ops: vec![],
            terminator: Terminator::Return { values: vec![] },
        },
        disconnected,
    ];
    assert_eq!(
        GlobalValueNumbering::dominator_tree(&disconnected_fn).len(),
        2
    );
}
