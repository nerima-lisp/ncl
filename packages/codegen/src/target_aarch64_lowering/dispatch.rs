//! Dispatch a propagating non-local exit (or fall through to an ordinary
//! return) once a call may have set `pending`.
//!
//! See the top-level README's "return-code propagation" design note. This
//! lowers both `Terminator::Return` and `Terminator::Throw`: a non-local
//! exit is intercepted here, at whichever call site's terminator first
//! reaches a covering handler region, rather than through raw stack/register
//! surgery.

use std::{cmp::Ordering, collections::HashMap};

use ncl_asm_aarch64::{Assembler, Cond, Inst, Label, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{BlockId, Function, HandlerKind, HandlerRegion, ValueId};

use super::{context_mem, emit, load_value, store_value};
use crate::{Allocation, CodegenError, ContextField, RuntimeAbi, RuntimeFunction};

fn mv_area_mem(abi: &dyn RuntimeAbi, index: i32) -> Result<MemOperand, CodegenError> {
    let base = abi
        .field_offset(ContextField::MultipleValueArea)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    let byte_offset = index
        .checked_mul(8)
        .and_then(|delta| base.checked_add(delta))
        .ok_or(CodegenError::FrameOverflow)?;
    Ok(MemOperand::Unsigned {
        base: RegOrSp::Reg(Reg(21)),
        offset: u16::try_from(byte_offset).map_err(|_| CodegenError::FrameOverflow)?,
        scale: 8,
    })
}

/// Handler regions covering `block`, innermost first.
///
/// A region's `protected` block list always includes every block of any
/// region nested within it (a nested region starts later, at a higher block
/// id, and every block from a region's own start onward is included in its
/// own `protected` list), so ordering by the size of `protected` recovers
/// innermost-first order without needing the front end to populate
/// `depth`/`parent` (which it currently leaves at their defaults).
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

/// Emit the epilogue for an ordinary return of `values`, or, when `values`
/// is empty, the "propagate to my caller" epilogue: it leaves the
/// multiple-value area untouched (so a tag/value pair already written by a
/// `throw` further down the call chain survives), and reports a return
/// count no ordinary `Terminator::Return` would ever produce on its own.
pub(super) fn emit_epilogue(
    assembler: &mut Assembler,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    body_bytes: u32,
    values: &[ValueId],
    preserve_mv_count: bool,
) -> Result<(), CodegenError> {
    if values.len() > ncl_sys::MULTIPLE_VALUE_AREA_WORDS {
        return Err(CodegenError::MultipleValueAreaOverflow {
            count: values.len(),
            capacity: ncl_sys::MULTIPLE_VALUE_AREA_WORDS,
        });
    }
    for (index, value) in values.iter().copied().enumerate() {
        load_value(assembler, allocation, value, Reg(16))?;
        let index = i32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?;
        emit(
            assembler,
            Inst::Str {
                rt: Reg(16),
                mem: mv_area_mem(abi, index)?,
            },
        )?;
    }
    if let Some(value) = values.first() {
        load_value(assembler, allocation, *value, Reg(0))?;
    } else {
        for instruction in ncl_asm_aarch64::mov_imm64(Reg(0), ncl_sys::Word::ZERO.bits()) {
            emit(assembler, instruction)?;
        }
    }
    let count = u64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(1), count) {
        emit(assembler, instruction)?;
    }
    if !preserve_mv_count {
        emit(
            assembler,
            Inst::Str {
                rt: Reg(1),
                mem: context_mem(abi, ContextField::MultipleValueCount)?,
            },
        )?;
    }
    if body_bytes > 0 {
        emit(
            assembler,
            Inst::AddImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: u16::try_from(body_bytes).map_err(|_| CodegenError::FrameOverflow)?,
                shift: false,
            },
        )?;
    }
    emit(
        assembler,
        Inst::Ldp {
            rt: Reg(29),
            rt2: Reg(30),
            mem: MemOperand::PostIndex {
                base: RegOrSp::Sp,
                offset: 32,
            },
        },
    )?;
    emit(assembler, Inst::Ret { rn: Reg(30) })
}

