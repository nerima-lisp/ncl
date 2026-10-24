use super::{
    CallCheckContext, CheckedCallMode, emit, emit_safepoint_call, lower_builtin, store_value,
};
use crate::machine_verify::SafepointSite;
use crate::{Allocation, CodegenError, Location, RuntimeAbi};
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{FixnumOp, FixnumSpecialization, ValueId};

#[derive(Clone, Copy)]
pub(super) struct CheckedFixnumBuiltin<'a> {
    pub(super) name: &'a str,
    pub(super) args: &'a [ValueId],
    pub(super) specialization: FixnumSpecialization,
}

pub(super) fn lower_checked_fixnum_builtin_op(
    assembler: &mut Assembler,
    builtin: CheckedFixnumBuiltin<'_>,
    result: Option<ValueId>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let CheckedFixnumBuiltin {
        name,
        args,
        specialization,
    } = builtin;
    if ncl_ir::FixnumOp::from_builtin(name, args.len()) != Some(specialization.operation()) {
        // check-added-lines: allow(unsupported) reject mismatched metadata if the IR verifier is bypassed.
        return Err(CodegenError::Unsupported(
            "fixnum specialization does not match its builtin".into(),
        ));
    }

    let slow = assembler.new_label();
    let done = assembler.new_label();
    load_fixnum_operands_and_check_tags(assembler, args, specialization, slow, allocation)?;
    emit_fixnum_fast_operation(assembler, specialization.operation(), slow)?;

    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    emit(assembler, Inst::B { label: done })?;

    assembler
        .bind(slow)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    lower_builtin(assembler, name, args, allocation, abi)?;
    let call_pc = emit_safepoint_call(assembler)?;
    if let Some(result) = result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    assembler
        .bind(done)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    super::super::dispatch::lower_checked_call_check(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
    )?;
    Ok(call_pc)
}

fn load_fixnum_operands_and_check_tags(
    assembler: &mut Assembler,
    args: &[ValueId],
    specialization: FixnumSpecialization,
    slow: ncl_asm_aarch64::Label,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    for (index, argument) in args.iter().copied().enumerate() {
        let register = if index == 0 { Reg(16) } else { Reg(17) };
        load_fixnum_operand(assembler, allocation, argument, register)?;
        if specialization.needs_tag_check(index) {
            emit(
                assembler,
                Inst::TstImm {
                    rn: register,
                    imm: 1,
                },
            )?;
            emit(
                assembler,
                Inst::BCond {
                    cond: Cond::Ne,
                    label: slow,
                },
            )?;
        }
    }
    Ok(())
}

