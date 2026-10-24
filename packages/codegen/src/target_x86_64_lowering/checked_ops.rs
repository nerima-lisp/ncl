use super::{
    RETURN_VALUE, checked_call, dispatch::CallCheckContext, emit_call, lower_builtin, lower_call,
    lower_closure_call, lower_runtime_builtin, store_closure_capture, store_slot,
};
use crate::machine_verify::SafepointSite;
use crate::{CodegenError, RuntimeFunction};
use ncl_asm_x86_64::Assembler;
use ncl_ir::{OpKind, ValueId};

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
        OpKind::Builtin { name, args, .. } => {
            let checked = checked_call(assembler, context, |assembler| {
                lower_builtin(assembler, name, args, slots, abi)?;
                emit_call(assembler)
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
