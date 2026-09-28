#![allow(
    missing_docs,
    clippy::expect_used,
    clippy::too_many_lines,
    clippy::unwrap_used
)]

use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{AllocationTarget, Location, allocate, compile_function_x86_64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
#[cfg(test)]
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
    assert!(slot_is_live, "{}", slot_message); // check-added-lines: allow(panic) test-only assertion
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
fn x86_64_closure_call_keeps_captures_in_argument_registers() -> Result<(), String> {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(77),
        "closure-capture-abi",
        Vec::new(),
        vec![Ty::Word],
    );
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let entry = builder
        .push_op(OpKind::Const { result: entry }, &[Ty::Word])
        .map_err(|error| format!("entry: {error:?}"))?[0];
    let argc = builder.add_constant(Constant::Fixnum(0));
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .map_err(|error| format!("argc: {error:?}"))?[0];
    let argument = builder.add_constant(Constant::Fixnum(1));
    let argument = builder
        .push_op(OpKind::Const { result: argument }, &[Ty::Word])
        .map_err(|error| format!("argument: {error:?}"))?[0];
    let second_argument = builder.add_constant(Constant::Fixnum(2));
    let second_argument = builder
        .push_op(
            OpKind::Const {
                result: second_argument,
            },
            &[Ty::Word],
        )
        .map_err(|error| format!("second argument: {error:?}"))?[0];
    let closure = builder
        .push_op(
            OpKind::MakeClosure {
                entry,
                captures: vec![entry; 3],
            },
            &[Ty::Word],
        )
        .map_err(|error| format!("closure: {error:?}"))?[0];
    let result = builder
        .push_op(
            OpKind::CallClosure {
                closure,
                args: vec![
                    argc,
                    argument,
                    second_argument,
                    argument,
                    second_argument,
                    argument,
                ],
                named_symbol: None,
            },
            &[Ty::Word],
        )
        .map_err(|error| format!("call closure: {error:?}"))?[0];
    builder
        .terminate(Terminator::Return {
            values: vec![result],
        })
        .map_err(|error| format!("return: {error:?}"))?;

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi)
        .map_err(|error| format!("compile: {error:?}"))?;
    let code = &compiled.code;
    for capture_load in [
        [0x48, 0x8b, 0x70, 0x28], // mov 40(%rax), %rsi
        [0x48, 0x8b, 0x50, 0x30], // mov 48(%rax), %rdx
        [0x48, 0x8b, 0x48, 0x38], // mov 56(%rax), %rcx
    ] {
        assert!(
            code.windows(capture_load.len())
                .any(|bytes| bytes == capture_load),
            "capture load missing: {capture_load:02x?} in {code:02x?}"
        );
    }
    for capture_overwrite in [
        [0x4c, 0x89, 0xde], // mov %r11, %rsi
        [0x4c, 0x89, 0xda], // mov %r11, %rdx
        [0x4c, 0x89, 0xd9], // mov %r11, %rcx
    ] {
        assert!(
            !code
                .windows(capture_overwrite.len())
                .any(|bytes| bytes == capture_overwrite),
            "capture was overwritten by ENTRY: {capture_overwrite:02x?} in {code:02x?}"
        );
    }
    assert!(
        code.windows(3).any(|bytes| bytes == [0x4c, 0x89, 0x5d]),
        "fifth closure argument was not spilled through the outgoing ABI area: {code:02x?}"
    );
    let outgoing_store_displacements = code
        .windows(4)
        .filter_map(|bytes| (bytes[..3] == [0x4c, 0x89, 0x5d]).then_some(bytes[3]))
        .collect::<Vec<_>>();
    assert!(
        outgoing_store_displacements.windows(4).any(|window| {
            window[1] == window[0].wrapping_add(8)
                && window[2] == window[1].wrapping_add(8)
                && window[3] == window[2].wrapping_add(8)
        }),
        "closure overflow arguments must pack from the lowest address: {outgoing_store_displacements:02x?} in {code:02x?}"
    );
    Ok(())
}

