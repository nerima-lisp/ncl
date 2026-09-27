use crate::{
    Allocation, CodegenError, ContextField, Location, RuntimeAbi, RuntimeFunction,
    common_lisp_builtin,
};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Constant, ConstantIndex, ValueId};

pub(super) use primitives::{ClosureLayout, closure_layout};

pub(super) fn constant_table_entry(
    constants: &[Constant],
    index: ConstantIndex,
) -> Result<&Constant, CodegenError> {
    let raw_index = index.0;
    let index = usize::try_from(raw_index).map_err(|_| CodegenError::InvalidConstantIndex {
        index: raw_index,
        length: constants.len(),
    })?;
    constants
        .get(index)
        .ok_or(CodegenError::InvalidConstantIndex {
            index: raw_index,
            length: constants.len(),
        })
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
    assembler
        .emit(&instruction)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

fn spill_offset(allocation: &Allocation, value: ValueId) -> Result<u16, CodegenError> {
    let Location::Spill(index) = allocation
        .location(value)
        .ok_or(CodegenError::UnknownValue(value))?
    else {
        return Err(CodegenError::Unsupported(
            "register value has no spill slot".into(),
        ));
    };
    u16::try_from((index.saturating_add(1)).saturating_mul(8))
        .map_err(|_| CodegenError::FrameOverflow)
}

pub(super) fn load_value(
    assembler: &mut Assembler,
    allocation: &Allocation,
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    match allocation
        .location(value)
        .ok_or(CodegenError::UnknownValue(value))?
    {
        Location::Register(source) => emit(
            assembler,
            Inst::Mov {
                rd: RegOrSp::Reg(register),
                rn: RegOrSp::Reg(Reg(
                    u8::try_from(source).map_err(|_| CodegenError::FrameOverflow)?
                )),
            },
        ),
        Location::Spill(_) => {
            let offset = spill_offset(allocation, value)?;
            if offset <= 4095 {
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(register),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: offset,
                        shift: false,
                    },
                )?;
            } else {
                let offset_register = if register == Reg(17) {
                    Reg(16)
                } else {
                    Reg(17)
                };
                for instruction in ncl_asm_aarch64::mov_imm64(offset_register, u64::from(offset)) {
                    emit(assembler, instruction)?;
                }
                emit(
                    assembler,
                    Inst::Sub {
                        rd: RegOrSp::Reg(register),
                        rn: RegOrSp::Reg(Reg(29)),
                        rm: offset_register,
                        shift: ncl_asm_aarch64::Shift::Lsl(0),
                    },
                )?;
            }
            emit(
                assembler,
                Inst::Ldr {
                    rt: register,
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(register),
                        offset: 0,
                    },
                },
            )
        }
    }
}

pub(super) fn store_value(
    assembler: &mut Assembler,
    allocation: &Allocation,
    value: ValueId,
    register: Reg,
) -> Result<(), CodegenError> {
    match allocation
        .location(value)
        .ok_or(CodegenError::UnknownValue(value))?
    {
        Location::Register(destination) => emit(
            assembler,
            Inst::Mov {
                rd: RegOrSp::Reg(Reg(
                    u8::try_from(destination).map_err(|_| CodegenError::FrameOverflow)?
                )),
                rn: RegOrSp::Reg(register),
            },
        ),
        Location::Spill(_) => {
            let offset = spill_offset(allocation, value)?;
            let source = if register == Reg(16) {
                emit(
                    assembler,
                    Inst::Mov {
                        rd: RegOrSp::Reg(Reg(17)),
                        rn: RegOrSp::Reg(Reg(16)),
                    },
                )?;
                Reg(17)
            } else {
                register
            };
            if offset <= 4095 {
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(Reg(16)),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: offset,
                        shift: false,
                    },
                )?;
            } else {
                for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(offset)) {
                    emit(assembler, instruction)?;
                }
                emit(
                    assembler,
                    Inst::Sub {
                        rd: RegOrSp::Reg(Reg(16)),
                        rn: RegOrSp::Reg(Reg(29)),
                        rm: Reg(16),
                        shift: ncl_asm_aarch64::Shift::Lsl(0),
                    },
                )?;
            }
            emit(
                assembler,
                Inst::Str {
                    rt: source,
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(16)),
                        offset: 0,
                    },
                },
            )
        }
    }
}

