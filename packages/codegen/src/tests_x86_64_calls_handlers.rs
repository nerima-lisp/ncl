use super::*;

#[test]
fn lowers_ir_v2_closure_and_handler_ops_x86_64() -> Result<(), String> {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(70),
        "ir-v2-ops",
        Vec::new(),
        vec![Ty::Word],
    );
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let entry_values = builder.push_op(OpKind::Const { result: entry }, &[Ty::Word]);
    assert!(entry_values.is_ok());
    let entry_value = entry_values
        .map_err(|error| format!("entry constant: {error:?}"))?
        .first()
        .copied()
        .ok_or_else(|| "entry constant result missing".to_owned())?;
    let argc = builder.add_constant(Constant::Nil);
    let argc_values = builder.push_op(OpKind::Const { result: argc }, &[Ty::Word]);
    assert!(argc_values.is_ok());
    let argc_value = argc_values
        .map_err(|error| format!("argc constant: {error:?}"))?
        .first()
        .copied()
        .ok_or_else(|| "argc constant result missing".to_owned())?;
    let closure_values = builder.push_op(
        OpKind::MakeClosure {
            entry: entry_value,
            captures: Vec::new(),
        },
        &[Ty::Word],
    );
    assert!(closure_values.is_ok());
    let closure = closure_values
        .map_err(|error| format!("make closure: {error:?}"))?
        .first()
        .copied()
        .ok_or_else(|| "closure result missing".to_owned())?;
    let result_values = builder.push_op(
        OpKind::CallClosure {
            closure,
            args: vec![argc_value],
            named_symbol: None,
        },
        &[Ty::Word],
    );
    assert!(result_values.is_ok());
    let result = result_values
        .map_err(|error| format!("call closure: {error:?}"))?
        .first()
        .copied()
        .ok_or_else(|| "call closure result missing".to_owned())?;
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
    let compiled = compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi)
        .expect("closure and handler function compiles");
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
    let argc = builder.add_constant(Constant::Nil);
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .map_err(|error| format!("argc: {error:?}"))?[0];
    let argument = builder.add_constant(Constant::T);
    let argument = builder
        .push_op(OpKind::Const { result: argument }, &[Ty::Word])
        .map_err(|error| format!("argument: {error:?}"))?[0];
    let second_argument = builder.add_constant(Constant::Unbound);
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
    let tag = builder.add_constant(Constant::T);
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
        depth: 1,
        parent: Some(catch),
    });
    builder
        .position_at(ncl_ir::BlockId(0))
        .map_err(|error| format!("entry block: {error}"))?;
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
        .push_op(OpKind::LeaveHandler { region: unwind }, &[])
        .map_err(|error| format!("leave unwind-protect: {error:?}"))?;
    builder
        .terminate(Terminator::Jump {
            target: cleanup,
            args: Vec::new(),
        })
        .map_err(|error| format!("protected jump: {error:?}"))?;

    builder
        .position_at(cleanup)
        .map_err(|error| format!("cleanup block: {error}"))?;
    builder
        .push_op(
            OpKind::Builtin {
                name: "cleanup-test".to_owned(),
                args: Vec::new(),
                specialization: None,
            },
            &[Ty::Word],
        )
        .map_err(|error| format!("cleanup builtin: {error:?}"))?;
    builder
        .push_op(OpKind::LeaveHandler { region: catch }, &[])
        .map_err(|error| format!("leave catch: {error:?}"))?;
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
fn x86_64_tail_call_restores_frame_and_jumps_without_safepoint() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(73),
        "tail-call",
        Vec::new(),
        vec![Ty::Word],
    );
    let callee = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let callee = builder
        .push_op(OpKind::Const { result: callee }, &[Ty::Word])
        .expect("callee constant operation succeeds")
        .first()
        .copied()
        .expect("callee constant operation produces a value");
    let argc = builder.add_constant(Constant::Nil);
    let argc = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::Word])
        .expect("argc constant operation succeeds")
        .first()
        .copied()
        .expect("argc constant operation produces a value");
    assert!(
        builder
            .terminate(Terminator::TailCall {
                function: callee,
                args: vec![argc],
            })
            .is_ok()
    );

    let compiled =
        compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi).expect("tail-call lowering");
    // push rbp; mov rbp, rsp; mov [rbp+16], r10; mov r11, 0;
    // mov [rbp+24], r11; sub rsp, 96
    assert_eq!(
        compiled.code[0..23],
        [
            0x55, 0x48, 0x89, 0xe5, 0x4c, 0x89, 0x55, 0x10, 0x49, 0xc7, 0xc3, 0x00, 0x00, 0x00,
            0x00, 0x4c, 0x89, 0x5d, 0x18, 0x48, 0x83, 0xec, 0x60,
        ]
    );
    // mov rsp, rbp; pop rbp; mov rax, [rsp]; mov [rsp+8], r10;
    // mov [rsp+16], rax; jmp r11
    let tail_transfer_start = compiled.code.len() - 21;
    assert_eq!(
        compiled.code[tail_transfer_start..],
        [
            0x48, 0x89, 0xec, 0x5d, 0x48, 0x8b, 0x04, 0x24, 0x4c, 0x89, 0x54, 0x24, 0x08, 0x48,
            0x89, 0x44, 0x24, 0x10, 0x41, 0xff, 0xe3,
        ]
    );
    assert!(compiled.safepoint_maps.is_empty());
}
