use super::lowering::ValueSlots;
use crate::{CodegenError, FLAG_CALL, FrameLayout, SafepointMap};
use ncl_ir::Function;

pub(super) fn add_map(
    maps: &mut Vec<SafepointMap>,
    pc: u32,
    function: &Function,
    frame: FrameLayout,
    values: &ValueSlots,
    position: u32,
    flags: u32,
) -> Result<(), CodegenError> {
    let slots = u16::try_from(frame.frame_words).map_err(|_| CodegenError::FrameOverflow)?;
    let (registers, mut live_slots) = values.roots(function, position)?;
    if let Some(base) = values.incoming_args_base {
        for offset in 0..5 {
            live_slots.push(
                u16::try_from(
                    base.checked_add(offset)
                        .and_then(|slot| slot.checked_add(4))
                        .ok_or(CodegenError::FrameOverflow)?,
                )
                .map_err(|_| CodegenError::FrameOverflow)?,
            );
        }
    }
    if flags & FLAG_CALL != 0 {
        for offset in 0..frame.outgoing_words {
            live_slots.push(
                u16::try_from(
                    values
                        .outgoing_base
                        .checked_add(offset)
                        .and_then(|slot| slot.checked_add(4))
                        .ok_or(CodegenError::FrameOverflow)?,
                )
                .map_err(|_| CodegenError::FrameOverflow)?,
            );
        }
        live_slots.sort_unstable();
        live_slots.dedup();
    }
    let map = SafepointMap::new(pc, slots, slots, &live_slots, &registers, flags)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    crate::machine_verify::verify_safepoint_map(
        function,
        &values.allocation,
        frame,
        values.spill_base,
        position,
        flags,
        &map,
    )?;
    maps.push(map);
    Ok(())
}
