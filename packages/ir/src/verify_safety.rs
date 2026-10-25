use super::{AddressLivenessBoundary, VerifyError, compute_dominators, verify_detail};
use crate::{BasicBlock, BlockId, Function, Op, OpKind, Terminator, ValueId};
use std::collections::{HashMap, HashSet};
use verify_detail::successors;

#[path = "verify_safety/memory.rs"]
mod memory;

pub(super) fn check_safepoints(
    block: &BasicBlock,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
    errors: &mut Vec<VerifyError>,
) {
    for (index, op) in block.ops.iter().enumerate() {
        let preceded_by_safepoint = index
            .checked_sub(1)
            .and_then(|previous| block.ops.get(previous))
            .is_some_and(|previous| matches!(previous.kind, OpKind::Safepoint));
        let followed_by_safepoint = index
            .checked_add(1)
            .and_then(|next| block.ops.get(next))
            .is_some_and(|next| matches!(next.kind, OpKind::Safepoint));
        if matches!(
            op.kind,
            OpKind::Call { .. }
                | OpKind::CallIndirect { .. }
                | OpKind::CallClosure { .. }
                | OpKind::Builtin { .. }
        ) && !preceded_by_safepoint
        {
            errors.push(VerifyError::SafepointWarning(block.id));
        }
        if matches!(op.kind, OpKind::Alloc { .. }) && !followed_by_safepoint {
            errors.push(VerifyError::SafepointWarning(block.id));
        }
    }
    if successors(&block.terminator).into_iter().any(|target| {
        dominators
            .get(&block.id)
            .is_some_and(|set| set.contains(&target))
    }) && !matches!(block.ops.last().map(|op| &op.kind), Some(OpKind::Safepoint))
    {
        errors.push(VerifyError::SafepointWarning(block.id));
    }
}

pub(super) fn check_address_liveness(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    errors: &mut Vec<VerifyError>,
) {
    let address_values = memory::address_provenance(function, blocks);
    if address_values.is_empty() {
        return;
    }
    memory::check_stored_address_boundaries(function, blocks, &address_values, errors);

    let mut cfg_predecessors = blocks
        .keys()
        .map(|id| (*id, Vec::new()))
        .collect::<HashMap<_, _>>();
    for block in &function.blocks {
        for target in successors(&block.terminator)
            .into_iter()
            .chain(block.ops.iter().flat_map(verify_detail::op_successors))
        {
            if let Some(predecessors) = cfg_predecessors.get_mut(&target) {
                predecessors.push(block.id);
            }
        }
    }

    let mut handler_successors = HashMap::<BlockId, Vec<BlockId>>::new();
    for region in &function.handler_regions {
        for protected in &region.protected {
            if !blocks.contains_key(protected) {
                continue;
            }
            let targets = handler_successors.entry(*protected).or_default();
            targets.push(region.handler);
            if let Some(cleanup) = region.cleanup {
                targets.push(cleanup);
            }
        }
    }
    for (source, targets) in &handler_successors {
        for target in targets {
            if let Some(predecessors) = cfg_predecessors.get_mut(target) {
                predecessors.push(*source);
            }
        }
    }

    let dominators = compute_dominators(function, blocks, &cfg_predecessors);
    let (live_in, live_entry) = compute_address_liveness(function, blocks, &handler_successors);
    let empty_liveness = HashSet::new();

    for block in &function.blocks {
        let terminator_live = terminator_live_values(
            block,
            blocks,
            &live_in,
            &live_entry,
            handler_successors.get(&block.id),
        );
        if is_call_terminator(&block.terminator) {
            report_address_values(
                block.id,
                &terminator_live,
                &address_values,
                AddressLivenessBoundary::Call,
                errors,
            );
        }

        for (target, args) in verify_detail::successor_edges(&block.terminator) {
            if is_backedge(block.id, target, &dominators) {
                let edge_live = edge_live_values(target, &args, blocks, &live_entry);
                report_address_values(
                    block.id,
                    &edge_live,
                    &address_values,
                    AddressLivenessBoundary::LoopBackedge { target },
                    errors,
                );
            }
        }

        let mut live = terminator_live_values(
            block,
            blocks,
            &live_in,
            &live_entry,
            handler_successors.get(&block.id),
        );
        for op in block.ops.iter().rev() {
            for target in verify_detail::op_successors(op) {
                live.extend(live_in.get(&target).into_iter().flatten().copied());
                if is_backedge(block.id, target, &dominators) {
                    report_address_values(
                        block.id,
                        live_in.get(&target).unwrap_or(&empty_liveness),
                        &address_values,
                        AddressLivenessBoundary::LoopBackedge { target },
                        errors,
                    );
                }
            }
            live.extend(live_from_handlers(
                handler_successors.get(&block.id),
                &live_in,
            ));

            let mut live_before = live.clone();
            for (value, _) in &op.results {
                live_before.remove(value);
            }
            live_before.extend(verify_detail::op_uses(op));

            if let Some(boundary) = op_boundary(&op.kind) {
                report_address_values(block.id, &live_before, &address_values, boundary, errors);
            }

            for target in handler_successors
                .get(&block.id)
                .into_iter()
                .flatten()
                .copied()
            {
                if is_backedge(block.id, target, &dominators) {
                    report_address_values(
                        block.id,
                        live_in.get(&target).unwrap_or(&empty_liveness),
                        &address_values,
                        AddressLivenessBoundary::LoopBackedge { target },
                        errors,
                    );
                }
            }
            live = live_before;
        }
    }
}

