use super::{
    ENTRY, FRAME_POINTER, FUNCTION_OBJECT, RETURN_VALUE, ValueSlots, emit, load_immediate,
    load_slot,
};
use crate::CodegenError;
use ncl_asm_x86_64::{BinOp, Inst, Mem};
use ncl_ir::ValueId;

pub fn load_heap_constant(
    assembler: &mut ncl_asm_x86_64::Assembler,
    index: ncl_ir::ConstantIndex,
) -> Result<(), CodegenError> {
    let function_code_offset = i32::try_from(
        (ncl_object::function_offset::CODE + 1)
            .checked_mul(8)
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    let code_constants_offset = i32::try_from(
        (ncl_object::code_offset::CONSTANTS + 1)
            .checked_mul(8)
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    let vector_element_offset = i32::try_from(
        (ncl_object::simple_vector_offset::DATA + 1)
            .checked_add(usize::try_from(index.0).map_err(|_| CodegenError::FrameOverflow)?)
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    emit(
        assembler,
        Inst::MovRM(FUNCTION_OBJECT, Mem::base(FRAME_POINTER, 16)),
    )?;
    emit(assembler, Inst::BinRI(BinOp::And, FUNCTION_OBJECT, -8))?;
    emit(
        assembler,
        Inst::MovRM(ENTRY, Mem::base(FUNCTION_OBJECT, function_code_offset)),
    )?;
    emit(assembler, Inst::BinRI(BinOp::And, ENTRY, -8))?;
    emit(
        assembler,
        Inst::MovRM(ENTRY, Mem::base(ENTRY, code_constants_offset)),
    )?;
    emit(assembler, Inst::BinRI(BinOp::And, ENTRY, -8))?;
    emit(
        assembler,
        Inst::MovRM(FUNCTION_OBJECT, Mem::base(ENTRY, vector_element_offset)),
    )
}

pub fn store_closure_capture(
    assembler: &mut ncl_asm_x86_64::Assembler,
    closure: ValueId,
    index: usize,
    capture: ValueId,
    slots: &ValueSlots,
) -> Result<(), CodegenError> {
    let offset = i32::try_from(
        ncl_object::function_offset::CAPTURES
            .checked_add(index)
            .and_then(|slot| slot.checked_add(1))
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)?;
    load_slot(assembler, slots, closure, FUNCTION_OBJECT)?;
    load_slot(assembler, slots, capture, RETURN_VALUE)?;
    load_immediate(
        assembler,
        ENTRY,
        i64::from_ne_bytes((!ncl_sys::LOWTAG_MASK).to_ne_bytes()),
    )?;
    emit(assembler, Inst::BinRR(BinOp::And, FUNCTION_OBJECT, ENTRY))?;
    emit(
        assembler,
        Inst::MovMR(Mem::base(FUNCTION_OBJECT, offset), RETURN_VALUE),
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, missing_docs)]
mod tests {
    use super::{load_heap_constant, store_closure_capture};
    use crate::{AllocationTarget, CodegenError, allocate};
    use ncl_asm_x86_64::Assembler;
    use ncl_ir::{FunctionBuilder, ValueId};

    #[test]
    fn support_helpers_reject_offsets_that_cannot_fit_the_machine_address() {
        let mut assembler = Assembler::new();
        assert_eq!(
            load_heap_constant(&mut assembler, ncl_ir::ConstantIndex(u32::MAX)),
            Err(CodegenError::FrameOverflow)
        );

        let function = FunctionBuilder::new(
            ncl_ir::FunctionId(217),
            "capture-offset",
            Vec::new(),
            Vec::new(),
        )
        .finish();
        let slots = super::super::slots(
            &function,
            0,
            allocate(&function, AllocationTarget::X86_64),
            0,
        )
        .0;
        assert_eq!(
            store_closure_capture(&mut assembler, ValueId(0), usize::MAX, ValueId(1), &slots,),
            Err(CodegenError::FrameOverflow)
        );
    }
}
