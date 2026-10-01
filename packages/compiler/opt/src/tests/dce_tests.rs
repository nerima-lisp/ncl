use super::tests_support::Fixture;
use crate::{DeadCodeElimination, FunctionPass, Module};
use ncl_ir::{
    BasicBlock, Constant, ConstantIndex, Function, FunctionBuilder, FunctionId, HandlerKind,
    HandlerRegion, Op, OpKind, Param, Terminator, Ty, ValueId,
};

#[test]
fn removes_unused_pure_operation_and_preserves_return_value() {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "dce",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let unused = builder
        .push_op(ncl_ir::OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .ok()
        .and_then(|values| values.into_iter().next());
    assert!(unused.is_some());
    let terminated = builder.terminate(Terminator::Return {
        values: vec![ValueId(0)],
    });
    assert!(terminated.is_ok());
    let mut function = builder.finish();
    let mut pass = DeadCodeElimination;
    let result = pass.run(&mut function, &Module::default());
    assert!(result.is_ok());
    assert!(result.unwrap_or(false));
    assert!(function.blocks[0].ops.is_empty());
}

#[test]
fn preserves_potentially_trapping_primitive_without_users() {
    let mut builder = FunctionBuilder::new(
        FunctionId(2),
        "dce-trap",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let result = builder
        .push_op(
            ncl_ir::OpKind::Prim {
                op: ncl_ir::Prim::Car,
                args: vec![ValueId(0)],
                condition: None,
            },
            &[Ty::Word],
        )
        .ok()
        .and_then(|values| values.into_iter().next());
    assert!(result.is_some());
    let terminated = builder.terminate(Terminator::Return {
        values: vec![ValueId(0)],
    });
    assert!(terminated.is_ok());
    let mut function = builder.finish();
    let mut pass = DeadCodeElimination;
    let changed = pass.run(&mut function, &Module::default());
    assert!(changed.is_ok());
    assert!(!changed.unwrap_or(false));
    assert_eq!(function.blocks[0].ops.len(), 1);
}

fn build_handler_entry_block() -> BasicBlock {
    BasicBlock {
        id: ncl_ir::BlockId(0),
        params: Vec::new(),
        ops: vec![
            Op {
                results: vec![(ValueId(1), Ty::Word)],
                kind: OpKind::Const {
                    result: ConstantIndex(0),
                },
                loc: None,
            },
            Op {
                results: vec![(ValueId(2), Ty::Word)],
                kind: OpKind::Move { value: ValueId(1) },
                loc: None,
            },
            Op {
                results: vec![(ValueId(5), Ty::I64)],
                kind: OpKind::Convert {
                    op: ncl_ir::Convert::WordToI64,
                    value: ValueId(1),
                },
                loc: None,
            },
            Op {
                results: Vec::new(),
                kind: OpKind::EnterHandler {
                    region: ncl_ir::HandlerRegionId(0),
                },
                loc: None,
            },
        ],
        terminator: Terminator::Branch {
            condition: ValueId(0),
            then_target: ncl_ir::BlockId(1),
            then_args: vec![ValueId(1), ValueId(2)],
            else_target: ncl_ir::BlockId(1),
            else_args: vec![ValueId(1), ValueId(2)],
        },
    }
}

fn build_handler_body_block() -> BasicBlock {
    BasicBlock {
        id: ncl_ir::BlockId(1),
        params: vec![
            ncl_ir::BlockParam {
                value: ValueId(3),
                ty: Ty::Word,
            },
            ncl_ir::BlockParam {
                value: ValueId(4),
                ty: Ty::Word,
            },
        ],
        ops: vec![Op {
            results: Vec::new(),
            kind: OpKind::LeaveHandler {
                region: ncl_ir::HandlerRegionId(0),
            },
            loc: None,
        }],
        terminator: Terminator::Return { values: Vec::new() },
    }
}

fn build_handler_fixture() -> Function {
    Function {
        id: FunctionId(3),
        name: "dce-handler-liveness".into(),
        params: vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        return_types: Vec::new(),
        blocks: vec![build_handler_entry_block(), build_handler_body_block()],
        locals: Vec::new(),
        constants: vec![Constant::Nil],
        handler_regions: vec![HandlerRegion {
            id: ncl_ir::HandlerRegionId(0),
            kind: HandlerKind::Catch,
            protected: vec![ncl_ir::BlockId(0), ncl_ir::BlockId(1)],
            handler: ncl_ir::BlockId(1),
            cleanup: None,
            catch_tag: Some(ValueId(1)),
            binding_targets: vec![ValueId(2)],
            depth: 0,
            parent: None,
        }],
        debug: Vec::new(),
    }
}

#[test]
fn preserves_handler_catch_tag_and_binding_target_definitions() {
    let mut function = build_handler_fixture();
    let mut pass = DeadCodeElimination;
    let changed = pass.run(&mut function, &Module::default());

    assert!(changed.is_ok(), "DCE rejected handler fixture: {changed:?}");
    assert!(
        function.blocks[0]
            .ops
            .iter()
            .any(|op| matches!(op.kind, OpKind::Const { .. }))
    );
    assert!(
        function.blocks[0]
            .ops
            .iter()
            .any(|op| { matches!(op.kind, OpKind::Move { value: ValueId(1) }) })
    );
    assert!(
        !function.blocks[0]
            .ops
            .iter()
            .any(|op| { matches!(op.kind, OpKind::Convert { .. }) })
    );
    assert!(ncl_ir::verify(&function).is_ok());
}

fn build_control_entry_block() -> BasicBlock {
    BasicBlock {
        id: ncl_ir::BlockId(0),
        params: Vec::new(),
        ops: vec![
            Op {
                results: vec![(ValueId(2), Ty::Bool)],
                kind: OpKind::Compare {
                    op: ncl_ir::Compare::Eq,
                    left: ValueId(0),
                    right: ValueId(0),
                },
                loc: None,
            },
            Op {
                results: vec![(ValueId(3), Ty::Word)],
                kind: OpKind::Move { value: ValueId(1) },
                loc: None,
            },
        ],
        terminator: Terminator::Switch {
            value: ValueId(0),
            cases: vec![(0, ncl_ir::BlockId(1), Vec::new())],
            default: ncl_ir::BlockId(2),
            default_args: Vec::new(),
        },
    }
}

fn build_control_branch_block() -> BasicBlock {
    BasicBlock {
        id: ncl_ir::BlockId(1),
        params: Vec::new(),
        ops: Vec::new(),
        terminator: Terminator::Branch {
            condition: ValueId(2),
            then_target: ncl_ir::BlockId(3),
            then_args: Vec::new(),
            else_target: ncl_ir::BlockId(4),
            else_args: Vec::new(),
        },
    }
}

fn build_control_default_block() -> BasicBlock {
    BasicBlock {
        id: ncl_ir::BlockId(2),
        params: Vec::new(),
        ops: Vec::new(),
        terminator: Terminator::Jump {
            target: ncl_ir::BlockId(5),
            args: Vec::new(),
        },
    }
}

fn build_control_return_block(id: u32) -> BasicBlock {
    BasicBlock {
        id: ncl_ir::BlockId(id),
        params: Vec::new(),
        ops: Vec::new(),
        terminator: Terminator::Return {
            values: vec![ValueId(1)],
        },
    }
}

fn build_control_unreachable_block() -> BasicBlock {
    BasicBlock {
        id: ncl_ir::BlockId(6),
        params: Vec::new(),
        ops: Vec::new(),
        terminator: Terminator::Unreachable,
    }
}

fn build_unreachable_fixture() -> Function {
    Function {
        id: FunctionId(4),
        name: "dce-control-flow".into(),
        params: vec![
            Param {
                name: "selector".into(),
                ty: Ty::I64,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        return_types: vec![Ty::Word],
        blocks: vec![
            build_control_entry_block(),
            build_control_branch_block(),
            build_control_default_block(),
            build_control_return_block(3),
            build_control_return_block(4),
            build_control_return_block(5),
            build_control_unreachable_block(),
        ],
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

#[test]
fn removes_unreachable_blocks_and_tracks_switch_branch_jump_liveness() {
    let mut function = build_unreachable_fixture();
    let mut pass = DeadCodeElimination;

    assert_eq!(pass.run(&mut function, &Module::default()), Ok(true));
    assert_eq!(
        function
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![
            ncl_ir::BlockId(0),
            ncl_ir::BlockId(1),
            ncl_ir::BlockId(2),
            ncl_ir::BlockId(3),
            ncl_ir::BlockId(4),
            ncl_ir::BlockId(5),
        ]
    );
    assert_eq!(function.blocks[0].ops.len(), 1);
    assert!(matches!(
        function.blocks[0].ops[0].kind,
        OpKind::Compare { .. }
    ));
}

#[test]
fn keeps_unwind_protect_handler_and_cleanup_blocks_reachable() {
    let function = Function {
        id: FunctionId(5),
        name: "dce-cleanup".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: vec![
            BasicBlock {
                id: ncl_ir::BlockId(0),
                params: Vec::new(),
                ops: vec![Op {
                    results: Vec::new(),
                    kind: OpKind::EnterHandler {
                        region: ncl_ir::HandlerRegionId(0),
                    },
                    loc: None,
                }],
                terminator: Terminator::Jump {
                    target: ncl_ir::BlockId(1),
                    args: Vec::new(),
                },
            },
            BasicBlock {
                id: ncl_ir::BlockId(1),
                params: Vec::new(),
                ops: vec![Op {
                    results: Vec::new(),
                    kind: OpKind::LeaveHandler {
                        region: ncl_ir::HandlerRegionId(0),
                    },
                    loc: None,
                }],
                terminator: Terminator::Return { values: Vec::new() },
            },
            BasicBlock {
                id: ncl_ir::BlockId(2),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Unreachable,
            },
            BasicBlock {
                id: ncl_ir::BlockId(3),
                params: Vec::new(),
                ops: vec![Op {
                    results: Vec::new(),
                    kind: OpKind::Safepoint,
                    loc: None,
                }],
                terminator: Terminator::Unreachable,
            },
            BasicBlock {
                id: ncl_ir::BlockId(4),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
        ],
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: vec![HandlerRegion {
            id: ncl_ir::HandlerRegionId(0),
            kind: HandlerKind::UnwindProtect,
            protected: vec![ncl_ir::BlockId(0), ncl_ir::BlockId(1)],
            handler: ncl_ir::BlockId(2),
            cleanup: Some(ncl_ir::BlockId(3)),
            catch_tag: None,
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        }],
        debug: Vec::new(),
    };
    let mut function = function;
    let mut pass = DeadCodeElimination;

    assert_eq!(pass.run(&mut function, &Module::default()), Ok(true));
    assert_eq!(
        function
            .blocks
            .iter()
            .map(|block| block.id)
            .collect::<Vec<_>>(),
        vec![
            ncl_ir::BlockId(0),
            ncl_ir::BlockId(1),
            ncl_ir::BlockId(2),
            ncl_ir::BlockId(3),
        ]
    );
    assert!(matches!(function.blocks[3].ops[0].kind, OpKind::Safepoint));
}

fn add_memory_side_effects(builder: &mut FunctionBuilder) {
    builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(1),
                field: 0,
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::Store {
                address: ValueId(0),
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    builder
        .push_op(
            OpKind::StoreField {
                object: ValueId(1),
                field: 0,
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    builder
        .push_op(OpKind::Alloc { words: 1 }, &[Ty::Address])
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Address])
        .fixture();
    builder
        .push_op(OpKind::LoadCapture { index: 0 }, &[Ty::Word])
        .fixture();
    builder
        .push_op(OpKind::LoadFunctionObject, &[Ty::Word])
        .fixture();
}

fn add_call_side_effects(builder: &mut FunctionBuilder) {
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::Call {
                function: ValueId(2),
                args: vec![ValueId(1)],
            },
            &[Ty::Word],
        )
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::CallIndirect {
                callee: ValueId(2),
                args: vec![ValueId(1)],
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::MakeClosure {
                entry: ValueId(2),
                captures: vec![ValueId(1)],
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(OpKind::MakeValueCell { value: ValueId(1) }, &[Ty::Word])
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::CallClosure {
                closure: ValueId(2),
                args: vec![ValueId(1)],
                named_symbol: Some(ValueId(2)),
            },
            &[Ty::Word],
        )
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::Builtin {
                name: "side-effect".into(),
                args: vec![ValueId(1)],
            },
            &[Ty::Word],
        )
        .fixture();
}

fn add_remaining_side_effects(builder: &mut FunctionBuilder) {
    builder
        .push_op(
            OpKind::Prim {
                op: ncl_ir::Prim::Car,
                args: vec![ValueId(1)],
                condition: None,
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: ValueId(1),
                right: ValueId(1),
            },
            &[Ty::Bool],
        )
        .fixture();
    builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::WordToI64,
                value: ValueId(1),
            },
            &[Ty::I64],
        )
        .fixture();
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![ValueId(1)],
            },
            &[],
        )
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
}

#[test]
fn preserves_side_effecting_operations_and_removes_unused_pure_operations() {
    let mut builder = FunctionBuilder::new(
        FunctionId(6),
        "dce-side-effects",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
            Param {
                name: "function".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    add_memory_side_effects(&mut builder);
    add_call_side_effects(&mut builder);
    add_remaining_side_effects(&mut builder);
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(1)],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = DeadCodeElimination;
    assert_eq!(pass.run(&mut function, &Module::default()), Ok(true));
    assert!(function.blocks[0].ops.iter().all(|op| {
        !matches!(
            op.kind,
            OpKind::Load { .. }
                | OpKind::LoadField { .. }
                | OpKind::LoadArg { .. }
                | OpKind::LoadCapture { .. }
                | OpKind::LoadFunctionObject
                | OpKind::Compare { .. }
                | OpKind::Convert { .. }
        )
    }));
    assert_eq!(function.blocks[0].ops.len(), 17);
    assert!(ncl_ir::verify(&function).is_ok());
}

#[test]
fn keeps_operands_live_for_terminal_calls_and_throws() {
    for terminator in [
        Terminator::CallReturn {
            function: ValueId(1),
            args: vec![ValueId(2)],
        },
        Terminator::TailCall {
            function: ValueId(1),
            args: vec![ValueId(2)],
        },
        Terminator::Throw {
            condition: ValueId(0),
        },
    ] {
        let mut builder = FunctionBuilder::new(
            FunctionId(7),
            "dce-terminal",
            vec![
                Param {
                    name: "condition".into(),
                    ty: Ty::Word,
                },
                Param {
                    name: "function".into(),
                    ty: Ty::Word,
                },
                Param {
                    name: "argument".into(),
                    ty: Ty::Word,
                },
            ],
            Vec::new(),
        );
        builder
            .push_op(OpKind::Move { value: ValueId(2) }, &[Ty::Word])
            .fixture();
        builder.terminate(terminator.clone()).fixture();

        let mut function = builder.finish();
        let mut pass = DeadCodeElimination;
        let result = pass.run(&mut function, &Module::default());
        assert_eq!(result, Ok(true), "terminal {terminator:?}");
        assert!(function.blocks[0].ops.is_empty());
        assert!(ncl_ir::verify(&function).is_ok());
    }
}

#[test]
fn tracks_conditional_primitive_reachability_and_live_operands() {
    let mut builder = FunctionBuilder::new(
        FunctionId(8),
        "dce-conditional-primitive",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "object".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let failure_block = builder.create_block(Vec::new());
    let failure_constant = builder.add_constant(Constant::Nil);
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let loaded = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let moved = builder
        .push_op(OpKind::Move { value: loaded }, &[Ty::Word])
        .fixture()[0];
    let field = builder
        .push_op(
            OpKind::LoadField {
                object: moved,
                field: 0,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let result = builder
        .push_op(
            OpKind::Prim {
                op: ncl_ir::Prim::Car,
                args: vec![field],
                condition: Some(failure_block),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .fixture();
    builder.position_at(failure_block).fixture();
    let failure_value = builder
        .push_op(
            OpKind::Const {
                result: failure_constant,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![failure_value],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = DeadCodeElimination;
    assert_eq!(pass.run(&mut function, &Module::default()), Ok(false));
    assert_eq!(function.blocks.len(), 2);
    assert_eq!(function.blocks[0].ops.len(), 4);
    assert!(ncl_ir::verify(&function).is_ok());
}