pub(super) fn lower_runtime_builtin(
    assembler: &mut Assembler,
    function: RuntimeFunction,
    immediate_args: &[u64],
    value_args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if immediate_args.len() + value_args.len() > 4 {
        return Err(CodegenError::Unsupported(
            "AArch64 runtime calls support at most four arguments".into(),
        ));
    }
    let address = abi
        .runtime_address(function)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), address) {
        emit(assembler, instruction)?;
    }
    for (index, value) in immediate_args.iter().copied().enumerate() {
        for instruction in ncl_asm_aarch64::mov_imm64(
            Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?),
            value,
        ) {
            emit(assembler, instruction)?;
        }
    }
    for (index, value) in value_args.iter().copied().enumerate() {
        load_value(
            assembler,
            allocation,
            value,
            Reg(u8::try_from(immediate_args.len() + index + 1)
                .map_err(|_| CodegenError::FrameOverflow)?),
        )?;
    }
    Ok(())
}

fn context_mem(abi: &dyn RuntimeAbi, field: ContextField) -> Result<MemOperand, CodegenError> {
    let offset = abi
        .field_offset(field)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    let offset = u16::try_from(offset).map_err(|_| CodegenError::FrameOverflow)?;
    Ok(MemOperand::Unsigned {
        base: RegOrSp::Reg(Reg(21)),
        offset,
        scale: 8,
    })
}

fn runtime_address(abi: &dyn RuntimeAbi, function: RuntimeFunction) -> Result<u64, CodegenError> {
    abi.runtime_address(function)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))
}

fn lower_alloc(
    assembler: &mut Assembler,
    words: u32,
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<u32, CodegenError> {
    let bytes = words
        .checked_mul(8)
        .and_then(|value| u16::try_from(value).ok())
        .ok_or(CodegenError::FrameOverflow)?;
    let slow = assembler.new_label();
    let done = assembler.new_label();
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: context_mem(abi, ContextField::TlabBump)?,
        },
    )?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(17),
            mem: context_mem(abi, ContextField::TlabLimit)?,
        },
    )?;
    emit(
        assembler,
        Inst::AddImm {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(16)),
            imm: bytes,
            shift: false,
        },
    )?;
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(0),
            rm: Reg(17),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::BCond {
            cond: Cond::Hi,
            label: slow,
        },
    )?;
    emit(
        assembler,
        Inst::Str {
            rt: Reg(0),
            mem: context_mem(abi, ContextField::TlabBump)?,
        },
    )?;
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    emit(assembler, Inst::B { label: done })?;
    assembler
        .bind(slow)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(1), u64::from(words)) {
        emit(assembler, instruction)?;
    }
    for instruction in ncl_asm_aarch64::mov_imm64(
        Reg(17),
        runtime_address(abi, RuntimeFunction::AllocateSlow)?,
    ) {
        emit(assembler, instruction)?;
    }
    emit(assembler, Inst::Blr { rn: Reg(17) })?;
    let call_pc = u32::try_from(assembler.offset()).map_err(|_| CodegenError::FrameOverflow)?;
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    assembler
        .bind(done)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    Ok(call_pc)
}

