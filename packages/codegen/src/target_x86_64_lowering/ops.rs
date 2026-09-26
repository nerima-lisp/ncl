use super::{
    ENTRY, FRAME_POINTER, FUNCTION_OBJECT, RETURN_VALUE, VALUE_COUNT, ValueSlots, emit, emit_call,
    load_immediate, load_slot, lower_alloc, lower_builtin, lower_call, lower_closure_call,
    lower_runtime_builtin, lower_safepoint, slot_mem_of, store_slot,
};
use crate::{CodegenError, ConstantName, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Mem};
use ncl_ir::{BlockParam, Compare, Function, Op, OpKind, Prim, ValueId};

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
        | ncl_ir::Constant::Object(_)
        | ncl_ir::Constant::StringBytes(_) => Err(CodegenError::Unsupported(
            // check-added-lines: allow(unsupported) existing codegen error variant
            "constant requires a runtime table".into(),
        )),
    }
}

fn load_heap_constant(
    assembler: &mut Assembler,
    index: ncl_ir::ConstantIndex,
) -> Result<(), CodegenError> {
    let function_code_offset = i32::try_from(
        (ncl_object::function_offset::CODE + 1)
            .checked_mul(8)
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    let code_constants_offset = i32::try_from(
        (ncl_object::code_offset::CONSTANTS + 1)
            .checked_mul(8)
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    let vector_element_offset = i32::try_from(
        (ncl_object::simple_vector_offset::DATA + 1)
            .checked_add(usize::try_from(index.0).map_err(|_| CodegenError::FrameOverflow)?)
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    emit(
        assembler,
        Inst::MovRM(FUNCTION_OBJECT, Mem::base(FRAME_POINTER, 16)),
    )?;
    emit(
        assembler,
        Inst::MovRM(ENTRY, Mem::base(FUNCTION_OBJECT, function_code_offset)),
    )?;
    emit(
        assembler,
        Inst::MovRM(ENTRY, Mem::base(ENTRY, code_constants_offset)),
    )?;
    emit(
        assembler,
        Inst::MovRM(FUNCTION_OBJECT, Mem::base(ENTRY, vector_element_offset)),
    )
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

fn closure_capture_count(function: &Function, closure: ValueId) -> usize {
    function
        .blocks
        .iter()
        .flat_map(|block| block.ops.iter())
        .find(|op| op.results.iter().any(|(value, _)| *value == closure))
        .and_then(|op| {
            if let OpKind::MakeClosure { captures, .. } = &op.kind {
                Some(captures.len())
            } else {
                None
            }
        })
        .unwrap_or(0)
}

#[allow(clippy::too_many_lines)]
fn lower_prim(
    assembler: &mut Assembler,
    prim: &Prim,
    args: &[ValueId],
    result: Option<ValueId>,
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
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
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => {
            // check-added-lines: allow(unsupported) existing codegen error variant
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
    slots: &ValueSlots,
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
            if matches!(
                value,
                ncl_ir::Constant::Symbol { .. }
                    | ncl_ir::Constant::Object(_)
                    | ncl_ir::Constant::StringBytes(_)
                    | ncl_ir::Constant::SingleFloat(_)
                    | ncl_ir::Constant::DoubleFloat(_)
            ) {
                load_heap_constant(assembler, *constant)?;
            } else {
                load_immediate(assembler, FUNCTION_OBJECT, constant_word(value, abi)?)?;
            }
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
        OpKind::Store { address, value }
        | OpKind::StoreField {
            object: address,
            value,
            ..
        } => {
            load_slot(assembler, slots, *address, FUNCTION_OBJECT)?;
            load_slot(assembler, slots, *value, ENTRY)?;
            let offset = match &op.kind {
                OpKind::StoreField { field, .. } => {
                    i32::try_from(field.saturating_add(1).saturating_mul(8))
                        .map_err(|_| CodegenError::FrameOverflow)?
                }
                _ => 0,
            };
            emit(
                assembler,
                Inst::MovMR(Mem::base(FUNCTION_OBJECT, offset), ENTRY),
            )?;
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
        OpKind::MakeClosure { entry, captures } => {
            let values = std::iter::once(*entry)
                .chain(captures.iter().copied())
                .collect::<Vec<_>>();
            lower_runtime_builtin(
                assembler,
                RuntimeFunction::MakeClosure,
                &[],
                &values,
                slots,
                abi,
            )?;
            call_pc = Some(emit_call(assembler)?);
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
            }
        }
        OpKind::CallClosure { closure, args } => {
            lower_closure_call(
                assembler,
                *closure,
                args,
                closure_capture_count(function, *closure),
                slots,
            )?;
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
        OpKind::EnterHandler { region } => {
            let definition = function
                .handler_regions
                .iter()
                .find(|candidate| candidate.id == *region)
                .ok_or_else(|| CodegenError::Unsupported("handler region is unavailable".into()))?;
            let (function, immediate_args, value_args) = match definition.kind {
                ncl_ir::HandlerKind::Catch => (
                    RuntimeFunction::EnterCatch,
                    vec![i64::from(region.0), i64::from(definition.depth)],
                    definition.catch_tag.into_iter().collect(),
                ),
                ncl_ir::HandlerKind::UnwindProtect => (
                    RuntimeFunction::EnterUnwindProtect,
                    vec![
                        i64::from(region.0),
                        i64::from(
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
                    vec![i64::from(region.0)],
                    definition.binding_targets.clone(),
                ),
            };
            lower_runtime_builtin(
                assembler,
                function,
                &immediate_args,
                &value_args,
                slots,
                abi,
            )?;
            call_pc = Some(emit_call(assembler)?);
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
            lower_runtime_builtin(assembler, function, &[i64::from(region.0)], &[], slots, abi)?;
            call_pc = Some(emit_call(assembler)?);
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
