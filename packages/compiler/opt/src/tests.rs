use super::tests_support::{Fixture, caller, leaf, scalar_caller, scalar_leaf};
use crate::{FunctionPass, GlobalValueNumbering, InlineDirectCalls, Module, PassManager};
use ncl_ir::{
    Constant, ConstantIndex, Convert, FunctionBuilder, FunctionId, Op, OpKind, Param, Prim,
    Terminator, Ty, ValueId,
};

#[test]
fn inline_direct_leaf_and_text_round_trip() {
    let text = leaf().to_string();
    assert_eq!(ncl_ir::parse(&text).fixture().to_string(), text);
    let mut module = Module {
        functions: vec![scalar_caller(), scalar_leaf()],
    };
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
fn inline_recursive_and_unsafe_callees_are_skipped() {
    let mut recursive = scalar_leaf();
    let entry = ConstantIndex(u32::try_from(recursive.constants.len()).fixture());
    recursive
        .constants
        .push(Constant::FunctionEntry(FunctionId(12)));
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
        functions: vec![scalar_caller(), recursive],
    };
    module.functions[0].blocks[0].ops.insert(
        3,
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
fn inline_unknown_word_arguments_are_skipped() {
    let mut module = Module {
        functions: vec![caller(), leaf()],
    };
    let before = module.functions[0].clone();
    let mut pass = InlineDirectCalls::default();
    let module_view = module.clone();
    assert!(!pass.run(&mut module.functions[0], &module_view).fixture());
    assert_eq!(module.functions[0], before);
}

#[test]
fn inline_proven_fixnum_word_arguments_are_accepted() {
    let mut builder =
        FunctionBuilder::new(FunctionId(20), "fixnum_word_caller", vec![], vec![Ty::Word]);
    let fixnum = builder.add_constant(Constant::Fixnum(7));
    let tagged = builder
        .push_op(OpKind::Const { result: fixnum }, &[Ty::I64])
        .fixture()[0];
    let word = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: tagged,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let entry = builder.add_constant(Constant::FunctionEntry(FunctionId(2)));
    let function = builder
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .fixture()[0];
    let result = builder
        .push_op(
            OpKind::Call {
                function,
                args: vec![word],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .fixture();
    let mut module = Module {
        functions: vec![builder.finish(), leaf()],
    };
    let mut pass = InlineDirectCalls::default();
    let module_view = module.clone();
    assert!(pass.run(&mut module.functions[0], &module_view).fixture());
    assert!(
        module.functions[0].blocks[0]
            .ops
            .iter()
            .all(|op| !matches!(op.kind, OpKind::Call { .. }))
    );
}

#[test]
fn inline_safepoint_and_allocation_callees_are_skipped() {
    for add_allocation in [false, true] {
        let mut callee = scalar_leaf();
        if add_allocation {
            callee.blocks[0].ops.push(Op {
                results: vec![(ValueId(2), Ty::Address)],
                kind: OpKind::Alloc { words: 1 },
                loc: None,
            });
        } else {
            callee.blocks[0].ops.push(Op {
                results: vec![],
                kind: OpKind::Safepoint,
                loc: None,
            });
        }
        let mut module = Module {
            functions: vec![scalar_caller(), callee],
        };
        let before = module.functions[0].clone();
        let mut pass = InlineDirectCalls::default();
        let module_view = module.clone();
        assert!(!pass.run(&mut module.functions[0], &module_view).fixture());
        assert_eq!(module.functions[0], before);
    }
}

#[test]
fn inline_address_conversion_callee_is_skipped() {
    let mut unsafe_leaf = FunctionBuilder::new(
        FunctionId(40),
        "addressconversionleaf",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let address = unsafe_leaf
        .push_op(
            OpKind::Convert {
                op: Convert::WordToAddress,
                value: ValueId(0),
            },
            &[Ty::Address],
        )
        .fixture()[0];
    let value = unsafe_leaf
        .push_op(
            OpKind::Convert {
                op: Convert::AddressToWord,
                value: address,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    unsafe_leaf
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .fixture();

    let mut caller_entry = FunctionBuilder::new(
        FunctionId(41),
        "addressconversioncaller",
        vec![],
        vec![Ty::Word],
    );
    let constant = caller_entry.add_constant(Constant::Fixnum(7));
    let fixnum = caller_entry
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .fixture()[0];
    let word = caller_entry
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: fixnum,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let entry = caller_entry.add_constant(Constant::FunctionEntry(FunctionId(40)));
    let function = caller_entry
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .fixture()[0];
    let result = caller_entry
        .push_op(
            OpKind::Call {
                function,
                args: vec![word],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    caller_entry
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .fixture();

    let mut module = Module {
        functions: vec![caller_entry.finish(), unsafe_leaf.finish()],
    };
    let before = module.functions[0].clone();
    let mut pass = InlineDirectCalls::default();
    let module_view = module.clone();
    assert!(!pass.run(&mut module.functions[0], &module_view).fixture());
    assert_eq!(module.functions[0], before);
}

#[test]
fn inlining_then_gvn_only_eliminates_duplicate_scalar_operations() {
    let mut scalar_target = FunctionBuilder::new(
        FunctionId(30),
        "duplicateScalarLeaf",
        vec![Param {
            name: "x".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let unboxed = scalar_target
        .push_op(
            OpKind::Convert {
                op: Convert::WordToI64,
                value: ValueId(0),
            },
            &[Ty::I64],
        )
        .fixture()[0];
    scalar_target
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![unboxed, unboxed],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    scalar_target
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![unboxed, unboxed],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    let returned = scalar_target
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    scalar_target
        .terminate(Terminator::Return {
            values: vec![returned],
        })
        .fixture();

    let mut scalar_entry =
        FunctionBuilder::new(FunctionId(31), "scalarCaller", vec![], vec![Ty::Word]);
    let fixnum = scalar_entry.add_constant(Constant::Fixnum(7));
    let value = scalar_entry
        .push_op(OpKind::Const { result: fixnum }, &[Ty::I64])
        .fixture()[0];
    let argument = scalar_entry
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let entry = scalar_entry.add_constant(Constant::FunctionEntry(FunctionId(30)));
    let function = scalar_entry
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .fixture()[0];
    let result = scalar_entry
        .push_op(
            OpKind::Call {
                function,
                args: vec![argument],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    scalar_entry
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .fixture();

    let mut module = Module {
        functions: vec![scalar_entry.finish(), scalar_target.finish()],
    };
    let mut manager = PassManager::new();
    manager.add_function_pass(InlineDirectCalls::default());
    manager.add_function_pass(GlobalValueNumbering);
    manager.run(&mut module).fixture();
    let ops = &module.functions[0].blocks[0].ops;
    assert!(ops.iter().all(|op| !matches!(op.kind, OpKind::Call { .. })));
    assert_eq!(
        ops.iter()
            .filter(|op| matches!(op.kind, OpKind::Prim { .. }))
            .count(),
        1
    );
    module.verify().fixture();
}

#[test]
fn inline_threshold_skips() {
    let mut module = Module {
        functions: vec![scalar_caller(), scalar_leaf()],
    };
    module.functions[0].blocks[0].ops.insert(
        3,
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