fn lower_safepoint(assembler: &mut Assembler, abi: &dyn RuntimeAbi) -> Result<u32, CodegenError> {
    let done = assembler.new_label();
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: context_mem(abi, ContextField::SafepointRequest)?,
        },
    )?;
    emit(
        assembler,
        Inst::Cbz {
            rt: Reg(16),
            label: done,
        },
    )?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(1)),
            rn: RegOrSp::Reg(Reg(29)),
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(
        Reg(17),
        runtime_address(abi, RuntimeFunction::SafepointSlow)?,
    ) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Adr {
            rd: Reg(2),
            label: done,
        },
    )?;
    emit(assembler, Inst::Blr { rn: Reg(17) })?;
    let call_pc = u32::try_from(assembler.offset()).map_err(|_| CodegenError::FrameOverflow)?;
    assembler
        .bind(done)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    Ok(call_pc)
}

fn lower_builtin(
    assembler: &mut Assembler,
    name: &str,
    args: &[ValueId],
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    if args.len() > 4 {
        return Err(CodegenError::Unsupported(
            "AArch64 builtins support at most four arguments".into(),
        ));
    }
    if name == "make-rest-list" {
        // check-added-lines: allow(index) intentional
        let [argc_value, start_value] = args else {
            // check-added-lines: allow(unsupported) intentional
            return Err(CodegenError::Unsupported(
                "make-rest-list requires argc and start".into(), // check-added-lines: allow(unsupported) intentional
            ));
        };
        emit(
            assembler,
            Inst::Mov {
                rd: RegOrSp::Reg(Reg(0)),
                rn: RegOrSp::Reg(Reg(21)),
            },
        )?;
        if let Some(base) = allocation.incoming_args_base {
            let offset = base
                .checked_add(1)
                .and_then(|slot| slot.checked_mul(8))
                .ok_or(CodegenError::FrameOverflow)?;
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), u64::from(offset)) {
                emit(assembler, instruction)?;
            }
            emit(
                assembler,
                Inst::Sub {
                    rd: RegOrSp::Reg(Reg(16)),
                    rn: RegOrSp::Reg(Reg(29)),
                    rm: Reg(16),
                    shift: Shift::Lsl(0),
                },
            )?;
            for (index, register) in [Reg(1), Reg(2), Reg(3), Reg(4), Reg(5)]
                .into_iter()
                .enumerate()
            {
                emit(
                    assembler,
                    Inst::Ldr {
                        rt: register,
                        mem: MemOperand::Unscaled {
                            base: RegOrSp::Reg(Reg(16)),
                            offset: i16::try_from(index.saturating_mul(8))
                                .map_err(|_| CodegenError::FrameOverflow)?,
                        },
                    },
                )?;
            }
        }
        load_value(assembler, allocation, *argc_value, Reg(6))?;
        load_value(assembler, allocation, *start_value, Reg(7))?;
        for instruction in ncl_asm_aarch64::mov_imm64(
            Reg(17),
            abi.builtin_address(common_lisp_builtin(name))
                .map_err(|error| CodegenError::Unsupported(error.to_string()))?, // check-added-lines: allow(unsupported) ABI address lookup failure.
        ) {
            emit(assembler, instruction)?;
        }
        return Ok(());
    }
    let address = abi
        .builtin_address(common_lisp_builtin(name))
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(0)),
            rn: RegOrSp::Reg(Reg(21)),
        },
    )?;
    for (index, argument) in args.iter().enumerate() {
        let register = Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?);
        load_value(assembler, allocation, *argument, register)?;
    }
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), address) {
        emit(assembler, instruction)?;
    }
    Ok(())
}

#[path = "target_aarch64_lowering/calls.rs"]
pub(super) mod calls;
#[path = "target_aarch64_lowering/dispatch.rs"]
pub(super) mod dispatch;
#[path = "target_aarch64_lowering/ops.rs"]
pub(super) mod ops;
#[path = "target_aarch64_lowering/primitives.rs"]
pub(super) mod primitives;
pub(super) use calls::{lower_call, lower_closure_call};
pub(super) use dispatch::{lower_pending_check, lower_return_or_throw};
pub(super) use ops::{lower_op, move_args};