fn propagates_address_provenance(op: &Op, address_values: &HashSet<ValueId>) -> bool {
    match &op.kind {
        OpKind::Move { value } | OpKind::TypeHint { value, .. } | OpKind::Convert { value, .. } => {
            address_values.contains(value)
        }
        OpKind::Prim {
            op:
                crate::Prim::FixnumAdd
                | crate::Prim::FixnumSub
                | crate::Prim::FixnumMul
                | crate::Prim::FixnumDiv,
            args,
            ..
        } => args.iter().any(|value| address_values.contains(value)),
        OpKind::Const { .. }
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
        | OpKind::SetMultipleValues { .. }
        | OpKind::Safepoint
        | OpKind::EnterHandler { .. }
        | OpKind::LeaveHandler { .. } => false,
    }
}

fn compute_address_liveness(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    handler_successors: &HashMap<BlockId, Vec<BlockId>>,
) -> (
    HashMap<BlockId, HashSet<ValueId>>,
    HashMap<BlockId, HashSet<ValueId>>,
) {
    let mut live_in = blocks
        .keys()
        .map(|id| (*id, HashSet::new()))
        .collect::<HashMap<_, _>>();
    let mut live_entry = live_in.clone();

    loop {
        let mut changed = false;
        for block in function.blocks.iter().rev() {
            let mut live = terminator_live_values(
                block,
                blocks,
                &live_in,
                &live_entry,
                handler_successors.get(&block.id),
            );
            for op in block.ops.iter().rev() {
                for target in verify_detail::op_successors(op) {
                    live.extend(live_in.get(&target).into_iter().flatten().copied());
                }
                live.extend(live_from_handlers(
                    handler_successors.get(&block.id),
                    &live_in,
                ));
                for (value, _) in &op.results {
                    live.remove(value);
                }
                live.extend(verify_detail::op_uses(op));
            }
            let entry = live;
            let mut external = entry.clone();
            for parameter in &block.params {
                external.remove(&parameter.value);
            }
            if live_entry.get(&block.id) != Some(&entry) {
                live_entry.insert(block.id, entry);
                changed = true;
            }
            if live_in.get(&block.id) != Some(&external) {
                live_in.insert(block.id, external);
                changed = true;
            }
        }
        if !changed {
            return (live_in, live_entry);
        }
    }
}

fn terminator_live_values(
    block: &BasicBlock,
    blocks: &HashMap<BlockId, &BasicBlock>,
    live_in: &HashMap<BlockId, HashSet<ValueId>>,
    live_entry: &HashMap<BlockId, HashSet<ValueId>>,
    handler_targets: Option<&Vec<BlockId>>,
) -> HashSet<ValueId> {
    let mut live = verify_detail::terminator_uses(&block.terminator)
        .into_iter()
        .collect::<HashSet<_>>();
    for (target, args) in verify_detail::successor_edges(&block.terminator) {
        live.extend(edge_live_values(target, &args, blocks, live_entry));
    }
    live.extend(live_from_handlers(handler_targets, live_in));
    live
}

