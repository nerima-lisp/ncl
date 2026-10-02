#![allow(clippy::unwrap_used)]

use crate::{DeadCodeElimination, FunctionPass, Module};
use ncl_ir::{Constant, FunctionBuilder, FunctionId, Param, Terminator, Ty, ValueId};

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

#[test]
fn removes_unreachable_block_without_changing_entry_return() {
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
    builder.position_at(dead).unwrap();
    let dead_constant = builder.add_constant(Constant::Fixnum(99));
    let dead_value = builder
        .push_op(
            ncl_ir::OpKind::Const {
                result: dead_constant,
            },
            &[Ty::I64],
        )
        .unwrap()[0];
    builder
        .terminate(Terminator::Return {
            values: vec![dead_value],
        })
        .unwrap();
    builder.position_at(ncl_ir::BlockId(0)).unwrap();
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(0)],
        })
        .unwrap();
    let mut function = builder.finish();
    assert_eq!(function.blocks.len(), 2);
    let before_return = function.blocks[0].terminator.clone();

    let mut pass = DeadCodeElimination;
    assert!(pass.run(&mut function, &Module::default()).unwrap());

    assert_eq!(function.blocks.len(), 1);
    assert_eq!(function.blocks[0].terminator, before_return);
    assert!(function.blocks[0].ops.is_empty());
    ncl_ir::verify(&function).unwrap();
}
