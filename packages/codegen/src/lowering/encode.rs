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
#[allow(clippy::expect_used, missing_docs)]
mod tests {
    use super::encode;
    use crate::{Block, CodegenError, FLAG_CALL, FrameLayout, MachineFunction, X86_64Abi};
    use ncl_asm_x86_64::{Assembler, Inst, Reg};
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

    fn function_with_blocks(blocks: Vec<BasicBlock>) -> Function {
        Function {
            id: FunctionId(2),
            name: "encode-terminators".into(),
            params: Vec::new(),
            return_types: Vec::new(),
            blocks,
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

    fn encoded(instructions: impl IntoIterator<Item = Inst>) -> Vec<u8> {
        let mut assembler = Assembler::new();
        for instruction in instructions {
            assembler.emit(&instruction).expect("instruction encoding");
        }
        assembler.bytes().to_vec()
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

    #[test]
    fn emits_branch_terminator_for_each_resolved_target() {
        let branch = function_with_blocks(vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Branch {
                    condition: ValueId(0),
                    then_target: BlockId(1),
                    else_target: BlockId(2),
                    then_args: Vec::new(),
                    else_args: Vec::new(),
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
            BasicBlock {
                id: BlockId(2),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
        ]);
        let compiled = encode(
            &branch,
            machine(
                vec![
                    Block::new(BlockId(0), Vec::new()),
                    Block::new(BlockId(1), Vec::new()),
                    Block::new(BlockId(2), Vec::new()),
                ],
                vec![(ValueId(0), 4)],
                5,
            ),
            &X86_64Abi,
        )
        .expect("branch encoded");
        let expected_compare = encoded([Inst::CmpRI(Reg::R10, 0)]);
        assert!(
            compiled
                .code
                .windows(expected_compare.len())
                .any(|bytes| bytes == expected_compare)
        );
        assert!(compiled.code.windows(2).any(|bytes| bytes == [0x0f, 0x85]));
        assert!(compiled.code.contains(&0xe9));
    }

    #[test]
    fn emits_switch_terminator_for_each_case_and_default_target() {
        let switch = function_with_blocks(vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Switch {
                    value: ValueId(0),
                    cases: vec![(7, BlockId(1), Vec::new()), (-2, BlockId(2), Vec::new())],
                    default: BlockId(3),
                    default_args: Vec::new(),
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
            BasicBlock {
                id: BlockId(2),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
            BasicBlock {
                id: BlockId(3),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
        ]);
        let compiled = encode(
            &switch,
            machine(
                vec![
                    Block::new(BlockId(0), Vec::new()),
                    Block::new(BlockId(1), Vec::new()),
                    Block::new(BlockId(2), Vec::new()),
                    Block::new(BlockId(3), Vec::new()),
                ],
                vec![(ValueId(0), 4)],
                5,
            ),
            &X86_64Abi,
        )
        .expect("switch encoded");
        for case in [7, -2] {
            let expected_compare = encoded([Inst::CmpRI(Reg::R10, case)]);
            assert!(
                compiled
                    .code
                    .windows(expected_compare.len())
                    .any(|bytes| bytes == expected_compare)
            );
        }
        assert!(
            compiled
                .code
                .windows(2)
                .filter(|bytes| *bytes == [0x0f, 0x84])
                .count()
                >= 2
        );
        assert!(compiled.code.contains(&0xe9));
    }

    #[test]
    fn emits_non_backedge_jump_terminator() {
        let jump = function_with_blocks(vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Jump {
                    target: BlockId(1),
                    args: Vec::new(),
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: Vec::new(),
                ops: Vec::new(),
                terminator: Terminator::Return { values: Vec::new() },
            },
        ]);
        let compiled = encode(
            &jump,
            machine(
                vec![
                    Block::new(BlockId(0), Vec::new()),
                    Block::new(BlockId(1), Vec::new()),
                ],
                Vec::new(),
                4,
            ),
            &X86_64Abi,
        )
        .expect("non-backedge jump encoded");
        assert!(compiled.code.contains(&0xe9));
    }

    #[test]
    fn emits_call_return_and_tail_call_safepoints() {
        for terminator in [
            Terminator::CallReturn {
                function: ValueId(0),
                args: Vec::new(),
            },
            Terminator::TailCall {
                function: ValueId(0),
                args: Vec::new(),
            },
        ] {
            let source = function(terminator, Vec::new());
            let compiled = encode(
                &source,
                machine(
                    vec![Block::new(BlockId(0), Vec::new())],
                    vec![(ValueId(0), 4)],
                    5,
                ),
                &X86_64Abi,
            )
            .expect("call terminator encoded");
            assert!(
                compiled
                    .code
                    .windows(3)
                    .any(|bytes| bytes == [0x41, 0xff, 0xd3])
            );
            assert_eq!(compiled.safepoint_maps.len(), 1);
            assert_eq!(compiled.safepoint_maps[0].map_flags, FLAG_CALL);
        }
    }

    #[test]
    fn emits_unreachable_and_rejects_throw_terminators() {
        let unreachable = function(Terminator::Unreachable, Vec::new());
        let compiled = encode(
            &unreachable,
            machine(vec![Block::new(BlockId(0), Vec::new())], Vec::new(), 4),
            &X86_64Abi,
        )
        .expect("unreachable encoded");
        assert!(compiled.code.ends_with(&[0x0f, 0x0b]));

        let throw = function(
            Terminator::Throw {
                condition: ValueId(0),
            },
            Vec::new(),
        );
        assert_eq!(
            encode(
                &throw,
                machine(
                    vec![Block::new(BlockId(0), Vec::new())],
                    vec![(ValueId(0), 4)],
                    5,
                ),
                &X86_64Abi,
            ),
            Err(CodegenError::NonLocalExitUnsupported)
        );
    }
}
