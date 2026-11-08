//! Control-flow and check-edge verification.

use super::{Verifier, VerifyError};
use crate::control::{ExitTarget, FunctionFlow, Terminator};
use crate::ids::{BlockId, CleanupId, NodeId};
use crate::node::{MultipleValueOperation, Node, NodeKind};
use std::collections::{HashMap, HashSet};

impl Verifier<'_> {
    pub(super) fn node_control(&self, id: NodeId, node: &Node) -> Result<(), VerifyError> {
        match &node.kind {
            NodeKind::Block { name, body } => {
                self.symbol(*name)?;
                self.children(node.scope, body)?;
            }
            NodeKind::ReturnFrom { target, value } => {
                self.node(*target)?;
                if !matches!(&self.node_ref(*target)?.kind, NodeKind::Block { .. }) {
                    return Err(VerifyError::InvalidExitTarget(*target));
                }
                if let Some(value) = value {
                    self.child(node.scope, *value)?;
                }
            }
            NodeKind::Tagbody { tags, forms } => {
                for tag in tags {
                    self.symbol(tag.name)?;
                    self.block(tag.block)?;
                }
                self.children(node.scope, forms)?;
            }
            NodeKind::Go { target } => self.block(*target)?,
            NodeKind::Catch { tag, body } => {
                self.child(node.scope, *tag)?;
                self.children(node.scope, body)?;
            }
            NodeKind::Throw { tag, value } => {
                self.child(node.scope, *tag)?;
                self.child(node.scope, *value)?;
            }
            NodeKind::UnwindProtect {
                cleanup,
                protected,
                cleanup_forms,
            } => {
                if cleanup.form() != self.input.form || !self.cleanups.contains(cleanup) {
                    return Err(VerifyError::InvalidCleanup(*cleanup));
                }
                self.child(node.scope, *protected)?;
                self.children(node.scope, cleanup_forms)?;
            }
            NodeKind::NonLocalExit {
                target,
                values,
                cleanups,
            } => {
                self.exit_target(*target)?;
                self.children(node.scope, values)?;
                self.cleanups_ref(cleanups)?;
            }
            _ => return Err(VerifyError::InvalidNode(id)), // check-added-lines: allow(wildcard) dispatch subset
        }
        Ok(())
    }

    pub(super) fn cfg(&self) -> Result<HashMap<NodeId, usize>, VerifyError> {
        let mut functions = HashSet::new();
        let mut blocks = HashSet::new();
        let mut check_edges = HashMap::new();
        let control_flow = self.input.control_flow;
        for (index, node) in self.input.nodes.iter() {
            let id = crate::ids::NodeId::try_from_index(self.input.form, index)?;
            if matches!(&node.kind, NodeKind::Tagbody { .. }) && control_flow.function(id).is_none()
            {
                return Err(VerifyError::InvalidFunctionFlow(id));
            }
        }
        for flow in control_flow.functions() {
            self.node(flow.function())?;
            if !functions.insert(flow.function()) || !flow.blocks().contains(&flow.entry()) {
                return Err(VerifyError::InvalidFunctionFlow(flow.function()));
            }
            let dominators = control_flow
                .dominators(flow.function())
                .ok_or_else(|| VerifyError::InvalidFunctionFlow(flow.function()))?;
            if matches!(
                &self.node_ref(flow.function())?.kind,
                NodeKind::Tagbody { .. }
            ) {
                for block_id in flow.blocks() {
                    let block = control_flow
                        .block(*block_id)
                        .ok_or(VerifyError::InvalidBlock(*block_id))?;
                    if matches!(block.terminator(), Terminator::Return { values } if !values.is_empty())
                    {
                        return Err(VerifyError::TagbodyReturnValues(*block_id));
                    }
                }
            }
            for id in flow.blocks() {
                if !blocks.insert(*id) {
                    return Err(VerifyError::InvalidFunctionBlock(*id));
                }
                let block = control_flow
                    .block(*id)
                    .ok_or(VerifyError::InvalidBlock(*id))?;
                if block.function() != flow.function() {
                    return Err(VerifyError::InvalidFunctionBlock(*id));
                }
                self.nodes_ref(block.operations())?;
                self.terminator(*id, flow, block.terminator(), &dominators, &mut check_edges)?;
            }
        }
        Ok(check_edges)
    }

    fn check_type_producer(
        &self,
        source: BlockId,
        flow: &FunctionFlow,
        check: NodeId,
        value: crate::ids::ValueId,
        dominators: &HashMap<BlockId, HashSet<BlockId>>,
    ) -> Result<(), VerifyError> {
        let producer = self
            .input
            .resources
            .value_producers
            .get(&value)
            .copied()
            .ok_or(VerifyError::MissingValueProducer(check))?;
        if producer == check {
            return Err(VerifyError::InvalidValueProducer { check, producer });
        }
        let mut producer_block = None;
        for block_id in flow.blocks() {
            let block = self
                .input
                .control_flow
                .block(*block_id)
                .ok_or(VerifyError::InvalidBlock(*block_id))?;
            let occurrences = block
                .operations()
                .iter()
                .filter(|operation| **operation == producer)
                .count();
            if occurrences > 1 {
                return Err(VerifyError::InvalidValueProducer { check, producer });
            }
            if occurrences == 1 {
                if producer_block.is_some() {
                    return Err(VerifyError::InvalidValueProducer { check, producer });
                }
                producer_block = Some(*block_id);
            }
        }
        let check_scope = self.node_ref(check)?.scope;
        let producer_scope = self.node_ref(producer)?.scope;
        let dominates = producer_block.is_some_and(|producer_block| {
            dominators
                .get(&source)
                .is_some_and(|block_dominators| block_dominators.contains(&producer_block))
        });
        if !dominates || !self.is_descendant(check_scope, producer_scope)? {
            return Err(VerifyError::InvalidValueProducer { check, producer });
        }
        Ok(())
    }

    pub(super) fn terminator(
        &self,
        source: BlockId,
        flow: &FunctionFlow,
        term: &Terminator,
        dominators: &HashMap<BlockId, HashSet<BlockId>>,
        check_edges: &mut HashMap<NodeId, usize>,
    ) -> Result<(), VerifyError> {
        match term {
            Terminator::Jump(target) => self.successor(source, flow, *target)?,
            Terminator::Conditional {
                condition,
                if_true,
                if_false,
            } => {
                self.node(*condition)?;
                self.successor(source, flow, *if_true)?;
                self.successor(source, flow, *if_false)?;
                if matches!(
                    &self.node_ref(*condition)?.kind,
                    NodeKind::CheckType { .. } | NodeKind::RuntimeCheck { .. }
                ) {
                    if let NodeKind::CheckType { value, .. } = &self.node_ref(*condition)?.kind {
                        self.check_type_producer(source, flow, *condition, *value, dominators)?;
                    }
                    if if_true == if_false {
                        return Err(VerifyError::InvalidCheckEdges(*condition));
                    }
                    *check_edges.entry(*condition).or_default() += 1;
                }
            }
            Terminator::Return { values } => self.nodes_ref(values)?,
            Terminator::NonLocalExit {
                target,
                values,
                cleanups,
            } => {
                self.exit_target(*target)?;
                self.nodes_ref(values)?;
                self.cleanups_ref(cleanups)?;
            }
            Terminator::Unreachable => {}
        }
        Ok(())
    }
    pub(super) fn check_edges(&self) -> Result<(), VerifyError> {
        for index in 0..self.input.nodes.len() {
            let id = NodeId::try_from_index(self.input.form, index)?;
            let node = self
                .input
                .nodes
                .get(id)
                .ok_or(VerifyError::InvalidNode(id))?;
            if matches!(
                &node.kind,
                NodeKind::CheckType { .. } | NodeKind::RuntimeCheck { .. }
            ) && self.check_edges.get(&id).copied() != Some(1)
            {
                return Err(VerifyError::InvalidCheckEdges(id));
            }
            if let NodeKind::Go { target } = &node.kind
                && !self.tag_blocks.contains(target)
            {
                return Err(VerifyError::InvalidTagTarget(*target));
            }
        }
        Ok(())
    }
    pub(super) fn multiple_values(
        &self,
        node: NodeId,
        scope: crate::binding::ScopeId,
        operation: &MultipleValueOperation,
    ) -> Result<(), VerifyError> {
        match operation {
            MultipleValueOperation::Produce { values }
            | MultipleValueOperation::Return { values } => self.children(scope, values),
            MultipleValueOperation::Bind {
                value,
                bindings,
                body_scope,
                body,
            } => {
                self.child(scope, *value)?;
                self.scope_descendant(scope, *body_scope)?;
                if !bindings.is_empty() && *body_scope == scope {
                    return Err(VerifyError::InvalidBindingScopes(node));
                }
                self.parameters(node, *body_scope, bindings)?;
                self.body_scope(*body_scope, body)
            }
            MultipleValueOperation::Call {
                function,
                arguments,
            } => {
                self.child(scope, *function)?;
                self.children(scope, arguments)
            }
            MultipleValueOperation::Prog1 { first, rest } => {
                self.child(scope, *first)?;
                self.children(scope, rest)
            }
        }
    }
    pub(super) fn exit_target(&self, target: ExitTarget) -> Result<(), VerifyError> {
        match target {
            ExitTarget::Block(id) => {
                self.node(id)?;
                if !matches!(&self.node_ref(id)?.kind, NodeKind::Block { .. }) {
                    return Err(VerifyError::InvalidExitTarget(id));
                }
            }
            ExitTarget::Function(id) | ExitTarget::Dynamic(id) => self.node(id)?,
            ExitTarget::Tag(id) => self.block(id)?,
        }
        Ok(())
    }
    pub(super) fn cleanups_ref(&self, ids: &[CleanupId]) -> Result<(), VerifyError> {
        for id in ids {
            if id.form() != self.input.form || !self.cleanups.contains(id) {
                return Err(VerifyError::InvalidCleanup(*id));
            }
        }
        Ok(())
    }
    pub(super) fn successor(
        &self,
        source: BlockId,
        flow: &FunctionFlow,
        target: BlockId,
    ) -> Result<(), VerifyError> {
        let block = self
            .input
            .control_flow
            .block(target)
            .ok_or(VerifyError::InvalidSuccessor { source, target })?;
        if !flow.blocks().contains(&target) || block.function() != flow.function() {
            return Err(VerifyError::InvalidSuccessor { source, target });
        }
        Ok(())
    }
}
