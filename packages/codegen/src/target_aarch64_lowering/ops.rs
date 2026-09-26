use super::{
    emit, load_value, lower_alloc, lower_builtin, lower_call, lower_closure_call,
    lower_runtime_builtin, lower_safepoint, store_value,
};
use crate::{Allocation, CodegenError, ConstantName, RuntimeAbi};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{BlockParam, Compare, Function, Op, OpKind, Prim, ValueId};

fn constant_word(constant: &ncl_ir::Constant, abi: &dyn RuntimeAbi) -> Result<u64, CodegenError> {
    match constant {
        ncl_ir::Constant::Fixnum(value) => Ok(abi.encode_fixnum(*value).cast_unsigned()),
        ncl_ir::Constant::Character(value) => Ok(abi.encode_character(*value).cast_unsigned()),
        ncl_ir::Constant::Nil => Ok(abi.encode_nil().cast_unsigned()),
        ncl_ir::Constant::Unbound => Ok(abi.encode_unbound().cast_unsigned()),
        ncl_ir::Constant::T => Ok(abi.encode_true().cast_unsigned()),
        ncl_ir::Constant::FunctionEntry(function) => abi
            .constant_word_named(ConstantName::new(&format!("function-entry:{}", function.0)))
            .map(i64::cast_unsigned)
            .ok_or_else(|| {
                CodegenError::Unsupported("function entry constant is unavailable".into())
            }),
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
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
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
        store_value(assembler, allocation, result, Reg(16))?;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub fn lower_op(
    assembler: &mut Assembler,
    op: &Op,
    function: &Function,
    allocation: &Allocation,
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
                store_value(assembler, allocation, result, Reg(16))?;
            }
        }
        OpKind::Move { value } | OpKind::Convert { value, .. } => {
            load_value(assembler, allocation, *value, Reg(16))?;
            if let Some(result) = result {
                store_value(assembler, allocation, result, Reg(16))?;
            }
        }
        OpKind::Load { address }
        | OpKind::LoadField {
            object: address, ..
        } => {
            load_value(assembler, allocation, *address, Reg(16))?;
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
                store_value(assembler, allocation, result, Reg(16))?;
            }
        }
        OpKind::Store { address, value }
        | OpKind::StoreField {
            object: address,
            value,
            ..
        } => {
            load_value(assembler, allocation, *address, Reg(16))?;
            load_value(assembler, allocation, *value, Reg(17))?;
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
                load_value(assembler, allocation, ValueId(u32::from(*index)), Reg(16))?;
                store_value(assembler, allocation, result, Reg(16))?;
            }
        }
        OpKind::Prim { op, args, .. } => lower_prim(assembler, op, args, result, allocation)?,
        OpKind::Compare { op, left, right } => {
            load_value(assembler, allocation, *left, Reg(16))?;
            load_value(assembler, allocation, *right, Reg(17))?;
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
                store_value(assembler, allocation, result, Reg(16))?;
            }
        }
        OpKind::SetMultipleValues { values } => {
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(1), values.len() as u64) {
                emit(assembler, instruction)?;
            }
            if let (Some(first), Some(result)) = (values.first(), result) {
                load_value(assembler, allocation, *first, Reg(16))?;
                store_value(assembler, allocation, result, Reg(16))?;
            }
        }
        OpKind::Alloc { words } => {
            call_pc = Some(lower_alloc(assembler, *words, result, allocation, abi)?);
        }
        OpKind::Safepoint => {
            call_pc = Some(lower_safepoint(assembler, abi)?);
        }
        OpKind::Call { function, args }
        | OpKind::CallIndirect {
            callee: function,
            args,
        } => {
            lower_call(assembler, *function, args, allocation)?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_value(assembler, allocation, result, Reg(0))?;
            }
        }
        OpKind::MakeClosure { entry, captures } => {
            let values = std::iter::once(*entry)
                .chain(captures.iter().copied())
                .collect::<Vec<_>>();
            lower_runtime_builtin(assembler, "make-closure", &[], &values, allocation, abi)?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_value(assembler, allocation, result, Reg(0))?;
            }
        }
        OpKind::CallClosure { closure, args } => {
            lower_closure_call(assembler, *closure, args, allocation)?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_value(assembler, allocation, result, Reg(0))?;
            }
        }
        OpKind::Builtin { name, args } => {
            lower_builtin(assembler, name, args, allocation, abi)?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_value(assembler, allocation, result, Reg(0))?;
            }
        }
        OpKind::EnterHandler { region } => {
            let definition = function
                .handler_regions
                .iter()
                .find(|candidate| candidate.id == *region)
                .ok_or_else(|| CodegenError::Unsupported("handler region is unavailable".into()))?;
            let (name, immediate_args, value_args) = match definition.kind {
                ncl_ir::HandlerKind::Catch => (
                    "enter-catch",
                    vec![u64::from(region.0), u64::from(definition.depth)],
                    definition.catch_tag.into_iter().collect(),
                ),
                ncl_ir::HandlerKind::UnwindProtect => (
                    "enter-unwind-protect",
                    vec![
                        u64::from(region.0),
                        u64::from(
                            definition
                                .cleanup
                                .ok_or_else(|| {
                                    CodegenError::Unsupported("cleanup block is unavailable".into())
                                })?
                                .0,
                        ),
                    ],
                    Vec::new(),
                ),
                ncl_ir::HandlerKind::Progv => (
                    "enter-progv",
                    vec![u64::from(region.0)],
                    definition.binding_targets.clone(),
                ),
            };
            lower_runtime_builtin(
                assembler,
                name,
                &immediate_args,
                &value_args,
                allocation,
                abi,
            )?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
        }
        OpKind::LeaveHandler { region } => {
            let definition = function
                .handler_regions
                .iter()
                .find(|candidate| candidate.id == *region)
                .ok_or_else(|| CodegenError::Unsupported("handler region is unavailable".into()))?;
            let name = match definition.kind {
                ncl_ir::HandlerKind::Catch => "leave-catch",
                ncl_ir::HandlerKind::UnwindProtect => "leave-unwind-protect",
                ncl_ir::HandlerKind::Progv => "leave-progv",
            };
            lower_runtime_builtin(
                assembler,
                name,
                &[u64::from(region.0)],
                &[],
                allocation,
                abi,
            )?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
        }
    }
    Ok(call_pc)
}

pub fn move_args(
    assembler: &mut Assembler,
    allocation: &Allocation,
    args: &[ValueId],
    params: &[BlockParam],
) -> Result<(), CodegenError> {
    if args.len() != params.len() {
        return Err(CodegenError::Unsupported(
            "block argument arity mismatch".into(),
        ));
    }
    for (argument, parameter) in args.iter().zip(params) {
        load_value(assembler, allocation, *argument, Reg(17))?;
        store_value(assembler, allocation, parameter.value, Reg(17))?;
    }
    Ok(())
}
