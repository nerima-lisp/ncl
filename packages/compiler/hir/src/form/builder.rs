use std::collections::{HashMap, HashSet};

use crate::arena::Arena;
use crate::binding::{BindingEnvironment, ScopeId};
use crate::cl_type::TypeArena;
use crate::control::ControlFlowBuilder;
use crate::ids::{BindingId, FormId, NodeId, SourceOriginId, SymbolId, TypeSpecConstId, ValueId};
use crate::node::{Literal, Node, NodeArena, NodeKind};
use crate::policy::OptimizationPolicy;
use crate::source::SourceOrigins;
use crate::verify::FactStore;

use super::build_validation::BuilderValidation;
use super::error::FormError;
use super::metadata::{
    FormMetadata, HirDeclaration, LambdaSignature, LambdaSignatureKey, ScopeMetadata,
};
use super::storage::{HirForm, TypeSpecConstantArena};

/// Mutable construction state for one top-level form.
#[derive(Debug)]
pub struct HirFormBuilder {
    pub(super) form: FormId,
    default_origin: SourceOriginId,
    pub(super) nodes: NodeArena,
    pub(super) bindings: BindingEnvironment,
    pub(super) types: TypeArena,
    control_flow: ControlFlowBuilder,
    pub(super) origins: SourceOrigins,
    pub(super) roots: Vec<NodeId>,
    pub(super) policy: OptimizationPolicy,
    pub(super) type_spec_constants: TypeSpecConstantArena,
    pub(super) symbol_ids: HashSet<SymbolId>,
    pub(super) value_ids: HashSet<ValueId>,
    pub(super) value_producers: HashMap<ValueId, NodeId>,
    pub(super) type_spec_constant_ids: HashSet<TypeSpecConstId>,
    pub(super) metadata: FormMetadata,
}

impl HirFormBuilder {
    /// Creates an empty HIR form with one root lexical scope.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the type arena cannot be initialized, the origin
    /// table belongs to another form, or `default_origin` is not present.
    pub fn new(
        form: FormId,
        origins: SourceOrigins,
        default_origin: SourceOriginId,
    ) -> Result<Self, FormError> {
        if origins.form() != form {
            return Err(FormError::ForeignForm(origins.form()));
        }
        if origins.get(default_origin).is_none() {
            return Err(FormError::InvalidOrigin(default_origin));
        }
        let bindings = BindingEnvironment::new(form);
        let root_scope = bindings.root_scope();
        let policy = OptimizationPolicy::default();
        let mut metadata = FormMetadata::default();
        let _ = metadata.scopes.insert(
            root_scope,
            ScopeMetadata {
                declarations: Vec::new(),
                policy,
            },
        );
        Ok(Self {
            form,
            default_origin,
            nodes: NodeArena::new(form),
            bindings,
            types: TypeArena::new(form).map_err(FormError::Type)?,
            control_flow: ControlFlowBuilder::new(form),
            origins,
            roots: Vec::new(),
            policy,
            type_spec_constants: Arena::new(form),
            symbol_ids: HashSet::new(),
            value_ids: HashSet::new(),
            value_producers: HashMap::new(),
            type_spec_constant_ids: HashSet::new(),
            metadata,
        })
    }

    /// Returns the identity of the form being built.
    #[must_use]
    pub const fn form(&self) -> FormId {
        self.form
    }

    /// Returns the root lexical scope.
    #[must_use]
    pub const fn root_scope(&self) -> ScopeId {
        self.bindings.root_scope()
    }

    /// Returns mutable node storage for constructing HIR nodes.
    pub const fn nodes_mut(&mut self) -> &mut NodeArena {
        &mut self.nodes
    }

    /// Returns mutable lexical scopes and bindings.
    pub const fn bindings_mut(&mut self) -> &mut BindingEnvironment {
        &mut self.bindings
    }

    /// Returns mutable Common Lisp type storage.
    pub const fn types_mut(&mut self) -> &mut TypeArena {
        &mut self.types
    }

