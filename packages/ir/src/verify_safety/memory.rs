use super::{
    AddressLivenessBoundary, VerifyError, is_call_terminator, op_boundary,
    propagates_address_provenance, successors, verify_detail,
};
use crate::{BasicBlock, BlockId, Function, OpKind, Ty, ValueId};
use std::collections::{HashMap, HashSet};

#[path = "memory/multiple_values.rs"]
mod multiple_values;

pub(super) fn address_provenance(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
) -> HashSet<ValueId> {
    let mut address_values = HashSet::new();
    for (index, parameter) in function.params.iter().enumerate() {
        if parameter.ty == Ty::Address {
            address_values.insert(ValueId(u32::try_from(index).unwrap_or(u32::MAX)));
        }
    }
    for block in &function.blocks {
        for parameter in &block.params {
            if parameter.ty == Ty::Address {
                address_values.insert(parameter.value);
            }
        }
        for op in &block.ops {
            for (value, ty) in &op.results {
                if *ty == Ty::Address {
                    address_values.insert(*value);
                }
            }
        }
    }

    loop {
        let mut changed = false;
        for block in &function.blocks {
            for op in &block.ops {
                if propagates_address_provenance(op, &address_values) {
                    for (value, ty) in &op.results {
                        if *ty != Ty::Bool && *ty != Ty::Unit {
                            changed |= address_values.insert(*value);
                        }
                    }
                }
            }
            for (target, args) in verify_detail::successor_edges(&block.terminator) {
                let Some(destination) = blocks.get(&target) else {
                    continue;
                };
                for (parameter, argument) in destination.params.iter().zip(args) {
                    if address_values.contains(&argument)
                        && parameter.ty != Ty::Bool
                        && parameter.ty != Ty::Unit
                    {
                        changed |= address_values.insert(parameter.value);
                    }
                }
            }
        }

        let (_, loaded_addresses) = memory_address_provenance(function, blocks, &address_values);
        for value in loaded_addresses {
            changed |= address_values.insert(value);
        }
        if !changed {
            return address_values;
        }
    }
}

