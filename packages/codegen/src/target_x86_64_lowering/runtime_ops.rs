use super::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FRAME_POINTER, FUNCTION_OBJECT, REST_ARGUMENT,
    RETURN_VALUE, VALUE_COUNT, ValueSlots, dispatch, emit, emit_call_instruction, load_immediate,
    load_slot, slot_mem_of, store_slot,
};
use crate::isa_x86_64::THREAD_CONTEXT;
use crate::machine_verify::SafepointSite;
use crate::{CodegenError, ContextField, RuntimeAbi, RuntimeFunction, common_lisp_builtin};
use ncl_asm_x86_64::{Assembler, Cond, Inst, Mem, Reg};
use ncl_ir::ValueId;

pub(super) fn lower_runtime_builtin(
    assembler: &mut Assembler,
    function: RuntimeFunction,
    immediate_args: &[i64],
    value_args: &[ValueId],
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if immediate_args.len() + value_args.len() > ARGUMENT_REGISTERS.len() {
        // check-added-lines: allow(unsupported) preserve the runtime helper arity diagnostic.
        return Err(CodegenError::Unsupported(
            "x86-64 runtime calls support at most four arguments".into(),
        ));
    }
    let address = abi
        .runtime_address(function)
        // check-added-lines: allow(unsupported) preserve runtime ABI lookup failures.
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?
        .cast_signed();
    emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
    load_immediate(assembler, ENTRY, address)?;
    for (index, value) in immediate_args.iter().copied().enumerate() {
        let register = ARGUMENT_REGISTERS
            .get(index)
            .copied()
            .ok_or(CodegenError::FrameOverflow)?;
        load_immediate(assembler, register, value)?;
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        let register_index = immediate_args
            .len()
            .checked_add(index)
            .ok_or(CodegenError::FrameOverflow)?;
        let register = ARGUMENT_REGISTERS
            .get(register_index)
            .copied()
            .ok_or(CodegenError::FrameOverflow)?;
        load_slot(assembler, slots, value, register)?;
    }
    Ok(())
}

fn context_mem(abi: &dyn RuntimeAbi, field: ContextField) -> Result<Mem, CodegenError> {
    let offset = abi
        .field_offset(field)
        // check-added-lines: allow(unsupported) preserve runtime ABI field lookup failures.
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    Ok(Mem::base(THREAD_CONTEXT, offset))
}

pub(super) fn store_return_values(
    assembler: &mut Assembler,
    slots: &ValueSlots,
    values: &[ValueId],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if values.len() > ncl_sys::MULTIPLE_VALUE_AREA_WORDS {
        return Err(CodegenError::MultipleValueAreaOverflow {
            count: values.len(),
            capacity: ncl_sys::MULTIPLE_VALUE_AREA_WORDS,
        });
    }
    let base = abi
        .field_offset(ContextField::MultipleValueArea)
        // check-added-lines: allow(unsupported) preserve runtime ABI field lookup failures.
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    for (index, value) in values.iter().copied().enumerate() {
        let byte_offset = i32::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(8))
            .and_then(|index| base.checked_add(index))
            .ok_or(CodegenError::FrameOverflow)?;
        load_slot(assembler, slots, value, FUNCTION_OBJECT)?;
        emit(
            assembler,
            Inst::MovMR(Mem::base(THREAD_CONTEXT, byte_offset), FUNCTION_OBJECT),
        )?;
    }
    Ok(())
}

fn runtime_address(abi: &dyn RuntimeAbi, function: RuntimeFunction) -> Result<i64, CodegenError> {
    abi.runtime_address(function)
        .map(u64::cast_signed)
        // check-added-lines: allow(unsupported) preserve runtime ABI lookup failures.
        .map_err(|error| CodegenError::Unsupported(error.to_string()))
}

pub(super) fn lower_alloc(
    assembler: &mut Assembler,
    words: u32,
    result: Option<ValueId>,
    context: &dispatch::CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let slots = context.slots;
    let abi = context.abi;
    let bytes = words
        .checked_mul(8)
        .and_then(|value| i32::try_from(value).ok())
        .ok_or(CodegenError::FrameOverflow)?;
    let slow = assembler.new_label();
    let done = assembler.new_label();
    emit(
        assembler,
        Inst::MovRM(FUNCTION_OBJECT, context_mem(abi, ContextField::TlabBump)?),
    )?;
    emit(
        assembler,
        Inst::MovRM(ENTRY, context_mem(abi, ContextField::TlabLimit)?),
    )?;
    emit(
        assembler,
        Inst::Lea(RETURN_VALUE, Mem::base(FUNCTION_OBJECT, bytes)),
    )?;
    emit(assembler, Inst::CmpRR(RETURN_VALUE, ENTRY))?;
    emit(assembler, Inst::Jcc(Cond::A, slow))?;
    emit(
        assembler,
        Inst::MovMR(context_mem(abi, ContextField::TlabBump)?, RETURN_VALUE),
    )?;
    if let Some(result) = result {
        store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
    }
    emit(assembler, Inst::Jmp(done))?;
    assembler.bind(slow);
    emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
    load_immediate(assembler, Reg::Rsi, i64::from(words))?;
    load_immediate(
        assembler,
        ENTRY,
        runtime_address(abi, RuntimeFunction::AllocateSlow)?,
    )?;
    let checked_call = dispatch::checked_call(assembler, context, |assembler| {
        emit_call_instruction(assembler, ENTRY)
    })?;
    if let Some(result) = result {
        store_slot(assembler, slots, result, RETURN_VALUE)?;
    }
    assembler.bind(done);
    Ok(checked_call.site())
}

