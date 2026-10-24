use crate::machine_verify::SafepointSite;
use crate::{
    AllocationTarget, CodegenError, CompiledFunction, FrameLayout, RuntimeAbi, allocate,
    checked_u32,
};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg};
use ncl_ir::{Function, OpKind, Terminator};

#[path = "target_x86_64_lowering.rs"]
mod lowering;
use lowering::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FRAME_POINTER, FUNCTION_OBJECT, REST_ARGUMENT,
    RETURN_VALUE, ValueSlots, emit, emit_call, load_slot, lower_call, lower_op,
    lower_return_or_throw, move_args, slots,
};
#[path = "target_x86_64_frame.rs"]
mod frame;
use frame::outgoing_words;
#[path = "target_x86_64_safepoint.rs"]
mod safepoint;
use safepoint::add_map;

/// Offset of the frame header's function-object word from the frame pointer.
const HEADER_FUNCTION_OBJECT_OFFSET: i32 = 16;
/// Offset of the frame header's flags word from the frame pointer.
const HEADER_FLAGS_OFFSET: i32 = 24;
/// Bytes of frame header materialised by `push rbp` and the caller's return address.
const INHERITED_HEADER_BYTES: u32 = 16;

fn spill_arguments(
    assembler: &mut Assembler,
    argument_values: &[ncl_ir::ValueId],
    generated_lambda: bool,
    value_slots: &ValueSlots,
) -> Result<(), CodegenError> {
    for (index, value) in argument_values.iter().copied().enumerate() {
        let index = u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?;
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
            lowering::store_slot(assembler, value_slots, value, register)?;
        } else {
            let overflow_base = if generated_lambda { 5 } else { 4 };
            let source_offset =
                i32::try_from(index.saturating_sub(overflow_base).saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                Inst::MovRM(ENTRY, Mem::base(REST_ARGUMENT, source_offset)),
            )?;
            lowering::store_slot(assembler, value_slots, value, ENTRY)?;
        }
    }
    Ok(())
}

fn emit_epilogue(assembler: &mut Assembler, callee_saved_base: u32) -> Result<(), CodegenError> {
    lowering::restore_callee_saved_registers(assembler, callee_saved_base)?;
    emit(assembler, Inst::MovRR(Reg::Rsp, FRAME_POINTER))?;
    emit(assembler, Inst::Pop(FRAME_POINTER))?;
    emit(assembler, Inst::Ret)
}

fn emit_tail_transfer(
    assembler: &mut Assembler,
    callee_saved_base: u32,
) -> Result<(), CodegenError> {
    lowering::restore_callee_saved_registers(assembler, callee_saved_base)?;
    emit(assembler, Inst::MovRR(Reg::Rsp, FRAME_POINTER))?;
    emit(assembler, Inst::Pop(FRAME_POINTER))?;
    emit(assembler, Inst::MovRM(RETURN_VALUE, Mem::base(Reg::Rsp, 0)))?;
    emit(
        assembler,
        Inst::MovMR(Mem::base(Reg::Rsp, 8), FUNCTION_OBJECT),
    )?;
    emit(
        assembler,
        Inst::MovMR(Mem::base(Reg::Rsp, 16), RETURN_VALUE),
    )?;
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
    compile_function_x86_64_with_multiple_values(function, abi, false)
}

/// Lowers an EVAL-compiled IR function while preserving its published value count.
///
/// # Errors
///
/// Returns [`CodegenError`] when the function cannot be represented by the
/// fixed frame and instruction templates.
pub fn compile_function_x86_64_preserving_multiple_values(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    compile_function_x86_64_with_multiple_values(function, abi, true)
}

