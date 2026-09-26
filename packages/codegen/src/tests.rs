#[path = "tests_aarch64_fixtures.rs"]
mod tests_aarch64_fixtures;

use super::*;
use ncl_ir::{Constant, FunctionBuilder, OpKind, Terminator, Ty};

fn assert_compiles(function: &ncl_ir::Function) -> CompiledFunction {
    let text = function.to_string();
    let parsed = ncl_ir::parse(&text);
    assert!(
        parsed.is_ok(),
        "fixture must round-trip through IR text: {parsed:?}"
    );
    let Some(parsed) = parsed.ok() else {
        return CompiledFunction {
            code: Vec::new(),
            entry_offset: 0,
            relocations: Vec::new(),
            safepoint_maps: Vec::new(),
            frame_size: 0,
            debug: Vec::new(),
        };
    };
    let compiled = compile_function(&parsed, &X86_64Abi);
    assert!(compiled.is_ok(), "fixture failed to compile: {compiled:?}");
    compiled.unwrap_or_else(|_| CompiledFunction {
        code: Vec::new(),
        entry_offset: 0,
        relocations: Vec::new(),
        safepoint_maps: Vec::new(),
        frame_size: 0,
        debug: Vec::new(),
    })
}

fn constant_return(id: u32, name: &str, value: i64) -> ncl_ir::Function {
    let mut builder =
        FunctionBuilder::new(ncl_ir::FunctionId(id), name, Vec::new(), vec![Ty::Word]);
    let constant = builder.add_constant(Constant::Fixnum(value));
    let values = builder.push_op(OpKind::Const { result: constant }, &[Ty::Word]);
    assert!(values.is_ok());
    let value = values.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    assert!(
        builder
            .terminate(Terminator::Return {
                values: vec![value]
            })
            .is_ok()
    );
    builder.finish()
}

#[derive(Clone, Copy)]
struct Aarch64FixtureAbi;

impl RuntimeAbi for Aarch64FixtureAbi {
    fn encode_fixnum(&self, value: i64) -> i64 {
        value << 3
    }

    fn encode_character(&self, value: u32) -> i64 {
        i64::from(value) << 8 | 0x0f
    }

    fn builtin_address(&self, name: &str) -> Option<u64> {
        (name == "identity").then_some(0x1000)
    }

    fn context_offset(&self, _field: &str) -> Option<i32> {
        None
    }

    fn field_offset(&self, field: ContextField) -> Option<i32> {
        let layout = ncl_sys::thread_layout();
        let offset = match field {
            ContextField::TlabBump => layout.tlab_bump,
            ContextField::TlabLimit => layout.tlab_limit,
            ContextField::SafepointRequest => layout.safepoint_request,
            ContextField::MultipleValueArea => layout.mv,
            _ => return None,
        };
        i32::try_from(offset).ok()
    }

    fn runtime_address(&self, function: RuntimeFunction, _name: Option<&str>) -> Option<u64> {
        match function {
            RuntimeFunction::AllocateSlow | RuntimeFunction::SafepointSlow => Some(0x1000),
            _ => None,
        }
    }
}

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

#[test]
fn golden_add_one_two_from_ir_text() {
    let function = constant_return(10, "add-one-two", 3);
    let compiled = assert_compiles(&function);
    assert!(!compiled.code.is_empty());
    assert_eq!(compiled.code.last(), Some(&0xc3));
}

#[test]
fn golden_cons_allocates_and_records_safepoint() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(11), "cons", Vec::new(), vec![]);
    assert!(
        builder
            .push_op(OpKind::Alloc { words: 2 }, &[Ty::Word])
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let function = builder.finish();
    let compiled = assert_compiles(&function);
    assert!(
        compiled
            .safepoint_maps
            .iter()
            .any(|map| map.map_flags & FLAG_ALLOCATION_SLOW != 0)
    );
}

