//! Fixed-template x86-64 lowering.

use crate::{
    Block, CodegenError, CompiledFunction, FLAG_CALL, FrameLayout, MachineFunction, MachineOp,
    RuntimeAbi, SafepointMap,
};
use ncl_asm_x86_64::{Assembler, Imm, Inst, Reg};
use ncl_ir::{Function, OpKind, Terminator, ValueId};
use std::collections::HashMap;

fn slots(function: &Function) -> (Vec<(ValueId, u32)>, u32) {
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
pub fn compile_function(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let Some(entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    let (value_slots, count) = slots(function);
    let frame = FrameLayout::new(function.params.len() as u32, count, 0)?;
    let known: HashMap<_, _> = function.blocks.iter().map(|block| (block.id, ())).collect();
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
                .filter_map(|op| {
                    op.results.first().map(|(value, _)| {
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
    let machine = MachineFunction {
        entry: entry.id,
        blocks,
        frame,
        safepoints: Vec::new(),
        relocations: Vec::new(),
        slots: value_slots,
    };
    encode(function, machine, abi)
}

fn validate_targets(
    terminator: &Terminator,
    known: &HashMap<ncl_ir::BlockId, ()>,
) -> Result<(), CodegenError> {
    let check = |id| {
        if known.contains_key(&id) {
            Ok(())
        } else {
            Err(CodegenError::UnknownBlock(id))
        }
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

fn encode(
    function: &Function,
    mut machine: MachineFunction,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let mut assembler = Assembler::new();
    let labels: HashMap<_, _> = machine
        .blocks
        .iter()
        .map(|block| (block.id, assembler.new_label()))
        .collect();
    let mut maps = Vec::new();
    let mut debug = Vec::new();
    let mut entry_offset = 0;
    for block in &mut machine.blocks {
        assembler.bind(labels[&block.id]);
        block.offset = assembler.bytes().len() as u32;
        if block.id == machine.entry {
            entry_offset = block.offset;
        }
        let Some(source) = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block.id)
        else {
            return Err(CodegenError::UnknownBlock(block.id));
        };
        for op in &source.ops {
            assembler
                .emit(&Inst::Nop(1))
                .map_err(|e| CodegenError::Encode(e.to_string()))?;
            if matches!(op.kind, OpKind::Safepoint) {
                maps.push(
                    SafepointMap::new(
                        assembler.bytes().len() as u32,
                        machine.frame.frame_words as u16,
                        machine.frame.frame_words as u16,
                        &[],
                        &[],
                        FLAG_CALL,
                    )
                    .map_err(|e| CodegenError::Encode(e.to_string()))?,
                );
            }
            debug.push(crate::DebugLocation {
                pc_offset: assembler.bytes().len() as u32,
                location: op.loc,
            });
        }
        match &source.terminator {
            Terminator::Return { .. } => {
                assembler
                    .emit(&Inst::MovRI(Reg::Rax, Imm::I64(abi.encode_fixnum(0))))
                    .map_err(|e| CodegenError::Encode(e.to_string()))?;
                assembler
                    .emit(&Inst::MovRI(Reg::Rdx, Imm::I64(0)))
                    .map_err(|e| CodegenError::Encode(e.to_string()))?;
                assembler
                    .emit(&Inst::Ret)
                    .map_err(|e| CodegenError::Encode(e.to_string()))?;
            }
            Terminator::Jump { target, .. } => assembler
                .emit(&Inst::Jmp(labels[target]))
                .map_err(|e| CodegenError::Encode(e.to_string()))?,
            Terminator::Branch { then_target, .. } => assembler
                .emit(&Inst::Jmp(labels[then_target]))
                .map_err(|e| CodegenError::Encode(e.to_string()))?,
            Terminator::Switch { default, .. } => assembler
                .emit(&Inst::Jmp(labels[default]))
                .map_err(|e| CodegenError::Encode(e.to_string()))?,
            Terminator::Unreachable
            | Terminator::CallReturn { .. }
            | Terminator::TailCall { .. }
            | Terminator::Throw { .. } => assembler
                .emit(&Inst::Ud2)
                .map_err(|e| CodegenError::Encode(e.to_string()))?,
        }
    }
    let blob = assembler
        .finish()
        .map_err(|e| CodegenError::Encode(e.to_string()))?;
    Ok(CompiledFunction {
        code: blob.bytes,
        entry_offset,
        relocations: machine.relocations,
        safepoint_maps: maps,
        frame_size: machine.frame.size_bytes(),
        debug,
    })
}
