use std::collections::HashSet;

use crate::binding::{BindingKind, ScopeId};
use crate::ids::{BindingId, NodeId, SymbolId, TypeId, TypeSpecConstId};
use crate::node::{Literal, NodeKind};

use super::builder::HirFormBuilder;
use super::error::FormError;
use super::metadata::{HirDeclaration, HirQuality, LambdaSignature, LambdaSignatureKey};

pub(super) trait BuilderValidation {
    fn validate_scope(&self, scope: ScopeId) -> Result<(), FormError>;
    fn validate_type(&self, ty: TypeId) -> Result<(), FormError>;
    fn validate_type_spec_constant(&self, id: TypeSpecConstId) -> Result<(), FormError>;
    fn register_declaration_symbols(
        &mut self,
        declarations: &[HirDeclaration],
    ) -> Result<(), FormError>;
    fn validate_signature(
        &self,
        key: LambdaSignatureKey,
        signature: &LambdaSignature,
    ) -> Result<(), FormError>;
    fn register_literal_symbols(&mut self, literal: &Literal) -> Result<(), FormError>;
}

impl BuilderValidation for HirFormBuilder {
    fn validate_scope(&self, scope: ScopeId) -> Result<(), FormError> {
        if scope.form() != self.form {
            return Err(FormError::ForeignScope(scope));
        }
        if self.bindings.scope_parent(scope).is_err() {
            return Err(FormError::InvalidScope(scope));
        }
        Ok(())
    }

    fn validate_type(&self, ty: TypeId) -> Result<(), FormError> {
        if ty.form() != self.form {
            return Err(FormError::ForeignType(ty));
        }
        if self.types.get(ty).is_none() {
            return Err(FormError::InvalidType(ty));
        }
        Ok(())
    }

    fn validate_type_spec_constant(&self, id: TypeSpecConstId) -> Result<(), FormError> {
        if id.form() != self.form {
            return Err(FormError::ForeignTypeSpecConstant(id));
        }
        if self.type_spec_constants.get(id).is_none() {
            return Err(FormError::InvalidTypeSpecConstant(id));
        }
        Ok(())
    }

