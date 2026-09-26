use crate::{
    InlineDirectCalls, Module, PassManager,
};
use super::tests_support::{caller, leaf, Fixture};
use ncl_ir::{Constant, ConstantIndex, FunctionId, Op, OpKind, Terminator, Ty, ValueId};

#[test]
fn direct_leaf_inlines_and_text_round_trips() {
    let mut module = Module {
        functions: vec![caller(), leaf()],
    };
    let text = module.functions[0].to_string();
    assert_eq!(ncl_ir::parse(&text).fixture().to_string(), text);
    let mut manager = PassManager::new();
    manager.add_function_pass(InlineDirectCalls::default());
    let report = manager.run(&mut module).fixture();
    assert!(report.stats.iter().any(|stat| stat.changed));
    assert!(
        module.functions[0].blocks[0]
            .ops
            .iter()
            .all(|op| !matches!(op.kind, OpKind::Call { .. }))
    );
    module.verify().fixture();
}

#[test]
fn recursive_and_unsafe_callees_are_skipped() {
    let mut recursive = leaf();
    recursive.id = FunctionId(2);
    let entry = ConstantIndex(u32::try_from(recursive.constants.len()).fixture());
    recursive
        .constants
        .push(Constant::FunctionEntry(FunctionId(2)));
    recursive.blocks[0].ops.insert(
        0,
        Op {
            results: vec![(ValueId(2), Ty::Word)],
            kind: OpKind::Const { result: entry },
            loc: None,
        },
    );
    recursive.blocks[0].ops.push(Op {
        results: vec![],
        kind: OpKind::Safepoint,
        loc: None,
    });
    recursive.blocks[0].ops.push(Op {
        results: vec![(ValueId(3), Ty::Word)],
        kind: OpKind::Call {
            function: ValueId(2),
            args: vec![ValueId(0)],
        },
        loc: None,
    });
    recursive.blocks[0].terminator = Terminator::Return {
        values: vec![ValueId(3)],
    };
    let mut module = Module {
        functions: vec![caller(), recursive],
    };
    module.functions[0].blocks[0].ops.insert(
        1,
        Op {
            results: vec![],
            kind: OpKind::Safepoint,
            loc: None,
        },
    );
    let before = module.functions[0].clone();
    let mut manager = PassManager::new();
    manager.add_function_pass(InlineDirectCalls::default());
    manager.run(&mut module).fixture();
    assert_eq!(module.functions[0], before);
}

#[test]
fn threshold_skips() {
    let mut module = Module {
        functions: vec![caller(), leaf()],
    };
    module.functions[0].blocks[0].ops.insert(
        1,
        Op {
            results: vec![],
            kind: OpKind::Safepoint,
            loc: None,
        },
    );
    let mut manager = PassManager::new();
    manager.add_function_pass(InlineDirectCalls { max_ops: 0 });
    manager.run(&mut module).fixture();
    assert!(
        module.functions[0].blocks[0]
            .ops
            .iter()
            .any(|op| matches!(op.kind, OpKind::Call { .. }))
    );
}
