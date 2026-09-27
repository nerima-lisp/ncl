use super::{emit, load_value, primitives};
use crate::{Allocation, CodegenError};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::ValueId;

#[allow(clippy::similar_names)]
#[allow(clippy::too_many_lines)]
pub fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let Some((argc, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) malformed IR lacks the required argc value.
        return Err(CodegenError::Unsupported(
            "calls require a tagged argc argument".into(),
        ));
    };
    load_value(assembler, allocation, callee, Reg(16))?;
    primitives::load_callable_address(assembler, Reg(16), Reg(17))?;
    load_value(assembler, allocation, *argc, Reg(0))?;
    let extra_count = rest.len().saturating_sub(4);
    if extra_count > 0 {
        let offset = allocation
            .outgoing_base
            .checked_add(u32::try_from(extra_count).map_err(|_| CodegenError::FrameOverflow)?)
            .and_then(|slot| slot.checked_add(1))
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?;
        emit(
            assembler,
            Inst::Str {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(29)),
                    offset: i16::try_from(offset)
                        .map_err(|_| CodegenError::FrameOverflow)?
                        .checked_neg()
                        .ok_or(CodegenError::FrameOverflow)?,
                },
            }, // check-added-lines: allow(index) intentional
        )?;
    }
    for (index, argument) in rest.iter().enumerate() {
        if index < 4 {
            load_value(
                assembler,
                allocation,
                *argument,
                Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?),
            )?;
        } else {
            if index == 4 {
                let offset = allocation
                    .outgoing_base
                    .checked_add(1)
                    .and_then(|slot| slot.checked_mul(8))
                    .ok_or(CodegenError::FrameOverflow)?;
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(Reg(5)),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: u16::try_from(offset).map_err(|_| CodegenError::FrameOverflow)?,
                        shift: false,
                    },
                )?;
            }
            load_value(assembler, allocation, *argument, Reg(16))?;
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(5)),
                        offset: i16::try_from((index - 4).saturating_mul(8))
                            .map_err(|_| CodegenError::FrameOverflow)?,
                    },
                },
            )?;
        }
    }
    if extra_count > 0 {
        let offset = allocation
            .outgoing_base
            .checked_add(u32::try_from(extra_count).map_err(|_| CodegenError::FrameOverflow)?)
            .and_then(|slot| slot.checked_add(1))
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?;
        emit(
            assembler,
            Inst::Ldr {
                rt: Reg(16),
                mem: MemOperand::Unscaled {
                    base: RegOrSp::Reg(Reg(29)),
                    offset: i16::try_from(offset)
                        .map_err(|_| CodegenError::FrameOverflow)?
                        .checked_neg()
                        .ok_or(CodegenError::FrameOverflow)?,
                },
            },
            // check-added-lines: allow(index) intentional
        )?;
    } else {
        load_value(assembler, allocation, callee, Reg(16))?;
    }
    Ok(())
}

pub fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    lower_call(assembler, closure, args, allocation)?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(17),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(17)),
                offset: i16::try_from((ncl_object::function_offset::ENTRY + 1) * 8)
                    .map_err(|_| CodegenError::FrameOverflow)?,
            },
        },
    )?;
    primitives::decode_function_entry(assembler, Reg(17))
}
