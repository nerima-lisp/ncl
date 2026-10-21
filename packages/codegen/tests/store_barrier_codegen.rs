//! Tests for write barriers emitted by native store code generation.

use ncl_codegen::{
    AbiError, AllocationTarget, CompiledFunction, ContextField, FLAG_CALL, Location, RuntimeAbi,
    RuntimeFunction, allocate, compile_function_aarch64, compile_function_x86_64,
};
use ncl_ir::{Constant, FunctionBuilder, OpKind, Prim, Terminator, Ty};

const WRITE_BARRIER_ADDRESS: u64 = 0x1111_2222_3333_4444;
const MAKE_CLOSURE_ADDRESS: u64 = 0x5555_6666_7777_1111;
const SAFEPOINT_ADDRESS: u64 = 0x2222_3333_4444_5555;
// 1024 Word captures exercise AArch64 capture offsets beyond the 4-KiB add-immediate split.
const CAPTURE_COUNT: usize = 1024;
const HIGH_SPILL_VALUE_COUNT: usize = 512;
const FIELD_STORE_BARRIER_CALL_COUNT: usize = 1;
const MUTATING_PRIMITIVE_STORE_BARRIER_CALL_COUNT: usize = 3;

fn push_word_constant(builder: &mut FunctionBuilder, value: Constant) -> ncl_ir::ValueId {
    let constant = builder.add_constant(value);
    let values = builder
        .push_op(OpKind::Const { result: constant }, &[Ty::Word])
        .unwrap_or_else(|error| panic!("could not add fixture word constant: {error}"));
    values[0]
}

fn count_subsequence(bytes: &[u8], needle: &[u8]) -> usize {
    assert!(!needle.is_empty());
    bytes
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn aarch64_mov_imm64_bytes(value: u64) -> Vec<u8> {
    ncl_asm_aarch64::mov_imm64(ncl_asm_aarch64::Reg(17), value)
        .into_iter()
        .flat_map(|instruction| aarch64_encode_instruction(&instruction))
        .collect()
}

fn aarch64_encode_instruction(instruction: &ncl_asm_aarch64::Inst) -> [u8; 4] {
    ncl_asm_aarch64::encode(instruction, 0)
        .unwrap_or_else(|error| panic!("could not encode fixture instruction: {error}"))
        .to_le_bytes()
}

fn aarch64_spill_load_bytes(offset: u64, register: ncl_asm_aarch64::Reg) -> Vec<u8> {
    let mut instructions = ncl_asm_aarch64::mov_imm64(ncl_asm_aarch64::Reg(17), offset);
    instructions.extend([
        ncl_asm_aarch64::Inst::Sub {
            rd: ncl_asm_aarch64::RegOrSp::Reg(register),
            rn: ncl_asm_aarch64::RegOrSp::Reg(ncl_asm_aarch64::Reg(29)),
            rm: ncl_asm_aarch64::Reg(17),
            shift: ncl_asm_aarch64::Shift::Lsl(0),
        },
        ncl_asm_aarch64::Inst::Ldr {
            rt: register,
            mem: ncl_asm_aarch64::MemOperand::Unscaled {
                base: ncl_asm_aarch64::RegOrSp::Reg(register),
                offset: 0,
            },
        },
    ]);
    instructions
        .into_iter()
        .flat_map(|instruction| aarch64_encode_instruction(&instruction))
        .collect()
}

fn find_subsequence(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .unwrap_or_else(|| panic!("instruction sequence was not emitted: {needle:02x?}"))
}

fn call_return_offset(code: &[u8], address: &[u8], call: &[u8]) -> u32 {
    let address_offset = code
        .windows(address.len())
        .position(|window| window == address)
        .unwrap_or_else(|| panic!("runtime address was not emitted: {address:02x?}"));
    let call_offset = address_offset
        .checked_add(address.len())
        .and_then(|offset| {
            code[offset..]
                .windows(call.len())
                .position(|window| window == call)
                .map(|relative| offset + relative + call.len())
        })
        .unwrap_or_else(|| panic!("runtime address was not followed by its call"));
    u32::try_from(call_offset).unwrap_or_else(|_| panic!("fixture call offset exceeds u32"))
}

fn build_store_barrier_function() -> ncl_ir::Function {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(16),
        "store-barrier-large-capture",
        Vec::new(),
        Vec::new(),
    );
    let entry = push_word_constant(&mut builder, Constant::T);
    let captured = push_word_constant(&mut builder, Constant::Nil);
    assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
    let closure_result = builder
        .push_op(
            OpKind::MakeClosure {
                entry,
                captures: vec![captured; CAPTURE_COUNT],
            },
            &[Ty::Word],
        )
        .unwrap_or_else(|error| panic!("could not add closure fixture: {error}"));
    let closure = closure_result[0];
    assert!(
        builder
            .push_op(
                OpKind::StoreField {
                    object: closure,
                    field: 7,
                    value: captured,
                },
                &[],
            )
            .is_ok()
    );
    for (op, args) in [
        (Prim::Rplaca, vec![closure, captured]),
        (Prim::Rplacd, vec![closure, captured]),
        (Prim::Aset, vec![closure, captured, captured]),
    ] {
        builder
            .push_op(
                OpKind::Prim {
                    op,
                    args,
                    condition: None,
                },
                &[Ty::Unit],
            )
            .unwrap_or_else(|error| panic!("could not add primitive store fixture: {error}"));
    }
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    builder.finish()
}

