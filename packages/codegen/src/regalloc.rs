//! Target-independent linear-scan allocation for the native backends.

#![allow(missing_docs)]

use crate::SafepointMap;
use ncl_ir::{Function, OpKind, Terminator, Ty, ValueId};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// The target register classes used by the phase 1b allocator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllocationTarget {
    X86_64,
    AArch64,
}

/// A value location after allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    Register(u16),
    Spill(u32),
}

/// A half-open live interval in linearized IR order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LiveInterval {
    pub value: ValueId,
    pub start: u32,
    pub end: u32,
    pub ty: Ty,
    pub crosses_call: bool,
    pub crosses_handler: bool,
}

/// Result of allocating one IR function.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Allocation {
    pub intervals: Vec<LiveInterval>,
    pub locations: Vec<(ValueId, Location)>,
    pub spill_words: u32,
    pub safepoint_registers: BTreeMap<u32, Vec<u16>>,
}

impl Allocation {
    #[must_use]
    pub fn location(&self, value: ValueId) -> Option<Location> {
        self.locations
            .iter()
            .find(|(id, _)| *id == value)
            .map(|(_, location)| *location)
    }

    /// Builds precise register roots for a safepoint position.
    ///
    /// # Errors
    ///
    /// Returns the map error when a register id cannot be represented.
    pub fn register_mask(&self, position: u32) -> Result<u16, crate::MapError> {
        let registers = self
            .safepoint_registers
            .get(&position)
            .map_or(&[][..], Vec::as_slice);
        SafepointMap::build_register_mask(registers)
    }
}

/// Computes intervals and performs linear-scan allocation without external crates.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn allocate(function: &Function, target: AllocationTarget) -> Allocation {
    let mut definitions = HashMap::<ValueId, (u32, Ty)>::new();
    let mut last_use = HashMap::<ValueId, u32>::new();
    let mut call_positions = BTreeSet::new();
    let mut safepoints = Vec::new();
    let mut handler_values = HashSet::new();
    let mut position = 0u32;

    for (index, parameter) in function.params.iter().enumerate() {
        if let Ok(value) = u32::try_from(index) {
            definitions.insert(ValueId(value), (position, parameter.ty));
        }
    }

    for block in &function.blocks {
        for param in &block.params {
            definitions.insert(param.value, (position, param.ty));
        }
        for op in &block.ops {
            for (value, ty) in &op.results {
                definitions.insert(*value, (position, *ty));
            }
            let mut operands = Vec::new();
            operands_of_op(&op.kind, &mut operands);
            for value in operands {
                last_use.insert(value, position);
            }
            if is_call(&op.kind) || matches!(op.kind, OpKind::Safepoint) {
                call_positions.insert(position);
            }
            if matches!(op.kind, OpKind::Safepoint) || is_call(&op.kind) {
                safepoints.push(position);
            }
            position = position.saturating_add(1);
        }
        let mut operands = Vec::new();
        operands_of_terminator(&block.terminator, &mut operands);
        for value in operands {
            last_use.insert(value, position);
        }
        if matches!(block.terminator, Terminator::Jump { target, .. } if target == block.id) {
            safepoints.push(position);
        }
        position = position.saturating_add(1);
    }

    let block_positions = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<HashMap<_, _>>();
    for region in &function.handler_regions {
        let mut handler_blocks = vec![region.handler];
        if let Some(cleanup) = region.cleanup {
            handler_blocks.push(cleanup);
        }
        for block_id in handler_blocks {
            if let Some(block) = block_positions.get(&block_id) {
                for param in &block.params {
                    handler_values.insert(param.value);
                }
                for op in &block.ops {
                    for (value, _) in &op.results {
                        handler_values.insert(*value);
                    }
                    let mut operands = Vec::new();
                    operands_of_op(&op.kind, &mut operands);
                    handler_values.extend(operands);
                }
            }
        }
        handler_values.extend(region.catch_tag);
        handler_values.extend(region.binding_targets.iter().copied());
    }

    let mut intervals = definitions
        .into_iter()
        .map(|(value, (start, ty))| {
            let end = last_use.get(&value).copied().unwrap_or(start);
            let crosses_call = call_positions
                .iter()
                .any(|&call| start < call && call <= end);
            let crosses_handler = handler_values.contains(&value);
            LiveInterval {
                value,
                start,
                end,
                ty,
                crosses_call,
                crosses_handler,
            }
        })
        .collect::<Vec<_>>();
    intervals.sort_by_key(|interval| (interval.start, interval.end, interval.value));

    let registers = allocatable_registers(target);
    let mut active = Vec::<(LiveInterval, u16)>::new();
    let mut locations = Vec::new();
    let mut next_spill = 0u32;
    for interval in intervals.iter().copied() {
        active.retain(|(old, _)| old.end >= interval.start);
        let occupied = active
            .iter()
            .map(|(_, register)| *register)
            .collect::<BTreeSet<_>>();
        let location = if interval.crosses_handler
            || (interval.crosses_call && target == AllocationTarget::AArch64)
        {
            let slot = next_spill;
            next_spill = next_spill.saturating_add(1);
            Location::Spill(slot)
        } else if let Some(register) = registers
            .iter()
            .copied()
            .find(|register| !occupied.contains(register))
        {
            active.push((interval, register));
            Location::Register(register)
        } else {
            let slot = next_spill;
            next_spill = next_spill.saturating_add(1);
            Location::Spill(slot)
        };
        locations.push((interval.value, location));
    }

    let mut safepoint_registers = BTreeMap::new();
    for point in safepoints {
        let mut roots = locations
            .iter()
            .filter_map(|(value, location)| {
                let interval = intervals.iter().find(|interval| interval.value == *value)?;
                (*location).into_register().filter(|_| {
                    interval.ty == Ty::Word && interval.start <= point && point <= interval.end
                })
            })
            .collect::<Vec<_>>();
        roots.sort_unstable();
        roots.dedup();
        safepoint_registers.insert(point, roots);
    }
    Allocation {
        intervals,
        locations,
        spill_words: next_spill,
        safepoint_registers,
    }
}

