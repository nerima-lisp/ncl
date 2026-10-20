use super::tests_support::Fixture;
use super::{GlobalValueNumbering, Module};
use crate::FunctionPass;
use ncl_ir::{
    Constant, Function, FunctionBuilder, FunctionId, HandlerKind, HandlerRegion, HandlerRegionId,
    OpKind, Param, Prim, Terminator, Ty, ValueId,
};

fn binary_duplicate() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "duplicate",
        vec![Param {
            name: "x".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    builder.finish()
}

#[test]
fn removes_redundant_pure_computation() {
    let mut function = binary_duplicate();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 1);
    assert!(
        matches!(&function.blocks[0].terminator, Terminator::Return { values } if values.len() == 1)
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn store_invalidates_load_value() {
    let mut builder = FunctionBuilder::new(
        FunctionId(2),
        "memory",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(
            OpKind::Store {
                address: ValueId(0),
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::Load { .. }))
            .count(),
        2
    );
    assert_ne!(first, second);
}

#[test]
fn call_invalidates_load_value() {
    let mut builder = FunctionBuilder::new(
        FunctionId(3),
        "call-memory",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "function".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::Call {
                function: ValueId(1),
                args: Vec::new(),
            },
            &[Ty::Word],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();
    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 4);
    assert_ne!(first, second);
    ncl_ir::verify(&function).fixture();
}

#[test]
fn handler_region_is_a_conservative_no_op() {
    let mut builder = FunctionBuilder::new(
        FunctionId(4),
        "handler",
        vec![Param {
            name: "address".into(),
            ty: Ty::Address,
        }],
        vec![Ty::Word],
    );
    let catch_tag = builder.add_constant(Constant::Nil);
    let tag = builder
        .push_op(OpKind::Const { result: catch_tag }, &[Ty::Word])
        .fixture()[0];
    builder
        .push_op(
            OpKind::EnterHandler {
                region: HandlerRegionId(0),
            },
            &[],
        )
        .fixture();
    let first = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(
            OpKind::LeaveHandler {
                region: HandlerRegionId(0),
            },
            &[],
        )
        .fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![first],
        })
        .fixture();
    builder.add_handler_region(HandlerRegion {
        id: HandlerRegionId(0),
        kind: HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0)],
        handler: ncl_ir::BlockId(0),
        cleanup: None,
        catch_tag: Some(tag),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    let mut function = builder.finish();
    let before = function.clone();
    ncl_ir::verify(&function).fixture();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function, before);
}

#[test]
fn closure_and_multiple_values_are_conservative_no_ops() {
    let mut builder = FunctionBuilder::new(
        FunctionId(5),
        "closure-values",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let entry_constant = builder.add_constant(Constant::FunctionEntry(FunctionId(5)));
    let entry = builder
        .push_op(
            OpKind::Const {
                result: entry_constant,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry,
                captures: vec![ValueId(0)],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::CallClosure {
                closure,
                args: vec![ValueId(0)],
                named_symbol: None,
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![ValueId(0)],
            },
            &[],
        )
        .fixture();
    let moved = builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![moved],
        })
        .fixture();
    let mut function = builder.finish();
    let before = function.clone();
    ncl_ir::verify(&function).fixture();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function, before);
}

#[test]
fn cyclic_cfg_is_a_conservative_no_op() {
    let mut builder = FunctionBuilder::new(
        FunctionId(6),
        "cycle",
        vec![
            Param {
                name: "condition".into(),
                ty: Ty::Bool,
            },
            Param {
                name: "value".into(),
                ty: Ty::I64,
            },
        ],
        vec![],
    );
    let loop_block = builder.create_block(Vec::new());
    let exit_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(0),
            then_target: loop_block,
            then_args: Vec::new(),
            else_target: exit_block,
            else_args: Vec::new(),
        })
        .fixture();
    builder.position_at(loop_block).fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .terminate(Terminator::Jump {
            target: loop_block,
            args: Vec::new(),
        })
        .fixture();
    builder.position_at(exit_block).fixture();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    let mut function = builder.finish();
    let before = function.clone();
    ncl_ir::verify(&function).fixture();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function, before);
}

