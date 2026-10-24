#[path = "checked_ops/fixnum.rs"]
mod fixnum;

use super::{
    CallCheckContext, CheckedCallMode, emit, emit_safepoint_call, lower_alloc, lower_builtin,
    lower_call, lower_checked_call, lower_runtime_builtin, lower_safepoint, primitives,
    store_value,
};
use crate::machine_verify::SafepointSite;
use crate::{Allocation, CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_aarch64::{Assembler, Inst, Reg};
use ncl_ir::{FixnumSpecialization, Function, HandlerRegionId, ValueId};

pub(super) fn lower_alloc_op(
    assembler: &mut Assembler,
    words: u32,
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    Ok(lower_alloc(assembler, words, result, allocation, abi, context)?.site())
}

pub(super) fn lower_safepoint_op(
    assembler: &mut Assembler,
    abi: &dyn RuntimeAbi,
) -> Result<SafepointSite, CodegenError> {
    lower_safepoint(assembler, abi)
}

pub(super) fn lower_call_op(
    assembler: &mut Assembler,
    function: ValueId,
    args: &[ValueId],
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let call_pc = lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            lower_call(assembler, function, args, allocation)?;
            emit_safepoint_call(assembler)
        },
    )?
    .site();
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    Ok(call_pc)
}

pub(super) fn lower_make_closure_op(
    assembler: &mut Assembler,
    entry: ValueId,
    captures: &[ValueId],
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let call_pc = lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            lower_runtime_builtin(
                assembler,
                RuntimeFunction::MakeClosure,
                &[ncl_sys::Word::fixnum(
                    i64::try_from(captures.len()).map_err(|_| CodegenError::FrameOverflow)?,
                )
                .bits()],
                &[entry],
                allocation,
                abi,
            )?;
            emit_safepoint_call(assembler)
        },
    )?
    .site();
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(0))?;
        // No allocation or safepoint occurs before these stores, so the new object cannot move.
        for (index, capture) in captures.iter().copied().enumerate() {
            store_closure_capture(assembler, result, index, capture, allocation, abi)?;
        }
    }
    Ok(call_pc)
}

pub(super) fn lower_make_value_cell_op(
    assembler: &mut Assembler,
    value: ValueId,
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let call_pc = lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            lower_runtime_builtin(
                assembler,
                RuntimeFunction::MakeValueCell,
                &[],
                &[value],
                allocation,
                abi,
            )?;
            emit_safepoint_call(assembler)
        },
    )?
    .site();
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    Ok(call_pc)
}

pub(super) fn lower_call_closure_op(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    named_symbol: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    primitives::lower_closure_call(assembler, closure, args, allocation, named_symbol, abi)?;
    let call_pc = lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        emit_safepoint_call,
    )?
    .site();
    Ok(call_pc)
}

#[derive(Clone, Copy)]
pub(super) struct BuiltinCall<'a> {
    name: &'a str,
    args: &'a [ValueId],
    specialization: Option<FixnumSpecialization>,
    specialization_disabled: bool,
}

impl<'a> BuiltinCall<'a> {
    pub(super) const fn new(
        name: &'a str,
        args: &'a [ValueId],
        specialization: Option<FixnumSpecialization>,
        specialization_disabled: bool,
    ) -> Self {
        Self {
            name,
            args,
            specialization,
            specialization_disabled,
        }
    }
}

pub(super) fn lower_builtin_op(
    assembler: &mut Assembler,
    builtin: BuiltinCall<'_>,
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    if !builtin.specialization_disabled
        && let Some(specialization) = builtin.specialization
    {
        return fixnum::lower_checked_fixnum_builtin_op(
            assembler,
            fixnum::CheckedFixnumBuiltin {
                name: builtin.name,
                args: builtin.args,
                specialization,
            },
            result,
            allocation,
            abi,
            context,
        );
    }

    let call_pc = lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            lower_builtin(assembler, builtin.name, builtin.args, allocation, abi)?;
            emit_safepoint_call(assembler)
        },
    )?
    .site();
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    Ok(call_pc)
}

