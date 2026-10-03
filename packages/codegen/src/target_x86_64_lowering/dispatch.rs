//! Dispatch a pending non-local exit at an x86-64 call or terminator.

use std::{cmp::Ordering, collections::HashMap};

use ncl_asm_x86_64::{Assembler, Cond, Inst, Label, Mem, Reg};
use ncl_ir::{BlockId, Function, HandlerKind, HandlerRegion, ValueId};

use super::{emit, emit_call, load_immediate, load_slot, store_return_values, store_slot};
use crate::isa_x86_64::THREAD_CONTEXT;
use crate::{CodegenError, ContextField, RuntimeAbi, RuntimeFunction};

fn context_mem(abi: &dyn RuntimeAbi, field: ContextField) -> Result<Mem, CodegenError> {
    let offset = abi
        .field_offset(field)
        .map_err(|error| CodegenError::Abi(error.to_string()))?;
    Ok(Mem::base(THREAD_CONTEXT, offset))
}

fn mv_area_mem(abi: &dyn RuntimeAbi, index: i32) -> Result<Mem, CodegenError> {
    let base = abi
        .field_offset(ContextField::MultipleValueArea)
        .map_err(|error| CodegenError::Abi(error.to_string()))?;
    let offset = index
        .checked_mul(8)
        .and_then(|delta| base.checked_add(delta))
        .ok_or(CodegenError::FrameOverflow)?;
    Ok(Mem::base(THREAD_CONTEXT, offset))
}

fn emit_epilogue(
    assembler: &mut Assembler,
    slots: &super::ValueSlots,
    abi: &dyn RuntimeAbi,
    values: &[ValueId],
    preserve_mv_count: bool,
) -> Result<(), CodegenError> {
    store_return_values(assembler, slots, values, abi)?;
    if let Some(value) = values.first() {
        load_slot(assembler, slots, *value, super::RETURN_VALUE)?;
    } else {
        load_immediate(
            assembler,
            super::RETURN_VALUE,
            i64::from_ne_bytes(ncl_sys::Word::ZERO.bits().to_ne_bytes()),
        )?;
    }
    load_immediate(
        assembler,
        super::VALUE_COUNT,
        i64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?,
    )?;
    if !preserve_mv_count {
        emit(
            assembler,
            Inst::MovMR(
                context_mem(abi, ContextField::MultipleValueCount)?,
                super::VALUE_COUNT,
            ),
        )?;
    }
    emit(assembler, Inst::MovRR(Reg::Rsp, super::FRAME_POINTER))?;
    emit(assembler, Inst::Pop(super::FRAME_POINTER))?;
    emit(assembler, Inst::Ret)
}

/// Handler regions covering `block`, innermost first.
fn covering_regions(function: &Function, block: BlockId) -> Vec<&HandlerRegion> {
    let mut regions = function
        .handler_regions
        .iter()
        .filter(|region| region.protected.contains(&block))
        .collect::<Vec<_>>();
    regions.sort_by(|left, right| {
        let left_is_inner = left.protected.len() < right.protected.len()
            && left
                .protected
                .iter()
                .all(|block| right.protected.contains(block));
        let right_is_inner = right.protected.len() < left.protected.len()
            && right
                .protected
                .iter()
                .all(|block| left.protected.contains(block));
        match (left_is_inner, right_is_inner) {
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) | (true, true) => right.id.0.cmp(&left.id.0),
        }
    });
    regions
}

fn try_dispatch_candidates(
    assembler: &mut Assembler,
    function: &Function,
    block: BlockId,
    slots: &super::ValueSlots,
    abi: &dyn RuntimeAbi,
    labels: &HashMap<BlockId, Label>,
) -> Result<(), CodegenError> {
    for region in covering_regions(function, block) {
        let target = *labels
            .get(&region.handler)
            .ok_or(CodegenError::UnknownBlock(region.handler))?;
        match region.kind {
            HandlerKind::Catch => {
                let Some(tag) = region.catch_tag else {
                    continue;
                };
                let mismatch = assembler.new_label();
                emit(
                    assembler,
                    Inst::MovRM(super::FUNCTION_OBJECT, mv_area_mem(abi, 0)?),
                )?;
                load_slot(assembler, slots, tag, super::ENTRY)?;
                emit(assembler, Inst::CmpRR(super::FUNCTION_OBJECT, super::ENTRY))?;
                emit(assembler, Inst::Jcc(Cond::Ne, mismatch))?;
                let handler = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == region.handler)
                    .ok_or(CodegenError::UnknownBlock(region.handler))?;
                if let Some(param) = handler.params.first() {
                    emit(assembler, Inst::MovRM(super::ENTRY, mv_area_mem(abi, 1)?))?;
                    store_slot(assembler, slots, param.value, super::ENTRY)?;
                }
                for (param, source) in handler
                    .params
                    .iter()
                    .skip(1)
                    .zip(region.binding_targets.iter())
                {
                    load_slot(assembler, slots, *source, super::ENTRY)?;
                    store_slot(assembler, slots, param.value, super::ENTRY)?;
                }
                emit(assembler, Inst::Jmp(target))?;
                assembler.bind(mismatch);
                super::lower_runtime_builtin(
                    assembler,
                    RuntimeFunction::LeaveCatch,
                    &[i64::from(region.id.0)],
                    &[],
                    slots,
                    abi,
                )?;
                emit_call(assembler)?;
            }
            HandlerKind::UnwindProtect | HandlerKind::Progv => {
                emit(assembler, Inst::Jmp(target))?;
            }
        }
    }
    Ok(())
}

