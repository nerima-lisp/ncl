#![allow(missing_docs, clippy::unwrap_used)]

use crate::{
    AllocationTarget, ContextField, RuntimeAbi, RuntimeFunction, allocate, compile_function_aarch64,
};
use ncl_ir::{Constant, Convert, FunctionBuilder, OpKind, Param, Prim, Terminator, Ty};

#[path = "tests_aarch64_call.rs"]
mod call_with_rest;
#[path = "tests_aarch64_safepoint_root.rs"]
mod safepoint_root;
#[path = "tests_aarch64_switch.rs"]
mod switch;

fn decoded_text(bytes: [u8; 4], label: &str) -> String {
    let decoded = ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &bytes, 0);
    assert!(decoded.is_ok(), "{label}");
    let Ok(decoded) = decoded else {
        return String::new();
    };
    let Some(instruction) = decoded.first() else {
        return String::new();
    };
    instruction.text.clone()
}

#[derive(Default)]
struct Aarch64FixtureAbi {
    missing_write_barrier: bool,
}

impl RuntimeAbi for Aarch64FixtureAbi {
    fn builtin_address(
        &self,
        _identifier: ncl_object::BuiltinIdentifier,
    ) -> Result<u64, crate::AbiError> {
        Ok(0x1000)
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, crate::AbiError> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::TlabBump => layout.tlab_bump,
            ContextField::TlabLimit => layout.tlab_limit,
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::MultipleValueArea => layout.mv,
            ContextField::Pending => layout.pending,
            ContextField::MultipleValueCount => layout.mv_count,
            ContextField::Handler => layout.handler,
            ContextField::Cleanup => layout.cleanup,
            ContextField::Catch => layout.catch,
            ContextField::RuntimeErrorFlags => layout.runtime_error_flags,
        };
        i32::try_from(offset).map_err(|_| crate::AbiError::UnsupportedContextField(field))
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, crate::AbiError> {
        if function == RuntimeFunction::WriteBarrier && self.missing_write_barrier {
            return Err(crate::AbiError::UnsupportedRuntimeFunction(function));
        }
        match function {
            RuntimeFunction::SafepointSlow
            | RuntimeFunction::MakeClosure
            | RuntimeFunction::EnterCatch
            | RuntimeFunction::EnterUnwindProtect
            | RuntimeFunction::EnterProgv
            | RuntimeFunction::WriteBarrier
            | RuntimeFunction::LeaveCatch
            | RuntimeFunction::LeaveUnwindProtect
            | RuntimeFunction::LeaveProgv => Ok(0x1000),
            _ => Err(crate::AbiError::UnsupportedRuntimeFunction(function)),
        }
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        (name == "function-entry:7").then_some(0x2000)
    }
}

