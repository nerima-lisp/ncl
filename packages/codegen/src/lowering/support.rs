//! Frame-slot addressing and instruction emission helpers.

use crate::CodegenError;
use ncl_asm_x86_64::{Assembler, Inst, Mem, Reg};
use ncl_ir::ValueId;

#[rustfmt::skip]
fn slot(slots: &[(ValueId, u32)], value: ValueId) -> Result<u32, CodegenError> { slots.iter().find(|(id, _)| *id == value).map(|(_, slot)| *slot).ok_or(CodegenError::UnknownValue(value)) }
#[rustfmt::skip]
fn memory(slot: u32) -> Result<Mem, CodegenError> { let bytes = slot.checked_add(1).and_then(|v| v.checked_mul(8)).ok_or(CodegenError::FrameOverflow)?; Ok(Mem::base(Reg::Rbp, -i32::try_from(bytes).map_err(|_| CodegenError::FrameOverflow)?)) }
#[rustfmt::skip]
pub(super) fn emit(assembler: &mut Assembler, inst: &Inst) -> Result<(), CodegenError> { assembler.emit(inst).map_err(|error| CodegenError::Encode(error.to_string())) }
#[rustfmt::skip]
pub(super) fn load(assembler: &mut Assembler, slots: &[(ValueId, u32)], value: ValueId, register: Reg) -> Result<(), CodegenError> { emit(assembler, &Inst::MovRM(register, memory(slot(slots, value)?)?)) }
#[rustfmt::skip]
pub(super) fn store(assembler: &mut Assembler, slots: &[(ValueId, u32)], value: ValueId, register: Reg) -> Result<(), CodegenError> { emit(assembler, &Inst::MovMR(memory(slot(slots, value)?)?, register)) }