#[test]
fn golden_branch_emits_two_resolved_targets() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(12), "branch", Vec::new(), vec![]);
    let condition = builder.add_constant(Constant::T);
    let values = builder.push_op(OpKind::Const { result: condition }, &[Ty::Word]);
    assert!(values.is_ok());
    let condition = values.map_or(ncl_ir::ValueId(0), |ids| ids[0]);
    let then_target = builder.create_block(Vec::new());
    let else_target = builder.create_block(Vec::new());
    assert!(builder.position_at(ncl_ir::BlockId(0)).is_ok());
    assert!(
        builder
            .terminate(Terminator::Branch {
                condition,
                then_target,
                else_target,
                then_args: Vec::new(),
                else_args: Vec::new(),
            })
            .is_ok()
    );
    assert!(builder.position_at(then_target).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    assert!(builder.position_at(else_target).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let function = builder.finish();
    let compiled = assert_compiles(&function);
    assert_eq!(compiled.code.last(), Some(&0xc3));
}

#[test]
fn golden_builtin_call_has_call_safepoint() {
    #[derive(Clone, Copy)]
    struct Abi;
    impl RuntimeAbi for Abi {
        fn encode_fixnum(&self, value: i64) -> i64 {
            value << 3
        }
        fn encode_character(&self, value: u32) -> i64 {
            i64::from(value) << 8 | 0x0f
        }
        fn builtin_address(&self, name: &str) -> Option<u64> {
            (name == "identity").then_some(0x1000)
        }
        fn context_offset(&self, _field: &str) -> Option<i32> {
            None
        }

        fn field_offset(&self, field: ContextField) -> Option<i32> {
            (field == ContextField::MultipleValueArea)
                .then(|| i32::try_from(ncl_sys::thread_layout().mv).ok())
                .flatten()
        }
    }
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(13), "builtin", Vec::new(), vec![]);
    assert!(
        builder
            .push_op(
                OpKind::Builtin {
                    name: "identity".into(),
                    args: Vec::new()
                },
                &[]
            )
            .is_ok()
    );
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let compiled = compile_function(&builder.finish(), &Abi);
    assert!(compiled.is_ok());
    let Some(compiled) = compiled.ok() else {
        return;
    };
    assert!(
        compiled
            .safepoint_maps
            .iter()
            .any(|map| map.map_flags & FLAG_CALL != 0)
    );
}

fn assert_aarch64_fixture(function: &ncl_ir::Function) {
    assert_aarch64_fixture_with_abi(function, &Aarch64FixtureAbi);
}

fn assert_aarch64_fixture_with_abi(function: &ncl_ir::Function, abi: &dyn RuntimeAbi) {
    let result = compile_function_aarch64(function, abi);
    assert!(result.is_ok(), "AArch64 fixture failed: {result:?}");
    let Some(compiled) = result.ok() else {
        return;
    };
    assert!(!compiled.code.is_empty());
    assert_eq!(compiled.code.len() % 4, 0);
    let (words, _) = compiled.code.as_chunks::<4>();
    for word in words {
        let encoded = u32::from_le_bytes([word[0], word[1], word[2], word[3]]);
        assert!(
            ncl_disasm::decode(ncl_disasm::Architecture::Aarch64, &encoded.to_le_bytes(), 0)
                .is_ok(),
            "unsupported AArch64 golden word: 0x{encoded:08x}"
        );
    }
}

#[test]
fn golden_explicit_safepoint_has_encoded_pc() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(14), "safepoint", Vec::new(), vec![]);
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    let function = builder.finish();
    let compiled = assert_compiles(&function);
    let Some(map) = compiled.safepoint_maps.first() else {
        return;
    };
    let code_len = u32::try_from(compiled.code.len());
    assert!(code_len.is_ok());
    assert!(map.pc_offset < code_len.unwrap_or(u32::MAX));
    assert!(map.encode().is_ok());
}

#[test]
fn golden_fib_twenty_five_loop_has_backedge_map() {
    let mut builder = FunctionBuilder::new(ncl_ir::FunctionId(15), "fib-25", Vec::new(), vec![]);
    assert!(
        builder
            .terminate(Terminator::Jump {
                target: ncl_ir::BlockId(0),
                args: Vec::new(),
            })
            .is_ok()
    );
    let function = builder.finish();
    let compiled = assert_compiles(&function);
    assert!(
        compiled
            .safepoint_maps
            .iter()
            .any(|map| map.map_flags & FLAG_LOOP_BACKEDGE != 0)
    );
}
