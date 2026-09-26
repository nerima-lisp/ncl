use super::{
    emit, load_slot, ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FUNCTION_OBJECT, ValueSlots,
};
use crate::CodegenError;
use ncl_asm_x86_64::{Assembler, Inst, Mem, Shift};
use ncl_ir::ValueId;

pub fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let Some((argc, arguments)) = args.split_first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "calls require a tagged argc argument".into(),
        ));
    };
    if arguments.len() > ARGUMENT_REGISTERS.len() {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "x86-64 calls support at most four register arguments".into(),
        ));
    }
    load_slot(assembler, slots, callee, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_slot(assembler, slots, *argc, ARGUMENT_COUNT)?;
    for (index, argument) in arguments.iter().enumerate() {
        load_slot(
            assembler,
            slots,
            *argument,
            *ARGUMENT_REGISTERS
                .get(index)
                .ok_or(CodegenError::FrameOverflow)?,
        )?;
    }
    Ok(())
}

pub fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    capture_count: usize,
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let Some((argc, arguments)) = args.split_first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "closure calls require a tagged argc argument".into(),
        ));
    };
    if args
        .len()
        .saturating_sub(1)
        .checked_add(capture_count)
        .is_none_or(|count| count > ARGUMENT_REGISTERS.len())
    {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "x86-64 closure calls support at most four forwarded arguments".into(),
        ));
    }
    load_slot(assembler, slots, closure, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_slot(assembler, slots, *argc, ARGUMENT_COUNT)?;
    for index in 0..capture_count {
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
                Mem::base(FUNCTION_OBJECT, offset),
            ),
        )?;
    }
    for (index, argument) in arguments.iter().enumerate() {
        load_slot(
            assembler,
            slots,
            *argument,
            *ARGUMENT_REGISTERS
                .get(capture_count + index)
                .ok_or(CodegenError::FrameOverflow)?,
        )?;
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
