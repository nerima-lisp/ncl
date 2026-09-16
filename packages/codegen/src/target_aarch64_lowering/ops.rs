use super::{emit, load_slot, lower_alloc, lower_builtin, lower_call, lower_safepoint, store_slot};
use crate::{CodegenError, RuntimeAbi};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{BlockParam, Compare, Function, Op, OpKind, Prim, ValueId};

fn constant_word(constant: &ncl_ir::Constant, abi: &dyn RuntimeAbi) -> Result<u64, CodegenError> {
    match constant {
        ncl_ir::Constant::Fixnum(value) => Ok(abi.encode_fixnum(*value).cast_unsigned()),
        ncl_ir::Constant::Character(value) => Ok(abi.encode_character(*value).cast_unsigned()),
        ncl_ir::Constant::Nil | ncl_ir::Constant::Unbound => Ok(0),
        ncl_ir::Constant::T => Ok(abi.encode_fixnum(1).cast_unsigned()),
        _ => Err(CodegenError::Unsupported(
            "constant requires a runtime table".into(),
        )),
    }
}

const fn compare_condition(op: Compare) -> Cond {
    match op {
        Compare::Eq => Cond::Eq,
        Compare::Ne => Cond::Ne,
        Compare::Lt => Cond::Lt,
        Compare::Le => Cond::Le,
        Compare::Gt => Cond::Gt,
        Compare::Ge => Cond::Ge,
    }
}

#[allow(clippy::too_many_lines)]
fn lower_prim(
    assembler: &mut Assembler,
    prim: &Prim,
    args: &[ValueId],
    result: Option<ValueId>,
    slots: &[(ValueId, u32)],
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
        return Err(CodegenError::Unsupported(
            "primitive has no operands".into(),
        ));
    };
    load_slot(assembler, slots, *first, Reg(16))?;
    if let Some(second) = args.get(1) {
        load_slot(assembler, slots, *second, Reg(17))?;
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
            emit(
                assembler,
                Inst::Cset {
                    rd: Reg(16),
                    cond: Cond::Eq,
                },
            )?;
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
            emit(
                assembler,
                Inst::Cset {
                    rd: Reg(16),
                    cond: Cond::Lt,
                },
            )?;
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
            emit(
                assembler,
                Inst::Cset {
                    rd: Reg(16),
                    cond: Cond::Le,
                },
            )?;
        }
        Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => {
            let offset = if matches!(prim, Prim::Cdr) { 8 } else { 0 };
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
                Inst::Str {
                    rt: Reg(17),
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset,
                    },
                },
            )?;
        }
        _ => {
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store_slot(assembler, slots, result, Reg(16))?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub fn lower_op(
    assembler: &mut Assembler,
    op: &Op,
    function: &Function,
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<Option<u32>, CodegenError> {
    let result = op.results.first().map(|(value, _)| *value);
    let mut call_pc = None;
    match &op.kind {
        OpKind::Const { result: constant } => {
            let value = function
                .constants
                .get(constant.0 as usize)
                .ok_or_else(|| CodegenError::Unsupported("constant index out of range".into()))?;
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), constant_word(value, abi)?) {
                emit(assembler, instruction)?;
            }
            if let Some(result) = result {
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Move { value } | OpKind::Convert { value, .. } => {
            load_slot(assembler, slots, *value, Reg(16))?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Load { address }
        | OpKind::LoadField {
            object: address, ..
        } => {
            load_slot(assembler, slots, *address, Reg(16))?;
            let offset = match &op.kind {
                OpKind::LoadField { field, .. } => i16::try_from(field.saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?,
                _ => 0,
            };
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
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Store { address, value }
        | OpKind::StoreField {
            object: address,
            value,
            ..
        } => {
            load_slot(assembler, slots, *address, Reg(16))?;
            load_slot(assembler, slots, *value, Reg(17))?;
            let offset = match &op.kind {
                OpKind::StoreField { field, .. } => i16::try_from(field.saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?,
                _ => 0,
            };
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
        OpKind::LoadArg { index } => {
            if let Some(result) = result {
                let offset = i16::from(*index + 1)
                    .checked_mul(-8)
                    .ok_or(CodegenError::FrameOverflow)?;
                emit(
                    assembler,
                    Inst::Ldr {
                        rt: Reg(16),
                        mem: MemOperand::Unscaled {
                            base: RegOrSp::Reg(Reg(29)),
                            offset,
                        },
                    },
                )?;
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Prim { op, args, .. } => lower_prim(assembler, op, args, result, slots)?,
        OpKind::Compare { op, left, right } => {
            load_slot(assembler, slots, *left, Reg(16))?;
            load_slot(assembler, slots, *right, Reg(17))?;
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            if let Some(result) = result {
                emit(
                    assembler,
                    Inst::Cset {
                        rd: Reg(16),
                        cond: compare_condition(*op),
                    },
                )?;
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::SetMultipleValues { values } => {
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(1), values.len() as u64) {
                emit(assembler, instruction)?;
            }
            if let (Some(first), Some(result)) = (values.first(), result) {
                load_slot(assembler, slots, *first, Reg(16))?;
                store_slot(assembler, slots, result, Reg(16))?;
            }
        }
        OpKind::Alloc { words } => {
            call_pc = Some(lower_alloc(assembler, *words, result, slots, abi)?);
        }
        OpKind::Safepoint => {
            call_pc = Some(lower_safepoint(assembler, abi)?);
        }
        OpKind::Call { function, args }
        | OpKind::CallIndirect {
            callee: function,
            args,
        } => {
            lower_call(assembler, *function, args, slots)?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, Reg(0))?;
            }
        }
        OpKind::Builtin { name, args } => {
            lower_builtin(assembler, name, args, slots, abi)?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, Reg(0))?;
            }
        }
    }
    Ok(call_pc)
}

pub fn move_args(
    assembler: &mut Assembler,
    slots: &[(ValueId, u32)],
    args: &[ValueId],
    params: &[BlockParam],
) -> Result<(), CodegenError> {
    if args.len() != params.len() {
        return Err(CodegenError::Unsupported(
            "block argument arity mismatch".into(),
        ));
    }
    for (argument, parameter) in args.iter().zip(params) {
        load_slot(assembler, slots, *argument, Reg(17))?;
        store_slot(assembler, slots, parameter.value, Reg(17))?;
    }
    Ok(())
}