fn build_high_spill_store_barrier_function() -> (ncl_ir::Function, ncl_ir::ValueId) {
    let mut builder = FunctionBuilder::new(
        ncl_ir::FunctionId(17),
        "store-barrier-high-spill-owner",
        Vec::new(),
        Vec::new(),
    );
    let live_values = (0..HIGH_SPILL_VALUE_COUNT)
        .map(|_| push_word_constant(&mut builder, Constant::Nil))
        .collect::<Vec<_>>();
    let owner = push_word_constant(&mut builder, Constant::Nil);
    assert!(
        builder
            .push_op(
                OpKind::StoreField {
                    object: owner,
                    field: 7,
                    value: live_values[0],
                },
                &[],
            )
            .is_ok()
    );
    for value in live_values {
        builder
            .push_op(OpKind::Move { value }, &[Ty::Word])
            .unwrap_or_else(|error| panic!("could not add spill-liveness fixture: {error}"));
    }
    assert!(
        builder
            .terminate(Terminator::Return { values: Vec::new() })
            .is_ok()
    );
    (builder.finish(), owner)
}

fn assert_store_barrier_artifact(
    compiled: &CompiledFunction,
    write_barrier: &[u8],
    make_closure: &[u8],
    safepoint: &[u8],
    indirect_call: &[u8],
    expected_barrier_calls: usize,
    expected_runtime_calls: usize,
) {
    assert_eq!(
        count_subsequence(&compiled.code, write_barrier),
        expected_barrier_calls
    );
    assert_eq!(count_subsequence(&compiled.code, make_closure), 1);
    assert_eq!(count_subsequence(&compiled.code, safepoint), 1);
    assert_eq!(
        count_subsequence(&compiled.code, indirect_call),
        expected_runtime_calls
    );
    assert_eq!(compiled.safepoint_maps.len(), 2);
    assert!(
        compiled
            .safepoint_maps
            .iter()
            .all(|map| map.map_flags == FLAG_CALL)
    );
    for address in [make_closure, safepoint] {
        let pc_offset = call_return_offset(&compiled.code, address, indirect_call);
        assert!(
            compiled
                .safepoint_maps
                .iter()
                .any(|map| map.pc_offset == pc_offset && map.map_flags == FLAG_CALL)
        );
    }
}

#[derive(Clone, Copy)]
struct StoreBarrierAbi;

impl RuntimeAbi for StoreBarrierAbi {
    fn builtin_address(&self, identifier: ncl_object::BuiltinIdentifier) -> Result<u64, AbiError> {
        Err(AbiError::MissingBuiltin(identifier))
    }

    fn field_offset(&self, field: ContextField) -> Result<i32, AbiError> {
        // Both built-in target ABIs use the same thread-layout mapping.
        <ncl_codegen::X86_64Abi as RuntimeAbi>::field_offset(&ncl_codegen::X86_64Abi, field)
    }

    fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, AbiError> {
        match function {
            RuntimeFunction::WriteBarrier => Ok(WRITE_BARRIER_ADDRESS),
            RuntimeFunction::MakeClosure => Ok(MAKE_CLOSURE_ADDRESS),
            RuntimeFunction::SafepointSlow => Ok(SAFEPOINT_ADDRESS),
            _ => Err(AbiError::UnsupportedRuntimeFunction(function)),
        }
    }
}

#[test]
fn native_store_lowerings_emit_barrier_calls_without_barrier_safepoints() {
    let function = build_store_barrier_function();
    let x86 = compile_function_x86_64(&function, &StoreBarrierAbi)
        .unwrap_or_else(|error| panic!("x86-64 fixture failed: {error:?}"));
    let aarch64 = compile_function_aarch64(&function, &StoreBarrierAbi)
        .unwrap_or_else(|error| panic!("AArch64 fixture failed: {error:?}"));

    let expected_barrier_calls = CAPTURE_COUNT
        + FIELD_STORE_BARRIER_CALL_COUNT
        + MUTATING_PRIMITIVE_STORE_BARRIER_CALL_COUNT;
    let expected_runtime_calls = expected_barrier_calls + 2;
    let x86_write_barrier = WRITE_BARRIER_ADDRESS.to_le_bytes();
    let x86_make_closure = MAKE_CLOSURE_ADDRESS.to_le_bytes();
    let x86_safepoint = SAFEPOINT_ADDRESS.to_le_bytes();
    // This encodes the x86-64 lowerer's CallReg(R11) instruction.
    let x86_indirect_call = [0x41, 0xff, 0xd3];
    assert_store_barrier_artifact(
        &x86,
        &x86_write_barrier,
        &x86_make_closure,
        &x86_safepoint,
        &x86_indirect_call,
        expected_barrier_calls,
        expected_runtime_calls,
    );

    let aarch64_write_barrier = aarch64_mov_imm64_bytes(WRITE_BARRIER_ADDRESS);
    let aarch64_make_closure = aarch64_mov_imm64_bytes(MAKE_CLOSURE_ADDRESS);
    let aarch64_safepoint = aarch64_mov_imm64_bytes(SAFEPOINT_ADDRESS);
    // This encodes the AArch64 lowerer's Blr(X17) instruction.
    let aarch64_indirect_call = 0xd63f_0220_u32.to_le_bytes();
    assert_store_barrier_artifact(
        &aarch64,
        &aarch64_write_barrier,
        &aarch64_make_closure,
        &aarch64_safepoint,
        &aarch64_indirect_call,
        expected_barrier_calls,
        expected_runtime_calls,
    );
}

#[test]
fn aarch64_write_barrier_target_survives_high_spill_argument_load() {
    let (function, owner) = build_high_spill_store_barrier_function();
    let allocation = allocate(&function, AllocationTarget::AArch64);
    let Location::Spill(spill) = allocation
        .location(owner)
        .unwrap_or_else(|| panic!("store owner has no allocated location"))
    else {
        panic!("store owner was not spilled");
    };
    let spill_offset = spill
        .checked_add(1)
        .and_then(|slot| slot.checked_mul(8))
        .unwrap_or_else(|| panic!("store owner spill offset overflowed"));
    assert!(spill_offset > 4095);

    let compiled = compile_function_aarch64(&function, &StoreBarrierAbi)
        .unwrap_or_else(|error| panic!("AArch64 high-spill fixture failed: {error:?}"));
    let owner_argument_load =
        aarch64_spill_load_bytes(u64::from(spill_offset), ncl_asm_aarch64::Reg(2));
    let write_barrier = aarch64_mov_imm64_bytes(WRITE_BARRIER_ADDRESS);
    let indirect_call = 0xd63f_0220_u32.to_le_bytes();
    let owner_load_offset = find_subsequence(&compiled.code, &owner_argument_load);
    let target_offset = find_subsequence(&compiled.code, &write_barrier);
    let call_offset = target_offset
        .checked_add(write_barrier.len())
        .and_then(|offset| {
            compiled.code[offset..]
                .windows(indirect_call.len())
                .position(|window| window == indirect_call)
                .map(|relative| offset + relative)
        })
        .unwrap_or_else(|| panic!("WriteBarrier target was not followed by BLR x17"));

    assert!(
        owner_load_offset < target_offset && target_offset < call_offset,
        "spilled owner load at {owner_load_offset}, runtime target at {target_offset}, call at {call_offset}"
    );
}
