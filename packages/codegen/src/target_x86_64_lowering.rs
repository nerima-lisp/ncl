use crate::isa_x86_64::{SCRATCH, THREAD_CONTEXT};
use crate::{
    common_lisp_builtin, Allocation, CodegenError, ContextField, Location, RuntimeAbi,
    RuntimeFunction,
};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg};
use ncl_ir::{Function, ValueId};

/// Register carrying the callee function object on entry, stored as frame header word 2.
pub(super) const FUNCTION_OBJECT: Reg = SCRATCH[0];
/// Scratch register holding the indirect call target.
pub(super) const ENTRY: Reg = SCRATCH[1];
/// Frame pointer, pointing at frame header word 0.
pub(super) const FRAME_POINTER: Reg = Reg::Rbp;
/// Return value register.
pub(super) const RETURN_VALUE: Reg = Reg::Rax;
/// Multiple-value count register.
pub(super) const VALUE_COUNT: Reg = Reg::Rdx;
/// Argument count register.
pub(super) const ARGUMENT_COUNT: Reg = Reg::Rdi;
/// First four argument registers.
pub(super) const ARGUMENT_REGISTERS: [Reg; 4] = [Reg::Rsi, Reg::Rdx, Reg::Rcx, Reg::R8];
/// Rest-argument register, holding a pointer to arguments beyond the fourth.
pub(super) const REST_ARGUMENT: Reg = Reg::R9;
/// Bytes reserved below `rsp` before a call so the callee can write frame header words 2 and 3.
const CALLEE_HEADER_RESERVE: i32 = 16;

#[allow(clippy::needless_pass_by_value)]
pub(super) fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
    assembler
        .emit(&instruction)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

/// Loads a 64-bit word, preferring the sign-extending `mov r64, imm32` form when it fits.
pub(super) fn load_immediate(
    assembler: &mut Assembler,
    register: Reg,
    value: i64,
) -> Result<(), CodegenError> {
    let immediate = i32::try_from(value).map_or(Imm::I64(value), Imm::I32);
    emit(assembler, Inst::MovRI(register, immediate))
}

pub(super) struct ValueSlots {
    values: Vec<(ValueId, u32)>,
    allocation: Allocation,
    spill_base: u32,
}

impl ValueSlots {
    pub(super) fn location(&self, value: ValueId) -> Result<Location, CodegenError> {
        self.allocation
            .location(value)
            .ok_or(CodegenError::UnknownValue(value))
    }

    pub(super) fn stack_index(&self, value: ValueId) -> Result<u32, CodegenError> {
        match self.location(value)? {
            Location::Spill(index) => self
                .spill_base
                .checked_add(index)
                .ok_or(CodegenError::FrameOverflow),
            Location::Register(_) => slot(self, value),
        }
    }

    pub(super) fn roots(&self, position: u32) -> (Vec<u16>, Vec<u16>) {
        let mut registers = Vec::new();
        let mut spills = Vec::new();
        for interval in &self.allocation.intervals {
            if interval.ty != ncl_ir::Ty::Word
                || interval.start >= position
                || position > interval.end
            {
                continue;
            }
            match self.allocation.location(interval.value) {
                Some(Location::Register(register)) => registers.push(register),
                Some(Location::Spill(spill)) => {
                    if let Some(slot) = self.spill_base.checked_add(spill)
                        && let Ok(slot) = u16::try_from(slot)
                    {
                        spills.push(slot);
                    }
                }
                None => {}
            }
        }
        registers.sort_unstable();
        registers.dedup();
        spills.sort_unstable();
        spills.dedup();
        (registers, spills)
    }
}

pub(super) fn slots(
    function: &Function,
    argument_words: u32,
    allocation: Allocation,
) -> (ValueSlots, u32) {
    let mut result = Vec::new();
    let mut next = argument_words;
    for block in &function.blocks {
        for parameter in &block.params {
            result.push((parameter.value, next));
            next = next.saturating_add(1);
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                result.push((*value, next));
                next = next.saturating_add(1);
            }
        }
    }
    let local_words = next.saturating_sub(argument_words);
    let spill_base = argument_words.saturating_add(local_words);
    (
        ValueSlots {
            values: result,
            allocation,
            spill_base,
        },
        local_words,
    )
}

fn slot(slots: &ValueSlots, value: ValueId) -> Result<u32, CodegenError> {
    slots
        .values
        .iter()
        .find(|(id, _)| *id == value)
        .map(|(_, index)| *index)
        .ok_or(CodegenError::UnknownValue(value))
}

/// Returns the frame address of a value slot by index.
pub(super) fn slot_mem_of(index: u32) -> Result<Mem, CodegenError> {
    let bytes = index
        .checked_add(1)
        .and_then(|value| value.checked_mul(8))
        .ok_or(CodegenError::FrameOverflow)?;
    Ok(Mem::base(
        FRAME_POINTER,
        -i32::try_from(bytes).map_err(|_| CodegenError::FrameOverflow)?,
    ))
}

fn value_location(slots: &ValueSlots, value: ValueId) -> Result<Location, CodegenError> {
    slots.location(value)
}

fn stack_index(slots: &ValueSlots, value: ValueId) -> Result<u32, CodegenError> {
    slots.stack_index(value)
}

fn slot_mem(slots: &ValueSlots, value: ValueId) -> Result<Mem, CodegenError> {
    slot_mem_of(stack_index(slots, value)?)
}

