use crate::isa_x86_64::SCRATCH;
use crate::machine_verify::SafepointSite;
use crate::{Allocation, CodegenError, Location, checked_u32};
use ncl_asm_x86_64::{Assembler, BinOp, Imm, Inst, Mem, Reg};
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
    pub(super) allocation: Allocation,
    pub(super) spill_base: u32,
    pub(super) outgoing_base: u32,
    pub(super) callee_saved_base: u32,
    pub(super) incoming_args_base: Option<u32>,
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

    pub(super) fn roots(
        &self,
        function: &Function,
        position: u32,
    ) -> Result<(Vec<u16>, Vec<u16>), CodegenError> {
        let (registers, slots) = crate::machine_verify::root_locations_at(
            function,
            &self.allocation,
            position,
            self.spill_base,
        );
        let slots = slots
            .into_iter()
            .map(|slot| u16::try_from(slot).map_err(|_| CodegenError::FrameOverflow))
            .collect::<Result<Vec<_>, _>>()?;
        Ok((registers, slots))
    }
}

pub(super) fn slots(
    function: &Function,
    argument_words: u32,
    allocation: Allocation,
    outgoing_base: u32,
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
            outgoing_base,
            callee_saved_base: 0,
            incoming_args_base: None,
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
pub(super) fn emit_call_instruction(
    assembler: &mut Assembler,
    register: Reg,
) -> Result<SafepointSite, CodegenError> {
    let start_offset = checked_u32(assembler.bytes().len())?;
    emit(assembler, Inst::CallReg(register))?;
    let end_offset = checked_u32(assembler.bytes().len())?;
    Ok(SafepointSite::call(start_offset, end_offset))
}

pub(super) fn emit_call(assembler: &mut Assembler) -> Result<SafepointSite, CodegenError> {
    emit(
        assembler,
        Inst::BinRI(BinOp::Sub, Reg::Rsp, CALLEE_HEADER_RESERVE),
    )?;
    let site = emit_call_instruction(assembler, ENTRY)?;
    emit(
        assembler,
        Inst::BinRI(BinOp::Add, Reg::Rsp, CALLEE_HEADER_RESERVE),
    )?;
    Ok(site)
}

#[path = "target_x86_64_lowering/runtime_ops.rs"]
mod runtime_ops;
use runtime_ops::{
    lower_alloc, lower_builtin, lower_runtime_builtin, lower_safepoint, store_return_values,
};

#[path = "target_x86_64_lowering/checked_ops.rs"]
mod checked_ops;

#[path = "target_x86_64_lowering/callee_saved.rs"]
mod callee_saved;
pub(super) use callee_saved::{
    CALLEE_SAVED_WORDS, restore_callee_saved_registers, save_callee_saved_registers,
};

#[path = "target_x86_64_lowering/calls.rs"]
mod calls;
pub(super) use calls::{lower_call, lower_closure_call, lower_load_capture};

#[path = "target_x86_64_lowering/dispatch.rs"]
pub(super) mod dispatch;
use dispatch::checked_call;
pub(super) use dispatch::lower_return_or_throw;

#[path = "target_x86_64_lowering/closure.rs"]
mod closure;
#[path = "target_x86_64_lowering/ops.rs"]
pub(super) mod ops;
#[path = "target_x86_64_lowering/ops_support.rs"]
mod ops_support;
#[path = "target_x86_64_lowering/primitives.rs"]
mod primitives;
pub(super) use closure::closure_capture_count;
pub(super) use ops_support::{load_heap_constant, store_closure_capture};

pub(super) use ops::{lower_op, move_args};
