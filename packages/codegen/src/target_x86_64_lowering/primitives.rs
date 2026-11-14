use super::ops::{lower_write_barrier, materialise_boolean, untag_function_object};
use super::{
    ARGUMENT_COUNT, ARGUMENT_REGISTERS, ENTRY, FUNCTION_OBJECT, RETURN_VALUE, ValueSlots, emit,
    emit_call, load_heap_constant, load_immediate, load_slot, lower_runtime_builtin, store_slot,
};
use crate::isa_x86_64::THREAD_CONTEXT;
use crate::machine_verify::SafepointSite;
use crate::{CodegenError, ConstantName, RuntimeAbi, RuntimeFunction};
use ncl_asm_x86_64::{Assembler, BinOp, Cond, Inst, Mem, Scale, Shift};
use ncl_ir::{DebugLocationId, Prim, ValueId};

fn simple_vector_data_offset() -> Result<i32, CodegenError> {
    ncl_object::simple_vector_offset::DATA
        .checked_add(1)
        .and_then(|slot| slot.checked_mul(std::mem::size_of::<ncl_sys::Word>()))
        .and_then(|offset| i32::try_from(offset).ok())
        .ok_or(CodegenError::FrameOverflow)
}

fn emit_sequence(
    assembler: &mut Assembler,
    instructions: impl IntoIterator<Item = Inst>,
) -> Result<(), CodegenError> {
    instructions
        .into_iter()
        .try_for_each(|instruction| emit(assembler, instruction))
}

fn lower_indexed_write_barrier(
    assembler: &mut Assembler,
    owner: ValueId,
    index: ValueId,
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let argument_register = super::ARGUMENT_REGISTERS
        .first()
        .copied()
        .ok_or(CodegenError::FrameOverflow)?;
    load_slot(assembler, slots, index, RETURN_VALUE)
        .and_then(|()| {
            emit_sequence(
                assembler,
                [
                    Inst::ShiftImm(
                        Shift::Sar,
                        RETURN_VALUE,
                        u8::try_from(ncl_sys::FIXNUM_TAG_BITS)
                            .map_err(|_| CodegenError::FrameOverflow)?,
                    ),
                    Inst::BinRI(
                        BinOp::Add,
                        RETURN_VALUE,
                        i32::try_from(ncl_object::simple_vector_offset::DATA)
                            .map_err(|_| CodegenError::FrameOverflow)?,
                    ),
                ],
            )
        })
        .and_then(|()| {
            lower_runtime_builtin(
                assembler,
                RuntimeFunction::WriteBarrier,
                &[0],
                &[owner],
                slots,
                abi,
            )
        })
        .and_then(|()| emit(assembler, Inst::MovRR(argument_register, RETURN_VALUE)))
        .and_then(|()| emit_call(assembler).map(|_| ()))
}

fn lower_simple_vector_reference(
    assembler: &mut Assembler,
    args: &[ValueId],
    result: Option<ValueId>,
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    if args.get(1).is_none() {
        // check-added-lines: allow(unsupported) report malformed simple-vector operands.
        return Err(CodegenError::Unsupported(
            "simple-vector reference has no index operand".into(),
        ));
    }
    emit_sequence(
        assembler,
        [
            Inst::BinRI(BinOp::And, FUNCTION_OBJECT, -8),
            Inst::MovRM(
                FUNCTION_OBJECT,
                Mem::indexed(
                    FUNCTION_OBJECT,
                    ENTRY,
                    Scale::Four,
                    simple_vector_data_offset()?,
                ),
            ),
        ],
    )
    .and_then(|()| {
        result.map_or(Ok(()), |result| {
            store_slot(assembler, slots, result, FUNCTION_OBJECT)
        })
    })
}

fn lower_simple_vector_set(
    assembler: &mut Assembler,
    owner: ValueId,
    args: &[ValueId],
    result: Option<ValueId>,
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
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
    load_slot(assembler, slots, value, RETURN_VALUE)
        .and_then(|()| {
            emit_sequence(
                assembler,
                [
                    Inst::BinRI(BinOp::And, FUNCTION_OBJECT, -8),
                    Inst::MovMR(
                        Mem::indexed(
                            FUNCTION_OBJECT,
                            ENTRY,
                            Scale::Four,
                            simple_vector_data_offset()?,
                        ),
                        RETURN_VALUE,
                    ),
                ],
            )
        })
        .and_then(|()| lower_indexed_write_barrier(assembler, owner, index, slots, abi))
        .and_then(|()| {
            result.map_or(Ok(()), |result| {
                store_slot(assembler, slots, result, FUNCTION_OBJECT)
            })
        })
}

