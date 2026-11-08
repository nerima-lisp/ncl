use std::collections::{HashMap, HashSet};

use crate::arena::Arena;
use crate::binding::{BindingEnvironment, ScopeId};
use crate::cl_type::TypeArena;
use crate::control::ControlFlowGraph;
use crate::ids::{BindingId, FormId, NodeId, SymbolId, TypeSpecConstId, ValueId};
use crate::node::{Literal, NodeArena};
use crate::policy::OptimizationPolicy;
use crate::source::SourceOrigins;
use crate::symbols::SymbolTable;
use crate::verify::{FactEpochs, FactStore, VerifyError, VerifyInput, VerifyResources, verify};

use super::metadata::{FormMetadata, LambdaSignature, LambdaSignatureKey, ScopeMetadata};

/// Storage for complete source type specifiers referenced by HIR checks.
pub type TypeSpecConstantArena = Arena<Literal, TypeSpecConstId>;

/// Immutable, owned HIR data for one top-level form.
#[derive(Debug)]
pub struct HirForm {
    pub(super) id: FormId,
    pub(super) nodes: NodeArena,
    pub(super) bindings: BindingEnvironment,
    pub(super) types: TypeArena,
    pub(super) control_flow: ControlFlowGraph,
    pub(super) origins: SourceOrigins,
    pub(super) facts: FactStore,
    pub(super) roots: Vec<NodeId>,
    pub(super) policy: OptimizationPolicy,
    pub(super) type_spec_constants: TypeSpecConstantArena,
    pub(super) symbol_ids: HashSet<SymbolId>,
    pub(super) value_ids: HashSet<ValueId>,
    pub(super) value_producers: HashMap<ValueId, NodeId>,
    pub(super) type_spec_constant_ids: HashSet<TypeSpecConstId>,
    pub(super) metadata: FormMetadata,
}

impl HirForm {
    /// Returns the identity of this top-level form.
    #[must_use]
    pub const fn id(&self) -> FormId {
        self.id
    }

    /// Returns semantic nodes in allocation order.
    #[must_use]
    pub const fn nodes(&self) -> &NodeArena {
        &self.nodes
    }

    /// Returns lexical bindings and scopes.
    #[must_use]
    pub const fn bindings(&self) -> &BindingEnvironment {
        &self.bindings
    }

    /// Returns the Common Lisp type lattice.
    #[must_use]
    pub const fn types(&self) -> &TypeArena {
        &self.types
    }

    /// Returns the per-form control-flow graph.
    #[must_use]
    pub const fn control_flow(&self) -> &ControlFlowGraph {
        &self.control_flow
    }

    /// Returns the validated source-origin table.
    #[must_use]
    pub const fn origins(&self) -> &SourceOrigins {
        &self.origins
    }

    /// Returns cached facts and their source epochs.
    #[must_use]
    pub const fn facts(&self) -> &FactStore {
        &self.facts
    }

    /// Returns the top-level expression roots in source order.
    #[must_use]
    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    /// Returns the active optimization policy.
    #[must_use]
    pub const fn policy(&self) -> OptimizationPolicy {
        self.policy
    }

    /// Returns a source type specifier by its form-local identity.
    #[must_use]
    pub fn type_spec_constant(&self, id: TypeSpecConstId) -> Option<&Literal> {
        self.type_spec_constants.get(id)
    }

    /// Returns symbols directly referenced while building this form.
    #[must_use]
    pub const fn symbol_ids(&self) -> &HashSet<SymbolId> {
        &self.symbol_ids
    }

    /// Returns SSA values directly referenced while building this form.
    #[must_use]
    pub const fn value_ids(&self) -> &HashSet<ValueId> {
        &self.value_ids
    }

    /// Returns the node producing an SSA value, when it is known.
    #[must_use]
    pub fn value_producer(&self, value: ValueId) -> Option<NodeId> {
        self.value_producers.get(&value).copied()
    }

    /// Returns the dense set of type-specifier constants owned by this form.
    #[must_use]
    pub const fn type_spec_constant_ids(&self) -> &HashSet<TypeSpecConstId> {
        &self.type_spec_constant_ids
    }

    /// Returns declarations and policies keyed by lexical scope.
    #[must_use]
    pub const fn scope_metadata(&self) -> &std::collections::HashMap<ScopeId, ScopeMetadata> {
        &self.metadata.scopes
    }

    /// Returns signatures keyed by lambda node or local-function binding.
    #[must_use]
    pub const fn lambda_signatures(
        &self,
    ) -> &std::collections::HashMap<LambdaSignatureKey, LambdaSignature> {
        &self.metadata.lambda_signatures
    }