#[test]
fn x86_64_dispatches_catch_after_unwind_protect_cleanup_builtin() -> Result<(), String> {
    let compiled = compile_function_x86_64(&cleanup_dispatch_function()?, &X86_64FixtureAbi)
        .map_err(|error| format!("compile: {error:?}"))?;
    let cleanup_call = compiled
        .code
        .windows(3)
        .enumerate()
        .position(|(offset, bytes)| {
            matches!(bytes, [0x41, 0xff, 0xd3])
                && compiled.code.get(offset + 3..offset + 7) == Some(CLEANUP_EPILOGUE)
                && compiled.code.get(offset + 7..offset + 10) == Some(CATCH_TAG_LOAD)
        })
        .ok_or_else(|| "cleanup builtin call missing".to_owned())?;
    let catch_bytes = compiled
        .code
        .get(cleanup_call + 3..)
        .ok_or_else(|| "cleanup call extends past generated code".to_owned())?;
    let catch_compare = catch_bytes
        .windows(3)
        .position(|bytes| bytes == [0x4d, 0x39, 0xda])
        .map(|offset| cleanup_call + 3 + offset)
        .ok_or_else(|| "catch tag comparison after cleanup builtin missing".to_owned())?;
    let jump = compiled
        .code
        .get(catch_compare + 3..catch_compare + 5)
        .ok_or_else(|| "catch comparison has no conditional branch".to_owned())?;
    if jump != [0x0f, 0x85] {
        return Err("catch comparison does not branch on mismatch".to_owned());
    }
    Ok(())
}

const CLEANUP_EPILOGUE: &[u8] = &[0x48, 0x83, 0xc4, 0x10];
const CATCH_TAG_LOAD: &[u8] = &[0x4d, 0x8b, 0x97];

fn cleanup_dispatch_function() -> Result<ncl_ir::Function, String> {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(76),
        "catch-unwind-cleanup-dispatch",
        Vec::new(),
        vec![],
    );
    let tag = builder.add_constant(Constant::Fixnum(1));
    let tag = builder
        .push_op(OpKind::Const { result: tag }, &[Ty::Word])
        .map_err(|error| format!("catch tag: {error:?}"))?
        .first()
        .copied()
        .ok_or_else(|| "catch tag result missing".to_owned())?;
    let protected = builder.create_block(Vec::new());
    let cleanup = builder.create_block(Vec::new());
    let catch_handler = builder.create_block(Vec::new());

    let catch = ncl_ir::HandlerRegionId(1);
    let unwind = ncl_ir::HandlerRegionId(2);
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: catch,
        kind: ncl_ir::HandlerKind::Catch,
        protected: vec![ncl_ir::BlockId(0), protected, cleanup],
        handler: catch_handler,
        cleanup: None,
        catch_tag: Some(tag),
        binding_targets: Vec::new(),
        depth: 0,
        parent: None,
    });
    builder.add_handler_region(ncl_ir::HandlerRegion {
        id: unwind,
        kind: ncl_ir::HandlerKind::UnwindProtect,
        protected: vec![protected],
        handler: cleanup,
        cleanup: Some(cleanup),
        catch_tag: None,
        binding_targets: Vec::new(),
        depth: 0,
        parent: Some(catch),
    });
    builder
        .push_op(OpKind::EnterHandler { region: catch }, &[])
        .map_err(|error| format!("enter catch: {error:?}"))?;
    builder
        .push_op(OpKind::EnterHandler { region: unwind }, &[])
        .map_err(|error| format!("enter unwind-protect: {error:?}"))?;
    builder
        .terminate(Terminator::Jump {
            target: protected,
            args: Vec::new(),
        })
        .map_err(|error| format!("entry jump: {error:?}"))?;

    builder
        .position_at(protected)
        .map_err(|error| format!("protected block: {error}"))?;
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .map_err(|error| format!("protected return: {error:?}"))?;

    builder
        .position_at(cleanup)
        .map_err(|error| format!("cleanup block: {error}"))?;
    builder
        .push_op(
            OpKind::Builtin {
                name: "cleanup-test".to_owned(),
                args: Vec::new(),
            },
            &[],
        )
        .map_err(|error| format!("cleanup builtin: {error:?}"))?;
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .map_err(|error| format!("cleanup return: {error:?}"))?;

    builder
        .position_at(catch_handler)
        .map_err(|error| format!("catch handler: {error}"))?;
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .map_err(|error| format!("catch return: {error:?}"))?;

    Ok(builder.finish())
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
    // mov rsp, rbp; pop rbp; mov [rsp+8], r10; mov rax, 0;
    // mov [rsp+16], rax; jmp r11
    let tail_transfer_start = compiled.code.len() - 24;
    assert_eq!(
        compiled.code[tail_transfer_start..],
        [
            0x48, 0x89, 0xec, 0x5d, 0x4c, 0x89, 0x54, 0x24, 0x08, 0x48, 0xc7, 0xc0, 0x00, 0x00,
            0x00, 0x00, 0x48, 0x89, 0x44, 0x24, 0x10, 0x41, 0xff, 0xe3,
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

    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert_eq!(
        allocation.location(ncl_ir::ValueId(0)),
        Some(Location::Register(11))
    );
    assert_eq!(
        allocation.location(ncl_ir::ValueId(1)),
        Some(Location::Register(12))
    );
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("argc lowering: {error:?}"),
    };
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xfc])
    );
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xf5])
    );
}

