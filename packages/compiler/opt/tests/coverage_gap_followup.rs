#![allow(clippy::expect_used, missing_docs)]

use ncl_ir::{
    BasicBlock, BlockId, Constant, Function, FunctionId, Op, OpKind, Param, Prim, Terminator, Ty,
    ValueId, verify,
};
use ncl_opt::{FunctionPass, Module, Sccp};

fn assert_ok<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|error| panic!("unexpected optimizer error: {error:?}"))
}

#[test]
fn sccp_keeps_a_branch_with_an_unknown_parameter_condition() {
    let function = Function {
        id: FunctionId(90),
        name: "unknown-branch".into(),
        params: vec![Param {
            name: "condition".into(),
            ty: Ty::Bool,
        }],
        return_types: Vec::new(),
        constants: Vec::new(),
        locals: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
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
    };
    let mut function = function;
    let changed = assert_ok(Sccp.run(&mut function, &Module::default()));
    assert!(!changed);
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Branch { .. }
    ));
    assert_ok(verify(&function));
}

#[test]
fn sccp_keeps_unknown_switches_and_overdefined_arithmetic() {
    let mut function = Function {
        id: FunctionId(91),
        name: "unknown-switch".into(),
        params: vec![Param {
            name: "value".into(),
            ty: Ty::I64,
        }],
        return_types: Vec::new(),
        constants: vec![Constant::Fixnum(1)],
        locals: Vec::new(),
        handler_regions: Vec::new(),
        debug: Vec::new(),
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops: vec![
                    Op {
                        results: vec![(ValueId(1), Ty::I64)],
                        loc: None,
                        kind: OpKind::LoadArg { index: 0 },
                    },
                    Op {
                        results: vec![(ValueId(2), Ty::I64)],
                        loc: None,
                        kind: OpKind::Prim {
                            op: Prim::FixnumAdd,
                            args: vec![ValueId(1), ValueId(1)],
                            condition: None,
                        },
                    },
                ],
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
    };
    let changed = assert_ok(Sccp.run(&mut function, &Module::default()));
    assert!(!changed);
    assert!(matches!(
        function.blocks[0].terminator,
        Terminator::Switch { .. }
    ));
    assert!(matches!(
        function.blocks[0].ops[1].kind,
        OpKind::Prim {
            op: Prim::FixnumAdd,
            ..
        }
    ));
    assert_ok(verify(&function));
}