pub fn lower_return_or_throw(
    assembler: &mut Assembler,
    function: &Function,
    block: BlockId,
    values: Option<&[ValueId]>,
    slots: &super::ValueSlots,
    abi: &dyn RuntimeAbi,
    labels: &HashMap<BlockId, Label>,
) -> Result<(), CodegenError> {
    let normal = assembler.new_label();
    emit(
        assembler,
        Inst::MovRM(
            super::FUNCTION_OBJECT,
            context_mem(abi, ContextField::Pending)?,
        ),
    )?;
    emit(assembler, Inst::CmpRI(super::FUNCTION_OBJECT, 0))?;
    emit(assembler, Inst::Jcc(Cond::E, normal))?;
    try_dispatch_candidates(assembler, function, block, slots, abi, labels)?;
    emit_epilogue(assembler, slots, abi, &[], true)?;
    assembler.bind(normal);
    emit_epilogue(assembler, slots, abi, values.unwrap_or(&[]), false)
}

pub fn lower_pending_check(
    assembler: &mut Assembler,
    function: &Function,
    block: BlockId,
    slots: &super::ValueSlots,
    abi: &dyn RuntimeAbi,
    labels: &HashMap<BlockId, Label>,
) -> Result<(), CodegenError> {
    let normal = assembler.new_label();
    emit(
        assembler,
        Inst::MovRM(
            super::FUNCTION_OBJECT,
            context_mem(abi, ContextField::Pending)?,
        ),
    )?;
    emit(assembler, Inst::CmpRI(super::FUNCTION_OBJECT, 0))?;
    emit(assembler, Inst::Jcc(Cond::E, normal))?;
    try_dispatch_candidates(assembler, function, block, slots, abi, labels)?;
    emit_epilogue(assembler, slots, abi, &[], true)?;
    assembler.bind(normal);
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]
mod tests {
    use super::{context_mem, lower_return_or_throw, mv_area_mem};
    use crate::{AllocationTarget, CodegenError, RuntimeAbi, RuntimeFunction, X86_64Abi, allocate};
    use ncl_asm_x86_64::Assembler;
    use ncl_ir::{FunctionBuilder, HandlerKind, HandlerRegion, HandlerRegionId, Terminator};
    use std::collections::HashMap;

    struct MissingAbi;

    impl RuntimeAbi for MissingAbi {
        fn builtin_address(
            &self,
            identifier: ncl_object::BuiltinIdentifier,
        ) -> Result<u64, crate::AbiError> {
            Err(crate::AbiError::MissingBuiltin(identifier))
        }

        fn field_offset(&self, field: crate::ContextField) -> Result<i32, crate::AbiError> {
            Err(crate::AbiError::UnsupportedContextField(field))
        }

        fn runtime_address(&self, function: RuntimeFunction) -> Result<u64, crate::AbiError> {
            Err(crate::AbiError::UnsupportedRuntimeFunction(function))
        }
    }

    #[test]
    fn dispatch_mem_helpers_preserve_abi_and_offset_errors() {
        assert!(matches!(
            context_mem(&MissingAbi, crate::ContextField::Pending),
            Err(CodegenError::Abi(message))
                if message.contains("context offset is unavailable")
        ));
        assert_eq!(
            mv_area_mem(&X86_64Abi, i32::MAX),
            Err(CodegenError::FrameOverflow)
        );
    }

    #[test]
    fn reports_a_handler_target_that_has_no_emitted_label() {
        let mut builder = FunctionBuilder::new(
            ncl_ir::FunctionId(212),
            "unknown-handler-target",
            Vec::new(),
            Vec::new(),
        );
        builder.add_handler_region(HandlerRegion {
            id: HandlerRegionId(0),
            kind: HandlerKind::UnwindProtect,
            protected: vec![ncl_ir::BlockId(0)],
            handler: ncl_ir::BlockId(99),
            cleanup: Some(ncl_ir::BlockId(99)),
            catch_tag: None,
            binding_targets: Vec::new(),
            depth: 0,
            parent: None,
        });
        builder
            .terminate(Terminator::Unreachable)
            .expect("unreachable terminator");
        let function = builder.finish();
        let slots = super::super::slots(
            &function,
            0,
            allocate(&function, AllocationTarget::X86_64),
            0,
        )
        .0;
        let mut assembler = Assembler::new();
        assert_eq!(
            lower_return_or_throw(
                &mut assembler,
                &function,
                ncl_ir::BlockId(0),
                Some(&[]),
                &slots,
                &X86_64Abi,
                &HashMap::new(),
            ),
            Err(CodegenError::UnknownBlock(ncl_ir::BlockId(99)))
        );
    }
}