    /// Returns the control-flow builder for this form.
    pub const fn control_flow_mut(&mut self) -> &mut ControlFlowBuilder {
        &mut self.control_flow
    }

    /// Sets the effective optimization policy for the form's root scope.
    pub fn set_policy(&mut self, policy: OptimizationPolicy) {
        self.policy = policy;
        let root = self.bindings.root_scope();
        self.metadata
            .scopes
            .entry(root)
            .and_modify(|metadata| metadata.policy = policy)
            .or_insert_with(|| ScopeMetadata {
                declarations: Vec::new(),
                policy,
            });
    }

    /// Retains declarations and effective policy for one lexical scope.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the scope or any referenced symbol, type, or
    /// source type-specifier constant is foreign or unknown.
    pub fn set_scope_metadata(
        &mut self,
        scope: ScopeId,
        metadata: ScopeMetadata,
    ) -> Result<(), FormError> {
        self.validate_scope(scope)?;
        for declaration in &metadata.declarations {
            match declaration {
                HirDeclaration::Type {
                    type_id,
                    source_specifier,
                    ..
                }
                | HirDeclaration::Ftype {
                    type_id,
                    source_specifier,
                    ..
                } => {
                    self.validate_type(*type_id)?;
                    self.validate_type_spec_constant(*source_specifier)?;
                }
                HirDeclaration::Special { .. }
                | HirDeclaration::Inline { .. }
                | HirDeclaration::Notinline { .. }
                | HirDeclaration::Optimize { .. }
                | HirDeclaration::DynamicExtent { .. }
                | HirDeclaration::Ignore { .. }
                | HirDeclaration::Ignorable { .. }
                | HirDeclaration::Unknown { .. } => {}
            }
        }
        self.register_declaration_symbols(&metadata.declarations)?;
        if scope == self.bindings.root_scope() {
            self.policy = metadata.policy;
        }
        let _ = self.metadata.scopes.insert(scope, metadata);
        Ok(())
    }

    /// Stores the signature for an anonymous lambda node.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the lambda or one of its signature references
    /// does not belong to this form.
    pub fn set_lambda_signature_for_lambda(
        &mut self,
        lambda: NodeId,
        signature: LambdaSignature,
    ) -> Result<(), FormError> {
        self.set_lambda_signature(LambdaSignatureKey::Lambda(lambda), signature)
    }

    /// Stores the signature for a local FLET or LABELS function.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the function or one of its signature references
    /// does not belong to this form.
    pub fn set_lambda_signature_for_local_function(
        &mut self,
        function: BindingId,
        signature: LambdaSignature,
    ) -> Result<(), FormError> {
        self.set_lambda_signature(LambdaSignatureKey::LocalFunction(function), signature)
    }

    /// Stores a signature under its lambda-node or local-function identity.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the function or one of its signature references
    /// does not belong to this form.
    pub fn set_lambda_signature(
        &mut self,
        key: LambdaSignatureKey,
        signature: LambdaSignature,
    ) -> Result<(), FormError> {
        self.validate_signature(key, &signature)?;
        for keyword in signature.keys.iter().map(|key| key.keyword) {
            self.register_symbol(keyword)?;
        }
        let _ = self.metadata.lambda_signatures.insert(key, signature);
        Ok(())
    }

    /// Records a symbol referenced by this form.
    ///
    /// # Errors
    ///
    /// Returns `FormError::ForeignSymbol` when the ID belongs to another
    /// compilation unit.
    pub fn register_symbol(&mut self, symbol: SymbolId) -> Result<(), FormError> {
        if symbol.compilation_unit() != self.form.compilation_unit() {
            return Err(FormError::ForeignSymbol(symbol));
        }
        let _ = self.symbol_ids.insert(symbol);
        Ok(())
    }

