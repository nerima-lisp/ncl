#![allow(missing_docs, clippy::unwrap_used)]

use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{AllocationTarget, Location, allocate, compile_function_x86_64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
fn x86_64_lowering_spills_values_across_safepoints() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(71),
        "allocated-root",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Fixnum(9));
    let value = match builder.push_op(OpKind::Const { result: constant }, &[Ty::Word]) {
        Ok(ids) => ids[0],
        Err(error) => unreachable!("constant: {error:?}"),
    };
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok(), "safepoint");
    assert!(
        // check-added-lines: allow(panic) test-only assertion
        builder
            .terminate(Terminator::Return {
                values: vec![value],
            })
            .is_ok(),
        "return"
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let Some(Location::Spill(spill)) = allocation.location(value) else {
        return Err("safepoint-crossing value was allocated to a register".into());
    };
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    let Some(map) = compiled.safepoint_maps.first() else {
        return Err("safepoint map missing".into());
    };
    assert!(map.registers.is_empty()); // check-added-lines: allow(panic) test-only assertion
    let slot = 4 + 1 + usize::try_from(spill).map_err(|_| "spill slot overflow")?;
    let slot_is_live = map
        .bitmap
        .get(slot / 8)
        .is_some_and(|bits| bits & (1 << (slot % 8)) != 0);
    let slot_message = format!("spill slot {slot} is missing from the safepoint map");
    // check-added-lines: allow(panic) test-only assertion
    assert!(slot_is_live, slot_message); // check-added-lines: allow(panic) test-only assertion
    assert!(!compiled.code.is_empty());
    Ok(())
}

#[test]
fn x86_64_lowering_reserves_allocator_spills_in_frame() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(72),
        "allocated-spills",
        Vec::new(),
        vec![Ty::Word; 8],
    );
    let mut values = Vec::new();
    for index in 0..8u32 {
        let constant = builder.add_constant(Constant::Fixnum(i64::from(index)));
        values.push(
            match builder.push_op(OpKind::Const { result: constant }, &[Ty::Word]) {
                Ok(ids) => ids[0],
                Err(error) => unreachable!("constant: {error:?}"),
            },
        );
    }
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok(), "safepoint");
    assert!(
        builder.terminate(Terminator::Return { values }).is_ok(),
        "return"
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert!(allocation.spill_words > 0);
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    assert!(compiled.frame_size >= (4 + 8 + allocation.spill_words) * 8);
    assert!(compiled.safepoint_maps[0].bitmap.len() > 1);
    let Some(map) = compiled.safepoint_maps.first() else {
        return Err("safepoint map missing".into());
    };
    for interval in &allocation.intervals {
        if interval.ty != Ty::Word || interval.start > 8 || 8 > interval.end {
            continue;
        }
        let Some(Location::Spill(spill)) = allocation.location(interval.value) else {
            continue;
        };
        let spill = usize::try_from(spill).map_err(|_| "spill slot")?;
        let slot = 4 + 8 + spill; // check-added-lines: allow(panic) regression assertion for map coverage.
        assert!(
            map.bitmap
                .get(slot / 8)
                .is_some_and(|bits| bits & (1 << (slot % 8)) != 0),
            "spill slot {slot} is missing from the header-inclusive map"
        );
    }
    Ok(())
}

