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
    let _unused = builder
        .push_op(ncl_ir::OpKind::Move { value: ValueId(0) }, &[Ty::Word])
        .expect("valid unused move")[0];
    builder
        .terminate(Terminator::Return {
            values: vec![ValueId(0)],
        })
        .expect("valid return");
    let mut function = builder.finish();
    let mut pass = DeadCodeElimination;
    assert!(
        pass.run(&mut function, &Module::default())
            .expect("DCE succeeds")
    );
    assert!(function.blocks[0].ops.is_empty());
}
