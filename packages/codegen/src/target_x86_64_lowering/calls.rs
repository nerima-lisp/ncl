use super::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FUNCTION_OBJECT, REST_ARGUMENT, ValueSlots, emit,
    load_slot, slot_mem_of,
};
use crate::CodegenError;
use ncl_asm_x86_64::{Assembler, Inst, Mem, Shift};
use ncl_ir::ValueId;

// `argc`/`args` mirror the calling convention's own argument-count/argument-
// list naming; that pairing is clearer here than any alternative spelling.
#[allow(clippy::similar_names)]
pub fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let Some((argc, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "calls require a tagged argc argument".into(),
        ));
    };
    load_slot(assembler, slots, callee, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_slot(assembler, slots, *argc, ARGUMENT_COUNT)?;
    for (index, argument) in rest.iter().enumerate() {
        if let Some(register) = ARGUMENT_REGISTERS.get(index) {
            load_slot(assembler, slots, *argument, *register)?;
        } else {
            let extra = u32::try_from(index - ARGUMENT_REGISTERS.len())
                .map_err(|_| CodegenError::FrameOverflow)?;
            if extra == 0 {
                emit(
                    assembler,
                    Inst::Lea(
                        REST_ARGUMENT,
                        slot_mem_of(
                            slots
                                .outgoing_base
                                .checked_add(extra)
                                .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                    ),
                )?;
            }
            load_slot(assembler, slots, *argument, FUNCTION_OBJECT)?;
            emit(
                assembler,
                Inst::MovMR(
                    slot_mem_of(
                        slots
                            .outgoing_base
                            .checked_add(extra)
                            .ok_or(CodegenError::FrameOverflow)?,
                    )?,
                    FUNCTION_OBJECT,
                ),
            )?;
        }
    }
    emit(assembler, Inst::MovRR(FUNCTION_OBJECT, ENTRY))?;
    Ok(())
}

// `argc`/`args` mirror the calling convention's own argument-count/argument-
// list naming; that pairing is clearer here than any alternative spelling.
#[allow(clippy::similar_names)]
pub fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    capture_count: usize,
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let Some((argc, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "closure calls require a tagged argc argument".into(),
        ));
    };
    load_slot(assembler, slots, closure, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_slot(assembler, slots, *argc, ARGUMENT_COUNT)?;
    let total = capture_count
        .checked_add(rest.len())
        .ok_or(CodegenError::FrameOverflow)?;
    for index in 0..total {
        let target = ARGUMENT_REGISTERS.get(index).copied();
        if target.is_none() && index == ARGUMENT_REGISTERS.len() {
            emit(
                assembler,
                Inst::Lea(
                    REST_ARGUMENT,
                    slot_mem_of(
                        slots
                            .outgoing_base
                            .checked_add(0)
                            .ok_or(CodegenError::FrameOverflow)?,
                    )?,
                ),
            )?;
        }
        if index < capture_count {
            let offset = i32::try_from(
                ncl_object::function_offset::CAPTURES
                    .checked_add(index)
                    .and_then(|slot| slot.checked_add(1))
                    .and_then(|slot| slot.checked_mul(8))
                    .ok_or(CodegenError::FrameOverflow)?,
            )
            .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                Inst::MovRM(ENTRY, Mem::base(FUNCTION_OBJECT, offset)),
            )?;
        } else {
            load_slot(assembler, slots, rest[index - capture_count], ENTRY)?;
        }
        if let Some(register) = target {
            emit(assembler, Inst::MovRR(register, ENTRY))?;
        } else {
            let extra = u32::try_from(index - ARGUMENT_REGISTERS.len())
                .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                Inst::MovMR(
                    slot_mem_of(
                        slots
                            .outgoing_base
                            .checked_add(extra)
                            .ok_or(CodegenError::FrameOverflow)?,
                    )?,
                    ENTRY,
                ),
            )?;
        }
    }
    emit(
        assembler,
        Inst::MovRM(
            ENTRY,
            Mem::base(
                FUNCTION_OBJECT,
                i32::try_from(
                    ncl_object::function_offset::ENTRY
                        .checked_add(1)
                        .and_then(|slot| slot.checked_mul(8))
                        .ok_or(CodegenError::FrameOverflow)?,
                )
                .map_err(|_| CodegenError::FrameOverflow)?,
            ),
        ),
    )?;
    emit(
        assembler,
        Inst::ShiftImm(
            Shift::Sar,
            ENTRY,
            u8::try_from(ncl_sys::FIXNUM_TAG_BITS).map_err(|_| CodegenError::FrameOverflow)?,
        ),
    )
}
