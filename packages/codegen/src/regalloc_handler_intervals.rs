//! A handler region's contribution to live-interval accounting: values
//! referenced only by its handler/cleanup blocks or by its `catch_tag`/
//! `binding_targets` (which reach `EnterHandler` through the region table,
//! not as an `OpKind` operand the main scan in `allocate` would see).

use std::collections::{HashMap, HashSet};

use ncl_ir::{Function, HandlerRegionId, ValueId};

use super::operands_of_op;

pub(super) fn extend_for_handler_regions(
    function: &Function,
    handler_values: &mut HashSet<ValueId>,
    last_use: &mut HashMap<ValueId, u32>,
    enter_positions: &HashMap<HandlerRegionId, u32>,
) {
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
        // Give each a `last_use` at the `EnterHandler` call's own position.
        if let Some(&enter_position) = enter_positions.get(&region.id) {
            for value in region.catch_tag.iter().chain(&region.binding_targets) {
                last_use
                    .entry(*value)
                    .and_modify(|end| *end = (*end).max(enter_position))
                    .or_insert(enter_position);
            }
        }
    }
}
