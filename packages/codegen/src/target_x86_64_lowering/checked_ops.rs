use super::{
    ENTRY, FUNCTION_OBJECT, RETURN_VALUE, checked_call, dispatch::CallCheckContext, emit,
    emit_call, load_immediate, load_slot, lower_builtin, lower_call, lower_closure_call,
    lower_runtime_builtin, store_closure_capture, store_slot,
};
use crate::machine_verify::SafepointSite;
use crate::{CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Label, Shift};
use ncl_ir::{FixnumOp, FixnumSpecialization, OpKind, ValueId};

fn emit_fixnum_tag_check(
    assembler: &mut Assembler,
    argument: ValueId,
    slow: Label,
    slots: &super::ValueSlots,
) -> Result<(), CodegenError> {
    let tag_mask = (1_u64 << ncl_sys::FIXNUM_TAG_BITS) - 1;
    let tag_mask = i32::try_from(tag_mask).map_err(|_| CodegenError::FrameOverflow)?;
    let tag = i32::try_from(ncl_sys::FIXNUM_TAG).map_err(|_| CodegenError::FrameOverflow)?;
    load_slot(assembler, slots, argument, FUNCTION_OBJECT)?;
    emit(
        assembler,
        Inst::BinRI(BinOp::And, FUNCTION_OBJECT, tag_mask),
    )?;
    emit(assembler, Inst::CmpRI(FUNCTION_OBJECT, tag))?;
    emit(assembler, Inst::Jcc(Cond::Ne, slow))
}

fn emit_fixnum_binary(
    assembler: &mut Assembler,
    args: &[ValueId],
    operation: BinOp,
    slow: Label,
    slots: &super::ValueSlots,
) -> Result<(), CodegenError> {
    let [left, right] = args else {
        // check-added-lines: allow(unsupported) reject malformed specialization arity before machine emission.
        return Err(CodegenError::Unsupported(
            "fixnum specialization requires two operands".into(),
        ));
    };
    load_slot(assembler, slots, *left, FUNCTION_OBJECT)?;
    load_slot(assembler, slots, *right, ENTRY)?;
    emit(assembler, Inst::BinRR(operation, FUNCTION_OBJECT, ENTRY))?;
    emit(assembler, Inst::Jcc(Cond::O, slow))
}

fn emit_fixnum_product(
    assembler: &mut Assembler,
    args: &[ValueId],
    slow: Label,
    slots: &super::ValueSlots,
) -> Result<(), CodegenError> {
    let [left, right] = args else {
        // check-added-lines: allow(unsupported) reject malformed specialization arity before machine emission.
        return Err(CodegenError::Unsupported(
            "fixnum specialization requires two operands".into(),
        ));
    };
    load_slot(assembler, slots, *left, FUNCTION_OBJECT)?;
    load_slot(assembler, slots, *right, ENTRY)?;
    emit(
        assembler,
        Inst::ShiftImm(
            Shift::Sar,
            ENTRY,
            u8::try_from(ncl_sys::FIXNUM_TAG_BITS).map_err(|_| CodegenError::FrameOverflow)?,
        ),
    )?;
    emit(assembler, Inst::ImulRR(FUNCTION_OBJECT, ENTRY))?;
    emit(assembler, Inst::Jcc(Cond::O, slow))
}

fn emit_fixnum_unary(
    assembler: &mut Assembler,
    argument: ValueId,
    operation: BinOp,
    slow: Label,
    slots: &super::ValueSlots,
) -> Result<(), CodegenError> {
    load_slot(assembler, slots, argument, FUNCTION_OBJECT)?;
    emit(assembler, Inst::BinRI(operation, FUNCTION_OBJECT, 2))?;
    emit(assembler, Inst::Jcc(Cond::O, slow))
}

