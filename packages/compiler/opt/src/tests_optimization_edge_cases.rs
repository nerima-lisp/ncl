#![allow(
    clippy::similar_names,
    clippy::single_element_loop,
    clippy::too_many_lines
)]

use super::tests_support::Fixture;
use crate::{
    DeadCodeElimination, FunctionPass, GlobalValueNumbering, InlineDirectCalls, Module, Sccp,
};
use ncl_ir::{
    BasicBlock, BlockId, Constant, Function, FunctionBuilder, FunctionId, HandlerKind,
    HandlerRegion, HandlerRegionId, OpKind, Param, Prim, Terminator, Ty, ValueId,
};

fn params() -> Vec<Param> {
    vec![
        Param {
            name: "address".into(),
            ty: Ty::Address,
        },
        Param {
            name: "word".into(),
            ty: Ty::Word,
        },
        Param {
            name: "integer".into(),
            ty: Ty::I64,
        },
        Param {
            name: "condition".into(),
            ty: Ty::Bool,
        },
    ]
}

#[test]
fn dce_removes_an_unreachable_block_without_changing_the_entry_return() {
    let mut builder = FunctionBuilder::new(
        FunctionId(3),
        "dce-unreachable",
        vec![Param {
            name: "value".into(),
            ty: Ty::I64,
        }],
        vec![Ty::I64],
    );
    let dead = builder.create_block(Vec::new());
    builder.position_at(dead).fixture();
    let dead_constant = builder.add_constant(Constant::Fixnum(99));
    let dead_value = builder
        .push_op(
            OpKind::Const {
                result: dead_constant,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![dead_value],
        })
        .fixture();
    builder.position_at(BlockId(0)).fixture();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(0)],
        })
        .fixture();
    let mut function = builder.finish();
    assert_eq!(function.blocks.len(), 2);
    let before_return = function.blocks[0].terminator.clone();

    assert!(
        DeadCodeElimination
            .run(&mut function, &Module::default())
            .fixture()
    );

    assert_eq!(function.blocks.len(), 1);
    assert_eq!(function.blocks[0].terminator, before_return);
    assert!(function.blocks[0].ops.is_empty());
    ncl_ir::verify(&function).fixture();
}

#[test]
fn inline_leaf_preserves_the_returned_argument_value() {
    let mut module = Module {
        functions: vec![super::tests_support::caller(), super::tests_support::leaf()],
    };
    let call_result = match module.functions[0].blocks[0].ops[1].results.as_slice() {
        [(value, _)] => *value,
        _ => unreachable!(),
    };
    let mut pass = InlineDirectCalls::default();
    let snapshot = module.clone();
    assert!(pass.run(&mut module.functions[0], &snapshot).fixture());

    let caller = &module.functions[0];
    assert_eq!(caller.blocks[0].ops.len(), 2);
    assert!(matches!(
        caller.blocks[0].ops[1].kind,
        OpKind::Move { value: ValueId(0) }
    ));
    let inlined_result = caller.blocks[0].ops[1].results[0].0;
    assert_ne!(inlined_result, call_result);
    assert_eq!(
        caller.blocks[0].terminator,
        Terminator::Return {
            values: vec![inlined_result]
        }
    );
    module.verify().fixture();
}

