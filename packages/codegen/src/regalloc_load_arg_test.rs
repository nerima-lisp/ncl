use super::{AllocationTarget, allocate};
use ncl_ir::{FunctionBuilder, FunctionId, OpKind, Terminator, Ty, ValueId};

#[test]
fn load_arg_keeps_parameter_live_until_a_later_block_use() {
    let mut builder = FunctionBuilder::new(
        FunctionId(3),
        "load-arg-liveness",
        vec![ncl_ir::Param {
            name: "arg".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let loaded = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .map_or(ValueId(1), |values| values[0]);
    let target = builder.create_block(Vec::new());
    assert!(builder.position_at(ncl_ir::BlockId(0)).is_ok());
    assert!(
        builder
            .terminate(Terminator::Jump {
                target,
                args: vec![]
            })
            .is_ok()
    );
    assert!(builder.position_at(target).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![loaded]
            })
            .is_ok()
    );
    let allocation = allocate(&builder.finish(), AllocationTarget::AArch64);
    let interval = allocation
        .intervals
        .iter()
        .find(|interval| interval.value == loaded);
    assert!(interval.is_some_and(|interval| interval.end > interval.start));
}
