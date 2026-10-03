use crate::CodegenError;
use ncl_ir::{Function, HandlerKind, OpKind, Terminator};

pub(super) fn outgoing_words(function: &Function) -> Result<u32, CodegenError> {
    let mut maximum = 0_usize;
    for block in &function.blocks {
        for op in &block.ops {
            let count = match &op.kind {
                OpKind::Call { args, .. } | OpKind::CallIndirect { args, .. } => {
                    args.len().saturating_sub(1)
                }
                OpKind::CallClosure { args, .. } => args.len().saturating_sub(1),
                OpKind::MakeClosure { .. } => 2,
                OpKind::MakeValueCell { .. } | OpKind::LeaveHandler { .. } => 1,
                OpKind::Builtin { args, .. } => args.len(),
                OpKind::EnterHandler { region } => handler_argument_count(function, *region),
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
                | OpKind::Safepoint => 0,
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

fn extra_words(argument_count: usize) -> usize {
    let extras = argument_count.saturating_sub(4);
    extras.saturating_add(usize::from(extras > 0))
}

fn handler_argument_count(function: &Function, region: ncl_ir::HandlerRegionId) -> usize {
    let Some(region) = function
        .handler_regions
        .iter()
        .find(|candidate| candidate.id == region)
    else {
        return 0;
    };
    match region.kind {
        HandlerKind::Catch => 2 + usize::from(region.catch_tag.is_some()),
        HandlerKind::UnwindProtect => 2,
        HandlerKind::Progv => 1 + region.binding_targets.len(),
    }
}
