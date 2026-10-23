use crate::BlockId;
use std::collections::{HashMap, HashSet};

pub(super) fn reachable_blocks(
    entry: BlockId,
    predecessors: &HashMap<BlockId, Vec<BlockId>>,
) -> HashSet<BlockId> {
    let mut successors = HashMap::<BlockId, Vec<BlockId>>::new();
    for (target, incoming) in predecessors {
        for predecessor in incoming {
            successors.entry(*predecessor).or_default().push(*target);
        }
    }
    let mut reachable = HashSet::new();
    let mut pending = vec![entry];
    while let Some(block) = pending.pop() {
        if !reachable.insert(block) {
            continue;
        }
        if let Some(next) = successors.get(&block) {
            pending.extend(next);
        }
    }
    reachable
}