impl Location {
    const fn into_register(self) -> Option<u16> {
        if let Self::Register(register) = self {
            Some(register)
        } else {
            None
        }
    }
}

const fn allocatable_registers(target: AllocationTarget) -> &'static [u16] {
    match target {
        AllocationTarget::X86_64 => &[10, 11, 12, 13],
        AllocationTarget::AArch64 => &[6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    }
}

const fn is_call(kind: &OpKind) -> bool {
    matches!(
        kind,
        OpKind::Call { .. }
            | OpKind::CallIndirect { .. }
            | OpKind::CallClosure { .. }
            | OpKind::Builtin { .. }
            | OpKind::MakeClosure { .. }
    )
}

fn operands_of_op(kind: &OpKind, out: &mut Vec<ValueId>) {
    match kind {
        OpKind::Move { value }
        | OpKind::Load { address: value }
        | OpKind::Convert { value, .. } => out.push(*value),
        OpKind::Store { address, value }
        | OpKind::StoreField {
            object: address,
            value,
            ..
        } => {
            out.extend([*address, *value]);
        }
        OpKind::LoadField { object, .. } => out.push(*object),
        OpKind::Call { function, args }
        | OpKind::CallIndirect {
            callee: function,
            args,
        } => {
            out.push(*function);
            out.extend(args);
        }
        OpKind::MakeClosure { entry, captures } => {
            out.push(*entry);
            out.extend(captures);
        }
        OpKind::CallClosure { closure, args } => {
            out.push(*closure);
            out.extend(args);
        }
        OpKind::Builtin { args, .. } | OpKind::SetMultipleValues { values: args } => {
            out.extend(args);
        }
        OpKind::Prim { args, .. } => out.extend(args),
        OpKind::Compare { left, right, .. } => out.extend([*left, *right]),
        OpKind::Const { .. }
        | OpKind::Alloc { .. }
        | OpKind::LoadArg { .. }
        | OpKind::Safepoint
        | OpKind::EnterHandler { .. }
        | OpKind::LeaveHandler { .. } => {}
    }
}

