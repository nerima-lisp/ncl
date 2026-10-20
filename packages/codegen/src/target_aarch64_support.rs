use crate::FLAG_CALL;
use crate::{Allocation, CodegenError, FrameLayout, SafepointMap};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::Function;

pub(super) fn add_map(
    maps: &mut Vec<SafepointMap>,
    pc: u32,
    function: &Function,
    frame: FrameLayout,
    allocation: &Allocation,
    position: u32,
    flags: u32,
) -> Result<(), CodegenError> {
    let slots = u16::try_from(frame.frame_words).map_err(|_| CodegenError::FrameOverflow)?;
    let (registers, roots) =
        crate::machine_verify::root_locations_at(function, allocation, position, 0);
    let mut live_slots = roots
        .into_iter()
        .map(|slot| u16::try_from(slot).map_err(|_| CodegenError::FrameOverflow))
        .collect::<Result<Vec<_>, _>>()?;
    if flags & FLAG_CALL != 0 {
        let base = 4u32
            .checked_add(allocation.outgoing_base)
            .ok_or(CodegenError::FrameOverflow)?;
        for offset in 0..frame.outgoing_words {
            live_slots.push(
                u16::try_from(
                    base.checked_add(offset)
                        .ok_or(CodegenError::FrameOverflow)?,
                )
                .map_err(|_| CodegenError::FrameOverflow)?,
            );
        }
    }
    if let Some(base) = allocation.incoming_args_base {
        for offset in 0..5 {
            live_slots.push(
                4u16.checked_add(
                    u16::try_from(
                        base.checked_add(offset)
                            .ok_or(CodegenError::FrameOverflow)?,
                    )
                    .map_err(|_| CodegenError::FrameOverflow)?,
                )
                .ok_or(CodegenError::FrameOverflow)?,
            );
        }
    }
    live_slots.sort_unstable();
    live_slots.dedup();
    let map = SafepointMap::new(pc, slots, slots, &live_slots, &registers, flags)
        .map_err(|error| CodegenError::Encode(error.to_string()))?;
    crate::machine_verify::verify_safepoint_map(
        function, allocation, frame, 0, position, flags, &map,
    )?;
    maps.push(map);
    Ok(())
}

pub(super) fn initialize_arguments(
    assembler: &mut Assembler,
    function: &Function,
    allocation: &Allocation,
) -> Result<(), CodegenError> {
    let generated_lambda = function
        .params
        .first()
        .is_some_and(|parameter| parameter.name == "argc");
    for (index, _parameter) in function.params.iter().enumerate() {
        let value = ncl_ir::ValueId(u32::try_from(index).map_err(|_| CodegenError::FrameOverflow)?);
        let register_index = if generated_lambda {
            index
        } else {
            index.saturating_add(1)
        };
        if register_index < 5 {
            super::lowering::store_value(
                assembler,
                allocation,
                value,
                Reg(u8::try_from(register_index).map_err(|_| CodegenError::FrameOverflow)?),
            )?;
        } else {
            let rest_offset = (register_index - 5)
                .checked_mul(8)
                .and_then(|offset| u16::try_from(offset).ok())
                .ok_or(CodegenError::FrameOverflow)?;
            assembler
                .emit(&Inst::Ldr {
                    rt: Reg(16),
                    mem: MemOperand::Unsigned {
                        base: RegOrSp::Reg(Reg(5)),
                        offset: rest_offset,
                        scale: 8,
                    },
                })
                .map_err(|error| CodegenError::Encode(error.to_string()))?;
            super::lowering::store_value(assembler, allocation, value, Reg(16))?;
        }
    }
    Ok(())
}