pub(super) fn load_slot(
    assembler: &mut Assembler,
    slots: &ValueSlots,
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    match value_location(slots, value)? {
        Location::Register(id) => {
            let source = Reg::from_id(u8::try_from(id).map_err(|_| CodegenError::FrameOverflow)?)
                .ok_or(CodegenError::FrameOverflow)?;
            emit(assembler, Inst::MovRR(register, source))
        }
        Location::Spill(_) => emit(assembler, Inst::MovRM(register, slot_mem(slots, value)?)),
    }
}

pub(super) fn store_slot(
    assembler: &mut Assembler,
    slots: &ValueSlots,
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    match value_location(slots, value)? {
        Location::Register(id) => {
            let destination =
                Reg::from_id(u8::try_from(id).map_err(|_| CodegenError::FrameOverflow)?)
                    .ok_or(CodegenError::FrameOverflow)?;
            if destination != register {
                emit(assembler, Inst::MovRR(destination, register))?;
            }
            Ok(())
        }
        Location::Spill(_) => emit(assembler, Inst::MovMR(slot_mem(slots, value)?, register)),
    }
}

/// Emits an indirect call and returns the offset of the callee's return address.
///
/// The caller reserves the callee's frame header words below `rsp` and releases
/// the reservation after the call. The returned offset is the instruction
/// immediately after the call, where the callee resumes, so that is where the
/// call's safepoint map must be registered: the map lookup selects the nearest
/// map at or before a PC, so a map placed after the result store would resolve a
/// caller frame to an older map.
pub(super) fn emit_call(assembler: &mut Assembler) -> Result<u32, CodegenError> {
    emit(
        assembler,
        Inst::BinRI(BinOp::Sub, Reg::Rsp, CALLEE_HEADER_RESERVE),
    )?;
    emit(assembler, Inst::CallReg(ENTRY))?;
    let return_offset =
        u32::try_from(assembler.bytes().len()).map_err(|_| CodegenError::FrameOverflow)?;
    emit(
        assembler,
        Inst::BinRI(BinOp::Add, Reg::Rsp, CALLEE_HEADER_RESERVE),
    )?;
    Ok(return_offset)
}

pub(super) fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    if args.len() > ARGUMENT_REGISTERS.len() {
        return Err(CodegenError::Unsupported(
            "x86-64 calls support at most four register arguments".into(),
        ));
    }
    load_slot(assembler, slots, callee, FUNCTION_OBJECT)?;
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    load_immediate(
        assembler,
        ARGUMENT_COUNT,
        i64::try_from(args.len()).map_err(|_| CodegenError::FrameOverflow)?,
    )?;
    for (index, argument) in args.iter().enumerate() {
        load_slot(assembler, slots, *argument, ARGUMENT_REGISTERS[index])?;
    }
    Ok(())
}

pub(super) fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    lower_call(assembler, closure, args, slots)?;
    emit(assembler, Inst::MovRM(ENTRY, Mem::base(FUNCTION_OBJECT, 0)))
}

pub(super) fn lower_runtime_builtin(
    assembler: &mut Assembler,
    function: RuntimeFunction,
    immediate_args: &[i64],
    value_args: &[ValueId],
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if immediate_args.len() + value_args.len() > ARGUMENT_REGISTERS.len() {
        return Err(CodegenError::Unsupported(
            "x86-64 runtime calls support at most four arguments".into(),
        ));
    }
    let address = abi
        .runtime_address(function)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?
        .cast_signed();
    emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
    load_immediate(assembler, ENTRY, address)?;
    for (index, value) in immediate_args.iter().copied().enumerate() {
        load_immediate(assembler, ARGUMENT_REGISTERS[index], value)?;
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        load_slot(
            assembler,
            slots,
            value,
            ARGUMENT_REGISTERS[immediate_args.len() + index],
        )?;
    }
    Ok(())
}

fn context_mem(abi: &dyn RuntimeAbi, field: ContextField) -> Result<Mem, CodegenError> {
    let offset = abi
        .field_offset(field)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    Ok(Mem::base(THREAD_CONTEXT, offset))
}

fn runtime_address(abi: &dyn RuntimeAbi, function: RuntimeFunction) -> Result<i64, CodegenError> {
    abi.runtime_address(function)
        .map(u64::cast_signed)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))
}

fn lower_alloc(
    assembler: &mut Assembler,
    words: u32,
    result: Option<ValueId>,
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<u32, CodegenError> {
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
    emit(assembler, Inst::CallReg(ENTRY))?;
    let call_pc =
        u32::try_from(assembler.bytes().len()).map_err(|_| CodegenError::FrameOverflow)?;
    if let Some(result) = result {
        store_slot(assembler, slots, result, RETURN_VALUE)?;
    }
    assembler.bind(done);
    Ok(call_pc)
}

fn lower_safepoint(assembler: &mut Assembler, abi: &dyn RuntimeAbi) -> Result<u32, CodegenError> {
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
    emit(assembler, Inst::CallReg(ENTRY))?;
    let call_pc =
        u32::try_from(assembler.bytes().len()).map_err(|_| CodegenError::FrameOverflow)?;
    assembler.bind(done);
    Ok(call_pc)
}

fn lower_builtin(
    assembler: &mut Assembler,
    name: &str,
    args: &[ValueId],
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if args.len() > ARGUMENT_REGISTERS.len() {
        return Err(CodegenError::Unsupported(
            "x86-64 builtins support at most four arguments".into(),
        ));
    }
    let address = abi
        .builtin_address(common_lisp_builtin(name))
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
    load_immediate(assembler, ENTRY, address.cast_signed())?;
    for (index, argument) in args.iter().enumerate() {
        load_slot(assembler, slots, *argument, ARGUMENT_REGISTERS[index])?;
    }
    Ok(())
}

#[path = "target_x86_64_lowering/ops.rs"]
pub(super) mod ops;
pub(super) use ops::{lower_op, move_args};