fn operands_of_terminator(terminator: &Terminator, out: &mut Vec<ValueId>) {
    match terminator {
        Terminator::Jump { args, .. } => out.extend(args),
        Terminator::Branch {
            condition,
            then_args,
            else_args,
            ..
        } => {
            out.push(*condition);
            out.extend(then_args);
            out.extend(else_args);
        }
        Terminator::Switch {
            value,
            cases,
            default_args,
            ..
        } => {
            out.push(*value);
            for (_, _, args) in cases {
                out.extend(args);
            }
            out.extend(default_args);
        }
        Terminator::CallReturn { function, args } | Terminator::TailCall { function, args } => {
            out.push(*function);
            out.extend(args);
        }
        Terminator::Return { values } => out.extend(values),
        Terminator::Throw { condition } => out.push(*condition),
        Terminator::Unreachable => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{AllocationTarget, Location, allocate};
    use ncl_ir::{
        Constant, FunctionBuilder, FunctionId, HandlerKind, HandlerRegion, OpKind, Terminator, Ty,
        ValueId,
    };

    #[test]
    fn linear_scan_spills_when_live_values_exceed_registers() {
        let mut builder =
            FunctionBuilder::new(FunctionId(0), "pressure", Vec::new(), vec![Ty::Word]);
        let mut values = Vec::new();
        for index in 0u32..8 {
            builder.add_constant(Constant::Fixnum(i64::from(index)));
            values.push(
                match builder.push_op(
                    OpKind::Const {
                        result: ncl_ir::ConstantIndex(index),
                    },
                    &[Ty::Word],
                ) {
                    Ok(result) => result[0],
                    Err(_) => return,
                },
            );
        }
        if builder
            .terminate(Terminator::Return {
                values: values.clone(),
            })
            .is_err()
        {
            return;
        }
        let allocation = allocate(&builder.finish(), AllocationTarget::X86_64);
        assert!(allocation.spill_words > 0);
        assert!(
            allocation
                .locations
                .iter()
                .any(|(_, location)| matches!(location, Location::Register(_)))
        );
    }

    #[test]
    fn handler_crossing_values_are_spilled_and_not_register_roots() {
        let mut builder = FunctionBuilder::new(
            FunctionId(1),
            "handler-crossing",
            Vec::new(),
            vec![Ty::Word],
        );
        let constant = builder.add_constant(Constant::Fixnum(7));
        let value = builder
            .push_op(OpKind::Const { result: constant }, &[Ty::Word])
            .map_or(ValueId(0), |values| values[0]);
        builder.add_handler_region(HandlerRegion {
            id: ncl_ir::HandlerRegionId(0),
            kind: HandlerKind::UnwindProtect,
            protected: vec![ncl_ir::BlockId(0)],
            handler: ncl_ir::BlockId(0),
            cleanup: Some(ncl_ir::BlockId(0)),
            catch_tag: None,
            binding_targets: vec![value],
            depth: 0,
            parent: None,
        });
        assert!(builder.push_op(OpKind::Safepoint, &[]).is_ok());
        assert!(
            builder
                .terminate(Terminator::Return {
                    values: vec![value]
                })
                .is_ok()
        );

        let allocation = allocate(&builder.finish(), AllocationTarget::X86_64);
        let interval = allocation
            .intervals
            .iter()
            .find(|interval| interval.value == value);
        let Some(interval) = interval else {
            panic!("handler-crossing interval");
        };
        assert!(interval.crosses_handler);
        assert!(matches!(
            allocation.location(value),
            Some(Location::Spill(_))
        ));
        assert!(allocation.safepoint_registers.values().all(Vec::is_empty));
    }
}
