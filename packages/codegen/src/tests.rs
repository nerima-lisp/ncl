use super::*;
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
fn lowers_fixnum_return_to_decodable_x86() {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(0), "one", Vec::new(), vec![Ty::Word]);
    let constant = builder.add_constant(Constant::Fixnum(1));
    let values_result = builder.push_op(OpKind::Const { result: constant }, &[Ty::Word]);
    assert!(
        values_result.is_ok(),
        "builder rejected constant: {values_result:?}"
    );
    let Ok(values) = values_result else {
        return;
    };
    let value = values[0];
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![value]
            })
            .is_ok()
    );
    let compiled_result = compile_function(&builder.finish(), &X86_64Abi);
    assert!(
        compiled_result.is_ok(),
        "valid IR failed to lower: {compiled_result:?}"
    );
    let Ok(compiled) = compiled_result else {
        return;
    };
    assert!(!compiled.code.is_empty());
    assert_eq!(compiled.code.last(), Some(&0xc3));
}

#[test]
fn generates_maps_for_all_safepoint_kinds() {
    let mut allocation =
        FunctionBuilder::new(ncl_ir::FunctionId(1), "alloc", Vec::new(), vec![Ty::Word]);
    assert!(
        allocation
            .push_op(OpKind::Alloc { words: 2 }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        allocation
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let allocation_result = compile_function(&allocation.finish(), &X86_64Abi);
    assert!(
        allocation_result.is_ok(),
        "allocation lowering failed: {allocation_result:?}"
    );
    let Ok(allocation) = allocation_result else {
        return;
    };
    assert!(
        allocation
            .safepoint_maps
            .iter()
            .any(|map| map.map_flags & FLAG_ALLOCATION_SLOW != 0)
    );

    let mut explicit =
        FunctionBuilder::new(ncl_ir::FunctionId(2), "poll", Vec::new(), vec![Ty::Word]);
    assert!(explicit.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        explicit
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let explicit_result = compile_function(&explicit.finish(), &X86_64Abi);
    assert!(
        explicit_result.is_ok(),
        "poll lowering failed: {explicit_result:?}"
    );
    let Ok(explicit) = explicit_result else {
        return;
    };
    assert!(
        explicit
            .safepoint_maps
            .iter()
            .any(|map| map.map_flags & FLAG_CALL != 0)
    );

    let mut loop_function =
        FunctionBuilder::new(ncl_ir::FunctionId(3), "loop", Vec::new(), vec![Ty::Word]);
    assert!(
        loop_function
            .terminate(Terminator::Jump {
                target: ncl_ir::BlockId(0),
                args: Vec::new(),
            })
            .is_ok()
    );
    let loop_result = compile_function(&loop_function.finish(), &X86_64Abi);
    assert!(loop_result.is_ok(), "loop lowering failed: {loop_result:?}");
    let Ok(loop_function) = loop_result else {
        return;
    };
    assert!(
        loop_function
            .safepoint_maps
            .iter()
            .any(|map| map.map_flags & FLAG_LOOP_BACKEDGE != 0)
    );
}

#[test]
fn encodes_safepoint_header_and_live_slot() {
    let map_result = SafepointMap::new(7, 6, 6, &[4], &[], FLAG_CALL);
    assert!(map_result.is_ok(), "valid map rejected: {map_result:?}");
    let Ok(map) = map_result else {
        return;
    };
    let bytes_result = map.encode();
    assert!(
        bytes_result.is_ok(),
        "valid wire format rejected: {bytes_result:?}"
    );
    let Ok(bytes) = bytes_result else {
        return;
    };
    assert_eq!(&bytes[..4], &7_u32.to_le_bytes());
    assert_eq!(bytes[16] & (1 << 2), 1 << 2);
    assert_eq!(bytes[16] & (1 << 4), 1 << 4);
}