#[test]
fn x86_64_prologue_loads_overflow_arguments_from_r9() {
    let params = (0..6)
        .map(|index| ncl_ir::Param {
            name: format!("arg{index}"),
            ty: Ty::Word,
        })
        .collect();
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(76),
        "overflow-arguments",
        params,
        Vec::new(),
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert!(matches!(
        allocation.location(ncl_ir::ValueId(4)),
        Some(Location::Spill(_))
    ));
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("overflow argument lowering: {error:?}"),
    };
    assert!(
        compiled
            .code
            .windows(4)
            .any(|bytes| bytes == [0x4d, 0x8b, 0x19, 0x4c])
    );
    assert!(
        compiled
            .code
            .windows(4)
            .any(|bytes| bytes == [0x4d, 0x8b, 0x59, 0x08])
    );
}

#[test]
fn x86_64_prologue_initializes_load_arg_sources() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(78),
        "load-arg-result-values",
        vec![
            ncl_ir::Param {
                name: "left".into(),
                ty: Ty::Word,
            },
            ncl_ir::Param {
                name: "right".into(),
                ty: Ty::Word,
            },
        ],
        vec![Ty::Word, Ty::Word],
    );
    let left = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .unwrap_or_else(|error| panic!("left LoadArg: {error}"))[0];
    let right = builder
        .push_op(OpKind::LoadArg { index: 2 }, &[Ty::Word])
        .unwrap_or_else(|error| panic!("right LoadArg: {error}"))[0];
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![left, right],
            })
            .is_ok()
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    assert_eq!(
        allocation.location(ncl_ir::ValueId(1)),
        Some(Location::Register(12))
    );
    assert_eq!(
        allocation.location(ncl_ir::ValueId(2)),
        Some(Location::Register(13))
    );
    let compiled = compile_function_x86_64(&function, &X86_64FixtureAbi)
        .unwrap_or_else(|error| panic!("entry parameter lowering: {error:?}"));
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xf4])
    );
    assert!(
        compiled
            .code
            .windows(3)
            .any(|bytes| bytes == [0x49, 0x89, 0xd5])
    );
}

#[test]
fn x86_64_load_heap_constant_untags_each_indirection() -> Result<(), String> {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(77),
        "untag-heap-constant",
        Vec::new(),
        vec![Ty::Word],
    );
    let constant = builder.add_constant(Constant::Symbol {
        package: "COMMON-LISP".to_owned(),
        name: "T".to_owned(),
    });
    let value = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .map_err(|error| format!("heap constant: {error:?}"))?[0];
    builder
        .terminate(Terminator::Return {
            values: vec![value],
        })
        .map_err(|error| format!("return: {error:?}"))?;

    let compiled = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi)
        .map_err(|error| format!("compile: {error:?}"))?;
    let untag_count = compiled
        .code
        .windows(4)
        .filter(|bytes| matches!(bytes, [0x49, 0x83, _, 0xf8]))
        .count();
    if untag_count != 3 {
        return Err(format!(
            "heap constant indirections must be untagged: found {untag_count}"
        ));
    }
    Ok(())
}