#[test]
fn aarch64_array_primitives_use_indexed_simple_vector_slots() {
    for (id, prim) in [(90, Prim::Aref), (91, Prim::Svref)] {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(id),
            "aarch64-array-reference",
            vec![
                Param {
                    name: "vector".into(),
                    ty: Ty::Word,
                },
                Param {
                    name: "index".into(),
                    ty: Ty::Word,
                },
            ],
            vec![Ty::Word],
        );
        let vector = builder
            .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
            .unwrap()[0];
        let index = builder
            .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
            .unwrap()[0];
        let value = builder
            .push_op(
                OpKind::Prim {
                    op: prim,
                    args: vec![vector, index],
                    condition: None,
                },
                &[Ty::Word],
            )
            .unwrap()[0];
        builder
            .terminate(Terminator::Return {
                values: vec![value],
            })
            .unwrap();
        let compiled =
            compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default()).unwrap();
        let expected = encode_aarch64([
            ncl_asm_aarch64::Inst::AndImm {
                rd: ncl_asm_aarch64::Reg(1),
                rn: ncl_asm_aarch64::Reg(1),
                imm: !ncl_sys::LOWTAG_MASK,
            },
            ncl_asm_aarch64::Inst::Add {
                rd: ncl_asm_aarch64::Reg(1).into(),
                rn: ncl_asm_aarch64::Reg(1).into(),
                rm: ncl_asm_aarch64::Reg(2),
                shift: ncl_asm_aarch64::Shift::Lsl(2),
            },
            ncl_asm_aarch64::Inst::Ldr {
                rt: ncl_asm_aarch64::Reg(16),
                mem: ncl_asm_aarch64::MemOperand::Unscaled {
                    base: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(1)),
                    offset: 16,
                },
            },
        ]);
        assert_code_contains_aarch64(
            &compiled.code,
            &expected,
            "array reference uses the tagged index and simple-vector payload offset",
        );
    }

    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(92),
        "aarch64-array-set",
        vec![
            Param {
                name: "vector".into(),
                ty: Ty::Word,
            },
            Param {
                name: "index".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        Vec::new(),
    );
    let vector = builder
        .push_op(OpKind::LoadArg { index: 0 }, &[Ty::Word])
        .unwrap()[0];
    let index = builder
        .push_op(OpKind::LoadArg { index: 1 }, &[Ty::Word])
        .unwrap()[0];
    let value = builder
        .push_op(OpKind::LoadArg { index: 2 }, &[Ty::Word])
        .unwrap()[0];
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::Aset,
                args: vec![vector, index, value],
                condition: None,
            },
            &[Ty::Unit],
        )
        .unwrap();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .unwrap();
    let compiled =
        compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default()).unwrap();
    let store = encode_aarch64([ncl_asm_aarch64::Inst::Str {
        rt: ncl_asm_aarch64::Reg(0),
        mem: ncl_asm_aarch64::MemOperand::Unscaled {
            base: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(1)),
            offset: 16,
        },
    }]);
    assert_code_contains_aarch64(
        &compiled.code,
        &store,
        "aset stores its value in the indexed simple-vector payload slot",
    );
    let barrier_slot = encode_aarch64([
        ncl_asm_aarch64::Inst::AsrImm {
            rd: ncl_asm_aarch64::Reg(1),
            rn: ncl_asm_aarch64::Reg(1),
            amount: 1,
        },
        ncl_asm_aarch64::Inst::AddImm {
            rd: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(1)),
            rn: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(1)),
            imm: 1,
            shift: false,
        },
    ]);
    assert_code_contains_aarch64(
        &compiled.code,
        &barrier_slot,
        "aset passes the untagged element index plus data slot to the write barrier",
    );
}

#[test]
fn aarch64_array_set_reports_a_missing_write_barrier() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(96),
        "aarch64-array-set-missing-write-barrier",
        vec![
            Param {
                name: "vector".into(),
                ty: Ty::Word,
            },
            Param {
                name: "index".into(),
                ty: Ty::Word,
            },
            Param {
                name: "value".into(),
                ty: Ty::Word,
            },
        ],
        Vec::new(),
    );
    builder
        .push_op(
            OpKind::Prim {
                op: Prim::Aset,
                args: vec![ncl_ir::ValueId(0), ncl_ir::ValueId(1), ncl_ir::ValueId(2)],
                condition: None,
            },
            &[Ty::Unit],
        )
        .unwrap();
    builder
        .terminate(Terminator::Return { values: Vec::new() })
        .unwrap();

    let result = compile_function_aarch64(
        &builder.finish(),
        &Aarch64FixtureAbi {
            missing_write_barrier: true,
        },
    );
    let error = result.expect_err("AArch64 Aset must fail without a write barrier");
    assert_eq!(
        error,
        crate::CodegenError::Unsupported(
            crate::AbiError::UnsupportedRuntimeFunction(RuntimeFunction::WriteBarrier).to_string()
        )
    );
}

fn encode_aarch64(instructions: impl IntoIterator<Item = ncl_asm_aarch64::Inst>) -> Vec<u8> {
    let mut assembler = ncl_asm_aarch64::Assembler::new();
    for instruction in instructions {
        assembler.emit(&instruction).unwrap();
    }
    assembler.finish().unwrap().bytes
}

fn assert_code_contains_aarch64(code: &[u8], expected: &[u8], behavior: &str) {
    assert!(
        code.windows(expected.len())
            .any(|window| window == expected),
        "{behavior}: expected {expected:02x?} in {code:02x?}"
    );
}

