use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::ValueId;

use super::{context_mem, emit, load_value, store_value};
use crate::{Allocation, CodegenError, ContextField, RuntimeAbi};

pub(super) fn mv_area_mem(abi: &dyn RuntimeAbi, index: i32) -> Result<MemOperand, CodegenError> {
    let base = abi
        .field_offset(ContextField::MultipleValueArea)
        .map_err(|error| CodegenError::Abi(error.to_string()))?;
    let byte_offset = index
        .checked_mul(8)
        .and_then(|delta| base.checked_add(delta))
        .ok_or(CodegenError::FrameOverflow)?;
    Ok(MemOperand::Unsigned {
        base: RegOrSp::Reg(Reg(21)),
        offset: u16::try_from(byte_offset).map_err(|_| CodegenError::FrameOverflow)?,
        scale: 8,
    })
}

pub(super) fn lower_set_multiple_values(
    assembler: &mut Assembler,
    values: &[ValueId],
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if values.len() > ncl_sys::MULTIPLE_VALUE_AREA_WORDS {
        return Err(CodegenError::MultipleValueAreaOverflow {
            count: values.len(),
            capacity: ncl_sys::MULTIPLE_VALUE_AREA_WORDS,
        });
    }
    let count = u64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(1), count) {
        emit(assembler, instruction)?;
    }
    if let (Some(first), Some(result)) = (values.first(), result) {
        load_value(assembler, allocation, *first, Reg(16))?;
        store_value(assembler, allocation, result, Reg(16))?;
    }
    for (index, value) in values.iter().copied().enumerate() {
        load_value(assembler, allocation, value, Reg(16))?;
        emit(
            assembler,
            Inst::Str {
                rt: Reg(16),
                mem: mv_area_mem(
                    abi,
                    i32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?,
                )?,
            },
        )?;
    }
    emit(
        assembler,
        Inst::Str {
            rt: Reg(1),
            mem: context_mem(abi, ContextField::MultipleValueCount)?,
        },
    )
}
