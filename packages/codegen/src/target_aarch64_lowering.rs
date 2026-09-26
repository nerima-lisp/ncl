use crate::{
    Allocation, CodegenError, ContextField, Location, RuntimeAbi, RuntimeFunction,
    common_lisp_builtin,
};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{Constant, ConstantIndex, ValueId};

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
fn emit(assembler: &mut Assembler, instruction: Inst) -> Result<(), CodegenError> {
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
            emit(
                assembler,
                Inst::SubImm {
                    rd: RegOrSp::Reg(register),
                    rn: RegOrSp::Reg(Reg(29)),
                    imm: offset,
                    shift: false,
                },
            )?;
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
            emit(
                assembler,
                Inst::SubImm {
                    rd: RegOrSp::Reg(Reg(16)),
                    rn: RegOrSp::Reg(Reg(29)),
                    imm: offset,
                    shift: false,
                },
            )?;
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

pub(super) fn lower_call(
    assembler: &mut Assembler,
    callee: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let Some((argc, arguments)) = args.split_first() else {
        // check-added-lines: allow(unsupported) malformed IR lacks the required argc value.
        return Err(CodegenError::Unsupported(
            "calls require a tagged argc argument".into(),
        ));
    };
    if arguments.len() > 4 {
        return Err(CodegenError::Unsupported(
            "AArch64 calls support at most four register arguments".into(),
        ));
    }
    load_value(assembler, allocation, callee, Reg(16))?;
    emit(
        assembler,
        Inst::Mov {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(16)),
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(17),
            rn: Reg(17),
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )?;
    load_value(assembler, allocation, *argc, Reg(0))?;
    for (index, argument) in arguments.iter().enumerate() {
        let register = Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?);
        load_value(assembler, allocation, *argument, register)?;
    }
    Ok(())
}

pub(super) fn lower_closure_call(
    assembler: &mut Assembler,
    closure: ValueId,
    args: &[ValueId],
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    lower_call(assembler, closure, args, allocation)?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(17),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(17)),
                offset: i16::try_from((ncl_object::function_offset::ENTRY + 1) * 8)
                    .map_err(|_| CodegenError::FrameOverflow)?,
            },
        },
    )?;
    emit(
        assembler,
        Inst::AsrImm {
            rd: Reg(17),
            rn: Reg(17),
            amount: u8::try_from(ncl_sys::FIXNUM_TAG_BITS)
                .map_err(|_| CodegenError::FrameOverflow)?,
        },
    )
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
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), address) {
        emit(assembler, instruction)?;
    }
    for (index, argument) in args.iter().enumerate() {
        let register = Reg(u8::try_from(index + 1).map_err(|_| CodegenError::FrameOverflow)?);
        load_value(assembler, allocation, *argument, register)?;
    }
    Ok(())
}

#[path = "target_aarch64_lowering/ops.rs"]
pub(super) mod ops;
#[path = "target_aarch64_lowering/primitives.rs"]
pub(super) mod primitives;
pub(super) use ops::{lower_op, move_args};
