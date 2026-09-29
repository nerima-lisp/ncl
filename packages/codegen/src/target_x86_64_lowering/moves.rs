use super::{ENTRY, ValueSlots, emit, load_slot, slot_mem_of, store_slot};
use crate::{CodegenError, Location};
use ncl_asm_x86_64::{Assembler, Inst};
use ncl_ir::{BlockParam, ValueId};

pub fn move_args(
    assembler: &mut Assembler,
    slots: &ValueSlots,
    args: &[ValueId],
    params: &[BlockParam],
) -> Result<(), CodegenError> {
    if args.len() != params.len() {
        #[rustfmt::skip]
        return Err(CodegenError::Unsupported( // check-added-lines: allow(unsupported) explicit block-arity lowering error
            // check-added-lines: allow(unsupported) block arity is an explicit lowering error
            "block argument arity mismatch".into(), // check-added-lines: allow(unsupported) explicit block-arity lowering error
        ));
    }
    let mut moves = Vec::new();
    for (argument, parameter) in args.iter().zip(params) {
        let source = slots.location(*argument)?;
        let destination = slots.location(parameter.value)?;
        if source != destination {
            moves.push((source, destination));
        }
    }
    let overlapping = moves
        .iter()
        .any(|(source, _)| moves.iter().any(|(_, destination)| destination == source));
    if overlapping {
        for (source, _) in &moves {
            match source {
                Location::Register(register) => emit(
                    assembler,
                    Inst::MovRR(
                        ENTRY,
                        ncl_asm_x86_64::Reg::from_id(
                            u8::try_from(*register).map_err(|_| CodegenError::FrameOverflow)?,
                        )
                        .ok_or(CodegenError::FrameOverflow)?,
                    ),
                )?,
                Location::Spill(spill) => emit(
                    assembler,
                    Inst::MovRM(
                        ENTRY,
                        slot_mem_of(
                            slots
                                .spill_base
                                .checked_add(*spill)
                                .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                    ),
                )?,
            }
            emit(assembler, Inst::Push(ENTRY))?;
        }
        for (_, destination) in moves.iter().rev() {
            emit(assembler, Inst::Pop(ENTRY))?;
            match destination {
                Location::Register(register) => emit(
                    assembler,
                    Inst::MovRR(
                        ncl_asm_x86_64::Reg::from_id(
                            u8::try_from(*register).map_err(|_| CodegenError::FrameOverflow)?,
                        )
                        .ok_or(CodegenError::FrameOverflow)?,
                        ENTRY,
                    ),
                )?,
                Location::Spill(spill) => emit(
                    assembler,
                    Inst::MovMR(
                        slot_mem_of(
                            slots
                                .spill_base
                                .checked_add(*spill)
                                .ok_or(CodegenError::FrameOverflow)?,
                        )?,
                        ENTRY,
                    ),
                )?,
            }
        }
        return Ok(());
    }
    for (argument, parameter) in args.iter().zip(params) {
        if slots.location(*argument)? != slots.location(parameter.value)? {
            load_slot(assembler, slots, *argument, ENTRY)?;
            store_slot(assembler, slots, parameter.value, ENTRY)?;
        }
    }
    Ok(())
}