pub(super) fn known_address_provenance(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
) -> HashSet<ValueId> {
    // Keep only values whose origin can be traced to a typed Address source.
    let mut known = HashSet::new();
    let mut incoming_args = HashMap::<BlockId, Vec<Vec<ValueId>>>::new();

    for (index, _) in function.params.iter().enumerate() {
        known.insert(ValueId(u32::try_from(index).unwrap_or(u32::MAX)));
    }
    for block in &function.blocks {
        for parameter in &block.params {
            known.insert(parameter.value);
        }
        for op in &block.ops {
            for (value, ty) in &op.results {
                if *ty != Ty::Bool && *ty != Ty::Unit {
                    known.insert(*value);
                }
            }
        }
        for (target, args) in verify_detail::successor_edges(&block.terminator) {
            if blocks.contains_key(&target) {
                incoming_args.entry(target).or_default().push(args);
            }
        }
        for target in block.ops.iter().flat_map(verify_detail::op_successors) {
            if blocks.contains_key(&target) {
                incoming_args.entry(target).or_default().push(Vec::new());
            }
        }
    }
    for region in &function.handler_regions {
        for protected in &region.protected {
            if blocks.contains_key(protected) {
                incoming_args
                    .entry(region.handler)
                    .or_default()
                    .push(Vec::new());
                if let Some(cleanup) = region.cleanup {
                    incoming_args.entry(cleanup).or_default().push(Vec::new());
                }
            }
        }
    }

    loop {
        let previous = known.clone();
        for (index, parameter) in function.params.iter().enumerate() {
            let value = ValueId(u32::try_from(index).unwrap_or(u32::MAX));
            if parameter.ty != Ty::Address {
                known.remove(&value);
            }
        }
        for block in &function.blocks {
            let edges = incoming_args.get(&block.id);
            for (index, parameter) in block.params.iter().enumerate() {
                if parameter.ty == Ty::Address {
                    continue;
                }
                let proven_on_all_edges = edges.is_some_and(|edges| {
                    !edges.is_empty()
                        && edges.iter().all(|args| {
                            args.len() == block.params.len()
                                && args
                                    .get(index)
                                    .is_some_and(|argument| previous.contains(argument))
                        })
                });
                if !proven_on_all_edges {
                    known.remove(&parameter.value);
                }
            }
            for op in &block.ops {
                let proven = match &op.kind {
                    OpKind::Alloc { .. } => true,
                    OpKind::LoadArg { .. } => op.results.iter().all(|(_, ty)| *ty == Ty::Address),
                    OpKind::Move { value } | OpKind::TypeHint { value, .. } => {
                        previous.contains(value)
                    }
                    OpKind::Convert {
                        op: crate::Convert::AddressToWord | crate::Convert::WordToAddress,
                        value,
                    } => previous.contains(value),
                    OpKind::Const { .. }
                    | OpKind::Load { .. }
                    | OpKind::Store { .. }
                    | OpKind::LoadField { .. }
                    | OpKind::StoreField { .. }
                    | OpKind::LoadCapture { .. }
                    | OpKind::LoadFunctionObject
                    | OpKind::Call { .. }
                    | OpKind::CallIndirect { .. }
                    | OpKind::MakeClosure { .. }
                    | OpKind::MakeValueCell { .. }
                    | OpKind::CallClosure { .. }
                    | OpKind::Builtin { .. }
                    | OpKind::Prim { .. }
                    | OpKind::Compare { .. }
                    | OpKind::Convert { .. }
                    | OpKind::SetMultipleValues { .. }
                    | OpKind::Safepoint
                    | OpKind::EnterHandler { .. }
                    | OpKind::LeaveHandler { .. } => false,
                };
                if !proven {
                    for (value, _) in &op.results {
                        known.remove(value);
                    }
                }
            }
        }
        if known == previous {
            break;
        }
    }

    let mut rooted = function
        .params
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.ty == Ty::Address)
        .map(|(index, _)| ValueId(u32::try_from(index).unwrap_or(u32::MAX)))
        .collect::<HashSet<_>>();
    let mut dependencies = HashMap::<ValueId, Vec<ValueId>>::new();
    for block in &function.blocks {
        let edges = incoming_args.get(&block.id);
        for (index, parameter) in block.params.iter().enumerate() {
            if parameter.ty == Ty::Address {
                rooted.insert(parameter.value);
            } else if let Some(edges) = edges {
                dependencies.insert(
                    parameter.value,
                    edges
                        .iter()
                        .filter_map(|args| args.get(index).copied())
                        .collect(),
                );
            }
        }
        for op in &block.ops {
            let (is_root, inputs) = match &op.kind {
                OpKind::Alloc { .. } => (true, Vec::new()),
                OpKind::LoadArg { .. } if op.results.iter().all(|(_, ty)| *ty == Ty::Address) => {
                    (true, Vec::new())
                }
                OpKind::Move { value }
                | OpKind::TypeHint { value, .. }
                | OpKind::Convert {
                    op: crate::Convert::AddressToWord | crate::Convert::WordToAddress,
                    value,
                } => (false, vec![*value]),
                OpKind::Const { .. }
                | OpKind::Load { .. }
                | OpKind::Store { .. }
                | OpKind::LoadField { .. }
                | OpKind::StoreField { .. }
                | OpKind::LoadCapture { .. }
                | OpKind::LoadFunctionObject
                | OpKind::LoadArg { .. }
                | OpKind::Call { .. }
                | OpKind::CallIndirect { .. }
                | OpKind::MakeClosure { .. }
                | OpKind::MakeValueCell { .. }
                | OpKind::CallClosure { .. }
                | OpKind::Builtin { .. }
                | OpKind::Prim { .. }
                | OpKind::Compare { .. }
                | OpKind::Convert { .. }
                | OpKind::SetMultipleValues { .. }
                | OpKind::Safepoint
                | OpKind::EnterHandler { .. }
                | OpKind::LeaveHandler { .. } => (false, Vec::new()),
            };
            for (value, _) in &op.results {
                if is_root {
                    rooted.insert(*value);
                } else if !inputs.is_empty() {
                    dependencies.insert(*value, inputs.clone());
                }
            }
        }
    }
    loop {
        let previous = rooted.len();
        for (value, inputs) in &dependencies {
            if known.contains(value)
                && !inputs.is_empty()
                && inputs.iter().all(|input| rooted.contains(input))
            {
                rooted.insert(*value);
            }
        }
        if rooted.len() == previous {
            break;
        }
    }
    known.retain(|value| rooted.contains(value));
    known
}

