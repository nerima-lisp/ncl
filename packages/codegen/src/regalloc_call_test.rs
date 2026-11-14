use super::{AllocationTarget, Location, allocate};
use ncl_ir::{FunctionBuilder, FunctionId, OpKind, Param, Prim, Terminator, Ty, ValueId};

#[test]
fn call_crossing_values_are_spilled_for_both_targets() {
    let mut builder = FunctionBuilder::new(
        FunctionId(2),
        "call-crossing",
        vec![
            Param {
                name: "function".into(),
                ty: Ty::Word,
            },
            Param {
                name: "live".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word],
    );
    let constant = builder.add_constant(ncl_ir::Constant::Fixnum(0));
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
                    args: Vec::new(),
                },
                &[Ty::Word],
            )
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![ValueId(1)],
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
fn cons_accessors_spill_operands_and_values_across_the_runtime_slow_path() {
    for (index, prim) in [Prim::Car, Prim::Cdr].into_iter().enumerate() {
        let mut builder = FunctionBuilder::new(
            FunctionId(u32::try_from(index).expect("function id")),
            "cons-accessor-live-value",
            vec![Param {
                name: "object".into(),
                ty: Ty::Word,
            }],
            vec![Ty::Word],
        );
        let live_constant = builder.add_constant(ncl_ir::Constant::Fixnum(17));
        let live = builder
            .push_op(
                OpKind::Const {
                    result: live_constant,
                },
                &[Ty::Word],
            )
            .expect("live value")[0];
        assert!(
            builder
                .push_op(
                    OpKind::Prim {
                        op: prim,
                        args: vec![ValueId(0)],
                        condition: None,
                    },
                    &[Ty::Word],
                )
                .is_ok()
        );
        assert!(
            builder
                .terminate(Terminator::Return { values: vec![live] })
                .is_ok()
        );

        let allocation = allocate(&builder.finish(), AllocationTarget::AArch64);
        let Some(datum_interval) = allocation
            .intervals
            .iter()
            .find(|interval| interval.value == ValueId(0))
        else {
            panic!("datum interval");
        };
        assert!(datum_interval.crosses_call);
        assert!(matches!(
            allocation.location(ValueId(0)),
            Some(Location::Spill(_))
        ));
        let Some(interval) = allocation
            .intervals
            .iter()
            .find(|interval| interval.value == live)
        else {
            panic!("live value interval");
        };
        assert!(interval.crosses_call);
        assert!(matches!(
            allocation.location(live),
            Some(Location::Spill(_))
        ));
    }
}
