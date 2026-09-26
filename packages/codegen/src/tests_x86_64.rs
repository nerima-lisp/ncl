#![allow(missing_docs, clippy::unwrap_used)]

use crate::tests_x86_64_fixture::X86_64FixtureAbi;
use crate::{AllocationTarget, allocate, compile_function_x86_64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

#[test]
fn x86_64_lowering_uses_allocator_register_roots() {
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
        builder
            .terminate(Terminator::Return {
                values: vec![value],
            })
            .is_ok(),
        "return"
    );
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::X86_64);
    let Some(expected) = allocation.safepoint_registers.get(&1) else {
        unreachable!("allocator safepoint roots");
    };
    assert!(!expected.is_empty());
    let compiled = match compile_function_x86_64(&function, &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(compiled.safepoint_maps[0].registers, *expected);
    assert!(!compiled.code.is_empty());
}

#[test]
fn x86_64_lowering_reserves_allocator_spills_in_frame() {
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
}

#[test]
fn lowers_ir_v2_closure_and_handler_ops_x86_64() {
    let mut builder = ncl_ir::FunctionBuilder::new(
        ncl_ir::FunctionId(70),
        "ir-v2-ops",
        Vec::new(),
        vec![Ty::Word],
    );
    let entry = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let entry_value = ncl_ir::ValueId(0);
    let closure = ncl_ir::ValueId(1);
    let result = ncl_ir::ValueId(2);
    assert!(
        builder
            .push_op(OpKind::Const { result: entry }, &[Ty::Word])
            .is_ok()
    );
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
                    args: Vec::new()
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
}

#[test]
fn x86_64_tail_call_restores_frame_and_jumps_without_safepoint() {
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
    assert!(
        builder
            .terminate(Terminator::TailCall {
                function: callee,
                args: Vec::new(),
            })
            .is_ok()
    );

    let compiled = match compile_function_x86_64(&builder.finish(), &X86_64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("tail-call lowering: {error:?}"),
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