fn memory_address_provenance(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    address_values: &HashSet<ValueId>,
) -> (HashMap<BlockId, HashSet<ValueId>>, HashSet<ValueId>) {
    let mut predecessors = blocks
        .keys()
        .map(|id| (*id, Vec::new()))
        .collect::<HashMap<_, _>>();
    for block in &function.blocks {
        for target in successors(&block.terminator)
            .into_iter()
            .chain(block.ops.iter().flat_map(verify_detail::op_successors))
        {
            if let Some(predecessors) = predecessors.get_mut(&target) {
                predecessors.push(block.id);
            }
        }
    }
    for region in &function.handler_regions {
        for protected in &region.protected {
            if let Some(predecessors) = predecessors.get_mut(&region.handler) {
                predecessors.push(*protected);
            }
            if let Some(cleanup) = region.cleanup {
                if let Some(predecessors) = predecessors.get_mut(&cleanup) {
                    predecessors.push(*protected);
                }
            }
        }
    }

    let mut incoming = blocks
        .keys()
        .map(|id| (*id, HashSet::new()))
        .collect::<HashMap<_, _>>();
    let mut outgoing = incoming.clone();
    let mut loaded_addresses = HashSet::new();
    loop {
        let mut changed = false;
        for block in &function.blocks {
            let mut stored_addresses = HashSet::new();
            if let Some(block_predecessors) = predecessors.get(&block.id) {
                for predecessor in block_predecessors {
                    stored_addresses
                        .extend(outgoing.get(predecessor).into_iter().flatten().copied());
                }
            }
            if incoming.get(&block.id) != Some(&stored_addresses) {
                incoming.insert(block.id, stored_addresses.clone());
                changed = true;
            }

            // Raw stores carry no GC-root metadata, so later loads may alias an escaped address.
            for op in &block.ops {
                match &op.kind {
                    OpKind::Load { .. } if !stored_addresses.is_empty() => {
                        for (value, ty) in &op.results {
                            if *ty != Ty::Bool && *ty != Ty::Unit {
                                changed |= loaded_addresses.insert(*value);
                            }
                        }
                    }
                    OpKind::Store { value, .. }
                        if address_values.contains(value) || loaded_addresses.contains(value) =>
                    {
                        stored_addresses.insert(*value);
                    }
                    OpKind::Load { .. }
                    | OpKind::Store { .. }
                    | OpKind::Const { .. }
                    | OpKind::LoadField { .. }
                    | OpKind::StoreField { .. }
                    | OpKind::Alloc { .. }
                    | OpKind::LoadArg { .. }
                    | OpKind::LoadCapture { .. }
                    | OpKind::LoadFunctionObject
                    | OpKind::Call { .. }
                    | OpKind::CallIndirect { .. }
                    | OpKind::MakeClosure { .. }
                    | OpKind::MakeValueCell { .. }
                    | OpKind::CallClosure { .. }
                    | OpKind::Builtin { .. }
                    | OpKind::Prim { .. }
                    | OpKind::Compare { .. }
                    | OpKind::SetMultipleValues { .. }
                    | OpKind::Safepoint
                    | OpKind::EnterHandler { .. }
                    | OpKind::LeaveHandler { .. }
                    | OpKind::Move { .. }
                    | OpKind::TypeHint { .. }
                    | OpKind::Convert { .. } => {}
                }
            }
            if outgoing.get(&block.id) != Some(&stored_addresses) {
                outgoing.insert(block.id, stored_addresses);
                changed = true;
            }
        }
        if !changed {
            return (incoming, loaded_addresses);
        }
    }
}

pub(super) fn check_stored_address_boundaries(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    address_values: &HashSet<ValueId>,
    errors: &mut Vec<VerifyError>,
) {
    let (memory_in, _) = memory_address_provenance(function, blocks, address_values);
    let multiple_values_in =
        multiple_values::multiple_value_address_provenance(function, blocks, address_values);
    for block in &function.blocks {
        let mut stored_addresses = memory_in.get(&block.id).cloned().unwrap_or_default();
        let mut multiple_values = multiple_values_in
            .get(&block.id)
            .cloned()
            .unwrap_or_default();
        for op in &block.ops {
            if let Some(boundary) = op_boundary(&op.kind) {
                for value in stored_addresses.iter().chain(&multiple_values) {
                    let error = VerifyError::AddressLiveAcross {
                        block: block.id,
                        value: *value,
                        boundary,
                    };
                    if !errors.contains(&error) {
                        errors.push(error);
                    }
                }
            }
            match &op.kind {
                OpKind::Store { value, .. } if address_values.contains(value) => {
                    stored_addresses.insert(*value);
                }
                OpKind::SetMultipleValues { values } => {
                    multiple_values = values
                        .iter()
                        .filter(|value| address_values.contains(value))
                        .copied()
                        .collect();
                }
                OpKind::Const { .. }
                | OpKind::Move { .. }
                | OpKind::TypeHint { .. }
                | OpKind::Load { .. }
                | OpKind::Store { .. }
                | OpKind::LoadField { .. }
                | OpKind::StoreField { .. }
                | OpKind::Alloc { .. }
                | OpKind::LoadArg { .. }
                | OpKind::LoadCapture { .. }
                | OpKind::LoadFunctionObject
                | OpKind::Call { .. }
                | OpKind::CallIndirect { .. }
                | OpKind::MakeClosure { .. }
                | OpKind::MakeValueCell { .. }
                | OpKind::CallClosure { .. }
                | OpKind::Builtin { .. }
                | OpKind::Prim { .. }
                | OpKind::Compare { .. }
                | OpKind::Convert { .. }
                | OpKind::Safepoint
                | OpKind::EnterHandler { .. }
                | OpKind::LeaveHandler { .. } => {}
            }
        }
        if is_call_terminator(&block.terminator) {
            for value in stored_addresses.iter().chain(&multiple_values) {
                let error = VerifyError::AddressLiveAcross {
                    block: block.id,
                    value: *value,
                    boundary: AddressLivenessBoundary::Call,
                };
                if !errors.contains(&error) {
                    errors.push(error);
                }
            }
        }
    }
}
