use super::super::{successors, verify_detail};
use crate::{BasicBlock, BlockId, Function, OpKind, ValueId};
use std::collections::{HashMap, HashSet};

pub(super) fn multiple_value_address_provenance(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    address_values: &HashSet<ValueId>,
) -> HashMap<BlockId, HashSet<ValueId>> {
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
    loop {
        let mut changed = false;
        for block in &function.blocks {
            let mut values = HashSet::new();
            if let Some(block_predecessors) = predecessors.get(&block.id) {
                for predecessor in block_predecessors {
                    values.extend(outgoing.get(predecessor).into_iter().flatten().copied());
                }
            }
            if incoming.get(&block.id) != Some(&values) {
                incoming.insert(block.id, values.clone());
                changed = true;
            }
            for op in &block.ops {
                if let OpKind::SetMultipleValues { values: new_values } = &op.kind {
                    values = new_values
                        .iter()
                        .filter(|value| address_values.contains(value))
                        .copied()
                        .collect();
                }
            }
            if outgoing.get(&block.id) != Some(&values) {
                outgoing.insert(block.id, values);
                changed = true;
            }
        }
        if !changed {
            return incoming;
        }
    }
}
