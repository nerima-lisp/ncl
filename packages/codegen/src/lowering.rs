//! Fixed-template x86-64 lowering.
mod encode;
mod frame;
mod ops;
mod support;

use crate::{
    Block, CodegenError, CompiledFunction, FrameLayout, MachineFunction, MachineOp, RuntimeAbi,
};
use encode::encode;
use frame::{slots, validate_targets};
use ncl_ir::{Function, Terminator};
use std::collections::HashSet;

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
            safepoints: Vec::new(),
            relocations: Vec::new(),
            slots: value_slots,
        },
        abi,
    )
}