#[allow(clippy::too_many_lines)]
fn compile_function_x86_64_with_multiple_values(
    function: &Function,
    abi: &dyn RuntimeAbi,
    preserve_multiple_values: bool,
) -> Result<CompiledFunction, CodegenError> {
    let Some(_entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    crate::machine_verify::verify_ir(function)?;
    let argument_words =
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?;
    let generated_lambda = function
        .params
        .first()
        .is_some_and(|parameter| parameter.name == "argc");
    let extra_frame_words = if generated_lambda { 5 } else { 0 };
    let allocation = allocate(function, AllocationTarget::X86_64);
    let spill_words = allocation.spill_words;
    let (mut value_slots, local_words) = slots(function, argument_words, allocation, 0);
    let outgoing_words = outgoing_words(function)?;
    value_slots.outgoing_base = argument_words
        .checked_add(local_words)
        .and_then(|words| words.checked_add(spill_words))
        .and_then(|words| words.checked_add(extra_frame_words))
        .ok_or(CodegenError::FrameOverflow)?;
    value_slots.callee_saved_base = value_slots
        .outgoing_base
        .checked_add(outgoing_words)
        .ok_or(CodegenError::FrameOverflow)?;
    let frame = FrameLayout::new(
        argument_words,
        local_words
            .checked_add(spill_words)
            .and_then(|words| words.checked_add(extra_frame_words))
            .and_then(|words| words.checked_add(lowering::CALLEE_SAVED_WORDS))
            .ok_or(CodegenError::FrameOverflow)?,
        outgoing_words,
    )?;
    let incoming_args_base = generated_lambda.then(|| {
        argument_words
            .saturating_add(local_words)
            .saturating_add(spill_words)
    });
    value_slots.incoming_args_base = incoming_args_base;
    value_slots.allocation.incoming_args_base = incoming_args_base;
    value_slots.allocation.outgoing_base = value_slots.outgoing_base;
    crate::machine_verify::verify_allocation(
        function,
        AllocationTarget::X86_64,
        &value_slots.allocation,
        frame,
        outgoing_words,
        value_slots.spill_base,
        Some(value_slots.callee_saved_base),
    )?;
    let mut assembler = Assembler::new();
    let labels = function
        .blocks
        .iter()
        .map(|block| (block.id, assembler.new_label()))
        .collect::<std::collections::HashMap<_, _>>();
    let mut maps = Vec::new();
    let mut safepoint_sites = Vec::new();
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
    lowering::save_callee_saved_registers(&mut assembler, value_slots.callee_saved_base)?;
    let argument_values = (0..argument_words).map(ncl_ir::ValueId).collect::<Vec<_>>();
    if let Some(base) = incoming_args_base {
        for (index, register) in lowering::ARGUMENT_REGISTERS.into_iter().enumerate() {
            emit(
                &mut assembler,
                Inst::MovMR(
                    lowering::slot_mem_of(
                        base.checked_add(
                            u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?,
                        )
                        .ok_or(CodegenError::FrameOverflow)?,
                    )?,
                    register,
                ),
            )?;
        }
        emit(
            &mut assembler,
            Inst::MovMR(
                lowering::slot_mem_of(base.checked_add(4).ok_or(CodegenError::FrameOverflow)?)?,
                lowering::REST_ARGUMENT,
            ),
        )?;
    }
    spill_arguments(
        &mut assembler,
        &argument_values,
        generated_lambda,
        &value_slots,
    )?;
    let mut position = 0u32;
    for block in &function.blocks {
        assembler.bind(labels[&block.id]);
        for op in &block.ops {
            let call_site = lower_op(
                &mut assembler,
                op,
                function,
                block.id,
                &value_slots,
                abi,
                &labels,
            )?;
            if matches!(op.kind, OpKind::Alloc { .. }) {
                let site = call_site.ok_or(CodegenError::MachineVerification(
                    crate::MachineVerificationError::MachineSafepointPcMismatch,
                ))?;
                add_map(
                    &mut maps,
                    site.end_offset,
                    function,
                    frame,
                    &value_slots,
                    position,
                    FLAG_ALLOCATION_SLOW,
                )?;
                safepoint_sites.push(site);
            } else if matches!(
                op.kind,
                OpKind::Call { .. }
                    | OpKind::CallIndirect { .. }
                    | OpKind::MakeClosure { .. }
                    | OpKind::CallClosure { .. }
                    | OpKind::MakeValueCell { .. }
                    | OpKind::Builtin { .. }
                    | OpKind::Safepoint
                    | OpKind::EnterHandler { .. }
                    | OpKind::LeaveHandler { .. }
            ) {
                let site = call_site.ok_or(CodegenError::MachineVerification(
                    crate::MachineVerificationError::MachineSafepointPcMismatch,
                ))?;
                add_map(
                    &mut maps,
                    site.end_offset,
                    function,
                    frame,
                    &value_slots,
                    position,
                    FLAG_CALL,
                )?;
                safepoint_sites.push(site);
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
                let start_offset = checked_u32(assembler.bytes().len())?;
                emit(&mut assembler, Inst::Jmp(labels[target]))?;
                let end_offset = checked_u32(assembler.bytes().len())?;
                if *target == block.id {
                    let site = SafepointSite::loop_backedge(start_offset, end_offset);
                    add_map(
                        &mut maps,
                        site.end_offset,
                        function,
                        frame,
                        &value_slots,
                        position,
                        FLAG_LOOP_BACKEDGE,
                    )?;
                    safepoint_sites.push(site);
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
                let context = lowering::dispatch::CallCheckContext::new(
                    function,
                    block.id,
                    &value_slots,
                    abi,
                    &labels,
                );
                lower_return_or_throw(
                    &mut assembler,
                    &context,
                    Some(values),
                    preserve_multiple_values,
                )?;
            }
            Terminator::CallReturn {
                function: callee,
                args,
            } => {
                lower_call(&mut assembler, *callee, args, &value_slots)?;
                let call_site = emit_call(&mut assembler)?;
                add_map(
                    &mut maps,
                    call_site.end_offset,
                    function,
                    frame,
                    &value_slots,
                    position,
                    FLAG_CALL,
                )?;
                safepoint_sites.push(call_site);
                emit_epilogue(&mut assembler, value_slots.callee_saved_base)?;
            }
            Terminator::TailCall { function, args } => {
                lower_call(&mut assembler, *function, args, &value_slots)?;
                emit_tail_transfer(&mut assembler, value_slots.callee_saved_base)?;
            }
            Terminator::Throw { .. } => {
                let context = lowering::dispatch::CallCheckContext::new(
                    function,
                    block.id,
                    &value_slots,
                    abi,
                    &labels,
                );
                lower_return_or_throw(&mut assembler, &context, None, false)?;
            }
            Terminator::Unreachable => {
                emit(&mut assembler, Inst::Ud2)?;
            }
        }
        position = position.saturating_add(1);
    }
    let blob = assembler
        .finish()
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    crate::machine_verify::verify_safepoint_sites(
        AllocationTarget::X86_64,
        &blob.bytes,
        &maps,
        &safepoint_sites,
    )?;
    Ok(CompiledFunction {
        code: blob.bytes,
        entry_offset: 0,
        relocations: Vec::new(),
        safepoint_maps: maps,
        frame_size: frame.size_bytes(),
        debug: Vec::new(),
    })
}
