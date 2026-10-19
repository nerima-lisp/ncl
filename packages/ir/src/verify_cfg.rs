use super::{BasicBlock, BlockId, Function, terminator_successors};
use std::collections::{HashMap, HashSet};

fn successors(function: &Function, block: &BasicBlock) -> Vec<BlockId> {
    let mut result = terminator_successors(&block.terminator);
    result.extend(
        function
            .handler_regions
            .iter()
            .filter(|region| region.protected.contains(&block.id))
            .map(|region| region.handler),
    );
    result
}

fn reachable_blocks(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
) -> HashSet<BlockId> {
    let Some(entry) = function.blocks.first().map(|block| block.id) else {
        return HashSet::new();
    };
    let mut reachable = HashSet::from([entry]);
    let mut pending = vec![entry];
    while let Some(id) = pending.pop() {
        let Some(block) = blocks.get(&id) else {
            continue;
        };
        for target in successors(function, block) {
            if blocks.contains_key(&target) && reachable.insert(target) {
                pending.push(target);
            }
        }
    }
    reachable
}

pub(super) fn predecessors(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
) -> HashMap<BlockId, Vec<BlockId>> {
    let mut result = blocks
        .keys()
        .map(|id| (*id, Vec::new()))
        .collect::<HashMap<_, _>>();
    for block in &function.blocks {
        for target in successors(function, block) {
            if let Some(preds) = result.get_mut(&target) {
                preds.push(block.id);
            }
        }
    }
    result
}

pub(super) fn compute_dominators(
    function: &Function,
    blocks: &HashMap<BlockId, &BasicBlock>,
    predecessors: &HashMap<BlockId, Vec<BlockId>>,
) -> HashMap<BlockId, HashSet<BlockId>> {
    let all = blocks.keys().copied().collect::<HashSet<_>>();
    let Some(entry) = function.blocks.first().map(|block| block.id) else {
        return HashMap::new();
    };
    let reachable = reachable_blocks(function, blocks);
    let mut dominators = blocks
        .keys()
        .map(|id| {
            (
                *id,
                if *id == entry || predecessors.get(id).is_none_or(Vec::is_empty) {
                    HashSet::from([*id])
                } else {
                    all.clone()
                },
            )
        })
        .collect::<HashMap<_, _>>();
    loop {
        let mut changed = false;
        for id in blocks.keys().copied().filter(|id| *id != entry) {
            let mut incoming = predecessors
                .get(&id)
                .into_iter()
                .flatten()
                .filter(|predecessor| reachable.contains(predecessor) == reachable.contains(&id))
                .filter_map(|pred| dominators.get(pred));
            let Some(first) = incoming.next() else {
                continue;
            };
            let mut next = first.clone();
            for set in incoming {
                next.retain(|candidate| set.contains(candidate));
            }
            next.insert(id);
            if dominators.get(&id) != Some(&next) {
                dominators.insert(id, next);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }
}