#[test]
fn dce_removes_dead_pure_values_but_keeps_side_effects_and_dependencies() {
    let mut builder = FunctionBuilder::new(FunctionId(20), "dce-shapes", params(), Vec::new());
    let constant = builder.add_constant(Constant::Fixnum(7));
    builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .fixture();
    builder
        .push_op(OpKind::Move { value: ValueId(1) }, &[Ty::Word])
        .fixture();
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
                field: 2,
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .fixture();
    builder
        .push_op(OpKind::LoadCapture { index: 0 }, &[Ty::Word])
        .fixture();
    builder
        .push_op(OpKind::LoadFunctionObject, &[Ty::Word])
        .fixture();
    builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left: ValueId(2),
                right: ValueId(2),
            },
            &[Ty::Bool],
        )
        .fixture();
    builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: ValueId(2),
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumLt,
                args: vec![ValueId(2), ValueId(2)],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture();
    let used_move = builder
        .push_op(OpKind::Move { value: ValueId(1) }, &[Ty::Word])
        .fixture()[0];
    builder
        .push_op(
            OpKind::Store {
                address: ValueId(0),
                value: used_move,
            },
            &[],
        )
        .fixture();
    builder
        .push_op(
            OpKind::StoreField {
                object: ValueId(1),
                field: 3,
                value: ValueId(1),
            },
            &[],
        )
        .fixture();
    let car = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Car,
                args: vec![ValueId(1)],
                condition: None,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(
            OpKind::SetMultipleValues {
                values: vec![car, ValueId(1)],
            },
            &[],
        )
        .fixture();
    builder
        .push_op(OpKind::Alloc { words: 2 }, &[Ty::Address])
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::Call {
                function: ValueId(1),
                args: vec![ValueId(1)],
            },
            &[Ty::Word],
        )
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry: ValueId(1),
                captures: vec![ValueId(1)],
            },
            &[Ty::Word],
        )
        .fixture()[0];
    builder
        .push_op(OpKind::MakeValueCell { value: ValueId(1) }, &[Ty::Word])
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::CallClosure {
                closure,
                args: vec![ValueId(1)],
                named_symbol: None,
            },
            &[Ty::Word],
        )
        .fixture();
    builder.push_op(OpKind::Safepoint, &[]).fixture();
    builder
        .push_op(
            OpKind::Builtin {
                name: "identity".into(),
                args: vec![ValueId(1)],
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();

    let mut function = builder.finish();
    assert!(
        DeadCodeElimination
            .run(&mut function, &Module::default())
            .fixture()
    );
    let kinds = function.blocks[0]
        .ops
        .iter()
        .map(|op| &op.kind)
        .collect::<Vec<_>>();
    assert!(!kinds.iter().any(|kind| {
        matches!(
            kind,
            OpKind::Const { .. }
                | OpKind::Load { .. }
                | OpKind::LoadField { .. }
                | OpKind::LoadArg { .. }
                | OpKind::LoadCapture { .. }
                | OpKind::LoadFunctionObject
                | OpKind::Compare { .. }
                | OpKind::Convert { .. }
                | OpKind::Prim {
                    op: Prim::FixnumLt,
                    condition: None,
                    ..
                }
        )
    }));
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| matches!(kind, OpKind::Move { .. }))
            .count(),
        1
    );
    for expected in [
        OpKind::Store {
            address: ValueId(0),
            value: used_move,
        },
        OpKind::StoreField {
            object: ValueId(1),
            field: 3,
            value: ValueId(1),
        },
    ] {
        assert!(kinds.iter().any(|kind| **kind == expected));
    }
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, OpKind::Prim { op: Prim::Car, .. }))
    );
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, OpKind::CallClosure { .. }))
    );
    assert!(
        kinds
            .iter()
            .any(|kind| matches!(kind, OpKind::Builtin { .. }))
    );
}

fn terminal_function(terminator: Terminator, params: Vec<Param>) -> Function {
    Function {
        id: FunctionId(21),
        name: "terminal".into(),
        params,
        return_types: Vec::new(),
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            ops: Vec::new(),
            terminator,
        }],
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    }
}

