use crate::{Block, CodegenError, CompiledFunction, FrameLayout, MachineFunction, MachineOp};
use crate::{RuntimeAbi, SafepointMap};
use ncl_ir::{Function, Terminator, ValueId};
use std::collections::HashSet;

use super::encode::encode;

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

/// Lowers one validated IR function using fixed stack-slot templates.
///
/// # Errors
///
/// Returns an error when the function is empty, references an unknown IR
/// value or block, exceeds frame limits, or requires an unavailable runtime
/// operation.
pub fn compile_function(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let Some(entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    let (value_slots, count) = slots(function);
    let arguments =
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?;
    let frame = FrameLayout::new(arguments, count, 0)?;
    let known: HashSet<_> = function.blocks.iter().map(|block| block.id).collect();
    for block in &function.blocks {
        validate_targets(&block.terminator, &known)?;
    }
    let blocks = function
        .blocks
        .iter()
        .map(|block| Block {
            id: block.id,
            operations: block
                .ops
                .iter()
                .map(|op| {
                    op.results.first().map_or(MachineOp::Return, |(value, _)| {
                        let slot = value_slots
                            .iter()
                            .find(|(id, _)| id == value)
                            .map_or(0, |(_, slot)| *slot);
                        MachineOp::Move {
                            source: slot,
                            destination: slot,
                        }
                    })
                })
                .chain(
                    matches!(block.terminator, Terminator::Return { .. })
                        .then_some(MachineOp::Return),
                )
                .collect(),
            offset: 0,
        })
        .collect();
    encode(
        function,
        MachineFunction {
            entry: entry.id,
            blocks,
            frame,
            safepoints: Vec::<SafepointMap>::new(),
            relocations: Vec::new(),
            slots: value_slots,
        },
        abi,
    )
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
