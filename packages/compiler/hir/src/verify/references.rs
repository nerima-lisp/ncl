//! Shared identity and lexical-reference checks.

use std::collections::HashSet;

use super::{Verifier, VerifyError};
use crate::binding::{BindingKind, ScopeId};
use crate::ids::{
    BindingId, BlockId, NodeId, SourceOriginId, SymbolId, TypeId, TypeSpecConstId, ValueId,
};
use crate::node::{CallTarget, Literal, Node, VariableTarget};

impl Verifier<'_> {
    pub(super) fn parameters(
        &self,
        node: NodeId,
        scope: ScopeId,
        ids: &[BindingId],
    ) -> Result<(), VerifyError> {
        self.scope(scope)?;
        for id in ids {
            let binding = self.binding(*id)?;
            if binding.kind() != BindingKind::LexicalVariable || binding.scope() != scope {
                return Err(VerifyError::InvalidBindingScopes(node));
            }
        }
        Ok(())
    }
    pub(super) fn body_scope(&self, scope: ScopeId, body: &[NodeId]) -> Result<(), VerifyError> {
        for id in body {
            if self.node_ref(*id)?.scope != scope {
                return Err(VerifyError::InvalidScope(scope));
            }
        }
        Ok(())
    }
    pub(super) fn child(&self, scope: ScopeId, id: NodeId) -> Result<(), VerifyError> {
        let node = self.node_ref(id)?;
        if node.scope == scope {
            Ok(())
        } else {
            Err(VerifyError::InvalidScope(node.scope))
        }
    }
    pub(super) fn children(&self, scope: ScopeId, ids: &[NodeId]) -> Result<(), VerifyError> {
        for id in ids {
            self.child(scope, *id)?;
        }
        Ok(())
    }
    pub(super) fn variable(
        &self,
        node: NodeId,
        scope: ScopeId,
        target: VariableTarget,
    ) -> Result<(), VerifyError> {
        match target {
            VariableTarget::Lexical(id) => {
                let binding = self.binding(id)?;
                if binding.kind() != BindingKind::LexicalVariable
                    || self
                        .input
                        .bindings
                        .resolve_variable(scope, binding.symbol())
                        .ok()
                        .flatten()
                        != Some(id)
                {
                    return Err(VerifyError::InvisibleVariable { node, binding: id });
                }
            }
            VariableTarget::Special(symbol) | VariableTarget::Global(symbol) => {
                self.symbol(symbol)?
            }
        }
        Ok(())
    }
    pub(super) fn call_target(
        &self,
        node: NodeId,
        scope: ScopeId,
        target: CallTarget,
    ) -> Result<(), VerifyError> {
        match target {
            CallTarget::Local(id) => {
                let binding = self.binding(id)?;
                if binding.kind() != BindingKind::LocalFunction
                    || self
                        .input
                        .bindings
                        .resolve_function(scope, binding.symbol())
                        .ok()
                        .flatten()
                        != Some(id)
                {
                    return Err(VerifyError::InvisibleFunction { node, binding: id });
                }
            }
            CallTarget::Global(symbol) | CallTarget::Intrinsic(symbol) => self.symbol(symbol)?,
            CallTarget::Value(id) => self.child(scope, id)?,
        }
        Ok(())
    }
    pub(super) fn literal(&self, value: &Literal) -> Result<(), VerifyError> {
        match value {
            Literal::Symbol(symbol) => self.symbol(*symbol)?,
            Literal::Cons(car, cdr) => {
                self.literal(car)?;
                self.literal(cdr)?;
            }
            Literal::Vector(items)
            | Literal::Array {
                elements: items, ..
            }
            | Literal::SpecializedArray {
                elements: items, ..
            } => {
                for item in items {
                    self.literal(item)?;
                }
            }
            Literal::Nil
            | Literal::True
            | Literal::Number(_)
            | Literal::Character(_)
            | Literal::String(_)
            | Literal::Pathname(_) => {}
        }
        Ok(())
    }
    pub(super) fn scope_descendant(
        &self,
        parent: ScopeId,
        child: ScopeId,
    ) -> Result<(), VerifyError> {
        if self.is_descendant(child, parent)? {
            Ok(())
        } else {
            Err(VerifyError::InvalidScope(child))
        }
    }
    pub(super) fn is_descendant(
        &self,
        mut child: ScopeId,
        parent: ScopeId,
    ) -> Result<bool, VerifyError> {
        self.scope(parent)?;
        let mut visited = HashSet::new();
        loop {
            self.scope(child)?;
            if child == parent {
                return Ok(true);
            }
            if !visited.insert(child) {
                return Err(VerifyError::InvalidScopeTree(child));
            }
            let next = self
                .input
                .bindings
                .scope_parent(child)
                .map_err(|_| VerifyError::InvalidScope(child))?;
            let Some(next) = next else {
                return Ok(false);
            };
            if next.index() >= child.index() {
                return Err(VerifyError::InvalidScopeTree(child));
            }
            child = next;
        }
    }
    pub(super) fn scope(&self, id: ScopeId) -> Result<(), VerifyError> {
        if id.form() != self.input.form {
            return Err(VerifyError::InvalidScope(id));
        }
        self.input
            .bindings
            .scope_parent(id)
            .map_err(|_| VerifyError::InvalidScope(id))?;
        Ok(())
    }
    pub(super) fn node(&self, id: NodeId) -> Result<(), VerifyError> {
        if self.input.nodes.get(id).is_some() {
            Ok(())
        } else {
            Err(VerifyError::InvalidNode(id))
        }
    }
    pub(super) fn node_ref(&self, id: NodeId) -> Result<&Node, VerifyError> {
        self.input.nodes.get(id).ok_or(VerifyError::InvalidNode(id))
    }
    pub(super) fn nodes_ref(&self, ids: &[NodeId]) -> Result<(), VerifyError> {
        for id in ids {
            self.node(*id)?;
        }
        Ok(())
    }
    pub(super) fn ty(&self, id: TypeId) -> Result<(), VerifyError> {
        if self.input.types.get(id).is_some() {
            Ok(())
        } else {
            Err(VerifyError::InvalidType(id))
        }
    }
    pub(super) fn binding(&self, id: BindingId) -> Result<&crate::binding::Binding, VerifyError> {
        self.input
            .bindings
            .binding(id)
            .ok_or(VerifyError::InvalidBinding(id))
    }
    pub(super) fn block(&self, id: BlockId) -> Result<(), VerifyError> {
        if self.input.control_flow.block(id).is_some() {
            Ok(())
        } else {
            Err(VerifyError::InvalidBlock(id))
        }
    }
    pub(super) fn origin(&self, id: SourceOriginId) -> Result<(), VerifyError> {
        if self.input.origins.get(id).is_some() {
            Ok(())
        } else {
            Err(VerifyError::InvalidOrigin(id))
        }
    }
    pub(super) fn symbol(&self, id: SymbolId) -> Result<(), VerifyError> {
        if id.compilation_unit() == self.input.resources.compilation_unit
            && self.input.resources.symbols.contains(&id)
        {
            Ok(())
        } else {
            Err(VerifyError::InvalidSymbol(id))
        }
    }
    pub(super) fn value(&self, id: ValueId) -> Result<(), VerifyError> {
        if id.form() == self.input.form && self.input.resources.values.contains(&id) {
            Ok(())
        } else {
            Err(VerifyError::InvalidValue(id))
        }
    }
    pub(super) fn type_constant(&self, id: TypeSpecConstId) -> Result<(), VerifyError> {
        if id.form() == self.input.form && self.input.resources.type_spec_constants.contains(&id) {
            Ok(())
        } else {
            Err(VerifyError::InvalidTypeSpecConstant(id))
        }
    }
}
