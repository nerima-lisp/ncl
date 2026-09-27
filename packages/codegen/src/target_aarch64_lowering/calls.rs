use super::{emit, load_value, primitives};
use crate::{Allocation, CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::ValueId;

#[allow(clippy::redundant_pub_crate)]
#[allow(clippy::similar_names)]
#[allow(clippy::too_many_lines)]
pub(crate) fn lower_call(
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

#[allow(clippy::redundant_pub_crate)]
pub(crate) fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
    named_symbol: Option<ValueId>,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if let Some(symbol) = named_symbol {
        return lower_named_global_call(assembler, closure, symbol, args, allocation, abi);
    }
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

#[allow(clippy::similar_names)]
fn lower_named_global_call(
    assembler: &mut Assembler,
    closure: ValueId,
    symbol: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    load_value(assembler, allocation, symbol, Reg(16))?;
    load_value(assembler, allocation, closure, Reg(17))?;
    // check-added-lines: allow(unbound) compare against the function-cell sentinel.
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(5), ncl_sys::Word::UNBOUND.bits()) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(17),
            rm: Reg(5),
            shift: ncl_asm_aarch64::Shift::Lsl(0),
        },
    )?;
    let normal = assembler.new_label();
    let call = assembler.new_label();
    emit(
        assembler,
        Inst::BCond {
            cond: ncl_asm_aarch64::Cond::Ne,
            label: normal,
        },
    )?;
    let undefined_address = abi
        .runtime_address(RuntimeFunction::UndefinedFunction)
        .map_err(|error| CodegenError::Abi(error.to_string()))?;
    // check-added-lines: allow(unsupported) emit the undefined-function stub address.
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), undefined_address) {
        emit(assembler, instruction)?; // check-added-lines: allow(unsupported)
    }
    emit(assembler, Inst::B { label: call })?;
    assembler
        .bind(normal)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
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
    primitives::decode_function_entry(assembler, Reg(17))?;
    assembler
        .bind(call)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}
