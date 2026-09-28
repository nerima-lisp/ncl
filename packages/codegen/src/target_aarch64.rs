use crate::{
    AllocationTarget, CodegenError, CompiledFunction, FrameLayout, RuntimeAbi, checked_u32,
};
use crate::{FLAG_ALLOCATION_SLOW, FLAG_CALL, FLAG_LOOP_BACKEDGE};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::{Function, OpKind, Terminator};

#[path = "target_aarch64_layout.rs"]
mod layout;
use layout::outgoing_words;
#[path = "target_aarch64_support.rs"]
mod support;
use support::{add_map, initialize_arguments};

#[path = "target_aarch64_lowering.rs"]
mod lowering;
use lowering::{
    load_value, lower_call, lower_op, lower_pending_check, lower_return_or_throw, move_args,
};

#[allow(clippy::needless_pass_by_value)]
fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
    assembler
        .emit(&instruction)
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
    let mut allocation = crate::allocate(function, AllocationTarget::AArch64);
    let outgoing_words = outgoing_words(function)?;
    let generated_lambda = function
        .params
        .first()
        .is_some_and(|parameter| parameter.name == "argc");
    let incoming_words = if generated_lambda { 5 } else { 0 };
    allocation.incoming_args_base = generated_lambda.then_some(allocation.spill_words);
    allocation.outgoing_base = allocation
        .spill_words
        .checked_add(incoming_words)
        .ok_or(CodegenError::FrameOverflow)?;
    let frame = FrameLayout::new(
        0,
        allocation
            .spill_words
            .checked_add(incoming_words)
            .ok_or(CodegenError::FrameOverflow)?,
        outgoing_words,
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
        if body_bytes <= 4095 {
            emit(
                &mut assembler,
                Inst::SubImm {
                    rd: RegOrSp::Sp,
                    rn: RegOrSp::Sp,
                    imm: u16::try_from(body_bytes).map_err(|_| CodegenError::FrameOverflow)?,
                    shift: false,
                },
            )?;
        } else {
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(body_bytes)) {
                emit(&mut assembler, instruction)?;
            }
            emit(
                &mut assembler,
                Inst::Sub {
                    rd: RegOrSp::Sp,
                    rn: RegOrSp::Sp,
                    rm: Reg(16),
                    shift: ncl_asm_aarch64::Shift::Lsl(0),
                },
            )?;
        }
    }
    if let Some(base) = allocation.incoming_args_base {
        let offset = base
            .checked_add(1)
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?;
        for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(offset)) {
            emit(&mut assembler, instruction)?;
        }
        emit(
            &mut assembler,
            Inst::Sub {
                rd: RegOrSp::Reg(Reg(16)),
                rn: RegOrSp::Reg(Reg(29)),
                rm: Reg(16),
                shift: ncl_asm_aarch64::Shift::Lsl(0),
            },
        )?;
        // check-added-lines: allow(index) fixed five-register ABI table.
        for (index, register) in [Reg(1), Reg(2), Reg(3), Reg(4), Reg(5)]
            .into_iter()
            .enumerate()
        {
            emit(
                &mut assembler,
                Inst::Str {
                    rt: register,
                    mem: ncl_asm_aarch64::MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset: lowering::spill_offset(index)?,
                    },
                },
            )?;
        }
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
                if matches!(
                    op.kind,
                    OpKind::Call { .. }
                        | OpKind::CallIndirect { .. }
                        | OpKind::CallClosure { .. }
                        | OpKind::MakeClosure { .. }
                        | OpKind::Builtin { .. }
                ) {
                    // A callee may be propagating a non-local exit (for
                    // example a closure crossed by `return-from`/`throw`)
                    // rather than returning normally; see
                    // `lowering::dispatch::lower_pending_check`.
                    lower_pending_check(
                        &mut assembler,
                        function,
                        block.id,
                        &allocation,
                        abi,
                        body_bytes,
                        &labels,
                    )?;
                }
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
                lower_return_or_throw(
                    &mut assembler,
                    function,
                    block.id,
                    Some(values),
                    &allocation,
                    abi,
                    body_bytes,
                    &labels,
                )?;
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
                    if body_bytes <= 4095 {
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
                    } else {
                        for instruction in
                            ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(body_bytes))
                        {
                            emit(&mut assembler, instruction)?;
                        }
                        emit(
                            &mut assembler,
                            Inst::Add {
                                rd: RegOrSp::Sp,
                                rn: RegOrSp::Sp,
                                rm: Reg(16),
                                shift: ncl_asm_aarch64::Shift::Lsl(0),
                            },
                        )?;
                    }
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
                    if body_bytes <= 4095 {
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
                    } else {
                        for instruction in
                            ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(body_bytes))
                        {
                            emit(&mut assembler, instruction)?;
                        }
                        emit(
                            &mut assembler,
                            Inst::Add {
                                rd: RegOrSp::Sp,
                                rn: RegOrSp::Sp,
                                rm: Reg(16),
                                shift: ncl_asm_aarch64::Shift::Lsl(0),
                            },
                        )?;
                    }
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
            Terminator::Throw { .. } => {
                lower_return_or_throw(
                    &mut assembler,
                    function,
                    block.id,
                    None,
                    &allocation,
                    abi,
                    body_bytes,
                    &labels,
                )?;
            }
            Terminator::Unreachable => {
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
