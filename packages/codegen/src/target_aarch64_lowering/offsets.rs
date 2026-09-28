use crate::{Allocation, CodegenError, Location};
use ncl_ir::ValueId;

pub fn spill_slot_offset(allocation: &Allocation, value: ValueId) -> Result<u16, CodegenError> {
    let Location::Spill(index) = allocation
        .location(value)
        .ok_or(CodegenError::UnknownValue(value))?
    else {
        // check-added-lines: allow(unsupported) register locations have no frame offset.
        return Err(CodegenError::Unsupported(
            "register value has no spill slot".into(),
        ));
    };
    u16::try_from(
        index
            .checked_add(1)
            .and_then(|slot| slot.checked_mul(8))
            .ok_or(CodegenError::FrameOverflow)?,
    )
    .map_err(|_| CodegenError::FrameOverflow)
}

pub fn spill_offset(index: usize) -> Result<i16, CodegenError> {
    let bytes = index.checked_mul(8).ok_or(CodegenError::FrameOverflow)?;
    i16::try_from(bytes)
        .map_err(|_| CodegenError::FrameOverflow)?
        .checked_neg()
        .ok_or(CodegenError::FrameOverflow)
}
