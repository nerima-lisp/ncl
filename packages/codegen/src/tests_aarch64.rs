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
    fn encode_fixnum(&self, value: i64) -> i64 {
        value << 3
    }

    fn encode_character(&self, value: u32) -> i64 {
        i64::from(value) << 8 | 0x0f
    }

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
    let word = |offset| {
        u32::from_le_bytes(
            compiled.code[offset..offset + 4]
                .try_into()
                .unwrap_or([0; 4]),
        )
    };
    assert_eq!(
        decoded_text(word(24).to_le_bytes(), "decode STR x1"),
        "str x1, [x29, #-8]"
    );
    assert_eq!(
        decoded_text(word(28).to_le_bytes(), "decode STR x2"),
        "str x2, [x29, #-16]"
    );
}