#[test]
fn lowers_ir_v2_closure_and_handler_ops_x86_64() -> Result<(), String> {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(70),
        "ir-v2-ops",
        Vec::new(),
        vec![Ty::Word],
    );
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let entry_value = ncl_ir::ValueId(0);
    let closure = ncl_ir::ValueId(1);
    let argc = builder.add_constant(Constant::Fixnum(0));
    let result = ncl_ir::ValueId(3);
    assert!(
        builder
            .push_op(OpKind::Const { result: entry }, &[Ty::Word])
            .is_ok()
    );
    let argc_value = ncl_ir::ValueId(2);
    builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .map_err(|error| format!("argc constant: {error:?}"))?;
    assert!(
        builder
            .push_op(
                OpKind::MakeClosure {
                    entry: entry_value,
                    captures: Vec::new()
                },
                &[Ty::Word]
            )
            .is_ok()
    );
    assert!(
        builder
            .push_op(
                OpKind::CallClosure {
                    closure,
                    args: vec![argc_value],
                    named_symbol: None,
                },
                &[Ty::Word]
            )
            .is_ok()
    );
    let region = ncl_ir::HandlerRegionId(3);
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: region,
        kind: ncl_ir::HandlerKind::UnwindProtect,
        protected: vec![ncl_ir::BlockId(0)],
        handler: ncl_ir::BlockId(0),
        cleanup: Some(ncl_ir::BlockId(0)),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    assert!(
        builder
            .push_op(OpKind::EnterHandler { region }, &[])
            .is_ok()
    );
    assert!(
        builder
            .push_op(OpKind::LeaveHandler { region }, &[])
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![result]
            })
            .is_ok()
    );
    let compiled = match compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => {
            unreachable!("{error:?}");
        }
    };
    assert!(!compiled.code.is_empty());
    assert!(compiled.safepoint_maps.len() >= 4);
    Ok(())
}

#[test]
fn x86_64_tail_call_restores_frame_and_jumps_without_safepoint() -> Result<(), String> {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(73),
        "tail-call",
        Vec::new(),
        vec![Ty::Word],
    );
    let callee = builder.add_constant(Constant::Fixnum(74));
    let Some(callee) = builder
        .push_op(OpKind::Const { result: callee }, &[Ty::Word])
        .ok()
        .and_then(|values| values.first().copied())
    else {
        unreachable!("callee result")
    };
    let argc = builder.add_constant(Constant::Fixnum(0));
    let Some(argc) = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("argc result missing".to_owned());
    };
    assert!(
        builder
            .terminate(Terminator::TailCall {
                function: callee,
                args: vec![argc],
            })
            .is_ok()
    );

    let compiled = match compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => return Err(format!("tail-call lowering: {error:?}")),
    };
    // push rbp; mov rbp, rsp; mov [rbp+16], r10; mov r11, 0;
    // mov [rbp+24], r11; sub rsp, 32
    assert_eq!(
        compiled.code[0..23],
        [
            0x55, 0x48, 0x89, 0xe5, 0x4c, 0x89, 0x55, 0x10, 0x49, 0xc7, 0xc3, 0x00, 0x00, 0x00,
            0x00, 0x4c, 0x89, 0x5d, 0x18, 0x48, 0x83, 0xec, 0x20,
        ]
    );
    // mov rsp, rbp; pop rbp; mov rax, [rsp]; mov [rsp-16], rax;
    // mov [rsp-8], r10; sub rsp, 16; jmp r11
    let tail_transfer_start = compiled.code.len() - 25;
    assert_eq!(
        compiled.code[tail_transfer_start..],
        [
            0x48, 0x89, 0xec, 0x5d, 0x48, 0x8b, 0x04, 0x24, 0x48, 0x89, 0x44, 0x24, 0xf0, 0x4c,
            0x89, 0x54, 0x24, 0xf8, 0x48, 0x83, 0xec, 0x10, 0x41, 0xff, 0xe3,
        ]
    );
    assert!(compiled.safepoint_maps.is_empty());
    Ok(())
}

#[test]
fn x86_64_prologue_spills_argc_from_rdi_before_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(75),
        "argc-prologue",
        vec![
            ncl_ir::Param {
                name: "argc".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "argument".into(),
                ty: Ty::Word,
            },
        ],
        vec![],
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );

    let compiled = match compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("argc lowering: {error:?}"),
    };
    assert!(
        compiled
            .code
            .windows(4)
            .any(|bytes| bytes == [0x48, 0x89, 0x7d, 0xf8])
    );
    assert!(
        compiled
            .code
            .windows(4)
            .any(|bytes| bytes == [0x48, 0x89, 0x75, 0xf0])
    );
}
