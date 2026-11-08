use std::collections::HashSet;

use crate::binding::{BindingKind, ScopeId};
use crate::node::NodeKind;
use crate::verify::VerifyError;

use super::metadata::{HirDeclaration, HirQuality, LambdaSignature, LambdaSignatureKey};
use super::storage::HirForm;

impl HirForm {
    pub(super) fn verify_metadata(&self) -> Result<(), VerifyError> {
        for (scope, metadata) in &self.metadata.scopes {
            self.verify_scope(*scope)?;
            for declaration in &metadata.declarations {
                match declaration {
                    HirDeclaration::Special { names }
                    | HirDeclaration::Inline { names }
                    | HirDeclaration::Notinline { names }
                    | HirDeclaration::DynamicExtent { names }
                    | HirDeclaration::Ignore { names }
                    | HirDeclaration::Ignorable { names } => {
                        for symbol in names {
                            self.verify_symbol(*symbol)?;
                        }
                    }
                    HirDeclaration::Type {
                        type_id,
                        source_specifier,
                        names,
                    } => {
                        self.verify_type(*type_id)?;
                        self.verify_type_spec_constant(*source_specifier)?;
                        for symbol in names {
                            self.verify_symbol(*symbol)?;
                        }
                    }
                    HirDeclaration::Ftype {
                        type_id,
                        source_specifier,
                        name,
                    } => {
                        self.verify_type(*type_id)?;
                        self.verify_type_spec_constant(*source_specifier)?;
                        self.verify_symbol(*name)?;
                    }
                    HirDeclaration::Optimize { settings } => {
                        for setting in settings {
                            if let HirQuality::Unknown(symbol) = setting.quality {
                                self.verify_symbol(symbol)?;
                            }
                        }
                    }
                    HirDeclaration::Unknown { name, arguments } => {
                        self.verify_symbol(*name)?;
                        for argument in arguments {
                            self.verify_literal(argument)?;
                        }
                    }
                }
            }
        }
        for (key, signature) in &self.metadata.lambda_signatures {
            let (definition_scope, body_scope) = self.signature_scopes(*key)?;
            self.verify_signature(signature, definition_scope, body_scope)?;
        }
        for (_, literal) in self.type_spec_constants.iter() {
            self.verify_literal(literal)?;
        }
        Ok(())
    }

    fn signature_scopes(&self, key: LambdaSignatureKey) -> Result<(ScopeId, ScopeId), VerifyError> {
        match key {
            LambdaSignatureKey::Lambda(node) => {
                let node_data = self.nodes.get(node).ok_or(VerifyError::InvalidNode(node))?;
                let NodeKind::Lambda { body_scope, .. } = &node_data.kind else {
                    return Err(VerifyError::InvalidNode(node));
                };
                self.verify_scope(node_data.scope)?;
                self.verify_scope(*body_scope)?;
                Ok((node_data.scope, *body_scope))
            }
            LambdaSignatureKey::LocalFunction(binding) => {
                let binding_data = self
                    .bindings
                    .binding(binding)
                    .ok_or(VerifyError::InvalidBinding(binding))?;
                if binding_data.kind() != BindingKind::LocalFunction {
                    return Err(VerifyError::InvalidBinding(binding));
                }
                let scopes = self
                    .nodes
                    .iter()
                    .flat_map(|(_, node)| match &node.kind {
                        NodeKind::LocalFunctions { definitions, .. } => definitions
                            .iter()
                            .find(|definition| definition.binding == binding)
                            .map(|definition| (definition.definition_scope, definition.body_scope)),
                        _ => None,
                    })
                    .next()
                    .ok_or(VerifyError::InvalidBinding(binding))?;
                self.verify_scope(scopes.0)?;
                self.verify_scope(scopes.1)?;
                Ok(scopes)
            }
        }
    }

    fn verify_signature(
        &self,
        signature: &LambdaSignature,
        definition_scope: ScopeId,
        body_scope: ScopeId,
    ) -> Result<(), VerifyError> {
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
                return Err(VerifyError::InvalidBinding(binding));
            }
            self.verify_parameter_binding(binding, body_scope)?;
            let binding_scope = self
                .bindings
                .binding(binding)
                .map(|info| info.scope())
                .ok_or(VerifyError::InvalidBinding(binding))?;
            if binding_scope == definition_scope
                || !self.scope_is_ancestor(visible_scope, binding_scope)
            {
                return Err(VerifyError::InvalidBinding(binding));
            }
            if let Some((initializer_scope, default)) = initializer {
                self.verify_scope(initializer_scope)?;
                if initializer_scope != visible_scope || initializer_scope == binding_scope {
                    return Err(VerifyError::InvalidScope(initializer_scope));
                }
                if let Some(node) = default {
                    let default_node =
                        self.nodes.get(node).ok_or(VerifyError::InvalidNode(node))?;
                    if default_node.scope != initializer_scope {
                        return Err(VerifyError::InvalidScope(initializer_scope));
                    }
                }
            }
            visible_scope = binding_scope;
        }
        if !self.scope_is_ancestor(visible_scope, body_scope) {
            return Err(VerifyError::InvalidScope(body_scope));
        }
        for key in &signature.keys {
            self.verify_symbol(key.keyword)?;
        }
        Ok(())
    }
}
