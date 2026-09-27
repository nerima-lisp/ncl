use crate::CodegenError;
use ncl_ir::{Function, OpKind, Terminator};

pub(super) fn outgoing_words(function: &Function) -> Result<u32, CodegenError> {
    let mut maximum = 0_usize;
    for block in &function.blocks {
        for op in &block.ops {
            let count = match &op.kind {
                OpKind::Call { args, .. } | OpKind::CallIndirect { args, .. } => {
                    args.len().saturating_sub(1)
                }
                OpKind::CallClosure { closure, args } => {
                    let captures = closure_capture_count(function, *closure)?;
                    captures.saturating_add(args.len().saturating_sub(1))
                }
                OpKind::Const { .. }
                | OpKind::Move { .. }
                | OpKind::Load { .. }
                | OpKind::Store { .. }
                | OpKind::LoadField { .. }
                | OpKind::StoreField { .. }
                | OpKind::Alloc { .. }
                | OpKind::LoadArg { .. }
                | OpKind::MakeClosure { .. }
                | OpKind::Builtin { .. }
                | OpKind::Prim { .. }
                | OpKind::Compare { .. }
                | OpKind::Convert { .. }
                | OpKind::SetMultipleValues { .. }
                | OpKind::Safepoint
                | OpKind::EnterHandler { .. }
                | OpKind::LeaveHandler { .. } => 0,
            };
            maximum = maximum.max(extra_words(count));
        }
        let count = match &block.terminator {
            Terminator::CallReturn { args, .. } | Terminator::TailCall { args, .. } => {
                args.len().saturating_sub(1)
            }
            Terminator::Jump { .. }
            | Terminator::Branch { .. }
            | Terminator::Switch { .. }
            | Terminator::Return { .. }
            | Terminator::Throw { .. }
            | Terminator::Unreachable => 0,
        };
        maximum = maximum.max(extra_words(count));
    }
    u32::try_from(maximum).map_err(|_| CodegenError::FrameOverflow)
}

fn closure_capture_count(
    function: &Function,
    closure: ncl_ir::ValueId,
) -> Result<usize, CodegenError> {
    let mut current = closure;
    for _ in 0..function
        .blocks
        .iter()
        .map(|block| block.ops.len())
        .sum::<usize>()
    {
        let definition = function
            .blocks
            .iter()
            .flat_map(|block| &block.ops)
            .find(|op| op.results.iter().any(|(value, _)| *value == current))
            .ok_or_else(|| {
                CodegenError::Unsupported("closure value definition is unavailable".into()) // check-added-lines: allow(unsupported) malformed closure IR cannot be sized safely.
            })?;
        match &definition.kind {
            OpKind::MakeClosure { captures, .. } => return Ok(captures.len()),
            OpKind::Move { value } | OpKind::Convert { value, .. } => current = *value,
            OpKind::Const { .. } | OpKind::LoadArg { .. } | OpKind::LoadField { .. } => {
                return Ok(0);
            }
            OpKind::Store { .. }
            | OpKind::StoreField { .. }
            | OpKind::Load { .. }
            | OpKind::Call { .. }
            | OpKind::CallIndirect { .. }
            | OpKind::CallClosure { .. }
            | OpKind::Builtin { .. }
            | OpKind::Alloc { .. }
            | OpKind::Prim { .. }
            | OpKind::Compare { .. }
            | OpKind::SetMultipleValues { .. }
            | OpKind::Safepoint
            | OpKind::EnterHandler { .. }
            | OpKind::LeaveHandler { .. } => {
                return Err(CodegenError::Unsupported(
                    // check-added-lines: allow(unsupported) malformed closure IR cannot be sized safely.
                    "closure capture metadata is unavailable".into(),
                ));
            }
        }
    }
    Err(CodegenError::Unsupported(
        // check-added-lines: allow(unsupported) cyclic closure definitions cannot be sized safely.
        "closure capture metadata has a cycle".into(),
    ))
}

fn extra_words(argument_count: usize) -> usize {
    let extras = argument_count.saturating_sub(4);
    extras.saturating_add(usize::from(extras > 0))
}
