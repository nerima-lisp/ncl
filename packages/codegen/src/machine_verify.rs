#![allow(missing_docs)]

use crate::{Allocation, AllocationTarget, CodegenError, FrameLayout, Location, allocate};
use ncl_ir::{Function, Ty, ValueId, VerifyError};
use std::collections::HashSet;

mod machine_function;
pub use machine_function::{verify_machine_function, verify_safepoint_map};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointInstruction {
    Call,
    LoopBackedge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SafepointSite {
    pub start_offset: u32,
    pub end_offset: u32,
    pub instruction: SafepointInstruction,
}

impl SafepointSite {
    pub const fn call(start_offset: u32, end_offset: u32) -> Self {
        Self {
            start_offset,
            end_offset,
            instruction: SafepointInstruction::Call,
        }
    }

    pub const fn loop_backedge(start_offset: u32, end_offset: u32) -> Self {
        Self {
            start_offset,
            end_offset,
            instruction: SafepointInstruction::LoopBackedge,
        }
    }
}

pub fn verify_safepoint_sites(
    target: AllocationTarget,
    code: &[u8],
    maps: &[crate::SafepointMap],
    sites: &[SafepointSite],
) -> Result<(), CodegenError> {
    let mismatch =
        || CodegenError::MachineVerification(MachineVerificationError::MachineSafepointPcMismatch);
    if maps.len() != sites.len() {
        return Err(mismatch());
    }
    for (map, site) in maps.iter().zip(sites) {
        let expected_instruction = match (
            map.map_flags & crate::FLAG_LOOP_BACKEDGE != 0,
            map.map_flags & (crate::FLAG_CALL | crate::FLAG_ALLOCATION_SLOW) != 0,
        ) {
            (true, false) => SafepointInstruction::LoopBackedge,
            (false, true) => SafepointInstruction::Call,
            (false, false) | (true, true) => return Err(mismatch()),
        };
        let start = usize::try_from(site.start_offset).map_err(|_| mismatch())?;
        let end = usize::try_from(site.end_offset).map_err(|_| mismatch())?;
        if site.instruction != expected_instruction
            || map.pc_offset != site.end_offset
            || start >= end
            || end > code.len()
        {
            return Err(mismatch());
        }
        let Some(bytes) = code.get(start..end) else {
            return Err(mismatch());
        };
        let valid_instruction = match (target, expected_instruction) {
            (AllocationTarget::X86_64, SafepointInstruction::Call) => bytes == [0x41, 0xff, 0xd3],
            (AllocationTarget::X86_64, SafepointInstruction::LoopBackedge) => {
                bytes.len() == 5 && bytes.first() == Some(&0xe9)
            }
            (AllocationTarget::AArch64, SafepointInstruction::Call) => {
                bytes == [0x20, 0x02, 0x3f, 0xd6]
            }
            (AllocationTarget::AArch64, SafepointInstruction::LoopBackedge) => bytes
                .try_into()
                .map(u32::from_le_bytes)
                .is_ok_and(|instruction| instruction & 0xfc00_0000 == 0x1400_0000),
        };
        if !valid_instruction {
            return Err(mismatch());
        }
    }
    Ok(())
}

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
    /// A safepoint map PC does not follow its captured target instruction.
    MachineSafepointPcMismatch,
}

impl core::fmt::Display for MachineVerificationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "machine verification failed: {self:?}")
    }
}

impl std::error::Error for MachineVerificationError {}

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

pub fn verify_ir(function: &Function) -> Result<(), MachineVerificationError> {
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
            if crate::regalloc::is_safepoint(&op.kind) {
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