#[test]
fn lowers_ir_v2_closure_and_handler_ops_aarch64() -> Result<(), String> {
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
    let result = ncl_ir::ValueId(4);
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
    let argc_value = ncl_ir::ValueId(2);
    builder
        .push_op(OpKind::Const { result: argc }, &[Ty::I64])
        .map_err(|error| format!("argc constant: {error:?}"))?;
    let argc_word = ncl_ir::ValueId(3);
    builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: argc_value,
            },
            &[Ty::Word],
        )
        .map_err(|error| format!("argc conversion: {error:?}"))?;
    assert!(
        builder
            .push_op(
                OpKind::CallClosure {
                    closure,
                    args: vec![argc_word],
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
    let compiled = match compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
    {
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
fn golden_aarch64_safepoint_pc_follows_emitted_instruction() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(26),
        "aarch64-pc-tracking",
        Vec::new(),
        vec![],
    );
    let constant = builder.add_constant(Constant::Fixnum(0x1234_5678));
    assert!(
        builder
            .push_op(OpKind::Const { result: constant }, &[Ty::I64])
            .is_ok()
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default());
    assert!(compiled.is_ok(), "AArch64 fixture failed: {compiled:?}");
    let Some(compiled) = compiled.ok() else {
        return;
    };
    let Some(map) = compiled.safepoint_maps.first() else {
        return;
    };
    let end = usize::try_from(map.pc_offset).unwrap_or(0);
    assert!(end >= 4);
    let word = u32::from_le_bytes(compiled.code[end - 4..end].try_into().unwrap_or([0; 4]));
    assert_eq!(decoded_text(word.to_le_bytes(), "decode BLR"), "blr x17");
    let adr = u32::from_le_bytes(compiled.code[end - 8..end - 4].try_into().unwrap_or([0; 4]));
    assert_eq!(
        decoded_text(adr.to_le_bytes(), "decode ADR"),
        "adr x2, #0x8"
    );
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)]
fn golden_aarch64_prologue_spills_register_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(28),
        "spill-arguments",
        vec![
            ncl_ir::Param {
                name: "left".into(),
                ty: ncl_ir::Ty::Word,
            },
            ncl_ir::Param {
                name: "right".into(),
                ty: ncl_ir::Ty::Word,
            },
        ],
        vec![],
    );
    assert!(builder.push_op(ncl_ir::OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(ncl_ir::Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled_result =
        compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default());
    assert!(compiled_result.is_ok());
    let Some(compiled) = compiled_result.ok() else {
        return;
    };
    let instructions = compiled
        .code
        .chunks_exact(4)
        .map(|bytes| {
            decoded_text(
                bytes.try_into().unwrap_or([0; 4]),
                "decode argument initialization",
            )
        })
        .collect::<Vec<_>>();
    assert!(
        instructions
            .iter()
            .any(|text| text == "orr x6, x31, x1, lsl #0")
    );
    assert!(
        instructions
            .iter()
            .any(|text| text == "orr x7, x31, x2, lsl #0")
    );
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)]
fn golden_aarch64_tail_call_restores_frame_and_branches() -> Result<(), String> {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(30), "tail-call", Vec::new(), Vec::new());
    let callee = builder.add_constant(Constant::FunctionEntry(ncl_ir::FunctionId(7)));
    let Some(callee) = builder
        .push_op(OpKind::Const { result: callee }, &[Ty::Word])
        .ok()
        .and_then(|values| values.first().copied())
    else {
        unreachable!("callee result")
    };
    let argc = builder.add_constant(Constant::Fixnum(0));
    let Some(argc_integer) = builder
        .push_op(OpKind::Const { result: argc }, &[Ty::I64])
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("argc result missing".to_owned());
    };
    let Some(argc) = builder
        .push_op(
            OpKind::Convert {
                op: Convert::I64ToWord,
                value: argc_integer,
            },
            &[Ty::Word],
        )
        .ok()
        .and_then(|values| values.first().copied())
    else {
        return Err("argc conversion missing".to_owned());
    };
    assert!(
        builder
            .terminate(Terminator::TailCall {
                function: callee,
                args: vec![argc]
            })
            .is_ok()
    );

    let compiled = match compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
    {
        Ok(compiled) => compiled,
        Err(error) => return Err(format!("AArch64 tail-call lowering: {error:?}")),
    };
    let instructions = compiled
        .code
        .chunks_exact(4)
        .map(|bytes| decoded_text(bytes.try_into().unwrap_or([0; 4]), "decode tail call"))
        .collect::<Vec<_>>();
    assert_eq!(instructions.last().map(String::as_str), Some("br x17"));
    assert!(
        instructions
            .iter()
            .any(|text| text == "ldp x29, x30, [sp], #32")
    );
    assert!(!instructions.iter().any(|text| text == "ret x30"));
    assert!(compiled.safepoint_maps.is_empty());
    Ok(())
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)]
fn golden_aarch64_parallel_copy_swaps_register_arguments() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(32),
        "parallel-copy-swap",
        vec![
            ncl_ir::Param {
                name: "self".into(),
                ty: Ty::Address,
            },
            ncl_ir::Param {
                name: "next".into(),
                ty: Ty::Address,
            },
        ],
        vec![Ty::Address, Ty::Address],
    );
    let swapped_self = builder.fresh_value();
    let swapped_next = builder.fresh_value();
    let destination = builder.create_block(vec![
        (Ty::Address, swapped_self),
        (Ty::Address, swapped_next),
    ]);
    let positioned = builder.position_at(ncl_ir::BlockId(0));
    assert!(positioned.is_ok(), "entry block: {positioned:?}");
    let Ok(()) = positioned else {
        return;
    };
    let terminated = builder.terminate(Terminator::Jump {
        target: destination,
        args: vec![ncl_ir::ValueId(1), ncl_ir::ValueId(0)],
    });
    assert!(terminated.is_ok(), "swap jump: {terminated:?}");
    let positioned = builder.position_at(destination);
    assert!(positioned.is_ok(), "destination block: {positioned:?}");
    let Ok(()) = positioned else {
        return;
    };
    let terminated = builder.terminate(Terminator::Return {
        values: vec![swapped_self, swapped_next],
    });
    assert!(terminated.is_ok(), "swap return: {terminated:?}");
    if terminated.is_err() {
        return;
    }
    let function = builder.finish();
    let allocation = allocate(&function, AllocationTarget::AArch64);
    assert_ne!(
        allocation.location(ncl_ir::ValueId(1)),
        allocation.location(swapped_self),
        "next must be copied to self's destination"
    );
    assert_ne!(
        allocation.location(ncl_ir::ValueId(0)),
        allocation.location(swapped_next),
        "self must be copied to next's destination"
    );

    let compiled_result = compile_function_aarch64(&function, &Aarch64FixtureAbi::default());
    assert!(
        compiled_result.is_ok(),
        "AArch64 parallel copy lowering: {compiled_result:?}"
    );
    let Ok(compiled) = compiled_result else {
        return;
    };
    let instructions = compiled
        .code
        .chunks_exact(4)
        .map(|bytes| decoded_text(bytes.try_into().unwrap_or([0; 4]), "decode parallel copy"))
        .collect::<Vec<_>>();
    assert!(
        instructions.iter().any(|text| text == "sub sp, sp, #16"),
        "parallel copy must reserve temporary stack space: {instructions:?}"
    );
    assert!(
        instructions.iter().any(|text| text == "add sp, sp, #16"),
        "parallel copy must release temporary stack space: {instructions:?}"
    );
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)]
fn golden_aarch64_generated_lambda_saves_argc_from_x0() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(31),
        "generated-lambda-argc",
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
        Vec::new(),
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );

    let compiled = match compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
    {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("AArch64 generated-lambda lowering: {error:?}"),
    };
    let instructions = compiled
        .code
        .chunks_exact(4)
        .map(|bytes| decoded_text(bytes.try_into().unwrap_or([0; 4]), "decode argc prologue"))
        .collect::<Vec<_>>();
    assert!(instructions.iter().any(|text| text.ends_with("x0, lsl #0")));
    assert!(instructions.iter().any(|text| text.ends_with("x1, lsl #0")));
}

