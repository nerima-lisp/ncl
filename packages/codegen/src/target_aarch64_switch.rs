use super::{CodegenError, emit, lowering};
use crate::Allocation;
use ncl_asm_aarch64::{Assembler, Cond, Inst, Label, Reg, Shift};
use ncl_ir::{BlockId, Function, ValueId};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct Switch<'a> {
    pub(super) value: ValueId,
    pub(super) cases: &'a [(i64, BlockId, Vec<ValueId>)],
    pub(super) default: BlockId,
    pub(super) default_args: &'a [ValueId],
}

pub(super) fn lower_switch(
    assembler: &mut Assembler,
    function: &Function,
    allocation: &Allocation,
    switch: Switch<'_>,
    labels: &HashMap<BlockId, Label>,
) -> Result<(), CodegenError> {
    let Switch {
        value,
        cases,
        default,
        default_args,
    } = switch;
    lowering::load_value(assembler, allocation, value, Reg(16))?;
    let case_labels = cases
        .iter()
        .map(|_| assembler.new_label())
        .collect::<Vec<_>>();
    for ((case, _, _), label) in cases.iter().zip(&case_labels) {
        for instruction in
            ncl_asm_aarch64::mov_imm64(Reg(17), u64::from_ne_bytes(case.to_ne_bytes()))
        {
            emit(assembler, instruction)?;
        }
        emit(
            assembler,
            Inst::Cmp {
                rn: Reg(16),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        )?;
        emit(
            assembler,
            Inst::BCond {
                cond: Cond::Eq,
                label: *label,
            },
        )?;
    }
    let default_block = function
        .blocks
        .iter()
        .find(|candidate| candidate.id == default)
        .ok_or(CodegenError::UnknownBlock(default))?;
    lowering::move_args(assembler, allocation, default_args, &default_block.params)?;
    let default_label = labels
        .get(&default)
        .copied()
        .ok_or(CodegenError::UnknownBlock(default))?;
    emit(
        assembler,
        Inst::B {
            label: default_label,
        },
    )?;
    for ((_, target, args), label) in cases.iter().zip(case_labels) {
        assembler
            .bind(label)
            .map_err(|error| CodegenError::Encode(error.to_string()))?;
        let case_block = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == *target)
            .ok_or(CodegenError::UnknownBlock(*target))?;
        lowering::move_args(assembler, allocation, args, &case_block.params)?;
        let case_label = labels
            .get(target)
            .copied()
            .ok_or(CodegenError::UnknownBlock(*target))?;
        emit(assembler, Inst::B { label: case_label })?;
    }
    Ok(())
}
