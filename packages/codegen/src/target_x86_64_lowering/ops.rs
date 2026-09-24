use super::{
    ENTRY, FRAME_POINTER, FUNCTION_OBJECT, RETURN_VALUE, VALUE_COUNT, emit, emit_call,
    load_immediate, load_slot, lower_alloc, lower_builtin, lower_call, lower_safepoint, store_slot,
};
use crate::{CodegenError, RuntimeAbi};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Mem};
use ncl_ir::{BlockParam, Compare, Function, Op, OpKind, Prim, ValueId};

fn constant_word(constant: &ncl_ir::Constant, abi: &dyn RuntimeAbi) -> Result<i64, CodegenError> {
    match constant {
        ncl_ir::Constant::Fixnum(value) => Ok(abi.encode_fixnum(*value)),
        ncl_ir::Constant::Character(value) => Ok(abi.encode_character(*value)),
        ncl_ir::Constant::Nil | ncl_ir::Constant::Unbound => Ok(0),
        ncl_ir::Constant::T => Ok(abi.encode_fixnum(1)),
        _ => Err(CodegenError::Unsupported(
            "constant requires a runtime table".into(),
        )),
    }
}

const fn compare_condition(op: Compare) -> Cond {
    match op {
        Compare::Eq => Cond::E,
        Compare::Ne => Cond::Ne,
        Compare::Lt => Cond::L,
        Compare::Le => Cond::Le,
        Compare::Gt => Cond::G,
        Compare::Ge => Cond::Ge,
    }
}

/// Materialises a boolean byte into a full word, since `setcc` leaves the upper bits stale.
fn materialise_boolean(assembler: &mut Assembler, condition: Cond) -> Result<(), CodegenError> {
    emit(assembler, Inst::Setcc(condition, FUNCTION_OBJECT))?;
    emit(assembler, Inst::Movzx(FUNCTION_OBJECT, FUNCTION_OBJECT, 8))
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
    load_slot(assembler, slots, *first, FUNCTION_OBJECT)?;
    if let Some(second) = args.get(1) {
        load_slot(assembler, slots, *second, ENTRY)?;
    }
    match prim {
        Prim::FixnumAdd => emit(assembler, Inst::BinRR(BinOp::Add, FUNCTION_OBJECT, ENTRY))?,
        Prim::FixnumSub => emit(assembler, Inst::BinRR(BinOp::Sub, FUNCTION_OBJECT, ENTRY))?,
        Prim::FixnumMul => emit(assembler, Inst::ImulRR(FUNCTION_OBJECT, ENTRY))?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => {
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            materialise_boolean(assembler, Cond::E)?;
        }
        Prim::FixnumLt => {
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            materialise_boolean(assembler, Cond::L)?;
        }
        Prim::FixnumLe => {
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            materialise_boolean(assembler, Cond::Le)?;
        }
        Prim::Car | Prim::Cdr | Prim::Svref | Prim::Aref => {
            let offset = if matches!(prim, Prim::Cdr) { 8 } else { 0 };
            emit(
                assembler,
                Inst::MovRM(FUNCTION_OBJECT, Mem::base(FUNCTION_OBJECT, offset)),
            )?;
        }
        Prim::Rplaca | Prim::Rplacd | Prim::Aset => {
            let offset = if matches!(prim, Prim::Rplacd) { 8 } else { 0 };
            emit(
                assembler,
                Inst::MovMR(Mem::base(FUNCTION_OBJECT, offset), ENTRY),
            )?;
        }
        _ => {
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
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
            load_immediate(assembler, FUNCTION_OBJECT, constant_word(value, abi)?)?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::Move { value } | OpKind::Convert { value, .. } => {
            load_slot(assembler, slots, *value, FUNCTION_OBJECT)?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::Load { address }
        | OpKind::LoadField {
            object: address, ..
        } => {
            load_slot(assembler, slots, *address, FUNCTION_OBJECT)?;
            let offset = match &op.kind {
                OpKind::LoadField { field, .. } => i32::try_from(field.saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?,
                _ => 0,
            };
            emit(
                assembler,
                Inst::MovRM(FUNCTION_OBJECT, Mem::base(FUNCTION_OBJECT, offset)),
            )?;
            if let Some(result) = result {
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::Store { address, value }
        | OpKind::StoreField {
            object: address,
            value,
            ..
        } => {
            load_slot(assembler, slots, *address, FUNCTION_OBJECT)?;
            load_slot(assembler, slots, *value, ENTRY)?;
            let offset = match &op.kind {
                OpKind::StoreField { field, .. } => i32::try_from(field.saturating_mul(8))
                    .map_err(|_| CodegenError::FrameOverflow)?,
                _ => 0,
            };
            emit(
                assembler,
                Inst::MovMR(Mem::base(FUNCTION_OBJECT, offset), ENTRY),
            )?;
        }
        OpKind::LoadArg { index } => {
            if let Some(result) = result {
                let offset = i32::from(*index + 1)
                    .checked_mul(-8)
                    .ok_or(CodegenError::FrameOverflow)?;
                emit(
                    assembler,
                    Inst::MovRM(FUNCTION_OBJECT, Mem::base(FRAME_POINTER, offset)),
                )?;
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::Prim { op, args, .. } => lower_prim(assembler, op, args, result, slots)?,
        OpKind::Compare { op, left, right } => {
            load_slot(assembler, slots, *left, FUNCTION_OBJECT)?;
            load_slot(assembler, slots, *right, ENTRY)?;
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            if let Some(result) = result {
                materialise_boolean(assembler, compare_condition(*op))?;
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::SetMultipleValues { values } => {
            load_immediate(
                assembler,
                VALUE_COUNT,
                i64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?,
            )?;
            if let (Some(first), Some(result)) = (values.first(), result) {
                load_slot(assembler, slots, *first, FUNCTION_OBJECT)?;
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
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
            call_pc = Some(emit_call(assembler)?);
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
            }
        }
        OpKind::Builtin { name, args } => {
            lower_builtin(assembler, name, args, slots, abi)?;
            call_pc = Some(emit_call(assembler)?);
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
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
        load_slot(assembler, slots, *argument, ENTRY)?;
        store_slot(assembler, slots, parameter.value, ENTRY)?;
    }
    Ok(())
}
