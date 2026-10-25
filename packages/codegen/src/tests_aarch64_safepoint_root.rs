#![allow(missing_docs, clippy::unwrap_used)]

use super::{Aarch64FixtureAbi, AllocationTarget, allocate, compile_function_aarch64};
use ncl_ir::{Constant, Convert, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
fn safepoint_map_keeps_root_passed_as_later_block_parameter() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(30),
        "later-block-parameter-root",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(7));
    let integer = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::I64])
        .unwrap()[0];
    let root = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: integer,
            },
            &[Ty::Word],
        )
        .unwrap()[0];
    let target = ncl_ir::BlockId(1);
    assert!(
        builder
            .terminate(Terminator::Jump {
                target,
                args: vec![root],
            })
            .is_ok()
    );
    let root_param = ncl_ir::ValueId(2);
    assert_eq!(builder.create_block(vec![(Ty::Word, root_param)]), target);
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![root_param],
            })
            .is_ok()
    );

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::AArch64);
    let spill = match allocation.location(root_param) {
        Some(crate::Location::Spill(spill)) => spill,
        location => panic!("expected block parameter spill across safepoint, got {location:?}"),
    };
    let compiled = compile_function_aarch64(&function, &Aarch64FixtureAbi::default()).unwrap();
    let map = compiled.safepoint_maps.first().unwrap();
    let root_slot = usize::try_from(crate::FRAME_HEADER_WORDS + spill).unwrap();
    let byte = map.bitmap.get(root_slot / 8).copied().unwrap_or_default();
    assert_ne!(byte & (1 << (root_slot % 8)), 0);
}
