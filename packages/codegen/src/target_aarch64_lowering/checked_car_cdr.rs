use super::{
    Allocation, CallCheckContext, CheckedCallMode, CodegenError, RuntimeAbi, RuntimeFunction, emit,
    emit_safepoint_call, load_value, lower_checked_call, store_value,
};
use crate::ConstantName;
use crate::machine_verify::SafepointSite;
use ncl_asm_aarch64::{Assembler, Cond, Inst, MemOperand, Reg, RegOrSp, Shift};
use ncl_ir::{DebugLocationId, Prim, ValueId};

fn move_immediate(
    assembler: &mut Assembler,
    register: Reg,
    value: u64,
) -> Result<(), CodegenError> {
    for instruction in ncl_asm_aarch64::mov_imm64(register, value) {
        emit(assembler, instruction)?;
    }
    Ok(())
}

fn branch(
    assembler: &mut Assembler,
    condition: Cond,
    label: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    emit(
        assembler,
        Inst::BCond {
            cond: condition,
            label,
        },
    )
}

fn bind(assembler: &mut Assembler, label: ncl_asm_aarch64::Label) -> Result<(), CodegenError> {
    assembler
        .bind(label)
        .map_err(|error| CodegenError::Encode(error.to_string()))
}

#[derive(Clone, Copy)]
pub(super) struct CarCdrOperation<'a> {
    pub(super) prim: &'a Prim,
    pub(super) args: &'a [ValueId],
    pub(super) result: Option<ValueId>,
    pub(super) source_origin: Option<DebugLocationId>,
}

pub(super) fn lower(
    assembler: &mut Assembler,
    operation: CarCdrOperation<'_>,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<SafepointSite, CodegenError> {
    let [datum] = operation.args else {
        // check-added-lines: allow(unsupported) reject malformed primitive IR arity
        return Err(CodegenError::Unsupported(
            "CAR or CDR requires one operand".into(),
        ));
    };
    let slow = assembler.new_label();
    let nil = assembler.new_label();
    let done = assembler.new_label();

    emit_fast_path(assembler, &operation, *datum, allocation, nil, slow)?;
    emit(assembler, Inst::B { label: done })?;

    bind(assembler, nil)?;
    move_immediate(assembler, Reg(16), ncl_sys::Word::NIL.bits())?;
    if let Some(result) = operation.result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    emit(assembler, Inst::B { label: done })?;

    bind(assembler, slow)?;
    let checked = emit_type_error_signal(assembler, &operation, *datum, allocation, abi, context)?;
    if let Some(result) = operation.result {
        store_value(assembler, allocation, result, Reg(0))?;
    }
    bind(assembler, done)?;
    Ok(checked.site())
}

fn emit_fast_path(
    assembler: &mut Assembler,
    operation: &CarCdrOperation<'_>,
    datum: ValueId,
    allocation: &Allocation,
    nil: ncl_asm_aarch64::Label,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    load_value(assembler, allocation, datum, Reg(16))?;
    move_immediate(assembler, Reg(17), ncl_sys::Word::NIL.bits())?;
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(16),
            rm: Reg(17),
            shift: Shift::Lsl(0),
        },
    )?;
    branch(assembler, Cond::Eq, nil)?;

    emit_cons_tag_guard(assembler, slow)?;

    let offset = if matches!(operation.prim, Prim::Cdr) {
        8
    } else {
        0
    };
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(16),
            rn: Reg(16),
            imm: !ncl_sys::LOWTAG_MASK,
        },
    )?;
    emit(
        assembler,
        Inst::Ldr {
            rt: Reg(16),
            mem: MemOperand::Unscaled {
                base: RegOrSp::Reg(Reg(16)),
                offset,
            },
        },
    )?;
    if let Some(result) = operation.result {
        store_value(assembler, allocation, result, Reg(16))?;
    }
    Ok(())
}

fn emit_cons_tag_guard(
    assembler: &mut Assembler,
    slow: ncl_asm_aarch64::Label,
) -> Result<(), CodegenError> {
    // Character immediates share the cons lowtag, so combine both predicates before branching.
    emit(
        assembler,
        Inst::LsrImm {
            rd: Reg(17),
            rn: Reg(16),
            amount: 32,
        },
    )?;
    emit(
        assembler,
        Inst::AddExt {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(31)),
            rm: Reg(17),
            extend: ncl_asm_aarch64::Extend::Sxtw,
            shift: 0,
        },
    )?;
    emit(
        assembler,
        Inst::SubImm {
            rd: RegOrSp::Reg(Reg(17)),
            rn: RegOrSp::Reg(Reg(17)),
            imm: 1,
            shift: false,
        },
    )?;
    emit(
        assembler,
        Inst::AndImm {
            rd: Reg(15),
            rn: Reg(16),
            imm: ncl_sys::LOWTAG_MASK,
        },
    )?;
    emit(
        assembler,
        Inst::EorImm {
            rd: Reg(15),
            rn: Reg(15),
            imm: 1,
        },
    )?;
    emit(
        assembler,
        Inst::Neg {
            rd: Reg(15),
            rn: Reg(15),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::Orr {
            rd: Reg(17),
            rn: Reg(17),
            rm: Reg(15),
            shift: Shift::Lsl(0),
        },
    )?;
    emit(
        assembler,
        Inst::Cmp {
            rn: Reg(17),
            rm: Reg(31),
            shift: Shift::Lsl(0),
        },
    )?;
    branch(assembler, Cond::Mi, slow)?;
    Ok(())
}

fn emit_type_error_signal(
    assembler: &mut Assembler,
    operation: &CarCdrOperation<'_>,
    datum: ValueId,
    allocation: &Allocation,
    abi: &dyn RuntimeAbi,
    context: &CallCheckContext<'_>,
) -> Result<super::CheckedCall, CodegenError> {
    let origin = operation
        .source_origin
        .map_or(-1, |location| i64::from(location.0));
    let address = abi
        .runtime_address(RuntimeFunction::SignalTypeError)
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?; // check-added-lines: allow(unsupported) report a missing signal entry in the runtime ABI
    let expected_type_index = abi
        .constant_word_named(ConstantName::new("type-error-expected:LIST"))
        .and_then(|index| u32::try_from(index).ok())
        .map(ncl_ir::ConstantIndex)
        .ok_or_else(|| {
            CodegenError::Unsupported("LIST type-error constant is unavailable".into()) // check-added-lines: allow(unsupported) the code object must carry LIST for TYPE-ERROR
        })?;
    lower_checked_call(
        assembler,
        context,
        allocation,
        abi,
        CheckedCallMode::RuntimeAndNonLocal,
        |assembler| {
            emit(
                assembler,
                Inst::Mov {
                    rd: RegOrSp::Reg(Reg(0)),
                    rn: RegOrSp::Reg(Reg(21)),
                },
            )?;
            load_value(assembler, allocation, datum, Reg(1))?;
            super::ops::load_heap_constant(assembler, expected_type_index)?;
            emit(
                assembler,
                Inst::Mov {
                    rd: RegOrSp::Reg(Reg(2)),
                    rn: RegOrSp::Reg(Reg(16)),
                },
            )?;
            move_immediate(assembler, Reg(3), u64::from_ne_bytes(origin.to_ne_bytes()))?;
            move_immediate(assembler, Reg(17), address)?;
            emit_safepoint_call(assembler)
        },
    )
}
