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
                    let captures = closure_capture_count(function, *closure);
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

fn closure_capture_count(function: &Function, closure: ncl_ir::ValueId) -> usize {
    super::lowering::closure_captures(function, closure).map_or(0, <[ncl_ir::ValueId]>::len)
}

fn extra_words(argument_count: usize) -> usize {
    let extras = argument_count.saturating_sub(4);
    extras.saturating_add(usize::from(extras > 0))
}