    /// Records an SSA value identity referenced by this form.
    ///
    /// # Errors
    ///
    /// Returns `FormError::ForeignValue` when the ID belongs to another form.
    pub fn register_value(&mut self, value: ValueId) -> Result<(), FormError> {
        if value.form() != self.form {
            return Err(FormError::ForeignValue(value));
        }
        let _ = self.value_ids.insert(value);
        Ok(())
    }

    /// Associates an SSA value with the node that produces it.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when either identity is foreign, the producer node is
    /// absent, or the value already has a producer.
    pub fn register_value_producer(
        &mut self,
        value: ValueId,
        producer: NodeId,
    ) -> Result<(), FormError> {
        if value.form() != self.form {
            return Err(FormError::ForeignValue(value));
        }
        if producer.form() != self.form {
            return Err(FormError::ForeignNode(producer));
        }
        if self.nodes.get(producer).is_none() {
            return Err(FormError::InvalidNode(producer));
        }
        if self.value_producers.contains_key(&value) {
            return Err(FormError::DuplicateValueProducer(value));
        }
        self.value_ids.insert(value);
        self.value_producers.insert(value, producer);
        Ok(())
    }

    /// Adds a complete source type specifier to the form-local constant pool.
    ///
    /// # Errors
    ///
    /// Returns `FormError` if a symbol in the datum belongs to another
    /// compilation unit or the constant index space is exhausted.
    pub fn add_type_spec_constant(
        &mut self,
        literal: Literal,
    ) -> Result<TypeSpecConstId, FormError> {
        self.register_literal_symbols(&literal)?;
        let id = self
            .type_spec_constants
            .allocate(literal)
            .map_err(FormError::Id)?;
        let _ = self.type_spec_constant_ids.insert(id);
        Ok(id)
    }

    /// Adds a semantic node using the form's default source origin.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the scope is foreign or missing, the origin is
    /// invalid, or the node index space is exhausted.
    pub fn alloc_node(&mut self, scope: ScopeId, kind: NodeKind) -> Result<NodeId, FormError> {
        self.alloc_node_at(scope, self.default_origin, kind)
    }

    /// Adds a semantic node with its own source or expansion origin.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the scope is foreign or missing, the origin is
    /// invalid, or the node index space is exhausted.
    pub fn alloc_node_at(
        &mut self,
        scope: ScopeId,
        origin: SourceOriginId,
        kind: NodeKind,
    ) -> Result<NodeId, FormError> {
        self.validate_scope(scope)?;
        if self.origins.get(origin).is_none() {
            return Err(FormError::InvalidOrigin(origin));
        }
        self.nodes
            .allocate(Node {
                scope,
                origin,
                kind,
            })
            .map_err(FormError::Id)
    }

    /// Adds an expression root for the top-level form.
    ///
    /// # Errors
    ///
    /// Returns `FormError` when the node is foreign or does not exist.
    pub fn add_root(&mut self, node: NodeId) -> Result<(), FormError> {
        if node.form() != self.form {
            return Err(FormError::ForeignNode(node));
        }
        if self.nodes.get(node).is_none() {
            return Err(FormError::InvalidNode(node));
        }
        self.roots.push(node);
        Ok(())
    }

    /// Completes construction and returns the immutable HIR form.
    ///
    /// # Errors
    ///
    /// Returns `FormError::Control` when a CFG block or function is incomplete.
    pub fn finish(self) -> Result<HirForm, FormError> {
        Ok(HirForm {
            id: self.form,
            nodes: self.nodes,
            bindings: self.bindings,
            types: self.types,
            control_flow: self.control_flow.finish().map_err(FormError::Control)?,
            origins: self.origins,
            facts: FactStore::new(self.form),
            roots: self.roots,
            policy: self.policy,
            type_spec_constants: self.type_spec_constants,
            symbol_ids: self.symbol_ids,
            value_ids: self.value_ids,
            value_producers: self.value_producers,
            type_spec_constant_ids: self.type_spec_constant_ids,
            metadata: self.metadata,
        })
    }
}
