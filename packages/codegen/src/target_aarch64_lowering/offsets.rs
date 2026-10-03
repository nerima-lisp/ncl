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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Allocation, Location};
    use std::collections::BTreeMap;

    fn allocation(location: Option<Location>) -> Allocation {
        Allocation {
            intervals: Vec::new(),
            locations: location
                .map(|location| vec![(ValueId(0), location)])
                .unwrap_or_default(),
            spill_words: 0,
            safepoint_registers: BTreeMap::new(),
            outgoing_base: 0,
            incoming_args_base: None,
        }
    }

    #[test]
    fn spill_offsets_cover_register_unknown_and_machine_boundaries() {
        assert_eq!(
            spill_slot_offset(&allocation(Some(Location::Spill(2))), ValueId(0)),
            Ok(24)
        );
        assert_eq!(
            spill_slot_offset(&allocation(Some(Location::Register(1))), ValueId(0)),
            Err(CodegenError::Unsupported(
                "register value has no spill slot".into()
            ))
        );
        assert_eq!(
            spill_slot_offset(&allocation(None), ValueId(0)),
            Err(CodegenError::UnknownValue(ValueId(0)))
        );
        assert_eq!(
            spill_slot_offset(&allocation(Some(Location::Spill(u32::MAX))), ValueId(0)),
            Err(CodegenError::FrameOverflow)
        );
        assert_eq!(spill_offset(0), Ok(0));
        assert_eq!(spill_offset(1), Ok(-8));
        assert_eq!(spill_offset(usize::MAX), Err(CodegenError::FrameOverflow));
        assert_eq!(spill_offset(4_096), Err(CodegenError::FrameOverflow));
    }
}
