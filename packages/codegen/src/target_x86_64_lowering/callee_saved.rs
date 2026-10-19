use super::{CodegenError, emit, slot_mem_of};
use ncl_asm_x86_64::{Assembler, Inst, Reg};

/// SysV callee-saved registers other than the frame pointer.
pub(super) const CALLEE_SAVED_REGISTERS: [Reg; 5] =
    [Reg::Rbx, Reg::R12, Reg::R13, Reg::R14, Reg::R15];
pub(super) const CALLEE_SAVED_WORDS: u32 = 5;

pub(super) fn save_callee_saved_registers(
    assembler: &mut Assembler,
    base_slot: u32,
) -> Result<(), CodegenError> {
    for (index, register) in CALLEE_SAVED_REGISTERS.into_iter().enumerate() {
        let slot = base_slot
            .checked_add(u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?)
            .ok_or(CodegenError::FrameOverflow)?;
        emit(assembler, Inst::MovMR(slot_mem_of(slot)?, register))?;
    }
    Ok(())
}

pub(super) fn restore_callee_saved_registers(
    assembler: &mut Assembler,
    base_slot: u32,
) -> Result<(), CodegenError> {
    for (index, register) in CALLEE_SAVED_REGISTERS.into_iter().enumerate() {
        let slot = base_slot
            .checked_add(u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?)
            .ok_or(CodegenError::FrameOverflow)?;
        emit(assembler, Inst::MovRM(register, slot_mem_of(slot)?))?;
    }
    Ok(())
}