pub(super) fn lower_enter_handler_op(
    assembler: &mut Assembler,
    function: &Function,
    region: HandlerRegionId,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let definition = function
        .handler_regions
        .iter()
        .find(|candidate| candidate.id == region)
        // check-added-lines: allow(unsupported) malformed IR can reference an unavailable handler region.
        .ok_or_else(|| CodegenError::Unsupported("handler region is unavailable".into()))?;
    let (runtime_function, immediate_args, value_args) = match definition.kind {
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
                            // check-added-lines: allow(unsupported) malformed unwind-protect IR may omit cleanup.
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
    lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            lower_runtime_builtin(
                assembler,
                runtime_function,
                &immediate_args,
                &value_args,
                allocation,
                abi,
            )?;
            emit_safepoint_call(assembler)
        },
    )
    .map(super::CheckedCall::site)
}

pub(super) fn lower_leave_handler_op(
    assembler: &mut Assembler,
    function: &Function,
    region: HandlerRegionId,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let definition = function
        .handler_regions
        .iter()
        .find(|candidate| candidate.id == region)
        // check-added-lines: allow(unsupported) malformed IR can reference an unavailable handler region.
        .ok_or_else(|| CodegenError::Unsupported("handler region is unavailable".into()))?;
    let runtime_function = match definition.kind {
        ncl_ir::HandlerKind::Catch => RuntimeFunction::LeaveCatch,
        ncl_ir::HandlerKind::UnwindProtect => RuntimeFunction::LeaveUnwindProtect,
        ncl_ir::HandlerKind::Progv => RuntimeFunction::LeaveProgv,
    };
    lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            lower_runtime_builtin(
                assembler,
                runtime_function,
                &[u64::from(region.0)],
                &[],
                allocation,
                abi,
            )?;
            emit_safepoint_call(assembler)
        },
    )
    .map(super::CheckedCall::site)
}

fn store_closure_capture(
    assembler: &mut Assembler,
    closure: ValueId,
    index: usize,
    capture: ValueId,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    use ncl_asm_aarch64::{MemOperand, RegOrSp};

    let slot = ncl_object::function_offset::CAPTURES
        .checked_add(index)
        .ok_or(CodegenError::FrameOverflow)?;
    let byte_offset = slot
        .checked_add(1)
        .and_then(|slot| slot.checked_mul(8))
        .ok_or(CodegenError::FrameOverflow)?;
    super::load_value(assembler, allocation, closure, Reg(16))?;
    super::load_value(assembler, allocation, capture, Reg(17))?;
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(16),
            rn: Reg(16),
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )?;
    let store_offset = if byte_offset <= 255 {
        i16::try_from(byte_offset).map_err(|_| CodegenError::FrameOverflow)?
    } else {
        let mut remaining = byte_offset;
        while remaining >= (1 << 12) {
            let high = u16::try_from((remaining >> 12).min(0x0fff))
                .map_err(|_| CodegenError::FrameOverflow)?;
            emit(
                assembler,
                Inst::AddImm {
                    rd: RegOrSp::Reg(Reg(16)),
                    rn: RegOrSp::Reg(Reg(16)),
                    imm: high,
                    shift: true,
                },
            )?;
            remaining -= usize::from(high) << 12;
        }
        if remaining != 0 {
            emit(
                assembler,
                Inst::AddImm {
                    rd: RegOrSp::Reg(Reg(16)),
                    rn: RegOrSp::Reg(Reg(16)),
                    imm: u16::try_from(remaining).map_err(|_| CodegenError::FrameOverflow)?,
                    shift: false,
                },
            )?;
        }
        0
    };
    emit(
        assembler,
        Inst::Str {
            rt: Reg(17),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(16)),
                offset: store_offset,
            },
        },
    )?;
    super::ops::lower_write_barrier_call(assembler, closure, slot, allocation, abi)
}