    /// Returns the signature attached to an anonymous lambda node.
    #[must_use]
    pub fn lambda_signature_for_lambda(&self, node: NodeId) -> Option<&LambdaSignature> {
        self.metadata
            .lambda_signatures
            .get(&LambdaSignatureKey::Lambda(node))
    }

    /// Returns the signature attached to a local function binding.
    #[must_use]
    pub fn lambda_signature_for_local_function(
        &self,
        binding: BindingId,
    ) -> Option<&LambdaSignature> {
        self.metadata
            .lambda_signatures
            .get(&LambdaSignatureKey::LocalFunction(binding))
    }

    /// Verifies this form against the compilation-unit symbol table.
    ///
    /// # Errors
    ///
    /// Returns the first structural, identity, scope, CFG, origin, or fact
    /// invariant rejected by the HIR verifier.
    pub fn verify(&self, symbols: &SymbolTable) -> Result<(), VerifyError> {
        if symbols.compilation_unit() != self.id.compilation_unit() {
            return Err(VerifyError::WrongForm);
        }
        for symbol in &self.symbol_ids {
            if symbols.get(*symbol).is_none() {
                return Err(VerifyError::InvalidSymbol(*symbol));
            }
        }
        self.verify_metadata()?;
        let symbol_ids = symbols.ids();
        verify(VerifyInput {
            form: self.id,
            nodes: &self.nodes,
            types: &self.types,
            bindings: &self.bindings,
            control_flow: &self.control_flow,
            origins: &self.origins,
            facts: &self.facts,
            epochs: FactEpochs::default(),
            policy: self.policy,
            resources: VerifyResources::new(
                self.id.compilation_unit(),
                &symbol_ids,
                &self.value_ids,
                &self.value_producers,
                &self.type_spec_constant_ids,
            ),
        })
    }

    pub(super) fn verify_symbol(&self, symbol: SymbolId) -> Result<(), VerifyError> {
        if symbol.compilation_unit() != self.id.compilation_unit()
            || !self.symbol_ids.contains(&symbol)
        {
            return Err(VerifyError::InvalidSymbol(symbol));
        }
        Ok(())
    }

    pub(super) fn verify_scope(&self, scope: ScopeId) -> Result<(), VerifyError> {
        if scope.form() != self.id || self.bindings.scope_parent(scope).is_err() {
            return Err(VerifyError::InvalidScope(scope));
        }
        Ok(())
    }

    pub(super) fn verify_type(&self, ty: crate::ids::TypeId) -> Result<(), VerifyError> {
        if self.types.get(ty).is_none() {
            return Err(VerifyError::InvalidType(ty));
        }
        Ok(())
    }

    pub(super) fn verify_type_spec_constant(&self, id: TypeSpecConstId) -> Result<(), VerifyError> {
        if self.type_spec_constants.get(id).is_none() {
            return Err(VerifyError::InvalidTypeSpecConstant(id));
        }
        Ok(())
    }

    pub(super) fn scope_is_ancestor(&self, ancestor: ScopeId, mut scope: ScopeId) -> bool {
        if scope == ancestor {
            return true;
        }
        while let Ok(Some(parent)) = self.bindings.scope_parent(scope) {
            if parent == ancestor {
                return true;
            }
            scope = parent;
        }
        scope == ancestor
    }

    pub(super) fn verify_parameter_binding(
        &self,
        binding: BindingId,
        body_scope: ScopeId,
    ) -> Result<(), VerifyError> {
        let Some(binding_data) = self.bindings.binding(binding) else {
            return Err(VerifyError::InvalidBinding(binding));
        };
        if binding_data.kind() != crate::binding::BindingKind::LexicalVariable
            || !self.scope_is_ancestor(binding_data.scope(), body_scope)
        {
            return Err(VerifyError::InvalidBinding(binding));
        }
        Ok(())
    }

    pub(super) fn verify_literal(&self, literal: &Literal) -> Result<(), VerifyError> {
        match literal {
            Literal::Symbol(symbol) => self.verify_symbol(*symbol),
            Literal::Cons(car, cdr) => {
                self.verify_literal(car)?;
                self.verify_literal(cdr)
            }
            Literal::Vector(items)
            | Literal::Array {
                elements: items, ..
            }
            | Literal::SpecializedArray {
                elements: items, ..
            } => {
                for item in items {
                    self.verify_literal(item)?;
                }
                Ok(())
            }
            Literal::Nil
            | Literal::True
            | Literal::Number(_)
            | Literal::Character(_)
            | Literal::String(_)
            | Literal::Pathname(_) => Ok(()),
        }
    }
}
