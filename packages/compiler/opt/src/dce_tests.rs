use crate::{DeadCodeElimination, FunctionPass, Module};
use ncl_ir::{FunctionBuilder, FunctionId, Param, Terminator, Ty, ValueId};

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
