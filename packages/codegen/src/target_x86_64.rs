use crate::{
    AllocationTarget, CodegenError, CompiledFunction, FrameLayout, RuntimeAbi, SafepointMap,
    allocate, checked_u32,
};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg};
use ncl_ir::{Function, OpKind, Terminator};

#[path = "target_x86_64_lowering.rs"]
mod lowering;
use lowering::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FRAME_POINTER, FUNCTION_OBJECT, REST_ARGUMENT,
    RETURN_VALUE, VALUE_COUNT, ValueSlots, emit, emit_call, load_immediate, load_slot, lower_call,
    lower_op, move_args, slots,
};

/// Offset of the frame header's function-object word from the frame pointer.
const HEADER_FUNCTION_OBJECT_OFFSET: i32 = 16;
/// Offset of the frame header's flags word from the frame pointer.
const HEADER_FLAGS_OFFSET: i32 = 24;
/// Bytes of frame header materialised by `push rbp` and the caller's return address.
const INHERITED_HEADER_BYTES: u32 = 16;

fn add_map(
    maps: &mut Vec<SafepointMap>,
    pc: u32,
    frame: FrameLayout,
    values: &ValueSlots,
    position: u32,
    flags: u32,
) -> Result<(), CodegenError> {
    let slots = u16::try_from(frame.frame_words).map_err(|_| CodegenError::FrameOverflow)?;
    let (registers, live_slots) = values.roots(position);
    SafepointMap::new(pc, slots, slots, &live_slots, &registers, flags)
        .map(|map| maps.push(map))
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

fn spill_arguments(
    assembler: &mut Assembler,
    argument_words: u32,
    generated_lambda: bool,
) -> Result<(), CodegenError> {
    for index in 0..argument_words {
        let offset = i32::try_from((index + 1).saturating_mul(8))
            .map_err(|_| CodegenError::FrameOverflow)?;
        let destination = Mem::base(FRAME_POINTER, -offset);
        let register_index = if generated_lambda {
            index.saturating_sub(1)
        } else {
            index
        };
        let register = if generated_lambda && index == 0 {
            Some(ARGUMENT_COUNT)
        } else {
            usize::try_from(register_index)
                .ok()
                .and_then(|slot| ARGUMENT_REGISTERS.get(slot).copied())
        };
        if let Some(register) = register {
            emit(assembler, Inst::MovMR(destination, register))?;
        } else {
            let overflow_base = if generated_lambda { 5 } else { 4 };
            let source_offset =
                i32::try_from(index.saturating_sub(overflow_base).saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                Inst::MovRM(ENTRY, Mem::base(REST_ARGUMENT, source_offset)),
            )?;
            emit(assembler, Inst::MovMR(destination, ENTRY))?;
        }
    }
    Ok(())
}

fn emit_epilogue(assembler: &mut Assembler) -> Result<(), CodegenError> {
    emit(assembler, Inst::MovRR(Reg::Rsp, FRAME_POINTER))?;
    emit(assembler, Inst::Pop(FRAME_POINTER))?;
    emit(assembler, Inst::Ret)
}

/// Tears down the current frame and transfers to an indirect callee.
///
/// The caller return address is moved below a fresh two-word header area so
/// the callee prologue sees the same layout as a regular indirect call.
fn emit_tail_transfer(assembler: &mut Assembler) -> Result<(), CodegenError> {
    emit(assembler, Inst::MovRR(Reg::Rsp, FRAME_POINTER))?;
    emit(assembler, Inst::Pop(FRAME_POINTER))?;
    emit(
        assembler,
        Inst::MovMR(Mem::base(Reg::Rsp, -16), FUNCTION_OBJECT),
    )?;
    emit(assembler, Inst::BinRI(BinOp::Sub, Reg::Rsp, 16))?;
    emit(assembler, Inst::JmpReg(ENTRY))
}

/// Lowers an IR function to x86-64 machine code using the native frame ABI.
///
/// # Errors
///
/// Returns [`CodegenError`] when the function cannot be represented by the
/// fixed frame and instruction templates.
#[allow(clippy::too_many_lines)]
pub fn compile_function_x86_64(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let Some(_entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    let argument_words =
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?;
    let allocation = allocate(function, AllocationTarget::X86_64);
    let spill_words = allocation.spill_words;
    let (value_slots, local_words) = slots(function, argument_words, allocation);
    let frame = FrameLayout::new(
        argument_words,
        local_words
            .checked_add(spill_words)
            .ok_or(CodegenError::FrameOverflow)?,
        0,
    )?;
    let mut assembler = Assembler::new();
    let labels = function
        .blocks
        .iter()
        .map(|block| (block.id, assembler.new_label()))
        .collect::<std::collections::HashMap<_, _>>();
    let mut maps = Vec::new();
    emit(&mut assembler, Inst::Push(FRAME_POINTER))?;
    emit(&mut assembler, Inst::MovRR(FRAME_POINTER, Reg::Rsp))?;
    emit(
        &mut assembler,
        Inst::MovMR(
            Mem::base(FRAME_POINTER, HEADER_FUNCTION_OBJECT_OFFSET),
            FUNCTION_OBJECT,
        ),
    )?;
    emit(&mut assembler, Inst::MovRI(ENTRY, Imm::I32(0)))?;
    emit(
        &mut assembler,
        Inst::MovMR(Mem::base(FRAME_POINTER, HEADER_FLAGS_OFFSET), ENTRY),
    )?;
    let body_bytes = frame.size_bytes().saturating_sub(INHERITED_HEADER_BYTES);
    if body_bytes > 0 {
        emit(
            &mut assembler,
            Inst::BinRI(BinOp::Sub, Reg::Rsp, body_bytes.cast_signed()),
        )?;
    }
    let generated_lambda = function
        .params
        .first()
        .is_some_and(|parameter| parameter.name == "argc");
    spill_arguments(&mut assembler, argument_words, generated_lambda)?;
    for (index, parameter) in function
        .blocks
        .first()
        .map(|block| block.params.iter())
        .into_iter()
        .flatten()
        .enumerate()
    {
        let offset = i32::try_from(index.saturating_add(1).saturating_mul(8))
            .map_err(|_| CodegenError::FrameOverflow)?;
        emit(
            &mut assembler,
            Inst::MovRM(ENTRY, Mem::base(FRAME_POINTER, -offset)),
        )?;
        lowering::store_slot(&mut assembler, &value_slots, parameter.value, ENTRY)?;
    }
    let mut position = 0u32;
    for block in &function.blocks {
        assembler.bind(labels[&block.id]);
        for op in &block.ops {
            let call_pc = lower_op(&mut assembler, op, function, &value_slots, abi)?;
            if matches!(op.kind, OpKind::Alloc { .. }) {
                add_map(
                    &mut maps,
                    call_pc.unwrap_or(checked_u32(assembler.bytes().len())?),
                    frame,
                    &value_slots,
                    position,
                    FLAG_ALLOCATION_SLOW,
                )?;
            } else if matches!(
                op.kind,
                OpKind::Call { .. }
                    | OpKind::CallIndirect { .. }
                    | OpKind::MakeClosure { .. }
                    | OpKind::CallClosure { .. }
                    | OpKind::Builtin { .. }
                    | OpKind::Safepoint
                    | OpKind::EnterHandler { .. }
                    | OpKind::LeaveHandler { .. }
            ) {
                add_map(
                    &mut maps,
                    call_pc.unwrap_or(checked_u32(assembler.bytes().len())?),
                    frame,
                    &value_slots,
                    position,
                    FLAG_CALL,
                )?;
            }
            position = position.saturating_add(1);
        }
        match &block.terminator {
            Terminator::Jump { target, args } => {
                let destination = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *target)
                    .ok_or(CodegenError::UnknownBlock(*target))?;
                move_args(&mut assembler, &value_slots, args, &destination.params)?;
                emit(&mut assembler, Inst::Jmp(labels[target]))?;
                if *target == block.id {
                    add_map(
                        &mut maps,
                        checked_u32(assembler.bytes().len())?,
                        frame,
                        &value_slots,
                        position,
                        FLAG_LOOP_BACKEDGE,
                    )?;
                }
            }
            Terminator::Branch {
                condition,
                then_target,
                then_args,
                else_target,
                else_args,
            } => {
                load_slot(&mut assembler, &value_slots, *condition, FUNCTION_OBJECT)?;
                let then_block = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *then_target)
                    .ok_or(CodegenError::UnknownBlock(*then_target))?;
                move_args(&mut assembler, &value_slots, then_args, &then_block.params)?;
                emit(&mut assembler, Inst::CmpRI(FUNCTION_OBJECT, 0))?;
                emit(&mut assembler, Inst::Jcc(Cond::Ne, labels[then_target]))?;
                let else_block = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *else_target)
                    .ok_or(CodegenError::UnknownBlock(*else_target))?;
                move_args(&mut assembler, &value_slots, else_args, &else_block.params)?;
                emit(&mut assembler, Inst::Jmp(labels[else_target]))?;
            }
            Terminator::Switch {
                value,
                cases,
                default,
                default_args,
            } => {
                load_slot(&mut assembler, &value_slots, *value, FUNCTION_OBJECT)?;
                let case_labels = cases
                    .iter()
                    .map(|_| assembler.new_label())
                    .collect::<Vec<_>>();
                for ((case, _, _), case_label) in cases.iter().zip(&case_labels) {
                    emit(
                        &mut assembler,
                        Inst::CmpRI(
                            FUNCTION_OBJECT,
                            i32::try_from(*case).map_err(|_| CodegenError::FrameOverflow)?,
                        ),
                    )?;
                    emit(&mut assembler, Inst::Jcc(Cond::E, *case_label))?;
                }
                let default_block = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *default)
                    .ok_or(CodegenError::UnknownBlock(*default))?;
                move_args(
                    &mut assembler,
                    &value_slots,
                    default_args,
                    &default_block.params,
                )?;
                emit(&mut assembler, Inst::Jmp(labels[default]))?;
                // Each case moves its own arguments in an out-of-line block, so a
                // taken case cannot have its arguments overwritten by an earlier
                // case's moves.
                for ((_, target, args), case_label) in cases.iter().zip(&case_labels) {
                    assembler.bind(*case_label);
                    let case_block = function
                        .blocks
                        .iter()
                        .find(|candidate| candidate.id == *target)
                        .ok_or(CodegenError::UnknownBlock(*target))?;
                    move_args(&mut assembler, &value_slots, args, &case_block.params)?;
                    emit(&mut assembler, Inst::Jmp(labels[target]))?;
                }
            }
            Terminator::Return { values } => {
                if let Some(value) = values.first() {
                    load_slot(&mut assembler, &value_slots, *value, RETURN_VALUE)?;
                } else {
                    load_immediate(&mut assembler, RETURN_VALUE, abi.encode_fixnum(0))?;
                }
                load_immediate(
                    &mut assembler,
                    VALUE_COUNT,
                    i64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?,
                )?;
                emit_epilogue(&mut assembler)?;
            }
            Terminator::CallReturn { function, args } => {
                lower_call(&mut assembler, *function, args, &value_slots)?;
                let call_pc = emit_call(&mut assembler)?;
                add_map(&mut maps, call_pc, frame, &value_slots, position, FLAG_CALL)?;
                emit_epilogue(&mut assembler)?;
            }
            Terminator::TailCall { function, args } => {
                lower_call(&mut assembler, *function, args, &value_slots)?;
                emit(
                    &mut assembler,
                    Inst::MovRM(FUNCTION_OBJECT, Mem::base(FRAME_POINTER, 8)),
                )?;
                // A tail transfer has no return PC in this frame, so there is
                // no new caller safepoint map or unwind point to register.
                emit_tail_transfer(&mut assembler)?;
            }
            Terminator::Throw { .. } | Terminator::Unreachable => {
                emit(&mut assembler, Inst::Ud2)?;
            }
        }
        position = position.saturating_add(1);
    }
    let blob = assembler
        .finish()
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    Ok(CompiledFunction {
        code: blob.bytes,
        entry_offset: 0,
        relocations: Vec::new(),
        safepoint_maps: maps,
        frame_size: frame.size_bytes(),
        debug: Vec::new(),
    })
}
