use std::collections::{HashMap, HashSet};

use crate::ids::BlockId;

use super::{ControlFlowGraph, FunctionFlow, terminator_blocks};

pub(super) fn compute(
    control_flow: &ControlFlowGraph,
    flow: &FunctionFlow,
) -> HashMap<BlockId, HashSet<BlockId>> {
    let blocks = flow.blocks.iter().copied().collect::<HashSet<_>>();
    let mut predecessors = blocks
        .iter()
        .copied()
        .map(|block| (block, HashSet::new()))
        .collect::<HashMap<_, _>>();

    for source in &flow.blocks {
        if let Some(block) = control_flow.block(*source) {
            for target in terminator_blocks(block.terminator()).into_iter().flatten() {
                if let Some(predecessors) = predecessors.get_mut(&target) {
                    let _ = predecessors.insert(*source);
                }
            }
        }
    }

    let mut reachable = HashSet::new();
    let mut pending = vec![flow.entry];
    while let Some(block_id) = pending.pop() {
        if reachable.insert(block_id)
            && let Some(block) = control_flow.block(block_id)
        {
            pending.extend(
                terminator_blocks(block.terminator())
                    .into_iter()
                    .flatten()
                    .filter(|successor| blocks.contains(successor)),
            );
        }
    }

    let mut dominators = HashMap::with_capacity(flow.blocks.len());
    for block in &flow.blocks {
        let mut block_dominators = if reachable.contains(block) && *block != flow.entry {
            reachable.clone()
        } else {
            HashSet::new()
        };
        let _ = block_dominators.insert(*block);
        dominators.insert(*block, block_dominators);
    }

    loop {
        let mut changed = false;
        for block in &flow.blocks {
            if *block == flow.entry || !reachable.contains(block) {
                continue;
            }
            let mut incoming = predecessors
                .get(block)
                .into_iter()
                .flat_map(HashSet::iter)
                .filter(|predecessor| reachable.contains(predecessor));
            let mut next = incoming
                .next()
                .and_then(|predecessor| dominators.get(predecessor).cloned())
                .unwrap_or_default();
            for predecessor in incoming {
                if let Some(predecessor_dominators) = dominators.get(predecessor) {
                    next.retain(|candidate| predecessor_dominators.contains(candidate));
                }
            }
            let _ = next.insert(*block);
            if dominators.get(block) != Some(&next) {
                let _ = dominators.insert(*block, next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    dominators
}
