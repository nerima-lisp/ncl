#![allow(missing_docs, clippy::unwrap_used)]

use crate::{ContextField, RuntimeAbi, RuntimeFunction, compile_function_aarch64};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

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

    fn runtime_address(&self, function: RuntimeFunction, _name: Option<&str>) -> Option<u64> {
        (function == RuntimeFunction::SafepointSlow).then_some(0x1000)
    }
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
    assert_eq!(
        ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &word.to_le_bytes(), 0)
            .expect("decode BLR")[0]
            .text,
        "blr x17"
    );
    let adr = u32::from_le_bytes(compiled.code[end - 8..end - 4].try_into().unwrap_or([0; 4]));
    assert_eq!(
        ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &adr.to_le_bytes(), 0)
            .expect("decode ADR")[0]
            .text,
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
        ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &word(24).to_le_bytes(), 0)
            .expect("decode STR x1")[0]
            .text,
        "str x1, [x29, #-8]"
    );
    assert_eq!(
        ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &word(28).to_le_bytes(), 0)
            .expect("decode STR x2")[0]
            .text,
        "str x2, [x29, #-16]"
    );
}