#[test]
#[allow(clippy::chunks_exact_to_as_chunks)]
fn allocator_locations_reach_aarch64_code_and_safepoint_map() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(29),
        "allocator-locations",
        Vec::new(),
        vec![Ty::Word; 12],
    );
    let mut values = Vec::new();
    for index in 0..12 {
        let constant = builder.add_constant(Constant::Fixnum(index));
        let integer = match builder.push_op(OpKind::Const { result: constant }, &[Ty::I64]) {
            Ok(ids) => ids[0],
            Err(error) => unreachable!("constant: {error:?}"),
        };
        values.push(
            match builder.push_op(
                OpKind::Convert {
                    op: Convert::I64ToWord,
                    value: integer,
                },
                &[Ty::Word],
            ) {
                Ok(ids) => ids[0],
                Err(error) => unreachable!("word conversion: {error:?}"),
            },
        );
    }
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok(), "safepoint");
    assert!(
        builder.terminate(Terminator::Return { values }).is_ok(),
        "return"
    );

    let compiled = match compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi::default())
    {
        Ok(compiled) => compiled,
        Err(error) => unreachable!("{error:?}"),
    };
    let Some(map) = compiled.safepoint_maps.first() else {
        unreachable!("safepoint map");
    };
    assert!(
        compiled.frame_size > 32,
        "spill slots must extend the frame"
    );
    assert!(map.bitmap.iter().any(|byte| byte & (1 << 4) != 0));
}
