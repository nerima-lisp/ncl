use super::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FUNCTION_OBJECT, RETURN_VALUE, ValueSlots, emit,
    load_immediate, load_slot,
};
use crate::{CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, Cond, Inst, Mem, Shift};
use ncl_ir::ValueId;

pub fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let Some((argc_value, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) malformed IR lacks the required argc value.
        return Err(CodegenError::Unsupported(
            "calls require a tagged argc argument".into(),
        ));
    };
    if rest.len() > ARGUMENT_REGISTERS.len() {
        // check-added-lines: allow(unsupported) the native ABI has four argument registers.
        return Err(CodegenError::Unsupported(
            "x86-64 calls support at most four register arguments".into(),
        ));
    }
    load_slot(assembler, slots, callee, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_slot(assembler, slots, *argc_value, ARGUMENT_COUNT)?;
    load_arguments(assembler, rest, slots, 0)
}

fn load_arguments(
    assembler: &mut Assembler,
    args: &[ValueId],
    slots: &ValueSlots,
    offset: usize,
) -> Result<(), CodegenError> {
    for (index, argument) in args.iter().enumerate() {
        load_slot(
            assembler,
            slots,
            *argument,
            *ARGUMENT_REGISTERS
                .get(offset + index)
                .ok_or(CodegenError::FrameOverflow)?,
        )?;
    }
    Ok(())
}

fn lower_named_global_call(
    assembler: &mut Assembler,
    closure: ValueId,
    symbol: ValueId,
    args: &[ValueId],
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let Some((argc_value, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) malformed IR lacks the required argc value.
        return Err(CodegenError::Unsupported(
            "closure calls require a tagged argc argument".into(),
        ));
    };
    if rest.len() > ARGUMENT_REGISTERS.len() {
        // check-added-lines: allow(unsupported) the native ABI has four argument registers.
        return Err(CodegenError::Unsupported(
            "x86-64 calls support at most four register arguments".into(),
        ));
    }
    load_slot(assembler, slots, symbol, FUNCTION_OBJECT)?;
    load_slot(assembler, slots, closure, ENTRY)?;
    load_slot(assembler, slots, *argc_value, ARGUMENT_COUNT)?;
    load_arguments(assembler, rest, slots, 0)?;

    let normal = assembler.new_label();
    let call = assembler.new_label();
    load_immediate(
        assembler,
        RETURN_VALUE,
        // check-added-lines: allow(unbound) compare against the function-cell sentinel.
        i64::from_ne_bytes(ncl_sys::Word::UNBOUND.bits().to_ne_bytes()),
    )?;
    emit(assembler, Inst::CmpRR(ENTRY, RETURN_VALUE))?;
    emit(assembler, Inst::Jcc(Cond::Ne, normal))?;
    load_immediate(
        assembler,
        ENTRY,
        i64::try_from(
            // check-added-lines: allow(unsupported) surface ABI lookup failure as codegen failure.
            abi.runtime_address(RuntimeFunction::UndefinedFunction)
                // check-added-lines: allow(unsupported) surface ABI lookup failure as codegen failure.
                .map_err(|error| CodegenError::Unsupported(error.to_string()))?,
        )
        .map_err(|_| CodegenError::FrameOverflow)?,
    )?;
    emit(assembler, Inst::Jmp(call))?;
    assembler.bind(normal);
    emit(assembler, Inst::MovRR(FUNCTION_OBJECT, ENTRY))?;
    load_function_entry(assembler)?;
    assembler.bind(call);
    Ok(())
}

fn load_function_entry(assembler: &mut Assembler) -> Result<(), CodegenError> {
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

pub fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    capture_count: usize,
    slots: &ValueSlots,
    named_symbol: Option<ValueId>,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let Some((argc_value, rest)) = args.split_first() else {
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
        // check-added-lines: allow(unsupported) the native ABI has four argument registers.
        return Err(CodegenError::Unsupported(
            "x86-64 closure calls support at most four forwarded arguments".into(),
        ));
    }
    if capture_count == 0
        && let Some(symbol) = named_symbol
    {
        return lower_named_global_call(assembler, closure, symbol, args, slots, abi);
    }
    load_slot(assembler, slots, closure, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_slot(assembler, slots, *argc_value, ARGUMENT_COUNT)?;
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
    load_arguments(assembler, rest, slots, capture_count)?;
    load_function_entry(assembler)
}
