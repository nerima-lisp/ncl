use super::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FRAME_POINTER, FUNCTION_OBJECT, REST_ARGUMENT,
    RETURN_VALUE, ValueSlots, emit, load_immediate, load_slot, slot_mem_of, store_slot,
};
use crate::{CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Mem, Shift};
use ncl_ir::ValueId;

pub fn lower_load_capture(
    assembler: &mut Assembler,
    index: u8,
    result: Option<ValueId>,
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let offset = i32::try_from(
        ncl_object::function_offset::CAPTURES
            .checked_add(usize::from(index))
            .and_then(|slot| slot.checked_add(1))
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    emit(
        assembler,
        Inst::MovRM(RETURN_VALUE, Mem::base(FRAME_POINTER, 16)),
    )?;
    load_immediate(
        assembler,
        ENTRY,
        i64::from_ne_bytes((!ncl_sys::LOWTAG_MASK).to_ne_bytes()),
    )?;
    emit(assembler, Inst::BinRR(BinOp::And, RETURN_VALUE, ENTRY))?;
    emit(
        assembler,
        Inst::MovRM(FUNCTION_OBJECT, Mem::base(RETURN_VALUE, offset)),
    )?;
    if let Some(result) = result {
        store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
    }
    Ok(())
}

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
    // `slot_mem_of(i)` addresses `rbp - (i + 1) * 8`, so it grows *downward*
    // (higher `i` means a lower address). `REST_ARGUMENT` must nonetheless
    // point at the start of an ascending array (`rest[0]`, `rest[1]`, ... at
    // increasing addresses) because every consumer on the other end (the
    // callee's own prologue, and the native-builtin dispatcher's
    // `copy_native_words`) reads it that way, matching aarch64's
    // `lower_call`. So the *last* overflow argument is stored at the lowest
    // address/outgoing slot (`outgoing_base + extra_count - 1`) and
    // `REST_ARGUMENT` is anchored there; earlier overflow arguments live at
    // increasing addresses (decreasing outgoing-slot indices) above it.
    let extra_count = rest.len().saturating_sub(ARGUMENT_REGISTERS.len());
    for (index, argument) in rest.iter().enumerate() {
        if let Some(register) = ARGUMENT_REGISTERS.get(index) {
            load_slot(assembler, slots, *argument, *register)?;
        } else {
            let extra = index - ARGUMENT_REGISTERS.len();
            if extra == 0 {
                emit(
                    assembler,
                    Inst::Lea(
                        REST_ARGUMENT,
                        slot_mem_of(
                            slots
                                .outgoing_base
                                .checked_add(
                                    u32::try_from(extra_count.saturating_sub(1))
                                        .map_err(|_| CodegenError::FrameOverflow)?,
                                )
                                .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                    ),
                )?;
            }
            let physical = extra_count.saturating_sub(1).saturating_sub(extra);
            load_slot(assembler, slots, *argument, FUNCTION_OBJECT)?;
            emit(
                assembler,
                Inst::MovMR(
                    slot_mem_of(
                        slots
                            .outgoing_base
                            .checked_add(
                                u32::try_from(physical).map_err(|_| CodegenError::FrameOverflow)?,
                            )
                            .ok_or(CodegenError::FrameOverflow)?,
                    )?,
                    FUNCTION_OBJECT,
                ),
            )?;
        }
    }
    if rest.len() > ARGUMENT_REGISTERS.len() {
        emit(assembler, Inst::MovRR(FUNCTION_OBJECT, ENTRY))?;
    }
    Ok(())
}

// `argc`/`args` mirror the calling convention's own argument-count/argument-
// list naming; that pairing is clearer here than any alternative spelling.
#[allow(clippy::similar_names)]
#[allow(clippy::too_many_lines)]
pub fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    slots: &ValueSlots,
    named_symbol: Option<ValueId>,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let Some((argc, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "closure calls require a tagged argc argument".into(),
        ));
    };
    let undefined_done = if let Some(symbol) = named_symbol {
        load_slot(assembler, slots, symbol, FUNCTION_OBJECT)?;
        load_slot(assembler, slots, closure, ENTRY)?;
        load_immediate(
            assembler,
            // check-added-lines: allow(unbound)
            RETURN_VALUE,
            i64::from_ne_bytes(
                ncl_sys::Word::from_bits(0xffff_ffff_ffff_fff9)
                    .bits()
                    .to_ne_bytes(),
            ),
        )?;
        emit(assembler, Inst::CmpRR(ENTRY, RETURN_VALUE))?;
        let normal = assembler.new_label();
        emit(assembler, Inst::Jcc(Cond::Ne, normal))?;
        load_immediate(
            assembler,
            ENTRY,
            i64::try_from(
                abi.runtime_address(RuntimeFunction::UndefinedFunction)
                    .map_err(|error| CodegenError::Abi(error.to_string()))?,
            )
            .map_err(|_| CodegenError::FrameOverflow)?,
        )?;
        let done = assembler.new_label();
        emit(assembler, Inst::Jmp(done))?;
        assembler.bind(normal);
        Some(done)
    } else {
        None
    };
    load_slot(assembler, slots, closure, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_immediate(
        assembler,
        RETURN_VALUE,
        i64::from_ne_bytes((!ncl_sys::LOWTAG_MASK).to_ne_bytes()),
    )?;
    emit(assembler, Inst::BinRR(BinOp::And, ENTRY, RETURN_VALUE))?;
    emit(assembler, Inst::MovRR(RETURN_VALUE, ENTRY))?;
    load_slot(assembler, slots, *argc, ARGUMENT_COUNT)?;
    // See the matching comment in `lower_call`: `REST_ARGUMENT` must point at
    // an ascending array, so it is anchored at the lowest-address outgoing
    // slot (`extra_count - 1`) and each overflow argument's physical slot
    // index is mirrored (`extra_count - 1 - extra`) so that later arguments
    // land at increasing addresses above it.
    let extra_count = rest.len().saturating_sub(ARGUMENT_REGISTERS.len());
    for (index, argument) in rest.iter().copied().enumerate() {
        let target = ARGUMENT_REGISTERS.get(index).copied();
        if target.is_none() && index == ARGUMENT_REGISTERS.len() {
            emit(
                assembler,
                Inst::Lea(
                    REST_ARGUMENT,
                    slot_mem_of(
                        slots
                            .outgoing_base
                            .checked_add(
                                u32::try_from(extra_count.saturating_sub(1))
                                    .map_err(|_| CodegenError::FrameOverflow)?,
                            )
                            .ok_or(CodegenError::FrameOverflow)?,
                    )?,
                ),
            )?;
        }
        load_slot(assembler, slots, argument, ENTRY)?;
        if let Some(register) = target {
            emit(assembler, Inst::MovRR(register, ENTRY))?;
        } else {
            let extra = index.saturating_sub(ARGUMENT_REGISTERS.len());
            let physical = extra_count.saturating_sub(1).saturating_sub(extra);
            emit(
                assembler,
                Inst::MovMR(
                    slot_mem_of(
                        slots
                            .outgoing_base
                            .checked_add(
                                u32::try_from(physical).map_err(|_| CodegenError::FrameOverflow)?,
                            )
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
                RETURN_VALUE,
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
    )?;
    if let Some(done) = undefined_done {
        assembler.bind(done);
    }
    Ok(())
}
