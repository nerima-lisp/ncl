#![allow(missing_docs)]

use crate::{
    Allocation, AllocationTarget, CodegenError, FrameLayout, Location, MachineFunction, MachineOp,
    SafepointMap, allocate,
};
use ncl_ir::{Function, Terminator, Ty, ValueId, VerifyError};
use std::collections::{HashMap, HashSet};

/// A structural failure found in allocated machine state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MachineVerificationError {
    /// The input IR contains a non-warning verifier error.
    InvalidIr(Vec<VerifyError>),
    /// Recomputed live intervals do not match the supplied allocation.
    IntervalMismatch,
    /// Recomputed locations or spill count do not match the supplied allocation.
    LocationMismatch,
    /// Safepoint roots do not match the recomputed allocation.
    SafepointMismatch,
    /// Frame areas or their allocation bases do not match the target layout.
    FrameMismatch,
    /// The fixed-slot table does not cover the IR definitions within its frame area.
    MachineSlotMismatch,
    /// The lowered blocks or their retained operations do not match the IR function.
    MachineStructureMismatch,
    /// A generated safepoint map does not match the frame and machine slots.
    MachineSafepointMismatch,
    /// A block or safepoint program counter is outside the encoded function.
    MachineCodeBoundsMismatch,
}

impl core::fmt::Display for MachineVerificationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "machine verification failed: {self:?}")
    }
}

impl std::error::Error for MachineVerificationError {}

