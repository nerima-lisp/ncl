use crate::{CodegenError, CompiledFunction, FrameLayout, RuntimeAbi, SafepointMap, checked_u32};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::{Function, OpKind, Terminator};

#[path = "target_aarch64_lowering.rs"]
mod lowering;
use lowering::{load_slot, lower_call, lower_op, move_args, slots};

#[allow(clippy::needless_pass_by_value)]
fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
    assembler
        .emit(&instruction)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

fn add_map(
    maps: &mut Vec<SafepointMap>,
    pc: u32,
    frame: FrameLayout,
    flags: u32,
) -> Result<(), CodegenError> {
    let slots = u16::try_from(frame.frame_words).map_err(|_| CodegenError::FrameOverflow)?;
    SafepointMap::new(pc, slots, slots, &[], &[0], flags)
        .map(|map| maps.push(map))
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

/// Lowers an IR function to `AArch64` machine code using the native frame ABI.
///
/// # Errors
///
/// Returns [`CodegenError`] when the function cannot be represented by the
/// fixed frame and instruction templates.
#[allow(clippy::too_many_lines)]
pub fn compile_function_aarch64(
    function: &Function,
    abi: &dyn RuntimeAbi,
) -> Result<CompiledFunction, CodegenError> {
    let Some(_entry) = function.blocks.first() else {
        return Err(CodegenError::EmptyFunction);
    };
    let (value_slots, local_words) = slots(function);
    let frame = FrameLayout::new(
        u32::try_from(function.params.len()).map_err(|_| CodegenError::FrameOverflow)?,
        local_words,
        0,
    )?;
    let mut assembler = Assembler::new();
    let labels = function
        .blocks
        .iter()
        .map(|block| (block.id, assembler.new_label()))
        .collect::<std::collections::HashMap<_, _>>();
    let mut maps = Vec::new();
    emit(
        &mut assembler,
        Inst::Stp {
            rt: Reg(29),
            rt2: Reg(30),
            mem: MemOperand::PreIndex {
                base: RegOrSp::Sp,
                offset: -32,
            },
        },
    )?;
    emit(
        &mut assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(29)),
            rn: RegOrSp::Sp,
        },
    )?;
    emit(
        &mut assembler,
        Inst::Str {
            rt: Reg(17),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(29)),
                offset: 16,
            },
        },
    )?;
    emit(
        &mut assembler,
        Inst::MovZ {
            rd: Reg(16),
            imm: 0,
            shift: 0,
        },
    )?;
    emit(
        &mut assembler,
        Inst::Str {
            rt: Reg(16),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(29)),
                offset: 24,
            },
        },
    )?;
    let body_bytes = frame.size_bytes().saturating_sub(32);
    if body_bytes > 0 {
        emit(
            &mut assembler,
            Inst::SubImm {
                rd: RegOrSp::Sp,
                rn: RegOrSp::Sp,
                imm: u16::try_from(body_bytes).map_err(|_| CodegenError::FrameOverflow)?,
                shift: false,
            },
        )?;
    }
    for block in &function.blocks {
        assembler
            .bind(labels[&block.id])
            .map_err(|error| CodegenError::Encode(error.to_string()))?;
        for op in &block.ops {
            lower_op(&mut assembler, op, function, &value_slots, abi)?;
            let pc = checked_u32(assembler.offset())?;
            if matches!(op.kind, OpKind::Alloc { .. }) {
                add_map(&mut maps, pc, frame, FLAG_ALLOCATION_SLOW)?;
            } else if matches!(
                op.kind,
                OpKind::Call { .. }
                    | OpKind::CallIndirect { .. }
                    | OpKind::Builtin { .. }
                    | OpKind::Safepoint
            ) {
                add_map(&mut maps, pc, frame, FLAG_CALL)?;
            }
        }
        match &block.terminator {
            Terminator::Jump { target, args } => {
                let destination = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *target)
                    .ok_or(CodegenError::UnknownBlock(*target))?;
                move_args(&mut assembler, &value_slots, args, &destination.params)?;
                emit(
                    &mut assembler,
                    Inst::B {
                        label: labels[target],
                    },
                )?;
                if *target == block.id {
                    add_map(
                        &mut maps,
                        checked_u32(assembler.offset())?,
                        frame,
                        FLAG_LOOP_BACKEDGE,
                    )?;
                }
            }
            Terminator::Branch {
                condition,
                then_target,
                then_args,
                else_target,
                else_args,
            } => {
                load_slot(&mut assembler, &value_slots, *condition, Reg(16))?;
                let then_block = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *then_target)
                    .ok_or(CodegenError::UnknownBlock(*then_target))?;
                move_args(&mut assembler, &value_slots, then_args, &then_block.params)?;
                emit(
                    &mut assembler,
                    Inst::Cbnz {
                        rt: Reg(16),
                        label: labels[then_target],
                    },
                )?;
                let else_block = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *else_target)
                    .ok_or(CodegenError::UnknownBlock(*else_target))?;
                move_args(&mut assembler, &value_slots, else_args, &else_block.params)?;
                emit(
                    &mut assembler,
                    Inst::B {
                        label: labels[else_target],
                    },
                )?;
            }
            Terminator::Switch { default, .. } => {
                emit(
                    &mut assembler,
                    Inst::B {
                        label: labels[default],
                    },
                )?;
            }
            Terminator::Return { values } => {
                if let Some(value) = values.first() {
                    load_slot(&mut assembler, &value_slots, *value, Reg(0))?;
                } else {
                    for instruction in
                        ncl_asm_aarch64::mov_imm64(Reg(0), abi.encode_fixnum(0).cast_unsigned())
                    {
                        emit(&mut assembler, instruction)?;
                    }
                }
                for instruction in ncl_asm_aarch64::mov_imm64(
                    Reg(1),
                    u64::try_from(values.len()).map_err(|_| CodegenError::FrameOverflow)?,
                ) {
                    emit(&mut assembler, instruction)?;
                }
                if body_bytes > 0 {
                    emit(
                        &mut assembler,
                        Inst::AddImm {
                            rd: RegOrSp::Sp,
                            rn: RegOrSp::Sp,
                            imm: u16::try_from(body_bytes)
                                .map_err(|_| CodegenError::FrameOverflow)?,
                            shift: false,
                        },
                    )?;
                }
                emit(
                    &mut assembler,
                    Inst::Ldp {
                        rt: Reg(29),
                        rt2: Reg(30),
                        mem: MemOperand::PostIndex {
                            base: RegOrSp::Sp,
                            offset: 32,
                        },
                    },
                )?;
                emit(&mut assembler, Inst::Ret { rn: Reg(30) })?;
            }
            Terminator::CallReturn { function, args } | Terminator::TailCall { function, args } => {
                lower_call(&mut assembler, *function, args, &value_slots)?;
                emit(&mut assembler, Inst::Blr { rn: Reg(17) })?;
                add_map(
                    &mut maps,
                    checked_u32(assembler.offset())?,
                    frame,
                    FLAG_CALL,
                )?;
                if body_bytes > 0 {
                    emit(
                        &mut assembler,
                        Inst::AddImm {
                            rd: RegOrSp::Sp,
                            rn: RegOrSp::Sp,
                            imm: u16::try_from(body_bytes)
                                .map_err(|_| CodegenError::FrameOverflow)?,
                            shift: false,
                        },
                    )?;
                }
                emit(
                    &mut assembler,
                    Inst::Ldp {
                        rt: Reg(29),
                        rt2: Reg(30),
                        mem: MemOperand::PostIndex {
                            base: RegOrSp::Sp,
                            offset: 32,
                        },
                    },
                )?;
                emit(&mut assembler, Inst::Ret { rn: Reg(30) })?;
            }
            Terminator::Throw { .. } | Terminator::Unreachable => {
                emit(&mut assembler, Inst::Brk { imm: 0 })?;
            }
        }
    }
    let blob = assembler
        .finish()
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    Ok(CompiledFunction {
        code: blob.bytes,
        entry_offset: 0,
        relocations: Vec::new(),
        safepoint_maps: maps,
        frame_size: frame.size_bytes(),
        debug: Vec::new(),
    })
}
