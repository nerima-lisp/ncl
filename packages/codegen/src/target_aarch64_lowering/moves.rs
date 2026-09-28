use super::{emit, load_value, store_value};
use crate::{Allocation, CodegenError};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::{BlockParam, ValueId};

pub fn move_args(
    assembler: &mut Assembler,
    allocation: &Allocation,
    args: &[ValueId],
    params: &[BlockParam],
) -> Result<(), CodegenError> {
    if args.len() != params.len() {
        // check-added-lines: allow(unsupported) arity is validated by the caller contract.
        return Err(CodegenError::Unsupported(
            "block argument arity mismatch".into(),
        ));
    }
    let mut moves = Vec::new();
    for (argument, parameter) in args.iter().zip(params) {
        let source = allocation
            .location(*argument)
            .ok_or(CodegenError::UnknownValue(*argument))?;
        let destination = allocation
            .location(parameter.value)
            .ok_or(CodegenError::UnknownValue(parameter.value))?;
        if source != destination {
            moves.push((*argument, parameter.value));
        }
    }
    let overlapping = moves.iter().any(|(argument, _)| {
        moves
            .iter()
            .any(|(_, parameter)| allocation.location(*argument) == allocation.location(*parameter))
    });
    if overlapping {
        let temporary_bytes = moves
            .len()
            .checked_mul(8)
            .and_then(|bytes| bytes.checked_add(15))
            .map(|bytes| bytes & !15)
            .and_then(|bytes| u16::try_from(bytes).ok())
            .ok_or(CodegenError::FrameOverflow)?;
        emit(
            assembler,
            Inst::SubImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: temporary_bytes,
                shift: false,
            },
        )?;
        for (index, (argument, _)) in moves.iter().enumerate() {
            load_value(assembler, allocation, *argument, Reg(16))?;
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Sp,
                        offset: i16::try_from(index.saturating_mul(8))
                            .map_err(|_| CodegenError::FrameOverflow)?,
                    },
                },
            )?;
        }
        for (index, (_, parameter)) in moves.iter().enumerate() {
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Sp,
                        offset: i16::try_from(index.saturating_mul(8))
                            .map_err(|_| CodegenError::FrameOverflow)?,
                    },
                },
            )?;
            store_value(assembler, allocation, *parameter, Reg(16))?;
        }
        emit(
            assembler,
            Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: temporary_bytes,
                shift: false,
            },
        )?;
        return Ok(());
    }
    for (argument, parameter) in moves {
        load_value(assembler, allocation, argument, Reg(17))?;
        store_value(assembler, allocation, parameter, Reg(17))?;
    }
    Ok(())
}
