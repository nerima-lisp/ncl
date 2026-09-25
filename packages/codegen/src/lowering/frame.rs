//! Frame-slot assignment and terminator target validation.

use crate::CodegenError;
use ncl_ir::{Function, Terminator, ValueId};
use std::collections::HashSet;

pub(super) fn slots(function: &Function) -> (Vec<(ValueId, u32)>, u32) {
    let mut result = Vec::new();
    let mut next = 4;
    for block in &function.blocks {
        for param in &block.params {
            result.push((param.value, next));
            next += 1;
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                result.push((*value, next));
                next += 1;
            }
        }
    }
    (result, next - 4)
}

pub(super) fn validate_targets(
    terminator: &Terminator,
    known: &HashSet<ncl_ir::BlockId>,
) -> Result<(), CodegenError> {
    let check = |id| {
        known
            .contains(&id)
            .then_some(())
            .ok_or(CodegenError::UnknownBlock(id))
    };
    match terminator {
        Terminator::Jump { target, .. } => check(*target),
        Terminator::Branch {
            then_target,
            else_target,
            ..
        } => {
            check(*then_target)?;
            check(*else_target)
        }
        Terminator::Switch { cases, default, .. } => {
            check(*default)?;
            for (_, target, _) in cases {
                check(*target)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
