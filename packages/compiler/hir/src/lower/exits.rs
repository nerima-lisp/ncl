use ncl_ir::{Constant, OpKind, Terminator as IrTerminator, Ty, ValueId};

use crate::control::Terminator as HirTerminator;
use crate::ids::{BlockId, NodeId};
use crate::node::TagbodyTag;

use super::{BlockTarget, FunctionLowerer, LowerError};

impl FunctionLowerer<'_> {
    pub(super) fn lower_block(
        &mut self,
        node: NodeId,
        body: &[NodeId],
    ) -> Result<ValueId, LowerError> {
        let source = self.current_block;
        let outer_handlers = self.active_handlers.clone();
        let values = self.builder.fresh_value();
        let exit = self.create_block(vec![(Ty::Word, values)]);
        self.position(source)?;
        let target = BlockTarget {
            block: exit,
            active_depth: outer_handlers.len(),
        };
        let _ = self.block_targets.insert(node, target);
        let value = self.sequence(body)?;
        if !self.is_terminated() {
            let values = self.capture_multiple_values(value)?;
            self.terminate(IrTerminator::Jump {
                target: exit,
                args: vec![values],
            })?;
        }
        let _ = self.block_targets.remove(&node);
        self.active_handlers = outer_handlers;
        self.position(exit)?;
        self.restore_multiple_values(values)
    }

    pub(super) fn return_from(
        &mut self,
        node: NodeId,
        target: NodeId,
        value: Option<NodeId>,
    ) -> Result<ValueId, LowerError> {
        let Some(target_info) = self.block_targets.get(&target).copied() else {
            return Self::unsupported(node, "return-from-cross-function-block");
        };
        let value = match value {
            Some(value) => self.node(value)?,
            None => self.word_constant(Constant::Nil)?,
        };
        if !self.is_terminated() {
            let values = self.capture_multiple_values(value)?;
            self.close_handlers_since(target_info.active_depth)?;
            self.terminate(IrTerminator::Jump {
                target: target_info.block,
                args: vec![values],
            })?;
        }
        Ok(value)
    }

    pub(super) fn tagbody(
        &mut self,
        node: NodeId,
        tags: &[TagbodyTag],
    ) -> Result<ValueId, LowerError> {
        let flow = self
            .form
            .control_flow()
            .function(node)
            .cloned()
            .ok_or(LowerError::MissingTagbodyFlow(node))?;
        let mut blocks = flow
            .blocks()
            .iter()
            .map(|id| {
                self.form
                    .control_flow()
                    .block(*id)
                    .cloned()
                    .map(|block| (*id, block))
                    .ok_or_else(|| LowerError::Ir("verified HIR CFG block is missing".to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let dominators = self
            .form
            .control_flow()
            .dominators(node)
            .ok_or(LowerError::MissingTagbodyFlow(node))?;
        blocks.sort_by_key(|(id, _)| dominators.get(id).map_or(0, std::collections::HashSet::len));
        let source = self.current_block;
        let base_handlers = self.active_handlers.clone();
        let mut targets = Vec::with_capacity(blocks.len());
        for (hir_block, _) in &blocks {
            let mir_block = self.create_block(Vec::new());
            targets.push((*hir_block, mir_block));
        }
        let exit = self.create_block(Vec::new());
        let entry = lookup_tag_target(&targets, flow.entry())?;
        self.position(source)?;
        if !self.is_terminated() {
            self.terminate(IrTerminator::Jump {
                target: entry,
                args: Vec::new(),
            })?;
        }
        for (hir_block, mir_block) in &targets {
            let _ = self
                .tag_targets
                .insert(*hir_block, (*mir_block, base_handlers.len()));
        }
        for tag in tags {
            if !self.tag_targets.contains_key(&tag.block) {
                return Err(LowerError::Ir(
                    "tagbody tag target is absent from its CFG".to_owned(),
                ));
            }
        }

        let previous_hir_block = self.hir_block;
        let parent_block_reachable = self.dominance_contexts.last().is_none_or(|context| {
            self.hir_block.is_some_and(|parent| {
                context
                    .dominators
                    .get(&parent)
                    .is_some_and(|dominators| dominators.contains(&context.entry))
            })
        });
        let previous_external_values_available = self.external_values_available;
        self.external_values_available = parent_block_reachable
            && (self.dominance_contexts.is_empty() || previous_external_values_available);
        self.dominance_contexts.push(super::DominanceContext {
            entry: flow.entry(),
            parent_block: previous_hir_block,
            dominators,
        });
        for (hir_id, hir_block) in blocks {
            let mir_id = lookup_tag_target(&targets, hir_id)?;
            self.position(mir_id)?;
            self.hir_block = Some(hir_id);
            self.active_handlers.clone_from(&base_handlers);
            for operation in hir_block.operations() {
                if self.is_terminated() {
                    break;
                }
                let _ = self.node(*operation)?;
            }
            if !self.is_terminated() {
                self.lower_cfg_terminator(node, hir_block.terminator(), &targets, exit)?;
            }
        }
        self.hir_block = previous_hir_block;
        let _ = self.dominance_contexts.pop();
        self.external_values_available = previous_external_values_available;

        for (hir_block, _) in targets {
            let _ = self.tag_targets.remove(&hir_block);
        }
        self.active_handlers = base_handlers;
        self.position(exit)?;
        self.word_constant(Constant::Nil)
    }

    fn lower_cfg_terminator(
        &mut self,
        node: NodeId,
        terminator: &HirTerminator,
        targets: &[(BlockId, ncl_ir::BlockId)],
        exit: ncl_ir::BlockId,
    ) -> Result<(), LowerError> {
        match terminator {
            HirTerminator::Jump(target) => {
                if self.is_tagbody_backedge(*target) {
                    self.push_none(OpKind::Safepoint)?;
                }
                self.terminate(IrTerminator::Jump {
                    target: lookup_tag_target(targets, *target)?,
                    args: Vec::new(),
                })
            }
            HirTerminator::Conditional {
                condition,
                if_true,
                if_false,
            } => {
                let condition = self.node(*condition)?;
                if self.is_terminated() {
                    return Ok(());
                }
                let nil = self.word_constant(Constant::Nil)?;
                let condition = self.push_one(
                    OpKind::Compare {
                        op: ncl_ir::Compare::Ne,
                        left: condition,
                        right: nil,
                    },
                    Ty::Bool,
                )?;
                if self.is_tagbody_backedge(*if_true) || self.is_tagbody_backedge(*if_false) {
                    self.push_none(OpKind::Safepoint)?;
                }
                self.terminate(IrTerminator::Branch {
                    condition,
                    then_target: lookup_tag_target(targets, *if_true)?,
                    then_args: Vec::new(),
                    else_target: lookup_tag_target(targets, *if_false)?,
                    else_args: Vec::new(),
                })
            }
            HirTerminator::Return { .. } => self.terminate(IrTerminator::Jump {
                target: exit,
                args: Vec::new(),
            }),
            HirTerminator::NonLocalExit {
                target,
                values,
                cleanups,
            } => self
                .non_local_exit(node, *target, values, cleanups)
                .map(|_| ()),
            HirTerminator::Unreachable => self.terminate(IrTerminator::Unreachable),
        }
    }

    pub(super) fn go(&mut self, node: NodeId, target: BlockId) -> Result<ValueId, LowerError> {
        let Some((block, active_depth)) = self.tag_targets.get(&target).copied() else {
            return Self::unsupported(node, "go-outside-lowered-tagbody");
        };
        let value = self.word_constant(Constant::Nil)?;
        if !self.is_terminated() {
            self.close_handlers_since(active_depth)?;
            if self.is_tagbody_backedge(target) {
                self.push_none(OpKind::Safepoint)?;
            }
            self.terminate(IrTerminator::Jump {
                target: block,
                args: Vec::new(),
            })?;
        }
        Ok(value)
    }

    fn is_tagbody_backedge(&self, target: BlockId) -> bool {
        self.hir_block.is_some_and(|source| {
            self.dominance_contexts
                .last()
                .and_then(|context| context.dominators.get(&source))
                .is_some_and(|dominators| dominators.contains(&target))
        })
    }
}

fn lookup_tag_target(
    targets: &[(BlockId, ncl_ir::BlockId)],
    target: BlockId,
) -> Result<ncl_ir::BlockId, LowerError> {
    targets
        .iter()
        .find_map(|(hir, mir)| (*hir == target).then_some(*mir))
        .ok_or_else(|| LowerError::Ir("verified HIR tag target is missing".to_owned()))
}
