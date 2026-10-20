use super::{AllocationTarget, Location, allocate};
use ncl_ir::{
    Constant, FunctionBuilder, FunctionId, HandlerKind, HandlerRegion, OpKind, Terminator, Ty,
    ValueId,
};

#[test]
fn linear_scan_spills_when_live_values_exceed_registers() {
    let mut builder = FunctionBuilder::new(FunctionId(0), "pressure", Vec::new(), vec![Ty::Word]);
    let mut values = Vec::new();
    for index in 0u32..8 {
        builder.add_constant(Constant::Fixnum(i64::from(index)));
        values.push(
            match builder.push_op(
                OpKind::Const {
                    result: ncl_ir::ConstantIndex(index),
                },
                &[Ty::Word],
            ) {
                Ok(result) => result[0],
                Err(_) => return,
            },
        );
    }
    if builder
        .terminate(Terminator::Return {
            values: values.clone(),
        })
        .is_err()
    {
        return;
    }
    let allocation = allocate(&builder.finish(), AllocationTarget::X86_64);
    assert!(allocation.spill_words > 0);
    assert!(
        allocation
            .locations
            .iter()
            .any(|(_, location)| matches!(location, Location::Register(_)))
    );
}

#[test]
fn handler_crossing_values_are_spilled_and_not_register_roots() {
    let mut builder = FunctionBuilder::new(
        FunctionId(1),
        "handler-crossing",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(7));
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .map_or(ValueId(0), |values| values[0]);
    builder.add_handler_region(HandlerRegion {
        id: ncl_ir::HandlerRegionId(0),
        kind: HandlerKind::UnwindProtect,
        protected: vec![ncl_ir::BlockId(0)],
        handler: ncl_ir::BlockId(0),
        cleanup: Some(ncl_ir::BlockId(0)),
        catch_tag: None,
        binding_targets: vec![value],
        depth: 0,
        parent: None,
    });
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![value]
            })
            .is_ok()
    );
    let allocation = allocate(&builder.finish(), AllocationTarget::X86_64);
    let Some(interval) = allocation
        .intervals
        .iter()
        .find(|interval| interval.value == value)
    else {
        return;
    };
    assert!(interval.crosses_handler);
    assert!(matches!(
        allocation.location(value),
        Some(Location::Spill(_))
    ));
    assert!(allocation.safepoint_registers.values().all(Vec::is_empty));
}

#[test]
fn call_crossing_values_are_spilled_for_both_targets() {
    let mut builder = FunctionBuilder::new(
        FunctionId(2),
        "call-crossing",
        vec![
            ncl_ir::Param {
                name: "function".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "live".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(0));
    assert!(
        builder
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::Call {
                    function: ValueId(0),
                    args: Vec::new()
                },
                &[Ty::Word]
            )
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![ValueId(1)]
            })
            .is_ok()
    );
    let function = builder.finish();
    for target in [AllocationTarget::X86_64, AllocationTarget::AArch64] {
        let allocation = allocate(&function, target);
        assert!(matches!(
            allocation.location(ValueId(1)),
            Some(Location::Spill(_))
        ));
    }
}

#[test]
fn move_results_coalesce_with_their_source_location() {
    let mut builder = FunctionBuilder::new(
        FunctionId(3),
        "coalesce-move",
        vec![ncl_ir::Param {
            name: "value".into(),
            ty: Ty::Word,
        }],
        vec![Ty::Word],
    );
    let moved = match builder.push_op(OpKind::Move { value: ValueId(0) }, &[Ty::Word]) {
        Ok(values) => values[0],
        Err(_) => return,
    };
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![moved]
            })
            .is_ok()
    );
    let allocation = allocate(&builder.finish(), AllocationTarget::AArch64);
    assert_eq!(allocation.location(ValueId(0)), allocation.location(moved));
}
