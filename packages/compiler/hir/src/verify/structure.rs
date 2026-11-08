//! Node, scope, type, and source-identity verification.

use super::{Verifier, VerifyError};
use crate::binding::{BindingKind, LocalFunctionKind, ScopeId};
use crate::cl_type::{ClTypeKind, FunctionParameters, MemberValue, OpaqueOperand};
use crate::ids::{BindingId, NodeId, SourceOriginId, TypeId, TypeSpecConstId, ValueId};
use crate::node::{LetKind, LocalFunctionDefinition, Node, NodeKind};
use crate::source::SourceOriginKind;

impl Verifier<'_> {
    pub(super) fn prepare_cleanups(&mut self) -> Result<(), VerifyError> {
        let nodes = self.input.nodes;
        let (cleanups, tag_blocks) = (&mut self.cleanups, &mut self.tag_blocks);
        for (_, node) in nodes.iter() {
            if let NodeKind::UnwindProtect { cleanup, .. } = &node.kind
                && (cleanup.form() != self.input.form || !cleanups.insert(*cleanup))
            {
                return Err(VerifyError::InvalidCleanup(*cleanup));
            }
            if let NodeKind::Tagbody { tags, .. } = &node.kind {
                tag_blocks.extend(tags.iter().map(|tag| tag.block));
            }
        }
        Ok(())
    }

    pub(super) fn owners(&self) -> Result<(), VerifyError> {
        let form = self.input.form;
        if self.input.nodes.owner() != form
            || self.input.types.any().form() != form
            || self.input.types.empty().form() != form
            || self.input.bindings.root_scope().form() != form
            || self.input.origins.form() != form
            || self.input.facts.form != form
            || self.input.resources.compilation_unit != form.compilation_unit()
        {
            return Err(VerifyError::WrongForm);
        }
        if self
            .input
            .resources
            .symbols
            .iter()
            .any(|id| id.compilation_unit() != form.compilation_unit())
            || self
                .input
                .resources
                .values
                .iter()
                .any(|id| id.form() != form)
            || self
                .input
                .resources
                .type_spec_constants
                .iter()
                .any(|id| id.form() != form)
        {
            return Err(VerifyError::WrongForm);
        }
        let resources = self.input.resources;
        for id in resources.symbols {
            let index = usize::try_from(id.index()).map_err(|_| VerifyError::InvalidSymbol(*id))?;
            if index >= resources.symbols.len() {
                return Err(VerifyError::InvalidSymbol(*id));
            }
        }
        for id in resources.values {
            let index = usize::try_from(id.index()).map_err(|_| VerifyError::InvalidValue(*id))?;
            if index >= resources.values.len() {
                return Err(VerifyError::InvalidValue(*id));
            }
        }
        for (value, producer) in resources.value_producers {
            if !resources.values.contains(value) {
                return Err(VerifyError::InvalidValue(*value));
            }
            self.node(*producer)?;
        }
        for id in resources.type_spec_constants {
            let index = usize::try_from(id.index())
                .map_err(|_| VerifyError::InvalidTypeSpecConstant(*id))?;
            if index >= resources.type_spec_constants.len() {
                return Err(VerifyError::InvalidTypeSpecConstant(*id));
            }
        }
        for binding in self.input.bindings.iter() {
            if binding.id().form() != form || binding.scope().form() != form {
                return Err(VerifyError::InvalidBinding(binding.id()));
            }
            self.symbol(binding.symbol())?;
            self.scope(binding.scope())?;
            for usage in binding.reads().iter().chain(binding.writes()) {
                self.node(*usage)?;
            }
        }
        Ok(())
    }
    pub(super) fn origins(&self) -> Result<(), VerifyError> {
        for (id, origin) in self.input.origins.iter() {
            let location = origin.location();
            if self.input.origins.get(id).is_none()
                || location.form() != self.input.form
                || location.file().compilation_unit() != self.input.form.compilation_unit()
            {
                return Err(VerifyError::InvalidOrigin(id));
            }
            if let SourceOriginKind::Expansion { parent, transform } = origin.kind() {
                if parent.index() >= id.index() || self.input.origins.get(parent).is_none() {
                    return Err(VerifyError::InvalidOrigin(parent));
                }
                self.symbol(transform.name())?;
            }
        }
        Ok(())
    }
    pub(super) fn types(&self) -> Result<(), VerifyError> {
        for (index, ty) in self.input.types.iter() {
            let id = TypeId::try_from_index(self.input.form, index)?;
            if self.input.types.get(id).is_none() {
                return Err(VerifyError::InvalidType(id));
            }
            match ty.kind() {
                ClTypeKind::Any
                | ClTypeKind::Empty
                | ClTypeKind::Named(_)
                | ClTypeKind::NumericRange(_) => {}
                ClTypeKind::Member(values) => {
                    for value in values {
                        if let MemberValue::Symbol(symbol) = value {
                            self.symbol(*symbol)?;
                        }
                    }
                }
                ClTypeKind::Union(types)
                | ClTypeKind::Intersection(types)
                | ClTypeKind::MultipleValues(types) => {
                    for ty in types {
                        self.ty(*ty)?;
                    }
                }
                ClTypeKind::Complement(ty) => self.ty(*ty)?,
                ClTypeKind::Cons { car, cdr } => {
                    self.ty(*car)?;
                    self.ty(*cdr)?;
                }
                ClTypeKind::Array(array) => {
                    if let Some(ty) = array.element_type {
                        self.ty(ty)?;
                    }
                }
                ClTypeKind::Function(function) => {
                    if let FunctionParameters::Fixed(parameters) = &function.parameters {
                        for ty in parameters {
                            self.ty(*ty)?;
                        }
                    }
                    self.ty(function.returns)?;
                }
                ClTypeKind::Class(symbol) => self.symbol(*symbol)?,
                ClTypeKind::Opaque {
                    specifier,
                    operands,
                } => {
                    self.symbol(*specifier)?;
                    for operand in operands {
                        match operand {
                            OpaqueOperand::Type(ty) => self.ty(*ty)?,
                            OpaqueOperand::Constant(id) => self.type_constant(*id)?,
                            OpaqueOperand::Symbol(symbol) => self.symbol(*symbol)?,
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub(super) fn nodes(&self) -> Result<(), VerifyError> {
        let nodes = self.input.nodes;
        for (index, node) in nodes.iter() {
            let id = NodeId::try_from_index(self.input.form, index)?;
            self.node_kind(id, node)?;
        }
        Ok(())
    }
    pub(super) fn node_kind(&self, id: NodeId, node: &Node) -> Result<(), VerifyError> {
        self.scope(node.scope)?;
        self.origin(node.origin)?;
        match &node.kind {
            NodeKind::Literal(value) => self.literal(value)?,
            NodeKind::VariableReference { .. }
            | NodeKind::FunctionReference { .. }
            | NodeKind::ClosureReference { .. }
            | NodeKind::Assignment { .. }
            | NodeKind::Call { .. }
            | NodeKind::KnownFunctionOperation { .. }
            | NodeKind::SequenceOperation { .. }
            | NodeKind::ArrayOperation { .. }
            | NodeKind::RuntimeCheck { .. }
            | NodeKind::TopLevelDefinition { .. } => self.node_references(id, node)?,
            NodeKind::Let { .. } => self.let_scopes(id, node)?,
            NodeKind::DynamicBinding {
                symbols,
                values,
                body,
            } => {
                self.child(node.scope, *symbols)?;
                self.child(node.scope, *values)?;
                self.children(node.scope, body)?;
            }
            NodeKind::Lambda {
                parameters,
                body_scope,
                body,
            } => self.lambda_node(id, node.scope, parameters, *body_scope, body)?,
            NodeKind::LocalFunctions {
                kind,
                body_scope,
                definitions,
                body,
            } => {
                self.local_function_nodes(id, node.scope, *kind, *body_scope, definitions, body)?;
            }
            NodeKind::If {
                test,
                then_branch,
                else_branch,
            } => {
                self.child(node.scope, *test)?;
                self.child(node.scope, *then_branch)?;
                if let Some(branch) = else_branch {
                    self.child(node.scope, *branch)?;
                }
            }
            NodeKind::Sequence { body_scope, forms } => {
                self.scope_descendant(node.scope, *body_scope)?;
                self.body_scope(*body_scope, forms)?;
            }
            NodeKind::Block { .. }
            | NodeKind::ReturnFrom { .. }
            | NodeKind::Tagbody { .. }
            | NodeKind::Go { .. }
            | NodeKind::Catch { .. }
            | NodeKind::Throw { .. }
            | NodeKind::UnwindProtect { .. }
            | NodeKind::NonLocalExit { .. } => self.node_control(id, node)?,
            NodeKind::MultipleValues(operation) => {
                self.multiple_values(id, node.scope, operation)?;
            }
            NodeKind::TypeAssertion {
                value,
                asserted_type,
                expected_type,
            } => self.type_assertion_node(node.scope, *value, *asserted_type, *expected_type)?,
            NodeKind::CheckType {
                value,
                accepted_type,
                expected_type,
                source_origin,
            } => self.check_type_node(
                id,
                node,
                *value,
                *accepted_type,
                *expected_type,
                *source_origin,
            )?,
            NodeKind::CompileTimeForm { form, .. } | NodeKind::LoadTimeValue { form, .. } => {
                self.child(node.scope, *form)?;
            }
        }
        Ok(())
    }
    fn lambda_node(
        &self,
        id: NodeId,
        scope: ScopeId,
        parameters: &[BindingId],
        body_scope: ScopeId,
        body: &[NodeId],
    ) -> Result<(), VerifyError> {
        self.scope_descendant(scope, body_scope)?;
        if !parameters.is_empty() && body_scope == scope {
            return Err(VerifyError::InvalidBindingScopes(id));
        }
        self.parameters(id, body_scope, parameters)?;
        self.body_scope(body_scope, body)
    }

    fn local_function_nodes(
        &self,
        id: NodeId,
        scope: ScopeId,
        kind: LocalFunctionKind,
        body_scope: ScopeId,
        definitions: &[LocalFunctionDefinition],
        body: &[NodeId],
    ) -> Result<(), VerifyError> {
        self.scope_descendant(scope, body_scope)?;
        if !definitions.is_empty() && body_scope == scope {
            return Err(VerifyError::InvalidBindingScopes(id));
        }
        for definition in definitions {
            let binding = self.binding(definition.binding)?;
            let definition_scope = match kind {
                LocalFunctionKind::Flet => scope,
                LocalFunctionKind::Labels => body_scope,
            };
            if binding.kind() != BindingKind::LocalFunction
                || binding.scope() != body_scope
                || definition.definition_scope != definition_scope
            {
                return Err(VerifyError::InvalidBindingScopes(id));
            }
            self.scope_descendant(definition.definition_scope, definition.body_scope)?;
            if !definition.parameters.is_empty()
                && definition.body_scope == definition.definition_scope
            {
                return Err(VerifyError::InvalidBindingScopes(id));
            }
            self.parameters(id, definition.body_scope, &definition.parameters)?;
            self.body_scope(definition.body_scope, &definition.body)?;
        }
        self.body_scope(body_scope, body)
    }

    fn type_assertion_node(
        &self,
        scope: ScopeId,
        value: NodeId,
        asserted_type: TypeId,
        expected_type: TypeSpecConstId,
    ) -> Result<(), VerifyError> {
        self.child(scope, value)?;
        self.ty(asserted_type)?;
        self.type_constant(expected_type)
    }

    fn check_type_node(
        &self,
        id: NodeId,
        node: &Node,
        value: ValueId,
        accepted_type: TypeId,
        expected_type: TypeSpecConstId,
        source_origin: SourceOriginId,
    ) -> Result<(), VerifyError> {
        self.value(value)?;
        let producer = self
            .input
            .resources
            .value_producers
            .get(&value)
            .copied()
            .ok_or(VerifyError::MissingValueProducer(id))?;
        self.node(producer)?;
        self.ty(accepted_type)?;
        self.type_constant(expected_type)?;
        self.origin(source_origin)?;
        if source_origin != node.origin {
            return Err(VerifyError::CheckTypeOriginMismatch(id));
        }
        Ok(())
    }

    pub(super) fn let_scopes(&self, id: NodeId, node: &Node) -> Result<(), VerifyError> {
        let NodeKind::Let {
            kind,
            bindings,
            initializer_scopes: inits,
            binding_scopes: scopes,
            body_scope,
            body,
        } = &node.kind
        else {
            return Err(VerifyError::InvalidNode(id));
        };
        let parent = node.scope;
        let kind = *kind;
        let body_scope = *body_scope;
        if bindings.len() != inits.len() || bindings.len() != scopes.len() {
            return Err(VerifyError::InvalidBindingScopes(id));
        }
        self.scope_descendant(parent, body_scope)?;
        let mut previous = parent;
        for (index, binding) in bindings.iter().enumerate() {
            let init = *inits
                .get(index)
                .ok_or(VerifyError::InvalidBindingScopes(id))?;
            let scope = *scopes
                .get(index)
                .ok_or(VerifyError::InvalidBindingScopes(id))?;
            self.scope(init)?;
            self.scope(scope)?;
            let invalid_scope = match kind {
                LetKind::Parallel => init != parent || scope != body_scope,
                LetKind::Sequential => init != previous,
            };
            if invalid_scope {
                return Err(VerifyError::InvalidBindingScopes(id));
            }
            if let Some(initializer) = binding.initializer {
                self.node(initializer)?;
                if self.node_ref(initializer)?.scope != init {
                    return Err(VerifyError::InvalidBindingScopes(id));
                }
            }
            let bound = self.binding(binding.binding)?;
            if bound.kind() != BindingKind::LexicalVariable || bound.scope() != scope {
                return Err(VerifyError::InvalidBinding(binding.binding));
            }
            if kind == LetKind::Sequential {
                if self.input.bindings.scope_parent(scope).ok().flatten() != Some(init) {
                    return Err(VerifyError::InvalidBindingScopes(id));
                }
                previous = scope;
            }
        }
        if kind == LetKind::Sequential
            && !bindings.is_empty()
            && !self.is_descendant(body_scope, previous)?
        {
            return Err(VerifyError::InvalidBindingScopes(id));
        }
        if kind == LetKind::Parallel && !bindings.is_empty() && body_scope == parent {
            return Err(VerifyError::InvalidBindingScopes(id));
        }
        self.body_scope(body_scope, body)
    }
}