pub(super) fn lower_prim(
    assembler: &mut Assembler,
    prim: &Prim,
    args: &[ValueId],
    result: Option<ValueId>,
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
) -> Result<(), CodegenError> {
    let Some(first) = args.first() else {
        // check-added-lines: allow(unsupported) existing codegen error variant
        return Err(CodegenError::Unsupported(
            "primitive has no operands".into(),
        ));
    };
    load_slot(assembler, slots, *first, FUNCTION_OBJECT)?;
    if let Some(second) = args.get(1) {
        load_slot(assembler, slots, *second, ENTRY)?;
    }
    match prim {
        Prim::FixnumAdd => emit(assembler, Inst::BinRR(BinOp::Add, FUNCTION_OBJECT, ENTRY))?,
        Prim::FixnumSub => emit(assembler, Inst::BinRR(BinOp::Sub, FUNCTION_OBJECT, ENTRY))?,
        Prim::FixnumMul => emit(assembler, Inst::ImulRR(FUNCTION_OBJECT, ENTRY))?,
        Prim::FixnumEq | Prim::Eq | Prim::Eql => {
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            materialise_boolean(assembler, Cond::E)?;
        }
        Prim::FixnumLt => {
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            materialise_boolean(assembler, Cond::L)?;
        }
        Prim::FixnumLe => {
            emit(assembler, Inst::CmpRR(FUNCTION_OBJECT, ENTRY))?;
            materialise_boolean(assembler, Cond::Le)?;
        }
        Prim::Car | Prim::Cdr => {
            // check-added-lines: allow(unsupported) keep unchecked CAR/CDR lowering unreachable
            return Err(CodegenError::Unsupported(
                "CAR and CDR require checked primitive lowering".into(),
            ));
        }
        Prim::Svref | Prim::Aref => {
            return lower_simple_vector_reference(assembler, args, result, slots);
        }
        Prim::Rplaca | Prim::Rplacd => {
            untag_function_object(assembler)?;
            let offset = if matches!(prim, Prim::Rplacd) { 8 } else { 0 };
            emit(
                assembler,
                Inst::MovMR(Mem::base(FUNCTION_OBJECT, offset), ENTRY),
            )?;
            lower_write_barrier(
                assembler,
                *first,
                i64::from(u8::from(matches!(prim, Prim::Rplacd))),
                slots,
                abi,
            )?;
        }
        Prim::Aset => {
            return lower_simple_vector_set(assembler, *first, args, result, slots, abi);
        }
        Prim::FixnumDiv | Prim::Typep | Prim::CharacterPredicate(_) | Prim::StructureSlot(_) => {
            // check-added-lines: allow(unsupported) existing codegen error variant
            return Err(CodegenError::Unsupported(format!(
                "primitive is not available: {prim:?}"
            )));
        }
    }
    if let Some(result) = result {
        store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) struct CheckedCarCdr<'a> {
    pub(super) prim: &'a Prim,
    pub(super) args: &'a [ValueId],
    pub(super) result: Option<ValueId>,
    pub(super) source_origin: Option<DebugLocationId>,
}