fn emit_fixnum_fast_operation(
    assembler: &mut Assembler,
    operation: FixnumOp,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    match operation {
        FixnumOp::Add => {
            emit(
                assembler,
                Inst::Adds {
                    rd: Reg(16),
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            branch_on_fixnum_overflow(assembler, slow)?;
        }
        FixnumOp::Sub => {
            emit(
                assembler,
                Inst::Subs {
                    rd: Reg(16),
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            branch_on_fixnum_overflow(assembler, slow)?;
        }
        FixnumOp::Mul => {
            lower_checked_fixnum_multiply(assembler, slow)?;
        }
        FixnumOp::LessThan => {
            emit(
                assembler,
                Inst::Cmp {
                    rn: Reg(16),
                    rm: Reg(17),
                    shift: Shift::Lsl(0),
                },
            )?;
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(16), ncl_sys::Word::TRUE.bits()) {
                emit(assembler, instruction)?;
            }
            for instruction in ncl_asm_aarch64::mov_imm64(Reg(17), ncl_sys::Word::NIL.bits()) {
                emit(assembler, instruction)?;
            }
            emit(
                assembler,
                Inst::Csel {
                    rd: Reg(16),
                    rn: Reg(16),
                    rm: Reg(17),
                    cond: Cond::Lt,
                },
            )?;
        }
        FixnumOp::OnePlus => {
            emit(
                assembler,
                Inst::AddsImm {
                    rd: RegOrSp::Reg(Reg(16)),
                    rn: RegOrSp::Reg(Reg(16)),
                    imm: 2,
                    shift: false,
                },
            )?;
            branch_on_fixnum_overflow(assembler, slow)?;
        }
        FixnumOp::OneMinus => {
            emit(
                assembler,
                Inst::SubsImm {
                    rd: RegOrSp::Reg(Reg(16)),
                    rn: RegOrSp::Reg(Reg(16)),
                    imm: 2,
                    shift: false,
                },
            )?;
            branch_on_fixnum_overflow(assembler, slow)?;
        }
    }
    Ok(())
}

fn load_fixnum_operand(
    assembler: &mut Assembler,
    allocation: &Allocation,
    value: ValueId,
    destination: Reg,
) -> Result<(), CodegenError> {
    match allocation
        .location(value)
        .ok_or(CodegenError::UnknownValue(value))?
    {
        Location::Register(source) => emit(
            assembler,
            Inst::Mov {
                rd: RegOrSp::Reg(destination),
                rn: RegOrSp::Reg(Reg(
                    u8::try_from(source).map_err(|_| CodegenError::FrameOverflow)?
                )),
            },
        ),
        Location::Spill(_) => {
            let offset = super::super::spill_slot_offset(allocation, value)?;
            if offset <= 4095 {
                emit(
                    assembler,
                    Inst::SubImm {
                        rd: RegOrSp::Reg(Reg(0)),
                        rn: RegOrSp::Reg(Reg(29)),
                        imm: offset,
                        shift: false,
                    },
                )?;
            } else {
                for instruction in ncl_asm_aarch64::mov_imm64(Reg(0), u64::from(offset)) {
                    emit(assembler, instruction)?;
                }
                emit(
                    assembler,
                    Inst::Sub {
                        rd: RegOrSp::Reg(Reg(0)),
                        rn: RegOrSp::Reg(Reg(29)),
                        rm: Reg(0),
                        shift: Shift::Lsl(0),
                    },
                )?;
            }
            emit(
                assembler,
                Inst::Ldr {
                    rt: destination,
                    mem: MemOperand::Unscaled {
                        base: RegOrSp::Reg(Reg(0)),
                        offset: 0,
                    },
                },
            )
        }
    }
}

fn branch_on_fixnum_overflow(
    assembler: &mut Assembler,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::BCond {
            cond: Cond::Vs,
            label: slow,
        },
    )
}

fn lower_checked_fixnum_multiply(
    assembler: &mut Assembler,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::AsrImm {
            rd: Reg(16),
            rn: Reg(16),
            amount: 1,
        },
    )?;
    emit(
        assembler,
        Inst::AsrImm {
            rd: Reg(17),
            rn: Reg(17),
            amount: 1,
        },
    )?;
    check_multiply_i64_overflow(assembler, slow)?;
    emit(
        assembler,
        Inst::Mul {
            rd: Reg(16),
            rn: Reg(16),
            rm: Reg(17),
        },
    )?;
    check_fixnum_multiply_range(assembler, slow)?;
    emit(
        assembler,
        Inst::LslImm {
            rd: Reg(16),
            rn: Reg(16),
            amount: 1,
        },
    )
}

fn check_multiply_i64_overflow(
    assembler: &mut Assembler,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    let division_done = assembler.new_label();
    emit(
        assembler,
        Inst::Mul {
            rd: Reg(0),
            rn: Reg(16),
            rm: Reg(17),
        },
    )?;
    emit(
        assembler,
        Inst::Cbz {
            rt: Reg(17),
            label: division_done,
        },
    )?;
    emit(
        assembler,
        Inst::Sdiv {
            rd: Reg(0),
            rn: Reg(0),
            rm: Reg(17),
        },
    )?;
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(0),
            rm: Reg(16),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::BCond {
            cond: Cond::Ne,
            label: slow,
        },
    )?;
    assembler
        .bind(division_done)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

fn check_fixnum_multiply_range(
    assembler: &mut Assembler,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    let range_ok = assembler.new_label();
    emit(
        assembler,
        Inst::AsrImm {
            rd: Reg(17),
            rn: Reg(16),
            amount: 62,
        },
    )?;
    emit(
        assembler,
        Inst::Cbz {
            rt: Reg(17),
            label: range_ok,
        },
    )?;
    for instruction in ncl_asm_aarch64::mov_imm64(Reg(0), u64::MAX) {
        emit(assembler, instruction)?;
    }
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(17),
            rm: Reg(0),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::BCond {
            cond: Cond::Ne,
            label: slow,
        },
    )?;
    assembler
        .bind(range_ok)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}