#[test]
fn dce_accepts_each_non_successor_terminator_shape() {
    let branch = Function {
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Branch {
                    condition: ValueId(0),
                    then_target: BlockId(1),
                    then_args: Vec::new(),
                    else_target: BlockId(2),
                    else_args: Vec::new(),
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
            BasicBlock {
                id: BlockId(2),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
        ],
        ..terminal_function(
            Terminator::Unreachable,
            vec![Param {
                name: "condition".into(),
                ty: Ty::Bool,
            }],
        )
    };
    let switch = Function {
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Switch {
                    value: ValueId(0),
                    cases: vec![(1, BlockId(1), Vec::new())],
                    default: BlockId(2),
                    default_args: Vec::new(),
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
            BasicBlock {
                id: BlockId(2),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
        ],
        ..terminal_function(
            Terminator::Unreachable,
            vec![Param {
                name: "key".into(),
                ty: Ty::I64,
            }],
        )
    };
    let mut functions = vec![
        Function {
            blocks: vec![
                BasicBlock {
                    id: BlockId(0),
                    params: Vec::new(),
                    ops: Vec::new(),
                    terminator: Terminator::Jump {
                        target: BlockId(1),
                        args: Vec::new(),
                    },
                },
                BasicBlock {
                    id: BlockId(1),
                    params: Vec::new(),
                    ops: Vec::new(),
                    terminator: Terminator::Return { values: Vec::new() },
                },
            ],
            ..terminal_function(Terminator::Unreachable, Vec::new())
        },
        terminal_function(
            Terminator::CallReturn {
                function: ValueId(0),
                args: Vec::new(),
            },
            vec![Param {
                name: "callee".into(),
                ty: Ty::Word,
            }],
        ),
        terminal_function(
            Terminator::TailCall {
                function: ValueId(0),
                args: Vec::new(),
            },
            vec![Param {
                name: "callee".into(),
                ty: Ty::Word,
            }],
        ),
        terminal_function(
            Terminator::Throw {
                condition: ValueId(0),
            },
            vec![Param {
                name: "condition".into(),
                ty: Ty::Word,
            }],
        ),
        branch,
        switch,
    ];
    let mut pass = DeadCodeElimination;
    for function in &mut functions {
        assert!(!pass.run(function, &Module::default()).fixture());
    }
}

#[test]
fn gvn_rewrites_duplicate_expression_kinds() {
    let mut builder = FunctionBuilder::new(
        FunctionId(22),
        "gvn-shapes",
        vec![
            Param {
                name: "address".into(),
                ty: Ty::Address,
            },
            Param {
                name: "object".into(),
                ty: Ty::Word,
            },
            Param {
                name: "integer".into(),
                ty: Ty::I64,
            },
        ],
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(4));
    let first_const = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .fixture()[0];
    let second_const = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .fixture()[0];
    let first_move = builder
        .push_op(OpKind::Move { value: first_const }, &[Ty::I64])
        .fixture()[0];
    let second_move = builder
        .push_op(
            OpKind::Move {
                value: second_const,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let first_load = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let second_load = builder
        .push_op(
            OpKind::Load {
                address: ValueId(0),
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let first_field = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(1),
                field: 2,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let second_field = builder
        .push_op(
            OpKind::LoadField {
                object: ValueId(1),
                field: 2,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let first_prim = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(2), ValueId(2)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let second_prim = builder
        .push_op(
            OpKind::Prim {
                op: Prim::FixnumAdd,
                args: vec![ValueId(2), ValueId(2)],
                condition: None,
            },
            &[Ty::I64],
        )
        .fixture()[0];
    let first_compare = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Le,
                left: first_prim,
                right: first_prim,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let second_compare = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Le,
                left: second_prim,
                right: second_prim,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let first_convert = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: first_prim,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let second_convert = builder
        .push_op(
            OpKind::Convert {
                op: ncl_ir::Convert::I64ToWord,
                value: second_prim,
            },
            &[Ty::Word],
        )
        .fixture()[0];
    let _ = (
        second_move,
        second_load,
        first_field,
        second_field,
        second_compare,
        first_convert,
    );
    builder
        .terminate(Terminator::Return {
            values: vec![
                second_convert,
                first_load,
                first_const,
                first_move,
                first_compare,
            ],
        })
        .fixture();
    let mut function = builder.finish();
    function.return_types = vec![Ty::Word, Ty::Word, Ty::I64, Ty::I64, Ty::Bool];

    assert!(
        GlobalValueNumbering
            .run(&mut function, &Module::default())
            .fixture()
    );
    assert_eq!(function.blocks[0].ops.len(), 7);
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Return {
            values: vec![
                first_convert,
                first_load,
                first_const,
                first_move,
                first_compare
            ]
        }
    );
}

#[test]
fn sccp_folds_self_comparisons_and_matching_switch() {
    let mut builder = FunctionBuilder::new(
        FunctionId(23),
        "self-comparisons",
        vec![Param {
            name: "object".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
    );
    let eq = builder
        .push_op(
            OpKind::Prim {
                op: Prim::Eq,
                args: vec![ValueId(0), ValueId(0)],
                condition: None,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    for primitive in [Prim::Eql] {
        builder
            .push_op(
                OpKind::Prim {
                    op: primitive,
                    args: vec![ValueId(0), ValueId(0)],
                    condition: None,
                },
                &[Ty::Bool],
            )
            .fixture();
    }
    for comparison in [
        ncl_ir::Compare::Eq,
        ncl_ir::Compare::Ne,
        ncl_ir::Compare::Lt,
        ncl_ir::Compare::Le,
        ncl_ir::Compare::Gt,
        ncl_ir::Compare::Ge,
    ] {
        builder
            .push_op(
                OpKind::Compare {
                    op: comparison,
                    left: ValueId(0),
                    right: ValueId(0),
                },
                &[Ty::Bool],
            )
            .fixture();
    }
    let then_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    let else_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    builder.position_at(BlockId(0)).fixture();
    builder
        .terminate(Terminator::Branch {
            condition: eq,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .fixture();
    let mut function = builder.finish();
    assert!(Sccp.run(&mut function, &Module::default()).fixture());
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Jump {
            target: then_block,
            args: Vec::new()
        }
    );
    assert_eq!(function.blocks.len(), 2);
    assert!(
        function.blocks[0]
            .ops
            .iter()
            .all(|op| { matches!(op.kind, OpKind::Prim { .. } | OpKind::Compare { .. }) })
    );
}

#[test]
fn sccp_keeps_overdefined_branches_and_empty_functions_unchanged() {
    let mut builder = FunctionBuilder::new(
        FunctionId(24),
        "unknown-branch",
        vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        Vec::new(),
    );
    let then_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    let else_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    builder.position_at(BlockId(0)).fixture();
    builder
        .terminate(Terminator::Branch {
            condition: ValueId(0),
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .fixture();
    let mut function = builder.finish();
    assert!(!Sccp.run(&mut function, &Module::default()).fixture());
    assert_eq!(function.blocks.len(), 3);
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Branch { .. }
    ));

    let mut empty = Function {
        id: FunctionId(25),
        name: "empty".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    assert!(!Sccp.run(&mut empty, &Module::default()).fixture());
}

#[test]
fn inline_remaps_nested_constants_and_all_call_results() {
    let callee = Function {
        id: FunctionId(27),
        name: "nested-constants".into(),
        params: Vec::new(),
        return_types: vec![Ty::Word; 4],
        constants: vec![
            Constant::Object(ncl_ir::ConstantIndex(1)),
            Constant::Fixnum(7),
            Constant::Character(65),
            Constant::Structure {
                kind: ncl_ir::StructureKind::Cons,
                elements: vec![ncl_ir::ConstantIndex(1), ncl_ir::ConstantIndex(2)],
            },
            Constant::Ratio {
                numerator: ncl_ir::ConstantIndex(1),
                denominator: ncl_ir::ConstantIndex(2),
            },
            Constant::Complex {
                real: ncl_ir::ConstantIndex(3),
                imaginary: ncl_ir::ConstantIndex(4),
            },
        ],
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            ops: vec![
                ncl_ir::Op {
                    results: vec![(ValueId(0), Ty::Word)],
                    kind: OpKind::Const {
                        result: ncl_ir::ConstantIndex(0),
                    },
                    loc: None,
                },
                ncl_ir::Op {
                    results: vec![(ValueId(1), Ty::Word)],
                    kind: OpKind::Const {
                        result: ncl_ir::ConstantIndex(3),
                    },
                    loc: None,
                },
                ncl_ir::Op {
                    results: vec![(ValueId(2), Ty::Word)],
                    kind: OpKind::Const {
                        result: ncl_ir::ConstantIndex(4),
                    },
                    loc: None,
                },
                ncl_ir::Op {
                    results: vec![(ValueId(3), Ty::Word)],
                    kind: OpKind::Const {
                        result: ncl_ir::ConstantIndex(5),
                    },
                    loc: None,
                },
            ],
            terminator: Terminator::Return {
                values: vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
            },
        }],
        locals: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let mut caller = Function {
        id: FunctionId(26),
        name: "caller".into(),
        params: Vec::new(),
        return_types: vec![Ty::Word; 4],
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            ops: vec![
                ncl_ir::Op {
                    results: vec![(ValueId(0), Ty::Word)],
                    kind: OpKind::Const {
                        result: ncl_ir::ConstantIndex(0),
                    },
                    loc: None,
                },
                ncl_ir::Op {
                    results: vec![
                        (ValueId(1), Ty::Word),
                        (ValueId(2), Ty::Word),
                        (ValueId(3), Ty::Word),
                        (ValueId(4), Ty::Word),
                    ],
                    kind: OpKind::Call {
                        function: ValueId(0),
                        args: Vec::new(),
                    },
                    loc: None,
                },
            ],
            terminator: Terminator::Return {
                values: vec![ValueId(1), ValueId(2), ValueId(3), ValueId(4)],
            },
        }],
        locals: Vec::new(),
        constants: vec![Constant::FunctionEntry(FunctionId(27))],
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    let module = Module {
        functions: vec![caller.clone(), callee],
    };
    assert!(
        InlineDirectCalls::default()
            .run(&mut caller, &module)
            .fixture()
    );
    assert!(
        !caller.blocks[0]
            .ops
            .iter()
            .any(|op| matches!(op.kind, OpKind::Call { .. }))
    );
    assert_eq!(
        caller.blocks[0].terminator,
        Terminator::Return {
            values: vec![ValueId(5), ValueId(6), ValueId(7), ValueId(8)]
        }
    );
    assert_eq!(caller.constants.len(), 7);
    assert_eq!(
        caller.constants[1],
        Constant::Object(ncl_ir::ConstantIndex(2))
    );
    assert_eq!(
        caller.constants[3],
        Constant::Structure {
            kind: ncl_ir::StructureKind::Cons,
            elements: vec![ncl_ir::ConstantIndex(2), ncl_ir::ConstantIndex(4)]
        }
    );
    assert_eq!(
        caller.constants[5],
        Constant::Ratio {
            numerator: ncl_ir::ConstantIndex(2),
            denominator: ncl_ir::ConstantIndex(4)
        }
    );
    assert_eq!(
        caller.constants[6],
        Constant::Complex {
            real: ncl_ir::ConstantIndex(3),
            imaginary: ncl_ir::ConstantIndex(5)
        }
    );
}

#[test]
fn dce_keeps_handler_targets_and_prim_condition_targets_reachable() {
    let mut builder = FunctionBuilder::new(
        FunctionId(28),
        "dce-reachability",
        vec![Param {
            name: "object".into(),
            ty: Ty::Word,
        }],
        Vec::new(),
    );
    let handler = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    builder.position_at(BlockId(0)).fixture();
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::Car,
                args: vec![ValueId(0)],
                condition: Some(handler),
            },
            &[Ty::Word],
        )
        .fixture();
    builder
        .push_op(
            OpKind::EnterHandler {
                region: HandlerRegionId(0),
            },
            &[],
        )
        .fixture();
    builder
        .push_op(
            OpKind::LeaveHandler {
                region: HandlerRegionId(0),
            },
            &[],
        )
        .fixture();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    let _dead = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    builder.position_at(BlockId(0)).fixture();
    builder.add_handler_region(HandlerRegion {
        id: HandlerRegionId(0),
        kind: HandlerKind::UnwindProtect,
        protected: vec![BlockId(0)],
        handler,
        cleanup: Some(handler),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });

    let mut function = builder.finish();
    let changed = DeadCodeElimination
        .run(&mut function, &Module::default())
        .unwrap_or_else(|error| panic!("DCE failed: {error:?}"));
    assert!(changed);
    assert_eq!(function.blocks.len(), 2);
}

#[test]
fn dce_handles_empty_functions_without_inventing_blocks() {
    let mut function = Function {
        id: FunctionId(29),
        name: "empty-dce".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    assert!(
        !DeadCodeElimination
            .run(&mut function, &Module::default())
            .fixture()
    );
    assert!(function.blocks.is_empty());
}

#[test]
fn gvn_invalidates_redundant_loads_after_mutating_primitives() {
    let mut builder = FunctionBuilder::new(
        FunctionId(30),
        "gvn-memory",
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
            values: vec![first, second],
        })
        .fixture();

    let mut function = builder.finish();
    let changed = GlobalValueNumbering
        .run(&mut function, &Module::default())
        .unwrap_or_else(|error| panic!("GVN failed: {error:?}"));
    assert!(!changed);
    assert_eq!(
        function.blocks[0]
            .ops
            .iter()
            .filter(|op| matches!(op.kind, OpKind::Prim { op: Prim::Car, .. }))
            .count(),
        2
    );
    ncl_ir::verify(&function).fixture();
}

#[test]
fn gvn_accepts_empty_functions_as_a_noop() {
    let mut function = Function {
        id: FunctionId(31),
        name: "empty-gvn".into(),
        params: Vec::new(),
        return_types: Vec::new(),
        blocks: Vec::new(),
        locals: Vec::new(),
        constants: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
    };
    assert!(
        !GlobalValueNumbering
            .run(&mut function, &Module::default())
            .fixture()
    );
}

#[test]
fn sccp_folds_false_branches_and_rewrites_only_matching_constant_types() {
    let mut builder = FunctionBuilder::new(
        FunctionId(32),
        "sccp-false",
        Vec::new(),
        vec![Ty::Word, Ty::F64],
    );
    let one = builder.add_constant(Constant::Fixnum(1));
    let two = builder.add_constant(Constant::Fixnum(2));
    let truth = builder.add_constant(Constant::T);
    let float = builder.add_constant(Constant::DoubleFloat(1.5));
    let left = builder
        .push_op(OpKind::Const { result: one }, &[Ty::I64])
        .fixture()[0];
    let right = builder
        .push_op(OpKind::Const { result: two }, &[Ty::I64])
        .fixture()[0];
    let false_condition = builder
        .push_op(
            OpKind::Compare {
                op: ncl_ir::Compare::Eq,
                left,
                right,
            },
            &[Ty::Bool],
        )
        .fixture()[0];
    let word_value = builder
        .push_op(OpKind::Const { result: truth }, &[Ty::Word])
        .fixture()[0];
    let word_move = builder
        .push_op(OpKind::Move { value: word_value }, &[Ty::Word])
        .fixture()[0];
    let float_value = builder
        .push_op(OpKind::Const { result: float }, &[Ty::F64])
        .fixture()[0];
    let float_move = builder
        .push_op(OpKind::Move { value: float_value }, &[Ty::F64])
        .fixture()[0];
    let then_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return {
            values: vec![word_move, float_move],
        })
        .fixture();
    let else_block = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return {
            values: vec![word_move, float_move],
        })
        .fixture();
    builder.position_at(BlockId(0)).fixture();
    builder
        .terminate(Terminator::Branch {
            condition: false_condition,
            then_target: then_block,
            then_args: Vec::new(),
            else_target: else_block,
            else_args: Vec::new(),
        })
        .fixture();

    let mut function = builder.finish();
    let changed = Sccp
        .run(&mut function, &Module::default())
        .unwrap_or_else(|error| panic!("SCCP failed: {error:?}"));
    assert!(changed);
    assert_eq!(
        function.blocks[0].terminator,
        Terminator::Jump {
            target: else_block,
            args: Vec::new()
        }
    );
    assert!(matches!(
        function.blocks[0]
            .ops
            .iter()
            .find(|op| op.results.iter().any(|(value, _)| *value == word_move))
            .map(|op| &op.kind),
        Some(OpKind::Const { .. })
    ));
    assert!(matches!(
        function.blocks[0]
            .ops
            .iter()
            .find(|op| op.results.iter().any(|(value, _)| *value == float_move))
            .map(|op| &op.kind),
        Some(OpKind::Const { .. })
    ));
}