pub(super) fn lower_checked_car_cdr(
    assembler: &mut Assembler,
    operation: CheckedCarCdr<'_>,
    slots: &ValueSlots,
    abi: &dyn RuntimeAbi,
    context: &super::dispatch::CallCheckContext<'_>,
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

    load_slot(assembler, slots, *datum, FUNCTION_OBJECT)?;
    emit(assembler, Inst::CmpRI(FUNCTION_OBJECT, 1))?;
    emit(assembler, Inst::Jcc(Cond::E, nil))?;

    // Character immediates share the cons lowtag, so combine both predicates before branching.
    emit(assembler, Inst::MovRR(ENTRY, FUNCTION_OBJECT))?;
    emit(assembler, Inst::ShiftImm(Shift::Shr, ENTRY, 32))?;
    emit(assembler, Inst::Movsx(ENTRY, ENTRY, 32))?;
    emit(assembler, Inst::Lea(ENTRY, Mem::base(ENTRY, -1)))?;
    emit(assembler, Inst::MovRR(RETURN_VALUE, FUNCTION_OBJECT))?;
    emit(
        assembler,
        Inst::BinRI(
            BinOp::And,
            RETURN_VALUE,
            i32::try_from(ncl_sys::LOWTAG_MASK).map_err(|_| CodegenError::FrameOverflow)?,
        ),
    )?;
    emit(assembler, Inst::BinRI(BinOp::Xor, RETURN_VALUE, 1))?;
    emit(assembler, Inst::Neg(RETURN_VALUE))?;
    emit(assembler, Inst::BinRR(BinOp::Or, ENTRY, RETURN_VALUE))?;
    emit(assembler, Inst::CmpRI(ENTRY, 0))?;
    emit(assembler, Inst::Jcc(Cond::L, slow))?;

    untag_function_object(assembler)?;
    let offset = if matches!(operation.prim, Prim::Cdr) {
        8
    } else {
        0
    };
    emit(
        assembler,
        Inst::MovRM(FUNCTION_OBJECT, Mem::base(FUNCTION_OBJECT, offset)),
    )?;
    if let Some(result) = operation.result {
        store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
    }
    emit(assembler, Inst::Jmp(done))?;

    assembler.bind(nil);
    load_immediate(
        assembler,
        FUNCTION_OBJECT,
        i64::from_ne_bytes(ncl_sys::Word::NIL.bits().to_ne_bytes()),
    )?;
    if let Some(result) = operation.result {
        store_slot(assembler, slots, result, FUNCTION_OBJECT)?;
    }
    emit(assembler, Inst::Jmp(done))?;

    assembler.bind(slow);
    let source_origin = operation
        .source_origin
        .map_or(-1, |location| i64::from(location.0));
    let expected_type_index = abi
        .constant_word_named(ConstantName::new("type-error-expected:LIST"))
        .and_then(|index| u32::try_from(index).ok())
        .map(ncl_ir::ConstantIndex)
        .ok_or_else(|| {
            CodegenError::Unsupported("LIST type-error constant is unavailable".into()) // check-added-lines: allow(unsupported) the code object must carry LIST for TYPE-ERROR
        })?;
    let checked = super::checked_call(assembler, context, |assembler| {
        let address = abi
            .runtime_address(RuntimeFunction::SignalTypeError)
            .map_err(|error| CodegenError::Unsupported(error.to_string()))?; // check-added-lines: allow(unsupported) report a missing signal entry in the runtime ABI
        emit(assembler, Inst::MovRR(ARGUMENT_COUNT, THREAD_CONTEXT))?;
        let datum_register = ARGUMENT_REGISTERS
            .first()
            .copied()
            .ok_or(CodegenError::FrameOverflow)?;
        let expected_register = ARGUMENT_REGISTERS
            .get(1)
            .copied()
            .ok_or(CodegenError::FrameOverflow)?;
        let origin_register = ARGUMENT_REGISTERS
            .get(2)
            .copied()
            .ok_or(CodegenError::FrameOverflow)?;
        load_slot(assembler, slots, *datum, datum_register)?;
        load_heap_constant(assembler, expected_type_index)?;
        emit(assembler, Inst::MovRR(expected_register, FUNCTION_OBJECT))?;
        load_immediate(assembler, origin_register, source_origin)?;
        load_immediate(assembler, ENTRY, address.cast_signed())?;
        emit_call(assembler)
    })?;
    if let Some(result) = operation.result {
        store_slot(assembler, slots, result, RETURN_VALUE)?;
    }
    assembler.bind(done);
    Ok(checked.site())
}

#[cfg(test)]
#[path = "checked_car_cdr_tests.rs"]
mod checked_car_cdr_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests_x86_64_fixture::X86_64FixtureAbi;
    use ncl_ir::ValueId;

    pub(super) fn empty_slots() -> ValueSlots {
        let function = ncl_ir::FunctionBuilder::new(
            ncl_ir::FunctionId(120),
            "malformed-primitive",
            Vec::new(),
            Vec::new(),
        )
        .finish();
        let allocation = crate::allocate(&function, crate::AllocationTarget::X86_64);
        super::super::slots(&function, 0, allocation, 0).0
    }

    #[test]
    fn malformed_primitive_operands_return_their_specific_errors() {
        let slots = empty_slots();
        let abi = X86_64FixtureAbi;
        let mut assembler = Assembler::new();
        assert!(matches!(
            lower_prim(&mut assembler, &Prim::FixnumAdd, &[], None, &slots, &abi,),
            Err(CodegenError::Unsupported(message)) if message == "primitive has no operands"
        ));

        assert!(matches!(
            lower_simple_vector_reference(&mut assembler, &[ValueId(0)], None, &slots,),
            Err(CodegenError::Unsupported(message)) if message == "simple-vector reference has no index operand"
        ));
        assert!(matches!(
            lower_simple_vector_set(&mut assembler, ValueId(0), &[], None, &slots, &abi,),
            Err(CodegenError::Unsupported(message)) if message == "aset has no index operand"
        ));
        assert!(matches!(
            lower_simple_vector_set(
                &mut assembler,
                ValueId(0),
                &[ValueId(0), ValueId(1)],
                None,
                &slots,
                &abi,
            ),
            Err(CodegenError::Unsupported(message)) if message == "aset has no value operand"
        ));
    }

}