fn lower_fixnum_specialization(
    assembler: &mut Assembler,
    name: &str,
    args: &[ValueId],
    specialization: FixnumSpecialization,
    slots: &super::ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<SafepointSite, CodegenError> {
    let slow = assembler.new_label();
    let done = assembler.new_label();
    for (index, argument) in args.iter().copied().enumerate() {
        if specialization.needs_tag_check(index) {
            emit_fixnum_tag_check(assembler, argument, slow, slots)?;
        }
    }

    match specialization.operation() {
        FixnumOp::Add => emit_fixnum_binary(assembler, args, BinOp::Add, slow, slots)?,
        FixnumOp::Sub => emit_fixnum_binary(assembler, args, BinOp::Sub, slow, slots)?,
        FixnumOp::Mul => emit_fixnum_product(assembler, args, slow, slots)?,
        FixnumOp::LessThan => {
            let [left, right] = args else {
                // check-added-lines: allow(unsupported) reject malformed specialization arity before machine emission.
                return Err(CodegenError::Unsupported(
                    "fixnum specialization requires two operands".into(),
                ));
            };
            let truth = assembler.new_label();
            load_slot(assembler, slots, *left, FUNCTION_OBJECT)?;
            load_slot(assembler, slots, *right, ENTRY)?;
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            emit(assembler, Inst::Jcc(Cond::L, truth))?;
            load_immediate(
                assembler,
                RETURN_VALUE,
                i64::from_ne_bytes(ncl_sys::Word::NIL.bits().to_ne_bytes()),
            )?;
            emit(assembler, Inst::Jmp(done))?;
            assembler.bind(truth);
            load_immediate(
                assembler,
                RETURN_VALUE,
                i64::from_ne_bytes(ncl_sys::Word::TRUE.bits().to_ne_bytes()),
            )?;
            emit(assembler, Inst::Jmp(done))?;
        }
        FixnumOp::OnePlus => {
            let [argument] = args else {
                // check-added-lines: allow(unsupported) reject malformed specialization arity before machine emission.
                return Err(CodegenError::Unsupported(
                    "fixnum specialization requires one operand".into(),
                ));
            };
            emit_fixnum_unary(assembler, *argument, BinOp::Add, slow, slots)?;
        }
        FixnumOp::OneMinus => {
            let [argument] = args else {
                // check-added-lines: allow(unsupported) reject malformed specialization arity before machine emission.
                return Err(CodegenError::Unsupported(
                    "fixnum specialization requires one operand".into(),
                ));
            };
            emit_fixnum_unary(assembler, *argument, BinOp::Sub, slow, slots)?;
        }
    }

    if !matches!(specialization.operation(), FixnumOp::LessThan) {
        emit(assembler, Inst::MovRR(RETURN_VALUE, FUNCTION_OBJECT))?;
        emit(assembler, Inst::Jmp(done))?;
    }
    assembler.bind(slow);
    lower_builtin(assembler, name, args, slots, abi)?;
    let site = emit_call(assembler)?;
    assembler.bind(done);
    Ok(site)
}

#[allow(clippy::too_many_lines)]
pub(super) fn lower_checked_ops(
    assembler: &mut Assembler,
    op: &OpKind,
    result: Option<ValueId>,
    context: &CallCheckContext<'_>,
) -> Result<Option<SafepointSite>, CodegenError> {
    let function = context.function;
    let slots = context.slots;
    let abi = context.abi;
    let mut call_pc = None;
    match op {
        OpKind::Call {
            function: callee,
            args,
        }
        | OpKind::CallIndirect { callee, args } => {
            let checked = checked_call(assembler, context, |assembler| {
                lower_call(assembler, *callee, args, slots)?;
                emit_call(assembler)
            })?;
            call_pc = Some(checked.site());
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
            }
        }
        OpKind::MakeClosure { entry, captures } => {
            let checked = checked_call(assembler, context, |assembler| {
                lower_runtime_builtin(
                    assembler,
                    RuntimeFunction::MakeClosure,
                    &[ncl_sys::Word::fixnum(
                        i64::try_from(captures.len()).map_err(|_| CodegenError::FrameOverflow)?,
                    )
                    .bits()
                    .cast_signed()],
                    &[*entry],
                    slots,
                    abi,
                )?;
                emit_call(assembler)
            })?;
            call_pc = Some(checked.site());
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
                // No allocation or safepoint occurs before these stores, so the new object cannot move.
                for (index, capture) in captures.iter().copied().enumerate() {
                    store_closure_capture(assembler, result, index, capture, slots, abi)?;
                }
            }
        }
        OpKind::MakeValueCell { value } => {
            let checked = checked_call(assembler, context, |assembler| {
                lower_runtime_builtin(
                    assembler,
                    RuntimeFunction::MakeValueCell,
                    &[],
                    &[*value],
                    slots,
                    abi,
                )?;
                emit_call(assembler)
            })?;
            call_pc = Some(checked.site());
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
            }
        }
        OpKind::CallClosure {
            closure,
            args,
            named_symbol,
        } => {
            let checked = checked_call(assembler, context, |assembler| {
                lower_closure_call(
                    assembler,
                    *closure,
                    args,
                    super::closure_capture_count(function, *closure)?.unwrap_or(0),
                    slots,
                    *named_symbol,
                    abi,
                )?;
                emit_call(assembler)
            })?;
            call_pc = Some(checked.site());
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
            }
        }
        OpKind::Builtin {
            name,
            args,
            specialization,
            specialization_disabled,
        } => {
            let checked = checked_call(assembler, context, |assembler| {
                let specialization = if *specialization_disabled {
                    None
                } else {
                    *specialization
                };
                if let Some(specialization) = specialization {
                    lower_fixnum_specialization(assembler, name, args, specialization, slots, abi)
                } else {
                    lower_builtin(assembler, name, args, slots, abi)?;
                    emit_call(assembler)
                }
            })?;
            call_pc = Some(checked.site());
            if let Some(result) = result {
                store_slot(assembler, slots, result, RETURN_VALUE)?;
            }
        }
        OpKind::EnterHandler { region } => {
            let definition = function
                .handler_regions
                .iter()
                .find(|candidate| candidate.id == *region)
                // check-added-lines: allow(unsupported) preserve the missing handler-region diagnostic.
                .ok_or_else(|| CodegenError::Unsupported("handler region is unavailable".into()))?;
            let (runtime_function, immediate_args, value_args) = match definition.kind {
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
                                    // check-added-lines: allow(unsupported) preserve the missing cleanup diagnostic.
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
            let checked = checked_call(assembler, context, |assembler| {
                lower_runtime_builtin(
                    assembler,
                    runtime_function,
                    &immediate_args,
                    &value_args,
                    slots,
                    abi,
                )?;
                emit_call(assembler)
            })?;
            call_pc = Some(checked.site());
        }
        OpKind::LeaveHandler { region } => {
            let definition = function
                .handler_regions
                .iter()
                .find(|candidate| candidate.id == *region)
                // check-added-lines: allow(unsupported) preserve the missing handler-region diagnostic.
                .ok_or_else(|| CodegenError::Unsupported("handler region is unavailable".into()))?;
            let runtime_function = match definition.kind {
                ncl_ir::HandlerKind::Catch => RuntimeFunction::LeaveCatch,
                ncl_ir::HandlerKind::UnwindProtect => RuntimeFunction::LeaveUnwindProtect,
                ncl_ir::HandlerKind::Progv => RuntimeFunction::LeaveProgv,
            };
            let checked = checked_call(assembler, context, |assembler| {
                lower_runtime_builtin(
                    assembler,
                    runtime_function,
                    &[i64::from(region.0)],
                    &[],
                    slots,
                    abi,
                )?;
                emit_call(assembler)
            })?;
            call_pc = Some(checked.site());
        }
        OpKind::Const { .. }
        | OpKind::Move { .. }
        | OpKind::TypeHint { .. }
        | OpKind::Load { .. }
        | OpKind::Store { .. }
        | OpKind::LoadField { .. }
        | OpKind::StoreField { .. }
        | OpKind::Alloc { .. }
        | OpKind::LoadArg { .. }
        | OpKind::LoadCapture { .. }
        | OpKind::LoadFunctionObject
        | OpKind::Prim { .. }
        | OpKind::Compare { .. }
        | OpKind::Convert { .. }
        | OpKind::SetMultipleValues { .. }
        | OpKind::Safepoint => {}
    }
    Ok(call_pc)
}
