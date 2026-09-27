use super::{emit, load_value, store_value};
use crate::{Allocation, CodegenError};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Function, OpKind, Prim, ValueId};

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

fn closure_captures(function: &Function, closure: ValueId) -> Option<&[ValueId]> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.ops)
        .find_map(|op| {
            op.results
                .iter()
                .any(|(value, _)| *value == closure)
                .then_some(&op.kind)
                .and_then(|kind| {
                    if let OpKind::MakeClosure { captures, .. } = kind {
                        Some(captures.as_slice())
                    } else {
                        None
                    }
                })
        })
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
    function: &Function,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let Some(captures) = closure_captures(function, closure) else {
        return super::lower_closure_call(assembler, closure, args, allocation);
    };
    let Some((argc, rest)) = args.split_first() else {
        // check-added-lines: allow(unsupported) malformed IR lacks the required argc value.
        return Err(CodegenError::Unsupported(
            "closure calls require a tagged argc argument".into(),
        ));
    };
    load_value(assembler, allocation, closure, Reg(16))?;
    load_callable_address(assembler, Reg(16), Reg(17))?;
    load_value(assembler, allocation, *argc, Reg(0))?;
    for (index, _) in captures.iter().enumerate() {
        let offset = ncl_object::function_offset::CAPTURES
            .checked_add(index)
            .and_then(|slot| slot.checked_add(1))
            .and_then(|slot| slot.checked_mul(8))
            .and_then(|offset| i16::try_from(offset).ok())
            .ok_or(CodegenError::FrameOverflow)?;
        if index < 4 {
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(17)),
                        offset,
                    },
                },
            )?;
        } else {
            if index == 4 {
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(Reg(5)),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: u16::try_from(
                            allocation
                                .outgoing_base
                                .checked_add(1)
                                .and_then(|slot| slot.checked_mul(8))
                                .ok_or(CodegenError::FrameOverflow)?,
                        )
                        .map_err(|_| CodegenError::FrameOverflow)?,
                        shift: false,
                    },
                )?;
            }
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(17)),
                        offset,
                    },
                },
            )?;
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
    for (index, argument) in rest.iter().enumerate() {
        let register_index = captures
            .len()
            .checked_add(index + 1)
            .ok_or(CodegenError::FrameOverflow)?;
        if register_index < 5 {
            let register =
                Reg(u8::try_from(register_index).map_err(|_| CodegenError::FrameOverflow)?);
            load_value(assembler, allocation, *argument, register)?;
        } else {
            if register_index == 5 {
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(Reg(5)),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: u16::try_from(
                            allocation
                                .outgoing_base
                                .checked_add(1)
                                .and_then(|slot| slot.checked_mul(8))
                                .ok_or(CodegenError::FrameOverflow)?,
                        )
                        .map_err(|_| CodegenError::FrameOverflow)?,
                        shift: false,
                    },
                )?;
            }
            load_value(assembler, allocation, *argument, Reg(16))?;
            let extra_index = index
                .saturating_sub(4usize.saturating_sub(captures.len()))
                .saturating_add(captures.len().saturating_sub(4));
            emit(
                assembler,
                Inst::Str {
                    rt: Reg(16),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(5)),
                        offset: i16::try_from(extra_index.saturating_mul(8))
                            .map_err(|_| CodegenError::FrameOverflow)?,
                    },
                },
            )?;
        }
    }
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
    decode_function_entry(assembler, Reg(17))
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
