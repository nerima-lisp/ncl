use super::{MachineVerificationError, root_locations_at, verify_ir};
use crate::{Allocation, CodegenError, FrameLayout, MachineFunction, MachineOp, SafepointMap};
use ncl_ir::{Function, Terminator, ValueId};
use std::collections::{HashMap, HashSet};

pub fn verify_machine_function(
    function: &Function,
    machine: &MachineFunction,
    maps: &[SafepointMap],
    entry_offset: u32,
    code_len: u32,
) -> Result<(), CodegenError> {
    verify_ir(function)?;
    let slots = verify_machine_layout(function, machine)?;
    verify_machine_structure(function, machine, &slots)?;
    verify_machine_code_bounds(machine, maps, entry_offset, code_len)?;
    verify_machine_maps(function, machine, &slots, maps)?;
    Ok(())
}

fn verify_machine_layout(
    function: &Function,
    machine: &MachineFunction,
) -> Result<HashMap<ValueId, u32>, MachineVerificationError> {
    let argument_words = u32::try_from(function.params.len())
        .map_err(|_| MachineVerificationError::FrameMismatch)?;
    let mut definitions = HashSet::new();
    for block in &function.blocks {
        for parameter in &block.params {
            if !definitions.insert(parameter.value) {
                return Err(MachineVerificationError::MachineSlotMismatch);
            }
        }
        for op in &block.ops {
            for (value, _) in &op.results {
                if !definitions.insert(*value) {
                    return Err(MachineVerificationError::MachineSlotMismatch);
                }
            }
        }
    }
    let local_words =
        u32::try_from(definitions.len()).map_err(|_| MachineVerificationError::FrameMismatch)?;
    let expected_frame =
        FrameLayout::new(argument_words, local_words, machine.frame.outgoing_words)
            .map_err(|_| MachineVerificationError::FrameMismatch)?;
    if machine.frame != expected_frame {
        return Err(MachineVerificationError::FrameMismatch);
    }
    let local_end = crate::FRAME_HEADER_WORDS
        .checked_add(machine.frame.local_words)
        .ok_or(MachineVerificationError::FrameMismatch)?;
    if machine.slots.len() != definitions.len() {
        return Err(MachineVerificationError::MachineSlotMismatch);
    }
    let mut slots = HashMap::with_capacity(machine.slots.len());
    let mut used_slots = HashSet::with_capacity(machine.slots.len());
    for (value, slot) in &machine.slots {
        if !definitions.contains(value)
            || *slot < crate::FRAME_HEADER_WORDS
            || *slot >= local_end
            || *slot >= machine.frame.frame_words
            || !used_slots.insert(*slot)
            || slots.insert(*value, *slot).is_some()
        {
            return Err(MachineVerificationError::MachineSlotMismatch);
        }
    }
    if slots.len() != definitions.len() {
        return Err(MachineVerificationError::MachineSlotMismatch);
    }
    Ok(slots)
}

fn verify_machine_code_bounds(
    machine: &MachineFunction,
    maps: &[SafepointMap],
    entry_offset: u32,
    code_len: u32,
) -> Result<(), MachineVerificationError> {
    let Some(first_block) = machine.blocks.first() else {
        return Err(MachineVerificationError::MachineCodeBoundsMismatch);
    };
    if entry_offset >= code_len || first_block.offset != entry_offset {
        return Err(MachineVerificationError::MachineCodeBoundsMismatch);
    }
    let mut previous_offset = entry_offset;
    for block in &machine.blocks {
        if block.offset < previous_offset || block.offset >= code_len {
            return Err(MachineVerificationError::MachineCodeBoundsMismatch);
        }
        previous_offset = block.offset;
    }
    if maps
        .iter()
        .any(|map| map.pc_offset < entry_offset || map.pc_offset > code_len)
    {
        return Err(MachineVerificationError::MachineCodeBoundsMismatch);
    }
    Ok(())
}

fn verify_machine_maps(
    function: &Function,
    machine: &MachineFunction,
    slots: &HashMap<ValueId, u32>,
    maps: &[SafepointMap],
) -> Result<(), MachineVerificationError> {
    let expected_flags = machine_safepoint_flags(function);
    if maps.len() != expected_flags.len()
        || maps
            .iter()
            .zip(expected_flags)
            .any(|(map, flags)| map.map_flags != flags)
    {
        return Err(MachineVerificationError::MachineSafepointMismatch);
    }
    let frame_words = u16::try_from(machine.frame.frame_words)
        .map_err(|_| MachineVerificationError::MachineSafepointMismatch)?;
    let mut live_slots = slots
        .values()
        .map(|slot| {
            u16::try_from(*slot).map_err(|_| MachineVerificationError::MachineSafepointMismatch)
        })
        .collect::<Result<Vec<_>, _>>()?;
    live_slots.sort_unstable();
    for map in maps {
        let expected = SafepointMap::new(
            map.pc_offset,
            frame_words,
            frame_words,
            &live_slots,
            &[],
            map.map_flags,
        )
        .map_err(|_| MachineVerificationError::MachineSafepointMismatch)?;
        if *map != expected {
            return Err(MachineVerificationError::MachineSafepointMismatch);
        }
    }
    Ok(())
}