pub(crate) fn verify_machine_function(
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

pub(crate) fn verify_safepoint_map(
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
    if machine.entry != entry.id || machine.blocks.len() != function.blocks.len() {
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

pub fn verify_allocation(
    function: &Function,
    target: AllocationTarget,
    allocation: &Allocation,
    frame: FrameLayout,
    outgoing_words: u32,
    actual_spill_base: u32,
    callee_saved_base: Option<u32>,
) -> Result<(), CodegenError> {
    verify_ir(function)?;
    verify_recomputed_allocation(function, target, allocation)?;
    let areas = frame_areas(function, target, allocation.spill_words)?;
    verify_frame_layout(
        target,
        allocation,
        frame,
        outgoing_words,
        actual_spill_base,
        callee_saved_base,
        areas,
    )?;
    verify_safepoint_maps(function, allocation, frame, areas.spill_base)?;
    Ok(())
}

fn verify_ir(function: &Function) -> Result<(), MachineVerificationError> {
    if let Err(errors) = ncl_ir::verify(function) {
        let errors = errors
            .into_iter()
            .filter(|error| !matches!(error, VerifyError::SafepointWarning(_)))
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            return Err(MachineVerificationError::InvalidIr(errors));
        }
    }
    Ok(())
}

fn verify_recomputed_allocation(
    function: &Function,
    target: AllocationTarget,
    allocation: &Allocation,
) -> Result<(), MachineVerificationError> {
    let mut expected = allocate(function, target);
    expected.incoming_args_base = allocation.incoming_args_base;
    expected.outgoing_base = allocation.outgoing_base;
    if expected.intervals != allocation.intervals {
        return Err(MachineVerificationError::IntervalMismatch);
    }
    if !valid_locations(allocation, target) {
        return Err(MachineVerificationError::LocationMismatch);
    }
    if expected.locations != allocation.locations || expected.spill_words != allocation.spill_words
    {
        return Err(MachineVerificationError::LocationMismatch);
    }
    if expected.safepoint_registers != allocation.safepoint_registers {
        return Err(MachineVerificationError::SafepointMismatch);
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct FrameAreas {
    arguments: u32,
    values: u32,
    incoming: u32,
    callee_saved: u32,
    spill_base: u32,
    incoming_base: Option<u32>,
    outgoing_base: u32,
}

fn frame_areas(
    function: &Function,
    target: AllocationTarget,
    spill_words: u32,
) -> Result<FrameAreas, MachineVerificationError> {
    let generated_lambda = function
        .params
        .first()
        .is_some_and(|parameter| parameter.name == "argc");
    match target {
        AllocationTarget::X86_64 => {
            let arguments = u32::try_from(function.params.len())
                .map_err(|_| MachineVerificationError::FrameMismatch)?;
            let values = function
                .blocks
                .iter()
                .try_fold(0u32, |count, block| {
                    let params = u32::try_from(block.params.len()).ok()?;
                    let ops = block.ops.iter().try_fold(0u32, |total, op| {
                        total.checked_add(u32::try_from(op.results.len()).ok()?)
                    })?;
                    count.checked_add(params)?.checked_add(ops)
                })
                .ok_or(MachineVerificationError::FrameMismatch)?;
            let spill_base = arguments
                .checked_add(values)
                .ok_or(MachineVerificationError::FrameMismatch)?;
            let incoming = if generated_lambda { 5 } else { 0 };
            let incoming_base = if generated_lambda {
                Some(
                    spill_base
                        .checked_add(spill_words)
                        .ok_or(MachineVerificationError::FrameMismatch)?,
                )
            } else {
                None
            };
            let outgoing_base = spill_base
                .checked_add(spill_words)
                .and_then(|base| base.checked_add(incoming))
                .ok_or(MachineVerificationError::FrameMismatch)?;
            Ok(FrameAreas {
                arguments,
                values,
                incoming,
                callee_saved: 5,
                spill_base,
                incoming_base,
                outgoing_base,
            })
        }
        AllocationTarget::AArch64 => {
            let incoming = if generated_lambda { 5 } else { 0 };
            let incoming_base = generated_lambda.then_some(spill_words);
            let outgoing_base = spill_words
                .checked_add(incoming)
                .ok_or(MachineVerificationError::FrameMismatch)?;
            Ok(FrameAreas {
                arguments: 0,
                values: 0,
                incoming,
                callee_saved: 0,
                spill_base: 0,
                incoming_base,
                outgoing_base,
            })
        }
    }
}

fn verify_frame_layout(
    target: AllocationTarget,
    allocation: &Allocation,
    frame: FrameLayout,
    outgoing_words: u32,
    actual_spill_base: u32,
    callee_saved_base: Option<u32>,
    areas: FrameAreas,
) -> Result<(), MachineVerificationError> {
    if actual_spill_base != areas.spill_base {
        return Err(MachineVerificationError::FrameMismatch);
    }
    let locals = areas
        .values
        .checked_add(allocation.spill_words)
        .and_then(|words| words.checked_add(areas.incoming))
        .and_then(|words| words.checked_add(areas.callee_saved))
        .ok_or(MachineVerificationError::FrameMismatch)?;
    let expected_callee_base = if target == AllocationTarget::X86_64 {
        Some(
            areas
                .outgoing_base
                .checked_add(outgoing_words)
                .ok_or(MachineVerificationError::FrameMismatch)?,
        )
    } else {
        None
    };
    let allocation_incoming_args_base = allocation.incoming_args_base;
    let expected_incoming_args_base = areas.incoming_base;
    if allocation_incoming_args_base != expected_incoming_args_base
        || allocation.outgoing_base != areas.outgoing_base
        || callee_saved_base != expected_callee_base
    {
        return Err(MachineVerificationError::FrameMismatch);
    }
    let expected_frame = FrameLayout::new(areas.arguments, locals, outgoing_words)
        .map_err(|_| MachineVerificationError::FrameMismatch)?;
    if frame != expected_frame {
        return Err(MachineVerificationError::FrameMismatch);
    }
    Ok(())
}

fn verify_safepoint_maps(
    function: &Function,
    allocation: &Allocation,
    frame: FrameLayout,
    spill_base: u32,
) -> Result<(), MachineVerificationError> {
    let spill_end = spill_base
        .checked_add(allocation.spill_words)
        .ok_or(MachineVerificationError::FrameMismatch)?;
    let first_spill_slot = crate::FRAME_HEADER_WORDS
        .checked_add(spill_base)
        .ok_or(MachineVerificationError::FrameMismatch)?;
    let end_spill_slot = crate::FRAME_HEADER_WORDS
        .checked_add(spill_end)
        .ok_or(MachineVerificationError::FrameMismatch)?;
    for position in safepoint_positions(function) {
        let (registers, roots) = root_locations_at(function, allocation, position, spill_base);
        if allocation.safepoint_registers.get(&position) != Some(&registers)
            || roots.iter().any(|slot| {
                *slot < first_spill_slot || *slot >= end_spill_slot || *slot >= frame.frame_words
            })
        {
            return Err(MachineVerificationError::SafepointMismatch);
        }
    }
    Ok(())
}

fn valid_locations(allocation: &Allocation, target: AllocationTarget) -> bool {
    let registers = crate::regalloc::allocatable_registers(target);
    let mut values = HashSet::new();
    if allocation.locations.len() != allocation.intervals.len() {
        return false;
    }
    for (value, location) in &allocation.locations {
        if !values.insert(*value) {
            return false;
        }
        let Some(interval) = allocation
            .intervals
            .iter()
            .find(|item| item.value == *value)
        else {
            return false;
        };
        match location {
            Location::Register(register)
                if !registers.contains(register)
                    || interval.crosses_call
                    || interval.crosses_safepoint
                    || interval.crosses_handler =>
            {
                return false;
            }
            Location::Spill(spill) if *spill >= allocation.spill_words => return false,
            Location::Register(_) | Location::Spill(_) => {}
        }
    }
    for (index, left) in allocation.intervals.iter().enumerate() {
        for right in allocation.intervals.iter().skip(index.saturating_add(1)) {
            if left.start <= right.end
                && right.start <= left.end
                && allocation.location(left.value) == allocation.location(right.value)
            {
                return false;
            }
        }
    }
    true
}

pub fn root_locations_at(
    function: &Function,
    allocation: &Allocation,
    position: u32,
    spill_base: u32,
) -> (Vec<u16>, Vec<u32>) {
    let mut entry_roots = (0..function.params.len())
        .filter_map(|index| u32::try_from(index).ok())
        .map(ValueId)
        .collect::<HashSet<_>>();
    entry_roots.extend(
        function
            .blocks
            .iter()
            .flat_map(|block| block.params.iter().map(|parameter| parameter.value)),
    );
    let mut registers = Vec::new();
    let mut slots = Vec::new();
    for interval in &allocation.intervals {
        let at_definition = interval.start == position && entry_roots.contains(&interval.value);
        if interval.ty != Ty::Word
            || !(at_definition || interval.start < position)
            || position > interval.end
        {
            continue;
        }
        match allocation.location(interval.value) {
            Some(Location::Register(register)) => registers.push(register),
            Some(Location::Spill(spill)) => {
                if let Some(slot) = spill_base
                    .checked_add(spill)
                    .and_then(|slot| slot.checked_add(crate::FRAME_HEADER_WORDS))
                {
                    slots.push(slot);
                }
            }
            None => {}
        }
    }
    registers.sort_unstable();
    registers.dedup();
    slots.sort_unstable();
    slots.dedup();
    (registers, slots)
}

fn safepoint_positions(function: &Function) -> HashSet<u32> {
    let mut positions = HashSet::new();
    let mut position = 0u32;
    for block in &function.blocks {
        for op in &block.ops {
            if matches!(
                op.kind,
                ncl_ir::OpKind::Safepoint | ncl_ir::OpKind::Alloc { .. }
            ) || crate::regalloc::is_call(&op.kind)
            {
                positions.insert(position);
            }
            position = position.saturating_add(1);
        }
        if matches!(block.terminator, ncl_ir::Terminator::CallReturn { .. })
            || matches!(block.terminator, ncl_ir::Terminator::Jump { target, .. } if target == block.id)
        {
            positions.insert(position);
        }
        position = position.saturating_add(1);
    }
    positions
}
