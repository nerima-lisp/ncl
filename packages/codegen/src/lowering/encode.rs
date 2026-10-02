use crate::{CodegenError, CompiledFunction, MachineFunction, RuntimeAbi, checked_u32};
use crate::{FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Reg};
use ncl_ir::{Function, Terminator};
use std::collections::HashMap;

use super::ops::{add_map, emit, emit_return, load, lower_op};

#[allow(clippy::too_many_lines)]
pub(super) fn encode(
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
    emit(&mut assembler, &Inst::Push(Reg::Rbp))?;
    emit(&mut assembler, &Inst::MovRR(Reg::Rsp, Reg::Rbp))?;
    emit(
        &mut assembler,
        &Inst::BinRI(
            BinOp::Sub,
            Reg::Rsp,
            machine.frame.size_bytes().cast_signed(),
        ),
    )?;
    let entry_offset = checked_u32(assembler.bytes().len())?;
    for block in &mut machine.blocks {
        assembler.bind(labels[&block.id]);
        block.offset = checked_u32(assembler.bytes().len())?;
        let source = function
            .blocks
            .iter()
            .find(|candidate| candidate.id == block.id)
            .ok_or(CodegenError::UnknownBlock(block.id))?;
        for op in &source.ops {
            lower_op(
                &mut assembler,
                op,
                function,
                &machine.slots,
                abi,
                machine.frame,
                &mut maps,
            )?;
            debug.push(crate::DebugLocation {
                pc_offset: checked_u32(assembler.bytes().len())?,
                location: op.loc,
            });
        }
        match &source.terminator {
            Terminator::Return { values } => {
                emit_return(&mut assembler, values, &machine.slots, abi)?;
            }
            Terminator::Jump { target, .. } => {
                emit(&mut assembler, &Inst::Jmp(labels[target]))?;
                if *target == block.id {
                    add_map(
                        &assembler,
                        machine.frame,
                        &machine.slots,
                        &mut maps,
                        FLAG_LOOP_BACKEDGE,
                    )?;
                }
            }
            Terminator::Branch {
                condition,
                then_target,
                else_target,
                ..
            } => {
                load(&mut assembler, &machine.slots, *condition, Reg::R10)?;
                emit(&mut assembler, &Inst::CmpRI(Reg::R10, 0))?;
                emit(&mut assembler, &Inst::Jcc(Cond::Ne, labels[then_target]))?;
                emit(&mut assembler, &Inst::Jmp(labels[else_target]))?;
            }
            Terminator::Switch {
                value,
                cases,
                default,
                ..
            } => {
                load(&mut assembler, &machine.slots, *value, Reg::R10)?;
                for (case, target, _) in cases {
                    emit(
                        &mut assembler,
                        &Inst::CmpRI(
                            Reg::R10,
                            i32::try_from(*case).map_err(|_| CodegenError::FrameOverflow)?,
                        ),
                    )?;
                    emit(&mut assembler, &Inst::Jcc(Cond::E, labels[target]))?;
                }
                emit(&mut assembler, &Inst::Jmp(labels[default]))?;
            }
            Terminator::CallReturn {
                function: callee, ..
            }
            | Terminator::TailCall {
                function: callee, ..
            } => {
                load(&mut assembler, &machine.slots, *callee, Reg::R11)?;
                emit(&mut assembler, &Inst::CallReg(Reg::R11))?;
                add_map(
                    &assembler,
                    machine.frame,
                    &machine.slots,
                    &mut maps,
                    FLAG_CALL,
                )?;
                emit_return(&mut assembler, &[], &machine.slots, abi)?;
            }
            Terminator::Throw { .. } => return Err(CodegenError::NonLocalExitUnsupported),
            Terminator::Unreachable => emit(&mut assembler, &Inst::Ud2)?,
        }
    }
    let blob = assembler
        .finish()
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    let relocations = crate::relocations_from_fixups(&blob.fixups).map_err(|error| {
        CodegenError::Encode(format!("relocation conversion failed: {error:?}"))
    })?;
    Ok(CompiledFunction {
        code: blob.bytes,
        entry_offset,
        relocations,
        safepoint_maps: maps,
        frame_size: machine.frame.size_bytes(),
        debug,
    })
}

#[cfg(test)]
mod tests {
    use super::encode;
    use crate::{Block, CodegenError, FrameLayout, MachineFunction, X86_64Abi};
    use ncl_ir::{
        BasicBlock, BlockId, DebugLocationId, Function, FunctionId, Op, OpKind, Terminator, Ty,
        ValueId,
    };

    fn function(terminator: Terminator, ops: Vec<Op>) -> Function {
        Function {
            id: FunctionId(2),
            name: "encode-test".into(),
            params: Vec::new(),
            return_types: Vec::new(),
            blocks: vec![BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops,
                terminator,
            }],
            locals: Vec::new(),
            constants: Vec::new(),
            handler_regions: Vec::new(),
            debug: Vec::new(),
        }
    }

    fn machine(
        blocks: Vec<Block>,
        slots: Vec<(ValueId, u32)>,
        frame_words: u32,
    ) -> MachineFunction {
        MachineFunction::new(
            blocks.first().map_or(BlockId(0), Block::id),
            blocks,
            FrameLayout::new(0, frame_words.saturating_sub(4), 0).expect("frame"),
            Vec::new(),
            Vec::new(),
            slots,
        )
    }

    #[test]
    fn rejects_a_machine_block_without_a_source_block() {
        let source = function(Terminator::Unreachable, Vec::new());
        let machine = machine(vec![Block::new(BlockId(9), Vec::new())], Vec::new(), 4);

        assert_eq!(
            encode(&source, machine, &X86_64Abi),
            Err(CodegenError::UnknownBlock(BlockId(9)))
        );
    }

    #[test]
    fn rejects_switch_case_values_that_do_not_fit_the_encoder() {
        let source = function(
            Terminator::Switch {
                value: ValueId(0),
                cases: vec![(i64::MAX, BlockId(0), Vec::new())],
                default: BlockId(0),
                default_args: Vec::new(),
            },
            Vec::new(),
        );
        let machine = machine(
            vec![Block::new(BlockId(0), Vec::new())],
            vec![(ValueId(0), 4)],
            5,
        );

        assert_eq!(
            encode(&source, machine, &X86_64Abi),
            Err(CodegenError::FrameOverflow)
        );
    }

    #[test]
    fn records_source_locations_after_each_lowered_operation() {
        let source = function(
            Terminator::Return {
                values: vec![ValueId(1)],
            },
            vec![Op {
                results: vec![(ValueId(1), Ty::Word)],
                kind: OpKind::Move { value: ValueId(0) },
                loc: Some(DebugLocationId(7)),
            }],
        );
        let machine = machine(
            vec![Block::new(BlockId(0), vec![])],
            vec![(ValueId(0), 4), (ValueId(1), 5)],
            6,
        );

        let compiled = encode(&source, machine, &X86_64Abi).expect("encoded move");
        assert_eq!(compiled.debug.len(), 1);
        assert_eq!(compiled.debug[0].location, Some(DebugLocationId(7)));
        assert!(compiled.debug[0].pc_offset > compiled.entry_offset);
    }
}
