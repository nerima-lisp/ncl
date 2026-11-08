//! Control-flow and check-edge verification.

use super::{Verifier, VerifyError};
use crate::control::{ExitTarget, FunctionFlow, Terminator};
use crate::ids::{BlockId, CleanupId, NodeId};
use crate::node::{MultipleValueOperation, NodeKind};
use std::collections::{HashMap, HashSet};

impl Verifier<'_> {
    pub(super) fn cfg(&self) -> Result<HashMap<NodeId, usize>, VerifyError> {
        let mut functions = HashSet::new();
        let mut blocks = HashSet::new();
        let mut check_edges = HashMap::new();
        let control_flow = self.input.control_flow;
        for flow in control_flow.functions() {
            self.node(flow.function())?;
            if !functions.insert(flow.function()) || !flow.blocks().contains(&flow.entry()) {
                return Err(VerifyError::InvalidFunctionFlow(flow.function()));
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
                self.terminator(*id, flow, block.terminator(), &mut check_edges)?;
            }
        }
        Ok(check_edges)
    }
    pub(super) fn terminator(
        &self,
        source: BlockId,
        flow: &FunctionFlow,
        term: &Terminator,
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
            if let NodeKind::Go { target } = &node.kind {
                if !self.tag_blocks.contains(target) {
                    return Err(VerifyError::InvalidTagTarget(*target));
                }
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
            ExitTarget::Catch(id) => {
                self.node(id)?;
                if !matches!(&self.node_ref(id)?.kind, NodeKind::Catch { .. }) {
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