/// Lower `Terminator::Return { values }` (`values = Some(...)`) or
/// `Terminator::Throw` (`values = None`).
///
/// When nothing is pending, an ordinary `Terminator::Return` proceeds with
/// `values` unchanged; `Terminator::Throw` always has something pending (the
/// `throw` builtin call immediately before it always sets `pending`, whether
/// or not it found a matching tag), so it always dispatches.
///
/// When something is pending, every handler region covering `block` is
/// tried innermost first: a `catch` intercepts when its tag matches word 0
/// of the multiple-value area (loading word 1 into the handler block's
/// parameter); an `unwind-protect`/`progv` always intercepts. If nothing
/// covering this block intercepts, this falls through to the "propagate"
/// epilogue, leaving the multiple-value area for an enclosing frame (this
/// function's own caller) to inspect after this call returns.
fn try_dispatch_candidates(
    assembler: &mut Assembler,
    function: &Function,
    block: BlockId,
    allocation: &Allocation,
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
                    Inst::Ldr {
                        rt: Reg(17),
                        mem: mv_area_mem(abi, 0)?,
                    },
                )?;
                load_value(assembler, allocation, tag, Reg(18))?;
                emit(
                    assembler,
                    Inst::Cmp {
                        rn: Reg(17),
                        rm: Reg(18),
                        shift: Shift::Lsl(0),
                    },
                )?;
                emit(
                    assembler,
                    Inst::BCond {
                        cond: Cond::Ne,
                        label: mismatch,
                    },
                )?;
                let handler = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == region.handler)
                    .ok_or(CodegenError::UnknownBlock(region.handler))?;
                if let Some(param) = handler.params.first() {
                    emit(
                        assembler,
                        Inst::Ldr {
                            rt: Reg(16),
                            mem: mv_area_mem(abi, 1)?,
                        },
                    )?;
                    store_value(assembler, allocation, param.value, Reg(16))?;
                }
                emit(assembler, Inst::B { label: target })?;
                assembler
                    .bind(mismatch)
                    .map_err(|error| CodegenError::Encode(error.to_string()))?;
                super::lower_runtime_builtin(
                    assembler,
                    RuntimeFunction::LeaveCatch,
                    &[u64::from(region.id.0)],
                    &[],
                    allocation,
                    abi,
                )?;
                emit(assembler, Inst::Blr { rn: Reg(17) })?;
            }
            HandlerKind::UnwindProtect | HandlerKind::Progv => {
                emit(assembler, Inst::B { label: target })?;
            }
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn lower_return_or_throw(
    assembler: &mut Assembler,
    function: &Function,
    block: BlockId,
    values: Option<&[ValueId]>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    body_bytes: u32,
    labels: &HashMap<BlockId, Label>,
) -> Result<(), CodegenError> {
    let normal = assembler.new_label();
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: context_mem(abi, ContextField::Pending)?,
        },
    )?;
    emit(
        assembler,
        Inst::Cbz {
            rt: Reg(16),
            label: normal,
        },
    )?;
    try_dispatch_candidates(assembler, function, block, allocation, abi, labels)?;
    emit_epilogue(assembler, allocation, abi, body_bytes, &[], true)?;
    assembler
        .bind(normal)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    emit_epilogue(
        assembler,
        allocation,
        abi,
        body_bytes,
        values.unwrap_or(&[]),
        false,
    )
}

/// Check `pending` after an ordinary call-like op (`Call`/`CallClosure`/
/// `CallIndirect`/`Builtin`/`MakeClosure`) and, if it is set, dispatch or
/// propagate exactly as [`lower_return_or_throw`] would; otherwise fall
/// through so the rest of the block lowers normally.
///
/// This is what makes a non-local exit crossing a `funcall`/closure-call
/// boundary (for example `return-from` reaching out of a closure back to an
/// enclosing `block` in the caller) work: the callee's own propagate
/// epilogue is an ordinary return, and it is this check, immediately after
/// the call that invoked it, that keeps the exit moving outward instead of
/// letting the caller treat the callee's sentinel return as a normal value.
pub fn lower_pending_check(
    assembler: &mut Assembler,
    function: &Function,
    block: BlockId,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    body_bytes: u32,
    labels: &HashMap<BlockId, Label>,
) -> Result<(), CodegenError> {
    let cleanup_block = function
        .handler_regions
        .iter()
        .any(|region| region.cleanup == Some(block));
    let normal = assembler.new_label();
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: context_mem(abi, ContextField::Pending)?,
        },
    )?;
    emit(
        assembler,
        Inst::Cbz {
            rt: Reg(16),
            label: normal,
        },
    )?;
    if !cleanup_block {
        try_dispatch_candidates(assembler, function, block, allocation, abi, labels)?;
    }
    emit_epilogue(assembler, allocation, abi, body_bytes, &[], true)?;
    assembler
        .bind(normal)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}