fn edge_live_values(
    target: BlockId,
    args: &[ValueId],
    blocks: &HashMap<BlockId, &BasicBlock>,
    live_entry: &HashMap<BlockId, HashSet<ValueId>>,
) -> HashSet<ValueId> {
    let Some(destination) = blocks.get(&target) else {
        return HashSet::new();
    };
    let mut live = live_entry.get(&target).cloned().unwrap_or_default();
    for (parameter, argument) in destination.params.iter().zip(args) {
        if live.remove(&parameter.value) {
            live.insert(*argument);
        }
    }
    live
}

fn live_from_handlers(
    handler_targets: Option<&Vec<BlockId>>,
    live_in: &HashMap<BlockId, HashSet<ValueId>>,
) -> HashSet<ValueId> {
    handler_targets
        .into_iter()
        .flatten()
        .filter_map(|target| live_in.get(target))
        .flatten()
        .copied()
        .collect()
}

fn report_address_values(
    block: BlockId,
    values: &HashSet<ValueId>,
    address_values: &HashSet<ValueId>,
    boundary: AddressLivenessBoundary,
    errors: &mut Vec<VerifyError>,
) {
    let mut live_addresses = values
        .iter()
        .filter(|value| address_values.contains(value))
        .copied()
        .collect::<Vec<_>>();
    live_addresses.sort_by_key(|value| value.0);
    for value in live_addresses {
        errors.push(VerifyError::AddressLiveAcross {
            block,
            value,
            boundary,
        });
    }
}

const fn op_boundary(kind: &OpKind) -> Option<AddressLivenessBoundary> {
    match kind {
        OpKind::Safepoint => Some(AddressLivenessBoundary::Safepoint),
        OpKind::Alloc { .. } => Some(AddressLivenessBoundary::Alloc),
        OpKind::Call { .. }
        | OpKind::CallIndirect { .. }
        | OpKind::CallClosure { .. }
        | OpKind::MakeClosure { .. }
        | OpKind::MakeValueCell { .. }
        | OpKind::Builtin { .. }
        | OpKind::StoreField { .. }
        | OpKind::EnterHandler { .. }
        | OpKind::LeaveHandler { .. }
        | OpKind::Prim {
            op:
                crate::Prim::Rplaca
                | crate::Prim::Rplacd
                | crate::Prim::Aset
                | crate::Prim::FixnumDiv
                | crate::Prim::Typep
                | crate::Prim::CharacterPredicate(_)
                | crate::Prim::StructureSlot(_),
            ..
        } => Some(AddressLivenessBoundary::Call),
        OpKind::Const { .. }
        | OpKind::Move { .. }
        | OpKind::TypeHint { .. }
        | OpKind::Load { .. }
        | OpKind::Store { .. }
        | OpKind::LoadField { .. }
        | OpKind::LoadArg { .. }
        | OpKind::LoadCapture { .. }
        | OpKind::LoadFunctionObject
        | OpKind::Prim {
            op:
                crate::Prim::Car
                | crate::Prim::Cdr
                | crate::Prim::Svref
                | crate::Prim::Aref
                | crate::Prim::FixnumAdd
                | crate::Prim::FixnumSub
                | crate::Prim::FixnumMul
                | crate::Prim::FixnumLt
                | crate::Prim::FixnumLe
                | crate::Prim::FixnumEq
                | crate::Prim::Eq
                | crate::Prim::Eql,
            ..
        }
        | OpKind::Compare { .. }
        | OpKind::Convert { .. }
        | OpKind::SetMultipleValues { .. } => None,
    }
}

const fn is_call_terminator(term: &Terminator) -> bool {
    matches!(
        term,
        Terminator::CallReturn { .. } | Terminator::TailCall { .. }
    )
}

fn is_backedge(
    source: BlockId,
    target: BlockId,
    dominators: &HashMap<BlockId, HashSet<BlockId>>,
) -> bool {
    dominators
        .get(&source)
        .is_some_and(|set| set.contains(&target))
}