    fn register_declaration_symbols(
        &mut self,
        declarations: &[HirDeclaration],
    ) -> Result<(), FormError> {
        for declaration in declarations {
            match declaration {
                HirDeclaration::Special { names }
                | HirDeclaration::Inline { names }
                | HirDeclaration::Notinline { names }
                | HirDeclaration::DynamicExtent { names }
                | HirDeclaration::Ignore { names }
                | HirDeclaration::Ignorable { names } => {
                    for symbol in names {
                        self.register_symbol(*symbol)?;
                    }
                }
                HirDeclaration::Type { names, .. } => {
                    for symbol in names {
                        self.register_symbol(*symbol)?;
                    }
                }
                HirDeclaration::Ftype { name, .. } => self.register_symbol(*name)?,
                HirDeclaration::Optimize { settings } => {
                    for setting in settings {
                        if let HirQuality::Unknown(symbol) = setting.quality {
                            self.register_symbol(symbol)?;
                        }
                    }
                }
                HirDeclaration::Unknown { name, arguments } => {
                    self.register_symbol(*name)?;
                    for argument in arguments {
                        self.register_literal_symbols(argument)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn validate_signature(
        &self,
        key: LambdaSignatureKey,
        signature: &LambdaSignature,
    ) -> Result<(), FormError> {
        let (definition_scope, body_scope) = match key {
            LambdaSignatureKey::Lambda(node) => {
                self.validate_node(node)?;
                let Some(node_data) = self.nodes.get(node) else {
                    return Err(FormError::InvalidFunctionSignature);
                };
                let NodeKind::Lambda { body_scope, .. } = &node_data.kind else {
                    return Err(FormError::InvalidFunctionSignature);
                };
                (node_data.scope, *body_scope)
            }
            LambdaSignatureKey::LocalFunction(binding) => {
                self.validate_binding(binding)?;
                let Some(info) = self.bindings.binding(binding) else {
                    return Err(FormError::InvalidFunctionSignature);
                };
                if info.kind() != BindingKind::LocalFunction {
                    return Err(FormError::InvalidFunctionSignature);
                }
                self.nodes
                    .iter()
                    .flat_map(|(_, node)| match &node.kind {
                        NodeKind::LocalFunctions { definitions, .. } => definitions
                            .iter()
                            .find(|definition| definition.binding == binding)
                            .map(|definition| (definition.definition_scope, definition.body_scope)),
                        _ => None,
                    })
                    .next()
                    .ok_or(FormError::InvalidFunctionSignature)?
            }
        };
        self.validate_scope(definition_scope)?;
        self.validate_scope(body_scope)?;
        let mut parameters = signature
            .required
            .iter()
            .copied()
            .map(|binding| (binding, None))
            .collect::<Vec<_>>();
        for parameter in &signature.optional {
            parameters.push((
                parameter.binding,
                Some((parameter.initializer_scope, parameter.default)),
            ));
            if let Some(supplied_p) = parameter.supplied_p {
                parameters.push((supplied_p, None));
            }
        }
        if let Some(rest) = signature.rest {
            parameters.push((rest, None));
        }
        for parameter in &signature.keys {
            parameters.push((
                parameter.binding,
                Some((parameter.initializer_scope, parameter.default)),
            ));
            if let Some(supplied_p) = parameter.supplied_p {
                parameters.push((supplied_p, None));
            }
        }
        for parameter in &signature.aux {
            parameters.push((
                parameter.binding,
                Some((parameter.initializer_scope, parameter.default)),
            ));
        }
        let mut visible_scope = definition_scope;
        let mut seen_bindings = HashSet::new();
        for (binding, initializer) in parameters {
            if !seen_bindings.insert(binding) {
                return Err(FormError::InvalidFunctionSignature);
            }
            self.validate_parameter_binding(binding, definition_scope)?;
            let parameter_scope = self
                .bindings
                .binding(binding)
                .map(|info| info.scope())
                .ok_or(FormError::InvalidFunctionSignature)?;
            if parameter_scope == definition_scope
                || !self.scope_is_ancestor(visible_scope, parameter_scope)
            {
                return Err(FormError::InvalidFunctionSignature);
            }
            if let Some((scope, default)) = initializer {
                self.validate_scope(scope)?;
                if scope != visible_scope || scope == parameter_scope {
                    return Err(FormError::InvalidFunctionSignature);
                }
                if let Some(node) = default {
                    self.validate_node(node)?;
                    if self.nodes.get(node).map(|node| node.scope) != Some(scope) {
                        return Err(FormError::InvalidFunctionSignature);
                    }
                }
            }
            visible_scope = parameter_scope;
        }
        if !self.scope_is_ancestor(visible_scope, body_scope) {
            return Err(FormError::InvalidFunctionSignature);
        }
        for parameter in &signature.keys {
            self.register_symbol_owner(parameter.keyword)?;
        }
        Ok(())
    }

    fn register_literal_symbols(&mut self, literal: &Literal) -> Result<(), FormError> {
        match literal {
            Literal::Symbol(symbol) => self.register_symbol(*symbol),
            Literal::Cons(car, cdr) => {
                self.register_literal_symbols(car)?;
                self.register_literal_symbols(cdr)
            }
            Literal::Vector(items)
            | Literal::Array {
                elements: items, ..
            }
            | Literal::SpecializedArray {
                elements: items, ..
            } => {
                for item in items {
                    self.register_literal_symbols(item)?;
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

impl HirFormBuilder {
    fn validate_node(&self, node: NodeId) -> Result<(), FormError> {
        if node.form() != self.form {
            return Err(FormError::ForeignNode(node));
        }
        if self.nodes.get(node).is_none() {
            return Err(FormError::InvalidNode(node));
        }
        Ok(())
    }

    fn validate_binding(&self, binding: BindingId) -> Result<(), FormError> {
        if binding.form() != self.form {
            return Err(FormError::ForeignBinding(binding));
        }
        if self.bindings.binding(binding).is_none() {
            return Err(FormError::InvalidFunctionSignature);
        }
        Ok(())
    }

    fn validate_parameter_binding(
        &self,
        binding: BindingId,
        function_scope: ScopeId,
    ) -> Result<(), FormError> {
        self.validate_binding(binding)?;
        let info = self
            .bindings
            .binding(binding)
            .ok_or(FormError::InvalidFunctionSignature)?;
        if info.kind() != BindingKind::LexicalVariable
            || !self.scope_is_ancestor(function_scope, info.scope())
        {
            return Err(FormError::InvalidFunctionSignature);
        }
        Ok(())
    }

    fn register_symbol_owner(&self, symbol: SymbolId) -> Result<(), FormError> {
        if symbol.compilation_unit() != self.form.compilation_unit() {
            return Err(FormError::ForeignSymbol(symbol));
        }
        Ok(())
    }

    fn scope_is_ancestor(&self, ancestor: ScopeId, mut scope: ScopeId) -> bool {
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
}
