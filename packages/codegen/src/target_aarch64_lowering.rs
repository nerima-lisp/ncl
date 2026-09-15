use crate::{CodegenError, RuntimeAbi};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{BlockParam, Compare, Function, Op, OpKind, Prim, ValueId};

#[allow(clippy::needless_pass_by_value)]
fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
    assembler
        .emit(&instruction)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

pub(super) fn slots(function: &Function) -> (Vec<(ValueId, u32)>, u32) {
    let mut result = Vec::new();
    let mut next = 0_u32;
    for block in &function.blocks {
        for parameter in &block.params {
            result.push((parameter.value, next));
            next = next.saturating_add(1);
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                result.push((*value, next));
                next = next.saturating_add(1);
            }
        }
    }
    (result, next)
}

fn slot(slots: &[(ValueId, u32)], value: ValueId) -> Result<u32, CodegenError> {
    slots
        .iter()
        .find(|(id, _)| *id == value)
        .map(|(_, index)| *index)
        .ok_or(CodegenError::UnknownValue(value))
}

fn slot_mem(slots: &[(ValueId, u32)], value: ValueId) -> Result<MemOperand, CodegenError> {
    let index = slot(slots, value)?;
    let offset = i16::try_from((index.saturating_add(1)).saturating_mul(8))
        .map_err(|_| CodegenError::FrameOverflow)?;
    Ok(MemOperand::Unscaled {
        base: RegOrSp::Reg(Reg(29)),
        offset: -offset,
    })
}

pub(super) fn load_slot(
    assembler: &mut Assembler,
    slots: &[(ValueId, u32)],
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::Ldr {
            rt: register,
            mem: slot_mem(slots, value)?,
        },
    )
}

fn store_slot(
    assembler: &mut Assembler,
    slots: &[(ValueId, u32)],
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::Str {
            rt: register,
            mem: slot_mem(slots, value)?,
        },
    )
}

pub(super) fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    slots: &[(ValueId, u32)],
) -> Result<(), CodegenError> {
    if args.len() > 4 {
        return Err(CodegenError::Unsupported(
            "AArch64 calls support at most four register arguments".into(),
        ));
    }
    load_slot(assembler, slots, callee, Reg(16))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(16)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(0), args.len() as u64) {
        emit(assembler, instruction)?;
    }
    for (index, argument) in args.iter().enumerate() {
        let register = Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?);
        load_slot(assembler, slots, *argument, register)?;
    }
    Ok(())
}

fn lower_builtin(
    assembler: &mut Assembler,
    name: &str,
    args: &[ValueId],
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if args.len() > 4 {
        return Err(CodegenError::Unsupported(
            "AArch64 builtins support at most four arguments".into(),
        ));
    }
    let address = abi.builtin_address(name).ok_or_else(|| {
        CodegenError::Unsupported(format!("builtin address is unavailable: {name}"))
    })?;
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
    for (index, argument) in args.iter().enumerate() {
        let register = Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?);
        load_slot(assembler, slots, *argument, register)?;
    }
    Ok(())
}

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
pub(super) fn lower_op(
    assembler: &mut Assembler,
    op: &Op,
    function: &Function,
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let result = op.results.first().map(|(value, _)| *value);
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
                if *index < 4 {
                    emit(
                        assembler,
                        Inst::Mov {
                            rd: RegOrSp::Reg(Reg(16)),
                            rn: RegOrSp::Reg(Reg(1 + *index)),
                        },
                    )?;
                } else {
                    let offset = i16::from(*index - 4)
                        .checked_mul(8)
                        .ok_or(CodegenError::FrameOverflow)?;
                    emit(
                        assembler,
                        Inst::Ldr {
                            rt: Reg(16),
                            mem: MemOperand::Unscaled {
                                base: RegOrSp::Reg(Reg(5)),
                                offset,
                            },
                        },
                    )?;
                }
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
        OpKind::Alloc { .. } | OpKind::Safepoint => emit(assembler, Inst::Nop)?,
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
    Ok(())
}

pub(super) fn move_args(
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
