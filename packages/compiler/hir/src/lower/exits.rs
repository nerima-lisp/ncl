use ncl_ir::{Constant, OpKind, Terminator as IrTerminator, Ty, ValueId};

use crate::control::{ExitTarget, Terminator as HirTerminator};
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
        let previous = self.block_targets.insert(node, target);
        let value = self.sequence(body)?;
        if !self.is_terminated() {
            let values = self.capture_multiple_values(value)?;
            self.terminate(IrTerminator::Jump {
                target: exit,
                args: vec![values],
            })?;
        }
        if let Some(previous) = previous {
            let _ = self.block_targets.insert(node, previous);
        } else {
            let _ = self.block_targets.remove(&node);
        }
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
            return self.unsupported(node, "return-from-cross-function-block");
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
        let flow =
            self.form
                .control_flow()
                .function(node)
                .cloned()
                .ok_or(LowerError::Unsupported {
                    node,
                    operation: "tagbody-without-control-flow",
                })?;
        let blocks = flow
            .blocks()
            .iter()
            .map(|id| {
                self.form
                    .control_flow()
                    .block(*id)
                    .cloned()
                    .map(|block| (*id, block))
                    .ok_or(LowerError::Ir(
                        "verified HIR CFG block is missing".to_owned(),
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
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

        for (hir_id, hir_block) in blocks {
            let mir_id = lookup_tag_target(&targets, hir_id)?;
            self.position(mir_id)?;
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
            HirTerminator::Jump(target) => self.terminate(IrTerminator::Jump {
                target: lookup_tag_target(targets, *target)?,
                args: Vec::new(),
            }),
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
                self.terminate(IrTerminator::Branch {
                    condition,
                    then_target: lookup_tag_target(targets, *if_true)?,
                    then_args: Vec::new(),
                    else_target: lookup_tag_target(targets, *if_false)?,
                    else_args: Vec::new(),
                })
            }
            HirTerminator::Return { values } => {
                if !values.is_empty() {
                    return Err(LowerError::Unsupported {
                        node,
                        operation: "tagbody-return-with-values",
                    });
                }
                self.terminate(IrTerminator::Jump {
                    target: exit,
                    args: Vec::new(),
                })
            }
            HirTerminator::NonLocalExit {
                target: ExitTarget::Tag(block),
                values,
                cleanups,
            } => self
                .non_local_exit(node, ExitTarget::Tag(*block), values, cleanups)
                .map(|_| ()),
            HirTerminator::NonLocalExit { .. } => Err(LowerError::Unsupported {
                node,
                operation: "tagbody-nonlocal-exit-outside-local-block-or-tag",
            }),
            HirTerminator::Unreachable => Err(LowerError::Unsupported {
                node,
                operation: "unreachable-hir-cfg-block",
            }),
        }
    }

    pub(super) fn go(&mut self, node: NodeId, target: BlockId) -> Result<ValueId, LowerError> {
        let Some((block, active_depth)) = self.tag_targets.get(&target).copied() else {
            return self.unsupported(node, "go-outside-lowered-tagbody");
        };
        let value = self.word_constant(Constant::Nil)?;
        if !self.is_terminated() {
            self.close_handlers_since(active_depth)?;
            self.terminate(IrTerminator::Jump {
                target: block,
                args: Vec::new(),
            })?;
        }
        Ok(value)
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
