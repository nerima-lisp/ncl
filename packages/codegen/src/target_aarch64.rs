use crate::{
    Allocation, AllocationTarget, CodegenError, CompiledFunction, FrameLayout, Location,
    RuntimeAbi, SafepointMap, checked_u32,
};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::{Function, OpKind, Terminator, Ty};

#[path = "target_aarch64_lowering.rs"]
mod lowering;
use lowering::{load_value, lower_call, lower_op, move_args, store_value};

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
    allocation: &Allocation,
    position: u32,
    flags: u32,
) -> Result<(), CodegenError> {
    let slots = u16::try_from(frame.frame_words).map_err(|_| CodegenError::FrameOverflow)?;
    let mut live_slots = Vec::new();
    let mut registers = Vec::new();
    for (value, location) in &allocation.locations {
        let Some(interval) = allocation
            .intervals
            .iter()
            .find(|item| item.value == *value)
        else {
            continue;
        };
        if !(interval.start <= position
            && position <= interval.end
            && matches!(interval.ty, Ty::Word | Ty::Address))
        {
            continue;
        }
        match location {
            Location::Spill(spill) => live_slots.push(
                4u16.checked_add(u16::try_from(*spill).map_err(|_| CodegenError::FrameOverflow)?)
                    .ok_or(CodegenError::FrameOverflow)?,
            ),
            Location::Register(register) => registers.push(*register),
        }
    }
    live_slots.sort_unstable();
    live_slots.dedup();
    registers.sort_unstable();
    registers.dedup();
    SafepointMap::new(pc, slots, slots, &live_slots, &registers, flags)
        .map(|map| maps.push(map))
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

fn initialize_arguments(
    assembler: &mut Assembler,
    function: &Function,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let generated_lambda = function
        .params
        .first()
        .is_some_and(|parameter| parameter.name == "argc");
    for (index, _parameter) in function.params.iter().enumerate() {
        let value = ncl_ir::ValueId(u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?);
        let register_index = if generated_lambda {
            index
        } else {
            index.saturating_add(1)
        };
        if register_index < 5 {
            store_value(
                assembler,
                allocation,
                value,
                Reg(u8::try_from(register_index).map_err(|_| CodegenError::FrameOverflow)?),
            )?;
        } else {
            // `MemOperand::Unsigned` takes a byte offset (a multiple of
            // `scale`), not a word index, so the index into the rest-args
            // array pointed to by `x5` must be scaled by the word size.
            let rest_offset = (register_index - 5)
                .checked_mul(8)
                .and_then(|offset| u16::try_from(offset).ok())
                .ok_or(CodegenError::FrameOverflow)?;
            emit(
                assembler,
                Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unsigned {
                        base: RegOrSp::Reg(Reg(5)),
                        offset: rest_offset,
                        scale: 8,
                    },
                },
            )?;
            store_value(assembler, allocation, value, Reg(16))?;
        }
    }
    Ok(())
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
    let allocation = crate::allocate(function, AllocationTarget::AArch64);
    let frame = FrameLayout::new(0, allocation.spill_words, 0)?;
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
            rt: Reg(16),
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
    initialize_arguments(&mut assembler, function, &allocation)?;
    let mut position = 0u32;
    for block in &function.blocks {
        assembler
            .bind(labels[&block.id])
            .map_err(|error| CodegenError::Encode(error.to_string()))?;
        for op in &block.ops {
            let call_pc = lower_op(&mut assembler, op, function, &allocation, abi)?;
            if matches!(op.kind, OpKind::Alloc { .. }) {
                add_map(
                    &mut maps,
                    call_pc.unwrap_or(checked_u32(assembler.offset())?),
                    frame,
                    &allocation,
                    position,
                    FLAG_ALLOCATION_SLOW,
                )?;
            } else if matches!(
                op.kind,
                OpKind::Call { .. }
                    | OpKind::CallIndirect { .. }
                    | OpKind::MakeClosure { .. }
                    | OpKind::CallClosure { .. }
                    | OpKind::Builtin { .. }
                    | OpKind::Safepoint
                    | OpKind::EnterHandler { .. }
                    | OpKind::LeaveHandler { .. }
            ) {
                add_map(
                    &mut maps,
                    call_pc.unwrap_or(checked_u32(assembler.offset())?),
                    frame,
                    &allocation,
                    position,
                    FLAG_CALL,
                )?;
            }
            position = position.saturating_add(1);
        }
        match &block.terminator {
            Terminator::Jump { target, args } => {
                let destination = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *target)
                    .ok_or(CodegenError::UnknownBlock(*target))?;
                move_args(&mut assembler, &allocation, args, &destination.params)?;
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
                        &allocation,
                        position,
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
                load_value(&mut assembler, &allocation, *condition, Reg(5))?;
                let then_block = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *then_target)
                    .ok_or(CodegenError::UnknownBlock(*then_target))?;
                move_args(&mut assembler, &allocation, then_args, &then_block.params)?;
                emit(
                    &mut assembler,
                    Inst::Cbnz {
                        rt: Reg(5),
                        label: labels[then_target],
                    },
                )?;
                let else_block = function
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == *else_target)
                    .ok_or(CodegenError::UnknownBlock(*else_target))?;
                move_args(&mut assembler, &allocation, else_args, &else_block.params)?;
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
                if values.len() > ncl_sys::MULTIPLE_VALUE_AREA_WORDS {
                    return Err(CodegenError::MultipleValueAreaOverflow {
                        count: values.len(),
                        capacity: ncl_sys::MULTIPLE_VALUE_AREA_WORDS,
                    });
                }
                let area_offset = abi
                    .field_offset(crate::ContextField::MultipleValueArea)
                    .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
                for (index, value) in values.iter().copied().enumerate() {
                    let byte_offset = i32::try_from(index)
                        .ok()
                        .and_then(|index| index.checked_mul(8))
                        .and_then(|index| area_offset.checked_add(index))
                        .ok_or(CodegenError::FrameOverflow)?;
                    load_value(&mut assembler, &allocation, value, Reg(16))?;
                    emit(
                        &mut assembler,
                        Inst::Str {
                            rt: Reg(16),
                            mem: MemOperand::Unsigned {
                                base: RegOrSp::Reg(Reg(21)),
                                offset: u16::try_from(byte_offset)
                                    .map_err(|_| CodegenError::FrameOverflow)?,
                                scale: 8,
                            },
                        },
                    )?;
                }
                if let Some(value) = values.first() {
                    load_value(&mut assembler, &allocation, *value, Reg(0))?;
                } else {
                    for instruction in
                        ncl_asm_aarch64::mov_imm64(Reg(0), ncl_sys::Word::fixnum(0).bits())
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
            Terminator::CallReturn { function, args } => {
                lower_call(&mut assembler, *function, args, &allocation)?;
                emit(&mut assembler, Inst::Blr { rn: Reg(17) })?;
                add_map(
                    &mut maps,
                    checked_u32(assembler.offset())?,
                    frame,
                    &allocation,
                    position,
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
            Terminator::TailCall { function, args } => {
                lower_call(&mut assembler, *function, args, &allocation)?;
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
                emit(&mut assembler, Inst::Br { rn: Reg(17) })?;
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
