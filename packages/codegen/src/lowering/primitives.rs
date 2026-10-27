use super::ops::{emit, load, lower_write_barrier, store};
use crate::isa_x86_64::THREAD_CONTEXT;
use crate::{CodegenError, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Imm, Inst, Mem, Reg, Scale, Shift};
use ncl_ir::{Prim, ValueId};

fn simple_vector_data_offset() -> Result<i32, CodegenError> {
    ncl_object::simple_vector_offset::DATA
        .checked_add(1)
        .and_then(|slot| slot.checked_mul(std::mem::size_of::<ncl_sys::Word>()))
        .and_then(|offset| i32::try_from(offset).ok())
        .ok_or(CodegenError::FrameOverflow)
}

fn lower_indexed_write_barrier(
    assembler: &mut Assembler,
    owner: ValueId,
    index: ValueId,
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    load(assembler, slots, index, Reg::Rax)?;
    emit(
        assembler,
        &Inst::ShiftImm(
            Shift::Sar,
            Reg::Rax,
            u8::try_from(ncl_sys::FIXNUM_TAG_BITS).map_err(|_| CodegenError::FrameOverflow)?,
        ),
    )?;
    emit(
        assembler,
        &Inst::BinRI(
            BinOp::Add,
            Reg::Rax,
            i32::try_from(ncl_object::simple_vector_offset::DATA)
                .map_err(|_| CodegenError::FrameOverflow)?,
        ),
    )?;
    emit(assembler, &Inst::MovRR(Reg::Rdi, THREAD_CONTEXT))?;
    emit(assembler, &Inst::MovRR(Reg::Rsi, Reg::Rax))?;
    load(assembler, slots, owner, Reg::Rdx)?;
    let address = abi
        .runtime_address(RuntimeFunction::WriteBarrier)
        // check-added-lines: allow(unsupported) preserve runtime ABI lookup failures.
        .map_err(|error| CodegenError::Unsupported(error.to_string()))?;
    emit(
        assembler,
        &Inst::MovRI(Reg::R11, Imm::I64(address.cast_signed())),
    )?;
    emit(assembler, &Inst::CallReg(Reg::R11))
}

pub(super) fn lower_prim(
    assembler: &mut Assembler,
    prim: &Prim,
    args: &[ValueId],
    result: Option<ValueId>,
    slots: &[(ValueId, u32)],
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
        // check-added-lines: allow(unsupported) report malformed primitive operands.
        return Err(CodegenError::Unsupported(
            "primitive has no operands".into(),
        ));
    };
    load(assembler, slots, *first, Reg::R10)?;
    if let Some(second) = args.get(1) {
        load(assembler, slots, *second, Reg::R11)?;
    }
    match prim {
        Prim::FixnumAdd => emit(assembler, &Inst::BinRR(BinOp::Add, Reg::R10, Reg::R11))?,
        Prim::FixnumSub => emit(assembler, &Inst::BinRR(BinOp::Sub, Reg::R10, Reg::R11))?,
        Prim::FixnumMul => emit(assembler, &Inst::ImulRR(Reg::R10, Reg::R11))?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => {
            emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?;
            emit(assembler, &Inst::Setcc(Cond::E, Reg::R10))?;
        }
        Prim::FixnumLt => {
            emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?;
            emit(assembler, &Inst::Setcc(Cond::L, Reg::R10))?;
        }
        Prim::FixnumLe => {
            emit(assembler, &Inst::CmpRR(Reg::R10, Reg::R11))?;
            emit(assembler, &Inst::Setcc(Cond::Le, Reg::R10))?;
        }
        Prim::Car | Prim::Cdr => {
            emit(assembler, &Inst::MovRM(Reg::R10, Mem::base(Reg::R10, 0)))?;
        }
        Prim::Svref | Prim::Aref => {
            if args.get(1).is_none() {
                // check-added-lines: allow(unsupported) report malformed simple-vector operands.
                return Err(CodegenError::Unsupported(
                    "simple-vector reference has no index operand".into(),
                ));
            }
            emit(assembler, &Inst::BinRI(BinOp::And, Reg::R10, -8))?;
            emit(
                assembler,
                &Inst::MovRM(
                    Reg::R10,
                    Mem::indexed(
                        Reg::R10,
                        Reg::R11,
                        Scale::Four,
                        simple_vector_data_offset()?,
                    ),
                ),
            )?;
        }
        Prim::Rplaca | Prim::Rplacd => {
            emit(assembler, &Inst::MovMR(Mem::base(Reg::R10, 0), Reg::R11))?;
            lower_write_barrier(assembler, *first, 0, slots, abi)?;
        }
        Prim::Aset => {
            let Some(index) = args.get(1).copied() else {
                // check-added-lines: allow(unsupported) report malformed aset operands.
                return Err(CodegenError::Unsupported(
                    "aset has no index operand".into(),
                ));
            };
            let Some(value) = args.get(2).copied() else {
                // check-added-lines: allow(unsupported) report malformed aset operands.
                return Err(CodegenError::Unsupported(
                    "aset has no value operand".into(),
                ));
            };
            emit(assembler, &Inst::BinRI(BinOp::And, Reg::R10, -8))?;
            load(assembler, slots, value, Reg::Rax)?;
            emit(
                assembler,
                &Inst::MovMR(
                    Mem::indexed(
                        Reg::R10,
                        Reg::R11,
                        Scale::Four,
                        simple_vector_data_offset()?,
                    ),
                    Reg::Rax,
                ),
            )?;
            lower_indexed_write_barrier(assembler, *first, index, slots, abi)?;
        }
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => {
            // check-added-lines: allow(unsupported) report primitives not implemented by fixed templates.
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available in fixed templates: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store(assembler, slots, result, Reg::R10)?;
    }
    Ok(())
}
