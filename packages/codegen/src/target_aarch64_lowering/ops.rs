use super::{
    constant_table_entry, emit, load_value, lower_alloc, lower_builtin, lower_call,
    lower_runtime_builtin, lower_safepoint, primitives, store_value,
};
use crate::{Allocation, CodegenError, ConstantName, RuntimeAbi, RuntimeFunction};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Compare, Function, Op, OpKind, ValueId};

fn constant_word(constant: &ncl_ir::Constant, abi: &dyn RuntimeAbi) -> Result<u64, CodegenError> {
    match constant {
        ncl_ir::Constant::Fixnum(value) => Ok(ncl_sys::Word::fixnum(*value).bits()),
        ncl_ir::Constant::Character(value) => Ok(ncl_sys::Word::character(*value).bits()),
        ncl_ir::Constant::Nil => Ok(ncl_sys::Word::NIL.bits()),
        ncl_ir::Constant::Unbound => Ok(ncl_sys::Word::UNBOUND.bits()),
        ncl_ir::Constant::T => Ok(ncl_sys::Word::TRUE.bits()),
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

fn load_heap_constant(
    assembler: &mut Assembler,
    index: ncl_ir::ConstantIndex,
) -> Result<(), CodegenError> {
    let element_offset = ncl_object::simple_vector_offset::DATA
        .checked_add(1)
        .and_then(|base| base.checked_add(usize::try_from(index.0).ok()?))
        .and_then(|value| value.checked_mul(8))
        .and_then(|value| u16::try_from(value).ok())
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
                offset: i16::try_from((ncl_object::function_offset::CODE + 1) * 8)
                    .map_err(|_| CodegenError::FrameOverflow)?,
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
                offset: i16::try_from((ncl_object::code_offset::CONSTANTS + 1) * 8)
                    .map_err(|_| CodegenError::FrameOverflow)?,
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
            mem: MemOperand::Unsigned {
                base: RegOrSp::Reg(Reg(16)),
                offset: element_offset,
                scale: 8,
            },
        },
    )
}

fn store_closure_capture(
    assembler: &mut Assembler,
    closure: ValueId,
    index: usize,
    capture: ValueId,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let offset = ncl_object::function_offset::CAPTURES
        .checked_add(index)
        .and_then(|slot| slot.checked_add(1))
        .and_then(|slot| slot.checked_mul(8))
        .and_then(|offset| i16::try_from(offset).ok())
        .ok_or(CodegenError::FrameOverflow)?;
    load_value(assembler, allocation, closure, Reg(16))?;
    load_value(assembler, allocation, capture, Reg(17))?;
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
    )
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
            let value = constant_table_entry(&function.constants, *constant)?;
            if matches!(
                value,
                ncl_ir::Constant::Symbol { .. }
                    | ncl_ir::Constant::Object(_)
                    | ncl_ir::Constant::StringBytes(_)
                    | ncl_ir::Constant::Structure { .. }
                    | ncl_ir::Constant::Bignum { .. }
                    | ncl_ir::Constant::Ratio { .. }
                    | ncl_ir::Constant::Complex { .. }
                    | ncl_ir::Constant::SingleFloat(_)
                    | ncl_ir::Constant::DoubleFloat(_)
                    | ncl_ir::Constant::FunctionEntry(_)
            ) {
                load_heap_constant(assembler, *constant)?;
            } else {
                for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), constant_word(value, abi)?) {
                    emit(assembler, instruction)?;
                }
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
            emit(
                assembler,
                Inst::AndImm {
                    rd: Reg(16),
                    rn: Reg(16),
                    imm: !ncl_sys::LOWTAG_MASK,
                },
            )?;
            let offset = match &op.kind {
                OpKind::LoadField { field, .. } => {
                    i16::try_from(field.saturating_add(1).saturating_mul(8))
                        .map_err(|_| CodegenError::FrameOverflow)?
                }
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
            emit(
                assembler,
                Inst::AndImm {
                    rd: Reg(16),
                    rn: Reg(16),
                    imm: !ncl_sys::LOWTAG_MASK,
                },
            )?;
            let offset = match &op.kind {
                OpKind::StoreField { field, .. } => {
                    i16::try_from(field.saturating_add(1).saturating_mul(8))
                        .map_err(|_| CodegenError::FrameOverflow)?
                }
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
        OpKind::LoadCapture { index } => {
            primitives::lower_load_capture(assembler, u32::from(*index), result, allocation)?;
        }
        OpKind::LoadFunctionObject => {
            if let Some(result) = result {
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
                store_value(assembler, allocation, result, Reg(16))?;
            }
        }
        OpKind::Prim { op, args, .. } => {
            primitives::lower_prim(assembler, op, args, result, allocation)?;
        }
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
            super::dispatch::lower_set_multiple_values(assembler, values, result, allocation, abi)?;
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
            lower_runtime_builtin(
                assembler,
                RuntimeFunction::MakeClosure,
                &[ncl_sys::Word::fixnum(
                    i64::try_from(captures.len()).map_err(|_| CodegenError::FrameOverflow)?,
                )
                .bits()],
                &[*entry],
                allocation,
                abi,
            )?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_value(assembler, allocation, result, Reg(0))?;
                // No allocation or safepoint occurs before these stores, so the new object cannot move.
                for (index, capture) in captures.iter().copied().enumerate() {
                    store_closure_capture(assembler, result, index, capture, allocation)?;
                }
            }
        }
        OpKind::MakeValueCell { value } => {
            lower_runtime_builtin(
                assembler,
                RuntimeFunction::MakeValueCell,
                &[],
                &[*value],
                allocation,
                abi,
            )?;
            emit(assembler, Inst::Blr { rn: Reg(17) })?;
            if let Some(result) = result {
                store_value(assembler, allocation, result, Reg(0))?;
            }
        }
        OpKind::CallClosure {
            closure,
            args,
            named_symbol,
        } => {
            primitives::lower_closure_call(
                assembler,
                *closure,
                args,
                allocation,
                *named_symbol,
                abi,
            )?;
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
            let (function, immediate_args, value_args) = match definition.kind {
                ncl_ir::HandlerKind::Catch => (
                    RuntimeFunction::EnterCatch,
                    vec![u64::from(region.0), u64::from(definition.depth)],
                    definition.catch_tag.into_iter().collect(),
                ),
                ncl_ir::HandlerKind::UnwindProtect => (
                    RuntimeFunction::EnterUnwindProtect,
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
                    RuntimeFunction::EnterProgv,
                    vec![u64::from(region.0)],
                    definition.binding_targets.clone(),
                ),
            };
            lower_runtime_builtin(
                assembler,
                function,
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
            let function = match definition.kind {
                ncl_ir::HandlerKind::Catch => RuntimeFunction::LeaveCatch,
                ncl_ir::HandlerKind::UnwindProtect => RuntimeFunction::LeaveUnwindProtect,
                ncl_ir::HandlerKind::Progv => RuntimeFunction::LeaveProgv,
            };
            lower_runtime_builtin(
                assembler,
                function,
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
