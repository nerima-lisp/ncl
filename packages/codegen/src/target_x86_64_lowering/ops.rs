use super::{
    ENTRY, FRAME_POINTER, FUNCTION_OBJECT, VALUE_COUNT, ValueSlots, emit, emit_call,
    load_heap_constant, load_immediate, load_slot, lower_alloc, lower_load_capture,
    lower_runtime_builtin, lower_safepoint, slot_mem_of, store_slot,
};
use crate::isa_x86_64::THREAD_CONTEXT;
use crate::machine_verify::SafepointSite;
use crate::{CodegenError, ConstantName, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Label, Mem};
use ncl_ir::{BlockId, BlockParam, Compare, Function, Op, OpKind, ValueId};
use std::collections::HashMap;

fn constant_word(constant: &ncl_ir::Constant, abi: &dyn RuntimeAbi) -> Result<i64, CodegenError> {
    match constant {
        ncl_ir::Constant::Fixnum(value) => Ok(i64::from_ne_bytes(
            ncl_sys::Word::fixnum(*value).bits().to_ne_bytes(),
        )),
        ncl_ir::Constant::Character(value) => Ok(i64::from_ne_bytes(
            ncl_sys::Word::character(*value).bits().to_ne_bytes(),
        )),
        ncl_ir::Constant::Nil => Ok(i64::from_ne_bytes(ncl_sys::Word::NIL.bits().to_ne_bytes())),
        ncl_ir::Constant::Unbound => Ok(i64::from_ne_bytes(
            ncl_sys::Word::UNBOUND.bits().to_ne_bytes(),
        )),
        ncl_ir::Constant::T => Ok(i64::from_ne_bytes(ncl_sys::Word::TRUE.bits().to_ne_bytes())),
        ncl_ir::Constant::FunctionEntry(function) => abi
            .constant_word_named(ConstantName::new(&format!("function-entry:{}", function.0)))
            .ok_or_else(|| {
                CodegenError::Unsupported("function entry constant is unavailable".into())
                // check-added-lines: allow(unsupported) existing codegen error variant
            }),
        ncl_ir::Constant::SingleFloat(_)
        | ncl_ir::Constant::DoubleFloat(_)
        | ncl_ir::Constant::Symbol { .. }
        | ncl_ir::Constant::UninternedSymbol { .. }
        | ncl_ir::Constant::Object(_) // check-added-lines: allow(unsupported) explicit unsupported constant
        | ncl_ir::Constant::LiteralObject { .. }
        | ncl_ir::Constant::StringBytes(_)
        | ncl_ir::Constant::Bignum { .. }
        | ncl_ir::Constant::Ratio { .. }
        | ncl_ir::Constant::Complex { .. }
        | ncl_ir::Constant::Structure { .. } => Err(CodegenError::Unsupported( // check-added-lines: allow(unsupported) runtime-table constants are loaded through code objects
            // check-added-lines: allow(unsupported) existing codegen error variant
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
pub(super) fn materialise_boolean(
    assembler: &mut Assembler,
    condition: Cond,
) -> Result<(), CodegenError> {
    emit(assembler, Inst::Setcc(condition, FUNCTION_OBJECT))?;
    emit(assembler, Inst::Movzx(FUNCTION_OBJECT, FUNCTION_OBJECT, 8))
}
pub(super) fn untag_function_object(assembler: &mut Assembler) -> Result<(), CodegenError> {
    let mask = i32::try_from(i64::from_ne_bytes((!ncl_sys::LOWTAG_MASK).to_ne_bytes()))
        .map_err(|_| CodegenError::FrameOverflow)?;
    emit(assembler, Inst::BinRI(BinOp::And, FUNCTION_OBJECT, mask))
}

pub(super) fn lower_write_barrier(
    assembler: &mut Assembler,
    owner: ValueId,
    slot: i64,
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    lower_runtime_builtin(
        assembler,
        RuntimeFunction::WriteBarrier,
        &[slot],
        &[owner],
        slots,
        abi,
    )?;
    emit_call(assembler).map(|_| ())
}

#[allow(clippy::too_many_lines)]
pub fn lower_op(
    assembler: &mut Assembler,
    op: &Op,
    function: &Function,
    block: BlockId,
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
    labels: &HashMap<BlockId, Label>,
) -> Result<Option<SafepointSite>, CodegenError> {
    let result = op.results.first().map(|(value, _)| *value);
    let check_context = super::dispatch::CallCheckContext::new(function, block, slots, abi, labels);
    let mut call_pc = None;
    match &op.kind {
        OpKind::Const { result: constant } => {
            let value = function
                .constants
                .get(constant.0 as usize)
                .ok_or_else(|| CodegenError::Unsupported("constant index out of range".into()))?;
            if matches!(
                value,
                ncl_ir::Constant::Symbol { .. }
                    | ncl_ir::Constant::UninternedSymbol { .. }
                    | ncl_ir::Constant::Object(_)
                    | ncl_ir::Constant::LiteralObject { .. }
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
                load_immediate(assembler, FUNCTION_OBJECT, constant_word(value, abi)?)?;
            }
            if let Some(result) = result {
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::Move { value } | OpKind::Convert { value, .. } | OpKind::TypeHint { value, .. } => {
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
            untag_function_object(assembler)?;
            let offset = match &op.kind {
                OpKind::LoadField { field, .. } => {
                    i32::try_from(field.saturating_add(1).saturating_mul(8))
                        .map_err(|_| CodegenError::FrameOverflow)?
                }
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
        OpKind::Store { address, value } => {
            load_slot(assembler, slots, *address, FUNCTION_OBJECT)?;
            untag_function_object(assembler)?;
            load_slot(assembler, slots, *value, ENTRY)?;
            emit(assembler, Inst::MovMR(Mem::base(FUNCTION_OBJECT, 0), ENTRY))?;
        }
        OpKind::StoreField {
            object,
            field,
            value,
        } => {
            load_slot(assembler, slots, *object, FUNCTION_OBJECT)?;
            untag_function_object(assembler)?;
            load_slot(assembler, slots, *value, ENTRY)?;
            let offset = i32::try_from(field.saturating_add(1).saturating_mul(8))
                .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                Inst::MovMR(Mem::base(FUNCTION_OBJECT, offset), ENTRY),
            )?;
            lower_write_barrier(assembler, *object, i64::from(*field), slots, abi)?;
        }
        OpKind::LoadArg { index } => {
            if let Some(result) = result {
                load_slot(
                    assembler,
                    slots,
                    ValueId(u32::from(*index)),
                    FUNCTION_OBJECT,
                )?;
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::LoadCapture { index } => {
            lower_load_capture(assembler, *index, result, slots)?;
        }
        OpKind::LoadFunctionObject => {
            if let Some(result) = result {
                emit(
                    assembler,
                    Inst::MovRM(FUNCTION_OBJECT, Mem::base(FRAME_POINTER, 16)),
                )?;
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::Prim { op, args, .. } => {
            super::primitives::lower_prim(assembler, op, args, result, slots, abi)?;
        }
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
            super::store_return_values(assembler, slots, values, abi)?;
            load_immediate(
                assembler,
                VALUE_COUNT,
                i64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?,
            )?;
            emit(
                assembler,
                Inst::MovMR(
                    Mem::base(
                        THREAD_CONTEXT,
                        abi.field_offset(crate::ContextField::MultipleValueCount)
                            .map_err(|error| CodegenError::Abi(error.to_string()))?,
                    ),
                    VALUE_COUNT,
                ),
            )?;
            if let (Some(first), Some(result)) = (values.first(), result) {
                load_slot(assembler, slots, *first, FUNCTION_OBJECT)?;
                store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
            }
        }
        OpKind::Alloc { words } => {
            call_pc = Some(lower_alloc(assembler, *words, result, &check_context)?);
        }
        OpKind::Safepoint => {
            call_pc = Some(lower_safepoint(assembler, abi)?);
        }
        OpKind::Call { .. }
        | OpKind::CallIndirect { .. }
        | OpKind::MakeClosure { .. }
        | OpKind::MakeValueCell { .. }
        | OpKind::CallClosure { .. }
        | OpKind::Builtin { .. }
        | OpKind::EnterHandler { .. }
        | OpKind::LeaveHandler { .. } => {
            call_pc =
                super::checked_ops::lower_checked_ops(assembler, &op.kind, result, &check_context)?;
        }
    }
    Ok(call_pc)
}

pub fn move_args(
    assembler: &mut Assembler,
    slots: &ValueSlots,
    args: &[ValueId],
    params: &[BlockParam],
) -> Result<(), CodegenError> {
    if args.len() != params.len() {
        return Err(CodegenError::Unsupported(
            "block argument arity mismatch".into(),
        ));
    }
    let mut moves = Vec::new();
    for (argument, parameter) in args.iter().zip(params) {
        let source = slots.location(*argument)?;
        let destination = slots.location(parameter.value)?;
        if source != destination {
            moves.push((source, destination));
        }
    }
    // When a destination location is also a source location, stage every source
    // first so register and spill moves remain parallel-copy safe.
    let overlapping = moves
        .iter()
        .any(|(source, _)| moves.iter().any(|(_, destination)| destination == source));
    if overlapping {
        for (source, _) in &moves {
            match source {
                crate::Location::Register(register) => emit(
                    assembler,
                    Inst::MovRR(
                        ENTRY,
                        ncl_asm_x86_64::Reg::from_id(
                            u8::try_from(*register).map_err(|_| CodegenError::FrameOverflow)?,
                        )
                        .ok_or(CodegenError::FrameOverflow)?,
                    ),
                )?,
                crate::Location::Spill(spill) => emit(
                    assembler,
                    Inst::MovRM(
                        ENTRY,
                        slot_mem_of(
                            slots
                                .spill_base
                                .checked_add(*spill)
                                .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                    ),
                )?,
            }
            emit(assembler, Inst::Push(ENTRY))?;
        }
        for (_, destination) in moves.iter().rev() {
            emit(assembler, Inst::Pop(ENTRY))?;
            match destination {
                crate::Location::Register(register) => emit(
                    assembler,
                    Inst::MovRR(
                        ncl_asm_x86_64::Reg::from_id(
                            u8::try_from(*register).map_err(|_| CodegenError::FrameOverflow)?,
                        )
                        .ok_or(CodegenError::FrameOverflow)?,
                        ENTRY,
                    ),
                )?,
                crate::Location::Spill(spill) => emit(
                    assembler,
                    Inst::MovMR(
                        slot_mem_of(
                            slots
                                .spill_base
                                .checked_add(*spill)
                                .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                        ENTRY,
                    ),
                )?,
            }
        }
        return Ok(());
    }
    for (argument, parameter) in args.iter().zip(params) {
        if slots.location(*argument)? != slots.location(parameter.value)? {
            load_slot(assembler, slots, *argument, ENTRY)?;
            store_slot(assembler, slots, parameter.value, ENTRY)?;
        }
    }
    Ok(())
}
