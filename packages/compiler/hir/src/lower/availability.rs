use crate::ids::BlockId;

use super::FunctionLowerer;

impl FunctionLowerer<'_> {
    pub(super) fn node_available_in_block(
        &self,
        use_block: Option<BlockId>,
        definition_block: Option<BlockId>,
    ) -> bool {
        match (definition_block, use_block) {
            (Some(definition), Some(use_block)) => {
                let Some(use_context_index) = self.dominance_contexts.len().checked_sub(1) else {
                    return false;
                };
                let Some(definition_context_index) = self
                    .dominance_contexts
                    .iter()
                    .rposition(|context| context.dominators.contains_key(&definition))
                else {
                    return false;
                };
                if definition_context_index == use_context_index {
                    return self
                        .dominance_contexts
                        .get(use_context_index)
                        .and_then(|context| context.dominators.get(&use_block))
                        .is_some_and(|dominators| dominators.contains(&definition));
                }
                if definition_context_index > use_context_index {
                    return false;
                }
                for child_index in definition_context_index + 1..=use_context_index {
                    let Some(parent) = self.dominance_contexts.get(child_index - 1) else {
                        return false;
                    };
                    let Some(child) = self.dominance_contexts.get(child_index) else {
                        return false;
                    };
                    let Some(parent_block) = child.parent_block else {
                        return false;
                    };
                    let Some(parent_dominators) = parent.dominators.get(&parent_block) else {
                        return false;
                    };
                    if !parent_dominators.contains(&parent.entry)
                        || (child_index == definition_context_index + 1
                            && !parent_dominators.contains(&definition))
                    {
                        return false;
                    }
                }
                self.dominance_contexts
                    .get(use_context_index)
                    .and_then(|context| context.dominators.get(&use_block))
                    .is_some_and(|dominators| {
                        self.dominance_contexts
                            .get(use_context_index)
                            .is_some_and(|context| dominators.contains(&context.entry))
                    })
            }
            (Some(_), None) => false,
            (None, Some(use_block)) => {
                self.external_values_available
                    && self.dominance_contexts.last().is_some_and(|context| {
                        context
                            .dominators
                            .get(&use_block)
                            .is_some_and(|dominators| dominators.contains(&context.entry))
                    })
            }
            (None, None) => true,
        }
    }
}
