use crate::FLAG_CALL;
use crate::{Allocation, CodegenError, FrameLayout, Location, SafepointMap};
use ncl_asm_aarch64::{Assembler, Inst, MemOperand, Reg, RegOrSp};
use ncl_ir::{Function, Ty};

pub(super) fn add_map(
    maps: &mut Vec<SafepointMap>,
    pc: u32,
    frame: FrameLayout,
    allocation: &Allocation,
    position: u32,
    flags: u32,
) -> Result<(), CodegenError> {
    let slots = u16::try_from(frame.frame_words).map_err(|_| CodegenError::FrameOverflow)?;
    let mut live_slots = Vec::new();
    let mut registers = Vec::new();
    for (value, location) in &allocation.locations {
        let Some(interval) = allocation
            .intervals
            .iter()
            .find(|item| item.value == *value)
        else {
            continue;
        };
        if !(interval.start <= position
            && position <= interval.end
            && matches!(interval.ty, Ty::Word | Ty::Address))
        {
            continue;
        }
        match location {
            Location::Spill(spill) => live_slots.push(
                4u16.checked_add(u16::try_from(*spill).map_err(|_| CodegenError::FrameOverflow)?)
                    .ok_or(CodegenError::FrameOverflow)?,
            ),
            Location::Register(register) => registers.push(*register),
        }
    }
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
    registers.sort_unstable();
    registers.dedup();
    SafepointMap::new(pc, slots, slots, &live_slots, &registers, flags)
        .map(|map| maps.push(map))
        .map_err(|error| CodegenError::Encode(error.to_string()))
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

#[cfg(test)]
#[allow(clippy::cast_possible_truncation, missing_docs)]
mod tests {
    use super::*;
    use crate::{Allocation, LiveInterval, Location};
    use ncl_ir::{FunctionId, Param, Ty, ValueId};
    use std::collections::BTreeMap;

    fn function(params: Vec<Param>) -> Function {
        Function {
            id: FunctionId(1),
            name: "argument-test".into(),
            params,
            return_types: Vec::new(),
            blocks: Vec::new(),
            locals: Vec::new(),
            constants: Vec::new(),
            handler_regions: Vec::new(),
            debug: Vec::new(),
        }
    }

    #[test]
    fn safepoint_maps_filter_live_types_and_preserve_call_abi_roots() {
        let frame = FrameLayout::new(0, 5, 2).unwrap_or_else(|error| panic!("frame: {error:?}"));
        let allocation = Allocation {
            intervals: vec![
                LiveInterval {
                    value: ValueId(0),
                    start: 0,
                    end: 2,
                    ty: Ty::Word,
                    crosses_call: false,
                    crosses_safepoint: false,
                    crosses_handler: false,
                },
                LiveInterval {
                    value: ValueId(1),
                    start: 0,
                    end: 2,
                    ty: Ty::Address,
                    crosses_call: false,
                    crosses_safepoint: false,
                    crosses_handler: false,
                },
                LiveInterval {
                    value: ValueId(2),
                    start: 0,
                    end: 2,
                    ty: Ty::I64,
                    crosses_call: false,
                    crosses_safepoint: false,
                    crosses_handler: false,
                },
                LiveInterval {
                    value: ValueId(3),
                    start: 4,
                    end: 5,
                    ty: Ty::Word,
                    crosses_call: false,
                    crosses_safepoint: false,
                    crosses_handler: false,
                },
            ],
            locations: vec![
                (ValueId(0), Location::Register(1)),
                (ValueId(1), Location::Spill(0)),
                (ValueId(2), Location::Register(2)),
                (ValueId(3), Location::Spill(1)),
                (ValueId(4), Location::Register(1)),
            ],
            spill_words: 2,
            safepoint_registers: BTreeMap::new(),
            outgoing_base: 0,
            incoming_args_base: Some(0),
        };
        let mut maps = Vec::new();
        add_map(&mut maps, 12, frame, &allocation, 1, FLAG_CALL)
            .unwrap_or_else(|error| panic!("safepoint map: {error:?}"));
        let map = maps.first().unwrap_or_else(|| panic!("map is recorded"));
        assert_eq!(map.registers, vec![1]);
        assert_eq!(map.map_flags, FLAG_CALL);
        assert!(map.bitmap.iter().any(|byte| *byte != 0));

        let mut maps = Vec::new();
        let no_call = Allocation {
            incoming_args_base: None,
            ..allocation.clone()
        };
        add_map(&mut maps, 16, frame, &no_call, 4, 0)
            .unwrap_or_else(|error| panic!("non-call map: {error:?}"));
        assert_eq!(maps.len(), 1);

        let too_wide = FrameLayout {
            argument_words: 0,
            local_words: 0,
            outgoing_words: 0,
            frame_words: u32::MAX,
        };
        assert_eq!(
            add_map(&mut Vec::new(), 0, too_wide, &allocation, 0, 0),
            Err(CodegenError::FrameOverflow)
        );
    }

    #[test]
    fn argument_initialization_covers_generated_lambda_and_stack_arguments() {
        let params = (0..6)
            .map(|index| Param {
                name: format!("arg-{index}"),
                ty: Ty::Word,
            })
            .collect::<Vec<_>>();
        let allocation = Allocation {
            intervals: Vec::new(),
            locations: (0..6)
                .map(|index| (ValueId(index), Location::Register(index as u16)))
                .collect(),
            spill_words: 0,
            safepoint_registers: BTreeMap::new(),
            outgoing_base: 0,
            incoming_args_base: None,
        };
        let mut assembler = Assembler::new();
        initialize_arguments(&mut assembler, &function(params), &allocation)
            .unwrap_or_else(|error| panic!("ordinary arguments: {error:?}"));
        assert!(
            !assembler
                .finish()
                .unwrap_or_else(|error| panic!("ordinary argument encoding: {error:?}"))
                .bytes
                .is_empty()
        );

        let params = (0..6)
            .map(|index| Param {
                name: if index == 0 {
                    "argc".into()
                } else {
                    format!("arg-{index}")
                },
                ty: Ty::Word,
            })
            .collect::<Vec<_>>();
        let mut assembler = Assembler::new();
        initialize_arguments(&mut assembler, &function(params), &allocation)
            .unwrap_or_else(|error| panic!("generated-lambda arguments: {error:?}"));
        assert!(
            !assembler
                .finish()
                .unwrap_or_else(|error| panic!("generated-lambda encoding: {error:?}"))
                .bytes
                .is_empty()
        );
    }
}
