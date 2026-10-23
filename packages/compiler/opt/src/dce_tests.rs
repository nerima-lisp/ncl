use crate::tests_support::Fixture;
use crate::{DeadCodeElimination, FunctionPass, Module};
use ncl_ir::{
    BlockId, FunctionBuilder, FunctionId, HandlerKind, HandlerRegion, HandlerRegionId, OpKind,
    Param, Terminator, Ty, ValueId,
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

#[test]
fn preserves_values_used_by_handler_region_metadata() {
    let mut builder = FunctionBuilder::new(
        FunctionId(3),
        "dce-handler-metadata",
        vec![
            Param {
                name: "symbols".into(),
                ty: Ty::Word,
            },
            Param {
                name: "values".into(),
                ty: Ty::Word,
            },
        ],
        Vec::new(),
    );
    let entry = BlockId(0);
    let region = HandlerRegionId(0);
    let symbols = builder
        .push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .fixture()[0];
    let values = builder
        .push_op(OpKind::Move { value: ValueId(1) }, &[Ty::Word])
        .fixture()[0];
    builder
        .push_op(OpKind::EnterHandler { region }, &[])
        .fixture();
    builder
        .push_op(OpKind::LeaveHandler { region }, &[])
        .fixture();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    let handler = builder.create_block(Vec::new());
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .fixture();
    builder.add_handler_region(HandlerRegion {
        id: region,
        kind: HandlerKind::Progv,
        protected: vec![entry],
        handler,
        cleanup: None,
        catch_tag: None,
        binding_targets: vec![symbols, values],
        depth: 0,
        parent: None,
    });

    let mut function = builder.finish();
    assert!(ncl_ir::verify(&function).is_ok());
    let changed = DeadCodeElimination.run(&mut function, &Module::default());
    assert!(changed.is_ok());
    assert!(!changed.unwrap_or(true));
    assert!(ncl_ir::verify(&function).is_ok());
    assert_eq!(function.blocks[0].ops.len(), 4);
}
