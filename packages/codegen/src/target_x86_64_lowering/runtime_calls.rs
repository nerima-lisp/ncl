use super::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FUNCTION_OBJECT, REST_ARGUMENT, ValueSlots, emit,
    load_immediate, load_slot, slot_mem_of,
};
use crate::{CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, Inst};
use ncl_ir::{Function, HandlerKind, ValueId};

pub fn handler_argument_count(function: &Function, region: ncl_ir::HandlerRegionId) -> usize {
    let Some(region) = function
        .handler_regions
        .iter()
        .find(|candidate| candidate.id == region)
    else {
        return 0;
    };
    match region.kind {
        HandlerKind::Catch => 2 + usize::from(region.catch_tag.is_some()),
        HandlerKind::UnwindProtect => 2,
        HandlerKind::Progv => 1 + region.binding_targets.len(),
    }
}

pub fn lower_runtime_builtin(
    assembler: &mut Assembler,
    function: RuntimeFunction,
    immediate_args: &[i64],
    value_args: &[ValueId],
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let argument_count = immediate_args.len().saturating_add(value_args.len());
    let extra_count = argument_count.saturating_sub(ARGUMENT_REGISTERS.len());
    let address = abi
        .runtime_address(function)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))? // check-added-lines: allow(unsupported) ABI lookup errors are surfaced
        .cast_signed();
    emit(
        assembler,
        Inst::MovRR(ARGUMENT_COUNT, super::THREAD_CONTEXT),
    )?;
    load_immediate(assembler, ENTRY, address)?;
    for (index, value) in immediate_args.iter().copied().enumerate() {
        if let Some(register) = ARGUMENT_REGISTERS.get(index) {
            load_immediate(assembler, *register, value)?;
        }
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        let argument_index = immediate_args.len().saturating_add(index);
        if let Some(register) = ARGUMENT_REGISTERS.get(argument_index) {
            load_slot(assembler, slots, value, *register)?;
        }
    }
    if extra_count == 0 {
        return Ok(());
    }
    emit(
        assembler,
        Inst::Lea(
            REST_ARGUMENT,
            slot_mem_of(
                slots
                    .outgoing_base
                    .checked_add(
                        u32::try_from(extra_count - 1).map_err(|_| CodegenError::FrameOverflow)?,
                    )
                    .ok_or(CodegenError::FrameOverflow)?,
            )?,
        ),
    )?;
    for (index, value) in immediate_args.iter().copied().enumerate().skip(4) {
        load_immediate(assembler, FUNCTION_OBJECT, value)?;
        store_extra(assembler, slots, extra_count - 1 - (index - 4))?;
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        let argument_index = immediate_args.len().saturating_add(index);
        if argument_index >= 4 {
            load_slot(assembler, slots, value, FUNCTION_OBJECT)?;
            store_extra(assembler, slots, extra_count - 1 - (argument_index - 4))?;
        }
    }
    Ok(())
}

fn store_extra(
    assembler: &mut Assembler,
    slots: &ValueSlots,
    index: usize,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::MovMR(
            slot_mem_of(
                slots
                    .outgoing_base
                    .checked_add(u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?)
                    .ok_or(CodegenError::FrameOverflow)?,
            )?,
            FUNCTION_OBJECT,
        ),
    )
}
