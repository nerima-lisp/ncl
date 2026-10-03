use super::{emit, load_value};
use crate::{Allocation, CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::ValueId;

pub fn lower_runtime_builtin(
    assembler: &mut Assembler,
    function: RuntimeFunction,
    immediate_args: &[u64],
    value_args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let argument_count = immediate_args.len().saturating_add(value_args.len());
    let extra_count = argument_count.saturating_sub(4);
    let address = abi
        .runtime_address(function)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), address) {
        emit(assembler, instruction)?;
    }
    for (index, value) in immediate_args.iter().copied().enumerate() {
        if index < 4 {
            for instruction in ncl_asm_aarch64::mov_imm64(
                Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?),
                value,
            ) {
                emit(assembler, instruction)?;
            }
        }
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        let argument_index = immediate_args.len().saturating_add(index);
        if argument_index < 4 {
            load_value(
                assembler,
                allocation,
                value,
                Reg(u8::try_from(argument_index + 1).map_err(|_| CodegenError::FrameOverflow)?),
            )?;
        }
    }
    if extra_count == 0 {
        return Ok(());
    }
    let offset = allocation
        .outgoing_base
        .checked_add(u32::try_from(extra_count).map_err(|_| CodegenError::FrameOverflow)?)
        .and_then(|slot| slot.checked_add(1))
        .and_then(|slot| slot.checked_mul(8))
        .ok_or(CodegenError::FrameOverflow)?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(offset)) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Sub {
            rd: RegOrSp::Reg(Reg(5)),
            rn: RegOrSp::Reg(Reg(29)),
            rm: Reg(16),
            shift: Shift::Lsl(0),
        },
    )?;
    for (index, value) in immediate_args.iter().copied().enumerate().skip(4) {
        for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), value) {
            emit(assembler, instruction)?;
        }
        store_extra(assembler, index - 4)?;
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        let argument_index = immediate_args.len().saturating_add(index);
        if argument_index >= 4 {
            load_value(assembler, allocation, value, Reg(16))?;
            store_extra(assembler, argument_index - 4)?;
        }
    }
    Ok(())
}

fn store_extra(assembler: &mut Assembler, index: usize) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::Str {
            rt: Reg(16),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(5)),
                offset: i16::try_from(index.saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?,
            },
        },
    )
}
