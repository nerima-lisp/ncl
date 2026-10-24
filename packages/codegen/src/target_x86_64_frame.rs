use ncl_ir::{Function, OpKind, Terminator};

use super::lowering::{ARGUMENT_REGISTERS, closure_capture_count};
use crate::CodegenError;

pub(super) fn outgoing_words(function: &Function) -> Result<u32, CodegenError> {
    let mut maximum = 0_usize;
    for block in &function.blocks {
        for op in &block.ops {
            let count = match &op.kind {
                OpKind::Call { args, .. } | OpKind::CallIndirect { args, .. } => args
                    .len()
                    .saturating_sub(1)
                    .saturating_sub(ARGUMENT_REGISTERS.len()),
                OpKind::CallClosure { closure, args, .. } => {
                    let capture_staging_words =
                        closure_capture_count(function, *closure)?.unwrap_or(0);
                    let stack_argument_words = args
                        .len()
                        .saturating_sub(1)
                        .saturating_sub(ARGUMENT_REGISTERS.len());
                    capture_staging_words
                        .checked_add(stack_argument_words)
                        .ok_or(CodegenError::FrameOverflow)?
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
                | OpKind::MakeClosure { .. }
                | OpKind::MakeValueCell { .. }
                | OpKind::Builtin { .. }
                | OpKind::Prim { .. }
                | OpKind::Compare { .. }
                | OpKind::Convert { .. }
                | OpKind::SetMultipleValues { .. }
                | OpKind::Safepoint
                | OpKind::EnterHandler { .. }
                | OpKind::LeaveHandler { .. } => 0,
            };
            maximum = maximum.max(count);
        }
        let count = match &block.terminator {
            Terminator::CallReturn { args, .. } | Terminator::TailCall { args, .. } => args
                .len()
                .saturating_sub(1)
                .saturating_sub(ARGUMENT_REGISTERS.len()),
            Terminator::Jump { .. }
            | Terminator::Branch { .. }
            | Terminator::Switch { .. }
            | Terminator::Return { .. }
            | Terminator::Throw { .. }
            | Terminator::Unreachable => 0,
        };
        maximum = maximum.max(count);
    }
    u32::try_from(maximum).map_err(|_| CodegenError::FrameOverflow)
}