pub(super) fn lower_safepoint(
    assembler: &mut Assembler,
    abi: &dyn RuntimeAbi,
) -> Result<SafepointSite, CodegenError> {
    let done = assembler.new_label();
    emit(
        assembler,
        Inst::MovRM(
            FUNCTION_OBJECT,
            context_mem(abi, ContextField::SafepointRequest)?,
        ),
    )?;
    emit(assembler, Inst::CmpRI(FUNCTION_OBJECT, 0))?;
    emit(assembler, Inst::Jcc(Cond::E, done))?;
    emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
    emit(assembler, Inst::MovRR(Reg::Rsi, FRAME_POINTER))?;
    load_immediate(
        assembler,
        ENTRY,
        runtime_address(abi, RuntimeFunction::SafepointSlow)?,
    )?;
    // The continuation PC is the address after the `call r11` below. That call is
    // three bytes (REX.B + FF /2 + ModRM), so the RIP-relative displacement from
    // the end of this `lea` to the instruction after the call is exactly three.
    emit(assembler, Inst::Lea(VALUE_COUNT, Mem::rip(3)))?;
    let site = emit_call_instruction(assembler, ENTRY)?;
    assembler.bind(done);
    Ok(site)
}

#[allow(clippy::similar_names)]
pub(super) fn lower_builtin(
    assembler: &mut Assembler,
    name: &str,
    args: &[ValueId],
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if args.len() > ARGUMENT_REGISTERS.len() {
        // check-added-lines: allow(unsupported) preserve the builtin argument-limit diagnostic.
        return Err(CodegenError::Unsupported(
            "x86-64 builtins support at most four arguments".into(),
        ));
    }
    if name == "make-rest-list" {
        // check-added-lines: allow(index) intentional
        let [argc_value, start_value] = args else {
            // check-added-lines: allow(unsupported) intentional
            return Err(CodegenError::Unsupported(
                "make-rest-list requires argc and start".into(), // check-added-lines: allow(unsupported) intentional
            ));
        };
        emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
        if let Some(base) = slots.incoming_args_base {
            for (index, register) in ARGUMENT_REGISTERS.into_iter().enumerate() {
                emit(
                    assembler,
                    Inst::MovRM(
                        register,
                        slot_mem_of(
                            base.checked_add(
                                u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?,
                            )
                            .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                    ),
                )?;
            }
            emit(
                assembler,
                Inst::MovRM(
                    REST_ARGUMENT,
                    slot_mem_of(base.checked_add(4).ok_or(CodegenError::FrameOverflow)?)?,
                ),
            )?;
        }
        for register in ARGUMENT_REGISTERS
            .into_iter()
            .chain(std::iter::once(REST_ARGUMENT))
        {
            emit(assembler, Inst::MovRR(register, register))?;
        }
        load_slot(assembler, slots, *argc_value, FUNCTION_OBJECT)?;
        load_slot(assembler, slots, *start_value, RETURN_VALUE)?;
        emit(
            assembler,
            Inst::MovMR(Mem::base(Reg::Rsp, -16), FUNCTION_OBJECT),
        )?;
        emit(
            assembler,
            Inst::MovMR(Mem::base(Reg::Rsp, -8), RETURN_VALUE),
        )?;
        load_immediate(
            assembler,
            ENTRY,
            abi.builtin_address(common_lisp_builtin(name))
                .map_err(|error| CodegenError::Unsupported(error.to_string()))? // check-added-lines: allow(unsupported) ABI address lookup failure.
                .cast_signed(),
        )?;
        return Ok(());
    }
    let address = abi
        .builtin_address(common_lisp_builtin(name))
        // check-added-lines: allow(unsupported) preserve builtin ABI lookup failures.
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
    load_immediate(assembler, ENTRY, address.cast_signed())?;
    for (index, argument) in args.iter().enumerate() {
        let register = ARGUMENT_REGISTERS
            .get(index)
            .copied()
            .ok_or(CodegenError::FrameOverflow)?;
        load_slot(assembler, slots, *argument, register)?;
    }
    Ok(())
}