#[test]
fn removes_redundant_expression_kinds() {
    let mut builder = FunctionBuilder::new(
        FunctionId(7),
        "expression-kinds",
        vec![Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word, Ty::Word, Ty::Word, Ty::Bool, Ty::I64],
    );
    let constant = builder.add_constant(Constant::Nil);
    let const_value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .fixture()[0];
    let duplicate_const = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .fixture()[0];
    let move_value = builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    let duplicate_move = builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    let field_value = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 3,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let duplicate_field = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 3,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let compare_value = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: ValueId(0),
                right: ValueId(0),
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let duplicate_compare = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: ValueId(0),
                right: ValueId(0),
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let converted_value = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::WordToI64,
                value: ValueId(0),
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let duplicate_converted = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::WordToI64,
                value: ValueId(0),
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![
                duplicate_const,
                duplicate_move,
                duplicate_field,
                duplicate_compare,
                duplicate_converted,
            ],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 5);
    assert!(matches!(
        &function.blocks[0].terminator,
        Terminator::Return { values }
            if values == &[const_value, move_value, field_value, compare_value, converted_value]
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn propagates_values_through_dominated_blocks_only() {
    let mut builder = FunctionBuilder::new(
        FunctionId(8),
        "dominator-traversal",
        vec![
            Param {
                name: "condition".into(),
                ty: Ty::Bool,
            },
            Param {
                name: "value".into(),
                ty: Ty::I64,
            },
        ],
        vec![Ty::I64],
    );
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    let first = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(1), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(0),
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .fixture();
    builder.position_at(then_block).fixture();
    let then_duplicate = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(1), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![then_duplicate],
        })
        .fixture();
    builder.position_at(else_block).fixture();
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(1), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![first],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    let then = function.blocks.iter().find(|block| block.id == then_block);
    assert!(then.is_some());
    let then = then.unwrap_or_else(|| unreachable!());
    let otherwise = function.blocks.iter().find(|block| block.id == else_block);
    assert!(otherwise.is_some());
    let otherwise = otherwise.unwrap_or_else(|| unreachable!());
    assert!(then.ops.is_empty());
    assert!(otherwise.ops.is_empty());
    assert!(matches!(
        &then.terminator,
        Terminator::Return { values } if values == &[first]
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn memory_writes_invalidate_loads() {
    let mut builder = FunctionBuilder::new(
        FunctionId(9),
        "memory-writes",
        vec![
            Param {
                name: "object".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 1,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(
            OpKind::StoreField {
                object: ValueId(0),
                field: 1,
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::Rplaca,
                args: vec![ValueId(0), ValueId(1)],
                condition: None,
            },
            &[Ty::Unit],
        )
        .fixture();
    let second = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 1,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::LoadField { .. }))
            .count(),
        2
    );
    assert_ne!(first, second);
    ncl_ir::verify(&function).fixture();
}

#[test]
fn memory_reads_are_value_numbered_until_a_write() {
    let mut builder = FunctionBuilder::new(
        FunctionId(17),
        "memory-reads",
        vec![Param {
            name: "object".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word, Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Car,
                args: vec![ValueId(0)],
                condition: None,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let duplicate = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Car,
                args: vec![ValueId(0)],
                condition: None,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![first, duplicate],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks[0].ops.len(), 1);
    assert!(matches!(
        &function.blocks[0].terminator,
        Terminator::Return { values } if values == &[ValueId(1), ValueId(1)]
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn indirect_call_and_value_cell_invalidate_memory_reads() {
    let mut builder = FunctionBuilder::new(
        FunctionId(18),
        "indirect-call-memory",
        vec![
            Param {
                name: "object".into(),
                ty: Ty::Word,
            },
            Param {
                name: "callee".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let first = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 2,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let moved_callee = builder
        .push_op(OpKind::Move { value: ValueId(1) }, &[Ty::Word])
        .fixture()[0];
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::CallIndirect {
                callee: moved_callee,
                args: vec![ValueId(2)],
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(OpKind::MakeValueCell { value: ValueId(2) }, &[Ty::Word])
        .fixture();
    let second = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(0),
                field: 2,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![second],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut function, &Module::default()).fixture());
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::LoadField { .. }))
            .count(),
        2
    );
    assert_ne!(first, second);
    assert!(matches!(
        &function.blocks[0].ops[3].kind,
        OpKind::CallIndirect { callee, args } if *callee == moved_callee && args == &[ValueId(2)]
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn join_block_uses_only_values_from_its_dominators() {
    let mut builder = FunctionBuilder::new(
        FunctionId(19),
        "dominator-join",
        vec![
            Param {
                name: "condition".into(),
                ty: Ty::Bool,
            },
            Param {
                name: "value".into(),
                ty: Ty::I64,
            },
        ],
        vec![Ty::I64],
    );
    let preheader = builder.create_block(Vec::new());
    let then_block = builder.create_block(Vec::new());
    let else_block = builder.create_block(Vec::new());
    let join_block = builder.create_block(Vec::new());
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    builder
        .terminate(Terminator::Jump {
            target: preheader,
            args: Vec::new(),
        })
        .fixture();
    builder.position_at(preheader).fixture();
    let common = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(1), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(0),
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .fixture();
    for block in [then_block, else_block] {
        builder.position_at(block).fixture();
        builder
            .terminate(Terminator::Jump {
                target: join_block,
                args: Vec::new(),
            })
            .fixture();
    }
    builder.position_at(join_block).fixture();
    let duplicate = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(1), ValueId(1)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![duplicate],
        })
        .fixture();

    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    assert!(pass.run(&mut function, &Module::default()).fixture());
    let join = function.blocks.iter().find(|block| block.id == join_block);
    assert!(join.is_some());
    let join = join.unwrap_or_else(|| unreachable!());
    assert!(join.ops.is_empty());
    assert!(matches!(
        &join.terminator,
        Terminator::Return { values } if values == &[common]
    ));
    ncl_ir::verify(&function).fixture();
}

#[test]
fn empty_function_is_a_conservative_no_op() {
    let mut empty = Function {
        id: FunctionId(20),
        name: "empty".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let mut pass = GlobalValueNumbering;
    assert!(!pass.run(&mut empty, &Module::default()).fixture());
    assert!(empty.blocks.is_empty());
}

fn terminator_after_gvn(id: u32, terminator: Terminator) -> Terminator {
    let mut builder = FunctionBuilder::new(
        FunctionId(id),
        "terminator-rewrite",
        vec![
            Param {
                name: "condition".into(),
                ty: Ty::Bool,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
            Param {
                name: "switch".into(),
                ty: Ty::I64,
            },
        ],
        vec![Ty::Word],
    );
    let first_target = builder.create_block(vec![(Ty::Word, ValueId(3))]);
    let second_target = builder.create_block(vec![(Ty::Word, ValueId(4))]);
    builder.position_at(first_target).fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(3)],
        })
        .fixture();
    builder.position_at(second_target).fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(4)],
        })
        .fixture();
    builder.position_at(ncl_ir::BlockId(0)).fixture();
    builder
        .push_op(OpKind::Move { value: ValueId(1) }, &[Ty::Word])
        .fixture();
    builder
        .push_op(OpKind::Move { value: ValueId(1) }, &[Ty::Word])
        .fixture();
    builder.terminate(terminator).fixture();

    let mut function = builder.finish();
    let mut pass = GlobalValueNumbering;
    pass.run(&mut function, &Module::default()).fixture();
    function.blocks[0].terminator.clone()
}

#[test]
fn rewrites_terminator_operands_after_value_elimination() {
    let jump = terminator_after_gvn(
        10,
        Terminator::Jump {
            target: ncl_ir::BlockId(1),
            args: vec![ValueId(6)],
        },
    );
    assert_eq!(
        jump,
        Terminator::Jump {
            target: ncl_ir::BlockId(1),
            args: vec![ValueId(5)],
        }
    );

    let branch = terminator_after_gvn(
        11,
        Terminator::Branch {
            condition: ValueId(0),
            then_target: ncl_ir::BlockId(1),
            then_args: vec![ValueId(6)],
            else_target: ncl_ir::BlockId(2),
            else_args: vec![ValueId(6)],
        },
    );
    assert_eq!(
        branch,
        Terminator::Branch {
            condition: ValueId(0),
            then_target: ncl_ir::BlockId(1),
            then_args: vec![ValueId(5)],
            else_target: ncl_ir::BlockId(2),
            else_args: vec![ValueId(5)],
        }
    );

    let switch = terminator_after_gvn(
        12,
        Terminator::Switch {
            value: ValueId(2),
            cases: vec![(0, ncl_ir::BlockId(1), vec![ValueId(6)])],
            default: ncl_ir::BlockId(2),
            default_args: vec![ValueId(6)],
        },
    );
    assert_eq!(
        switch,
        Terminator::Switch {
            value: ValueId(2),
            cases: vec![(0, ncl_ir::BlockId(1), vec![ValueId(5)])],
            default: ncl_ir::BlockId(2),
            default_args: vec![ValueId(5)],
        }
    );

    for (id, terminator, expected) in [
        (
            13,
            Terminator::CallReturn {
                function: ValueId(6),
                args: vec![ValueId(6)],
            },
            Terminator::CallReturn {
                function: ValueId(5),
                args: vec![ValueId(5)],
            },
        ),
        (
            14,
            Terminator::TailCall {
                function: ValueId(6),
                args: vec![ValueId(6)],
            },
            Terminator::TailCall {
                function: ValueId(5),
                args: vec![ValueId(5)],
            },
        ),
        (
            15,
            Terminator::Return {
                values: vec![ValueId(6)],
            },
            Terminator::Return {
                values: vec![ValueId(5)],
            },
        ),
        (
            16,
            Terminator::Throw {
                condition: ValueId(6),
            },
            Terminator::Throw {
                condition: ValueId(5),
            },
        ),
    ] {
        assert_eq!(terminator_after_gvn(id, terminator), expected);
    }
}
