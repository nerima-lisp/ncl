#![allow(missing_docs, clippy::unwrap_used)]

use crate::{ContextField, RuntimeAbi, RuntimeFunction, compile_function_aarch64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

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

struct Aarch64FixtureAbi;

impl RuntimeAbi for Aarch64FixtureAbi {
    fn builtin_address(&self, _name: &str) -> Option<u64> {
        None
    }

    fn context_offset(&self, _field: &str) -> Option<i32> {
        None
    }

    fn field_offset(&self, field: ContextField) -> Option<i32> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::MultipleValueArea => layout.mv,
            _ => return None,
        };
        i32::try_from(offset).ok()
    }

    fn runtime_address(&self, function: RuntimeFunction, name: Option<&str>) -> Option<u64> {
        match function {
            RuntimeFunction::SafepointSlow => Some(0x1000),
            RuntimeFunction::Builtin
                if matches!(
                    name,
                    Some("make-closure" | "enter-unwind-protect" | "leave-unwind-protect")
                ) =>
            {
                Some(0x1000)
            }
            _ => None,
        }
    }

    fn constant_word(&self, name: &str) -> Option<i64> {
        (name == "function-entry:7").then_some(0x2000)
    }
}

#[test]
fn lowers_ir_v2_closure_and_handler_ops_aarch64() {
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
    let compiled = match compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi) {
        Ok(compiled) => compiled,
        Err(error) => {
            unreachable!("{error:?}");
        }
    };
    assert!(!compiled.code.is_empty());
    assert!(compiled.safepoint_maps.len() >= 4);
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
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])
            .is_ok()
    );
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi);
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
        "adr x2, #0x2"
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
    let compiled_result = compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi);
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
fn allocator_locations_reach_aarch64_code_and_safepoint_map() {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(29),
        "allocator-locations",
        Vec::new(),
        vec![Ty::Word],
    );
    let mut values = Vec::new();
    for index in 0..12 {
        let constant = builder.add_constant(Constant::Fixnum(index));
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

    let compiled = match compile_function_aarch64(&builder.finish(), &Aarch64FixtureAbi) {
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
    assert!(
        map.registers.is_empty(),
        "safepoint-crossing values are spilled"
    );
    assert!(map.bitmap.iter().any(|byte| byte & (1 << 4) != 0));
}
