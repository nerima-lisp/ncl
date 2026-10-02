use super::{emit, load_value, store_value};
use crate::{Allocation, CodegenError};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Prim, ValueId};

pub(super) fn load_callable_address(
    assembler: &mut Assembler,
    source: Reg,
    destination: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(destination),
            rn: RegOrSp::Reg(source),
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: destination,
            rn: destination,
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )
}

pub(super) fn decode_function_entry(
    assembler: &mut Assembler,
    register: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::AsrImm {
            rd: register,
            rn: register,
            amount: u8::try_from(ncl_sys::FIXNUM_TAG_BITS)
                .map_err(|_| CodegenError::FrameOverflow)?,
        },
    )
}

/// Loads a lexical capture from the closure object retained in the generated
/// frame.
pub(super) fn lower_load_capture(
    assembler: &mut Assembler,
    index: u32,
    result: Option<ValueId>,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let offset = ncl_object::function_offset::CAPTURES
        .checked_add(usize::try_from(index).map_err(|_| CodegenError::FrameOverflow)?)
        .and_then(|slot| slot.checked_add(1))
        .and_then(|slot| slot.checked_mul(8))
        .and_then(|offset| i16::try_from(offset).ok())
        .ok_or(CodegenError::FrameOverflow)?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(29)),
                offset: 16,
            },
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(16),
            rn: Reg(16),
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(16)),
                offset,
            },
        },
    )?;
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    Ok(())
}

fn emit_lisp_boolean(assembler: &mut Assembler, condition: Cond) -> Result<(), CodegenError> {
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), ncl_sys::Word::TRUE.bits()) {
        emit(assembler, instruction)?;
    }
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), ncl_sys::Word::NIL.bits()) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Csel {
            rd: Reg(16),
            rn: Reg(16),
            rm: Reg(17),
            cond: condition,
        },
    )
}

// `argc`/`args` mirror the calling convention's own argument-count/argument-
// list naming; that pairing is clearer here than any alternative spelling.
#[allow(clippy::similar_names)]
#[allow(clippy::too_many_lines)]
pub(super) fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
    named_symbol: Option<ValueId>,
    abi: &dyn crate::RuntimeAbi,
) -> Result<(), CodegenError> {
    super::lower_closure_call(assembler, closure, args, allocation, named_symbol, abi)
}

#[allow(clippy::too_many_lines)]
pub(super) fn lower_prim(
    assembler: &mut Assembler,
    prim: &Prim,
    args: &[ValueId],
    result: Option<ValueId>,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "primitive has no operands".into(),
        ));
    };
    load_value(assembler, allocation, *first, Reg(16))?;
    if let Some(second) = args.get(1) {
        load_value(assembler, allocation, *second, Reg(17))?;
    }
    match prim {
        Prim::FixnumAdd => emit(
            assembler,
            Inst::Add {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        )?,
        Prim::FixnumSub => emit(
            assembler,
            Inst::Sub {
                rd: Reg(16).into(),
                rn: Reg(16).into(),
                rm: Reg(17),
                shift: Shift::Lsl(0),
            },
        )?,
        Prim::FixnumMul => emit(
            assembler,
            Inst::Mul {
                rd: Reg(16),
                rn: Reg(16),
                rm: Reg(17),
            },
        )?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit_lisp_boolean(assembler, Cond::Eq)?;
        }
        Prim::FixnumLt => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit_lisp_boolean(assembler, Cond::Lt)?;
        }
        Prim::FixnumLe => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            emit_lisp_boolean(assembler, Cond::Le)?;
        }
        Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => {
            let offset = if matches!(prim, Prim::Cdr) { 8 } else { 0 };
            emit(
                assembler,
                Inst::AndImm {
                    rd: Reg(16),
                    rn: Reg(16),
                    imm: !ncl_sys::LOWTAG_MASK,
                },
            )?;
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        Prim::Rplaca | Prim::Rplacd | Prim::Aset => {
            let offset = if matches!(prim, Prim::Rplacd) { 8 } else { 0 };
            emit(
                assembler,
                Inst::AndImm {
                    rd: Reg(16),
                    rn: Reg(16),
                    imm: !ncl_sys::LOWTAG_MASK,
                },
            )?;
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(17),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => {
            // check-added-lines: allow(unsupported) existing codegen error variant
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests_aarch64_lowering_primitives.rs"]
mod tests;