fn machine_safepoint_flags(function: &Function) -> Vec<u32> {
    let mut flags = Vec::new();
    for block in &function.blocks {
        for op in &block.ops {
            match &op.kind {
                ncl_ir::OpKind::Alloc { .. } => flags.push(crate::FLAG_ALLOCATION_SLOW),
                ncl_ir::OpKind::Call { .. }
                | ncl_ir::OpKind::CallIndirect { .. }
                | ncl_ir::OpKind::Builtin { .. }
                | ncl_ir::OpKind::Safepoint => flags.push(crate::FLAG_CALL),
                ncl_ir::OpKind::Const { .. }
                | ncl_ir::OpKind::Move { .. }
                | ncl_ir::OpKind::TypeHint { .. }
                | ncl_ir::OpKind::Load { .. }
                | ncl_ir::OpKind::Store { .. }
                | ncl_ir::OpKind::LoadField { .. }
                | ncl_ir::OpKind::StoreField { .. }
                | ncl_ir::OpKind::LoadArg { .. }
                | ncl_ir::OpKind::LoadCapture { .. }
                | ncl_ir::OpKind::LoadFunctionObject
                | ncl_ir::OpKind::MakeClosure { .. }
                | ncl_ir::OpKind::MakeValueCell { .. }
                | ncl_ir::OpKind::CallClosure { .. }
                | ncl_ir::OpKind::Prim { .. }
                | ncl_ir::OpKind::Compare { .. }
                | ncl_ir::OpKind::Convert { .. }
                | ncl_ir::OpKind::SetMultipleValues { .. }
                | ncl_ir::OpKind::EnterHandler { .. }
                | ncl_ir::OpKind::LeaveHandler { .. } => {}
            }
        }
        match &block.terminator {
            Terminator::CallReturn { .. } | Terminator::TailCall { .. } => {
                flags.push(crate::FLAG_CALL);
            }
            Terminator::Jump { target, .. } if *target == block.id => {
                flags.push(crate::FLAG_LOOP_BACKEDGE);
            }
            Terminator::Jump { .. }
            | Terminator::Branch { .. }
            | Terminator::Switch { .. }
            | Terminator::Return { .. }
            | Terminator::Throw { .. }
            | Terminator::Unreachable => {}
        }
    }
    flags
}

pub fn verify_safepoint_map(
    function: &Function,
    allocation: &Allocation,
    frame: FrameLayout,
    spill_base: u32,
    position: u32,
    flags: u32,
    map: &SafepointMap,
) -> Result<(), MachineVerificationError> {
    let (registers, mut live_slots) = root_locations_at(function, allocation, position, spill_base);
    if let Some(base) = allocation.incoming_args_base {
        for offset in 0..5 {
            let slot = base
                .checked_add(offset)
                .and_then(|slot| crate::FRAME_HEADER_WORDS.checked_add(slot))
                .ok_or(MachineVerificationError::SafepointMismatch)?;
            live_slots.push(slot);
        }
    }
    if flags & crate::FLAG_CALL != 0 {
        for offset in 0..frame.outgoing_words {
            let slot = allocation
                .outgoing_base
                .checked_add(offset)
                .and_then(|slot| crate::FRAME_HEADER_WORDS.checked_add(slot))
                .ok_or(MachineVerificationError::SafepointMismatch)?;
            live_slots.push(slot);
        }
    }
    live_slots.sort_unstable();
    live_slots.dedup();
    let live_slots = live_slots
        .into_iter()
        .map(|slot| u16::try_from(slot).map_err(|_| MachineVerificationError::SafepointMismatch))
        .collect::<Result<Vec<_>, _>>()?;
    let frame_words = u16::try_from(frame.frame_words)
        .map_err(|_| MachineVerificationError::SafepointMismatch)?;
    let expected = SafepointMap::new(
        map.pc_offset,
        frame_words,
        frame_words,
        &live_slots,
        &registers,
        flags,
    )
    .map_err(|_| MachineVerificationError::SafepointMismatch)?;
    if *map == expected {
        Ok(())
    } else {
        Err(MachineVerificationError::SafepointMismatch)
    }
}

fn verify_machine_structure(
    function: &Function,
    machine: &MachineFunction,
    slots: &HashMap<ValueId, u32>,
) -> Result<(), MachineVerificationError> {
    let Some(entry) = function.blocks.first() else {
        return Err(MachineVerificationError::MachineStructureMismatch);
    };
    if machine.entry != entry.id {
        return Err(MachineVerificationError::MachineStructureMismatch);
    }
    if machine.blocks.len() != function.blocks.len() {
        return Err(MachineVerificationError::MachineStructureMismatch);
    }
    let mut block_ids = HashSet::with_capacity(function.blocks.len());
    for (source, lowered) in function.blocks.iter().zip(&machine.blocks) {
        if source.id != lowered.id || !block_ids.insert(source.id) {
            return Err(MachineVerificationError::MachineStructureMismatch);
        }
        let mut expected = Vec::new();
        for op in &source.ops {
            if let Some((value, _)) = op.results.first() {
                let slot = slots
                    .get(value)
                    .copied()
                    .ok_or(MachineVerificationError::MachineSlotMismatch)?;
                expected.push(MachineOp::move_value(slot, slot));
            } else {
                expected.push(MachineOp::Return);
            }
        }
        if matches!(source.terminator, Terminator::Return { .. }) {
            expected.push(MachineOp::Return);
        }
        if lowered.operations != expected {
            return Err(MachineVerificationError::MachineStructureMismatch);
        }
    }
    Ok(())
}
