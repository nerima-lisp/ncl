use super::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FUNCTION_OBJECT, REST_ARGUMENT, RETURN_VALUE,
    ValueSlots, emit, load_immediate, load_slot, slot_mem_of,
};
use crate::{CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Mem, Shift};
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
    emit(assembler, Inst::BinRI(BinOp::And, ENTRY, -8))?;
    load_slot(assembler, slots, *argc, ARGUMENT_COUNT)?;
    let overflow_count = u32::try_from(rest.len().saturating_sub(ARGUMENT_REGISTERS.len()))
        .map_err(|_| CodegenError::FrameOverflow)?;
    if overflow_count > 0 {
        emit(
            assembler,
            Inst::Lea(
                REST_ARGUMENT,
                slot_mem_of(
                    slots
                        .outgoing_base
                        .checked_add(overflow_count - 1)
                        .ok_or(CodegenError::FrameOverflow)?,
                )?,
            ),
        )?;
    }
    for (index, argument) in rest.iter().enumerate() {
        if let Some(register) = ARGUMENT_REGISTERS.get(index) {
            load_slot(assembler, slots, *argument, *register)?;
        } else {
            let extra = u32::try_from(index - ARGUMENT_REGISTERS.len())
                .map_err(|_| CodegenError::FrameOverflow)?;
            let slot = overflow_count
                .checked_sub(extra.checked_add(1).ok_or(CodegenError::FrameOverflow)?)
                .ok_or(CodegenError::FrameOverflow)?;
            load_slot(assembler, slots, *argument, FUNCTION_OBJECT)?;
            emit(
                assembler,
                Inst::MovMR(
                    slot_mem_of(
                        slots
                            .outgoing_base
                            .checked_add(slot)
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
    capture_count: usize,
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
    let undefined_done = if capture_count == 0
        && let Some(symbol) = named_symbol
    {
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
    let total = capture_count
        .checked_add(rest.len())
        .ok_or(CodegenError::FrameOverflow)?;
    let overflow_count = u32::try_from(total.saturating_sub(ARGUMENT_REGISTERS.len()))
        .map_err(|_| CodegenError::FrameOverflow)?;
    if overflow_count > 0 {
        emit(
            assembler,
            Inst::Lea(
                REST_ARGUMENT,
                slot_mem_of(
                    slots
                        .outgoing_base
                        .checked_add(overflow_count - 1)
                        .ok_or(CodegenError::FrameOverflow)?,
                )?,
            ),
        )?;
    }
    for index in 0..total {
        let target = ARGUMENT_REGISTERS.get(index).copied();
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
                Inst::MovRM(
                    *ARGUMENT_REGISTERS
                        .get(index)
                        .ok_or(CodegenError::FrameOverflow)?,
                    Mem::base(RETURN_VALUE, offset),
                ),
            )?;
        } else {
            // check-added-lines: allow(index) capture layout bounds the rest offset.
            load_slot(assembler, slots, rest[index - capture_count], ENTRY)?;
            if let Some(register) = target {
                emit(assembler, Inst::MovRR(register, ENTRY))?;
            } else {
                let extra = u32::try_from(index - ARGUMENT_REGISTERS.len())
                    .map_err(|_| CodegenError::FrameOverflow)?;
                let slot = overflow_count
                    .checked_sub(extra.checked_add(1).ok_or(CodegenError::FrameOverflow)?)
                    .ok_or(CodegenError::FrameOverflow)?;
                emit(
                    assembler,
                    Inst::MovMR(
                        slot_mem_of(
                            slots
                                .outgoing_base
                                .checked_add(slot)
                                .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                        ENTRY,
                    ),
                )?;
            }
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
