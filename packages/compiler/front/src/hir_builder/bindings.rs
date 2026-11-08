use ncl_hir::binding::BindingUseKind;
use ncl_hir::{
    CallTarget, LambdaAux, LambdaKey, LambdaOptional, LambdaSignature, LetBinding as HirLetBinding,
    LetKind, LocalFunctionDefinition, LocalFunctionKind, NodeKind, VariableTarget,
};

use crate::ast::{Expr, LambdaExpr, LetBinding, LocalFunction};
use crate::declaration::Declaration;
use crate::symbols::SymbolRef;

use super::context::BuildContext;
use super::{HirBuildError, UnsupportedHirForm};

impl BuildContext<'_> {
    pub(super) fn lower_variable(
        &mut self,
        scope: ncl_hir::ScopeId,
        symbol: &SymbolRef,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let target = self.resolved_variable(scope, symbol)?;
        let node = self.node(scope, NodeKind::VariableReference { target })?;
        if let VariableTarget::Lexical(binding) = target {
            self.form
                .bindings_mut()
                .record_use(binding, node, BindingUseKind::Read)?;
        }
        Ok(node)
    }

    pub(super) fn lower_let(
        &mut self,
        scope: ncl_hir::ScopeId,
        sequential: bool,
        bindings: &[LetBinding],
        declarations: &[Declaration],
        body: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let mut initializer_scopes = Vec::with_capacity(bindings.len());
        let mut binding_scopes = Vec::with_capacity(bindings.len());
        let mut binding_ids = Vec::with_capacity(bindings.len());
        let body_scope;

        let special_names = self.special_declarations(declarations)?;
        if sequential {
            let mut current = scope;
            for binding in bindings {
                initializer_scopes.push(current);
                current = self.form.bindings_mut().push_scope(current)?;
                let symbol = self.intern(&binding.name)?;
                let id = self.form.bindings_mut().bind_variable(current, symbol)?;
                if special_names.contains(&symbol) {
                    self.add_special(current, symbol);
                }
                binding_ids.push(id);
                binding_scopes.push(current);
            }
            body_scope = self.form.bindings_mut().push_scope(current)?;
        } else {
            body_scope = self.form.bindings_mut().push_scope(scope)?;
            for binding in bindings {
                initializer_scopes.push(scope);
                let symbol = self.intern(&binding.name)?;
                let id = self.form.bindings_mut().bind_variable(body_scope, symbol)?;
                if special_names.contains(&symbol) {
                    self.add_special(body_scope, symbol);
                }
                binding_ids.push(id);
                binding_scopes.push(body_scope);
            }
        }

        self.push_scope_maps();
        self.attach_declarations(body_scope, declarations)?;
        let hir_bindings = bindings
            .iter()
            .zip(&binding_ids)
            .zip(&initializer_scopes)
            .map(|((binding, id), initializer_scope)| {
                let initializer = binding
                    .value
                    .as_ref()
                    .map(|value| self.lower_expr(value, *initializer_scope))
                    .transpose()?;
                Ok(HirLetBinding {
                    binding: *id,
                    initializer,
                })
            })
            .collect::<Result<Vec<_>, HirBuildError>>()?;
        let body_nodes = body
            .iter()
            .map(|form| self.lower_expr(form, body_scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.pop_scope_maps();
        self.node(
            scope,
            NodeKind::Let {
                kind: if sequential {
                    LetKind::Sequential
                } else {
                    LetKind::Parallel
                },
                bindings: hir_bindings,
                initializer_scopes,
                binding_scopes,
                body_scope,
                body: body_nodes,
            },
        )
    }

    pub(super) fn lower_setq(
        &mut self,
        scope: ncl_hir::ScopeId,
        pairs: &[(SymbolRef, Expr)],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let mut assignments = Vec::with_capacity(pairs.len());
        for (name, value) in pairs {
            let value = self.lower_expr(value, scope)?;
            let target = self.resolved_variable(scope, name)?;
            let node = self.node(scope, NodeKind::Assignment { target, value })?;
            if let VariableTarget::Lexical(binding) = target {
                self.form
                    .bindings_mut()
                    .record_use(binding, node, BindingUseKind::Write)?;
            }
            assignments.push(node);
        }
        if assignments.is_empty() {
            return self.node(scope, NodeKind::Literal(ncl_hir::Literal::Nil));
        }
        if assignments.len() == 1
            && let [assignment] = assignments.as_slice()
        {
            return Ok(*assignment);
        }
        self.node(
            scope,
            NodeKind::Sequence {
                body_scope: scope,
                forms: assignments,
            },
        )
    }

    pub(super) fn lower_lambda(
        &mut self,
        scope: ncl_hir::ScopeId,
        lambda: &LambdaExpr,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        if lambda.docstring.is_some() {
            return Err(HirBuildError::Unrepresentable(
                UnsupportedHirForm::Docstring,
            ));
        }
        let (body_scope, signature) = self.lambda_signature(scope, &lambda.lambda_list)?;
        self.push_scope_maps();
        self.attach_declarations(body_scope, &lambda.declarations)?;
        let body = lambda
            .body
            .iter()
            .map(|form| self.lower_expr(form, body_scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.pop_scope_maps();
        let function = self.node(
            scope,
            NodeKind::Lambda {
                parameters: Vec::new(),
                body_scope,
                body,
            },
        )?;
        self.form
            .set_lambda_signature_for_lambda(function, signature)?;
        self.node(scope, NodeKind::ClosureReference { function })
    }

    fn lambda_signature(
        &mut self,
        scope: ncl_hir::ScopeId,
        lambda_list: &crate::lambda_list::LambdaList,
    ) -> Result<(ncl_hir::ScopeId, LambdaSignature), HirBuildError> {
        if lambda_list.whole.is_some()
            || lambda_list.environment.is_some()
            || lambda_list.body.is_some()
        {
            return Err(HirBuildError::Unrepresentable(
                UnsupportedHirForm::LambdaPattern,
            ));
        }

        let mut current = scope;
        let mut required = Vec::with_capacity(lambda_list.required.len());
        for name in &lambda_list.required {
            required.push(self.push_parameter_scope(&mut current, name)?);
        }

        let optional = self.optional_parameters(&mut current, &lambda_list.optional)?;

        let rest = if let Some(parameter) = &lambda_list.rest {
            Some(self.push_parameter_scope(&mut current, parameter)?)
        } else {
            None
        };

        let keys = self.key_parameters(&mut current, &lambda_list.keys)?;
        let aux = self.aux_parameters(&mut current, &lambda_list.aux)?;

        if current == scope {
            current = self.form.bindings_mut().push_scope(scope)?;
        }
        Ok((
            current,
            LambdaSignature {
                required,
                optional,
                rest,
                keys,
                allow_other_keys: lambda_list.allow_other_keys,
                aux,
            },
        ))
    }

    fn optional_parameters(
        &mut self,
        current: &mut ncl_hir::ScopeId,
        parameters: &[crate::lambda_list::OptionalParam],
    ) -> Result<Vec<LambdaOptional>, HirBuildError> {
        let mut optional = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            Self::validate_parameter(&parameter.name)?;
            let initializer_scope = *current;
            let default = parameter
                .default
                .as_ref()
                .map(|form| self.lower_expr(form, initializer_scope))
                .transpose()?;
            let binding = self.push_parameter_scope(current, &parameter.name)?;
            let supplied_p = self.supplied_parameter(*current, parameter.supplied_p.as_ref())?;
            optional.push(LambdaOptional {
                binding,
                default,
                initializer_scope,
                supplied_p,
            });
        }
        Ok(optional)
    }

    fn key_parameters(
        &mut self,
        current: &mut ncl_hir::ScopeId,
        parameters: &[crate::lambda_list::KeyParam],
    ) -> Result<Vec<LambdaKey>, HirBuildError> {
        let mut keys = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            Self::validate_parameter(&parameter.name)?;
            let initializer_scope = *current;
            let default = parameter
                .default
                .as_ref()
                .map(|form| self.lower_expr(form, initializer_scope))
                .transpose()?;
            *current = self.form.bindings_mut().push_scope(*current)?;
            let keyword = self.intern(&parameter.keyword)?;
            let binding = self.bind_parameter(*current, &parameter.name)?;
            let supplied_p = self.supplied_parameter(*current, parameter.supplied_p.as_ref())?;
            keys.push(LambdaKey {
                keyword,
                binding,
                default,
                initializer_scope,
                supplied_p,
            });
        }
        Ok(keys)
    }

    fn aux_parameters(
        &mut self,
        current: &mut ncl_hir::ScopeId,
        parameters: &[crate::lambda_list::AuxParam],
    ) -> Result<Vec<LambdaAux>, HirBuildError> {
        let mut aux = Vec::with_capacity(parameters.len());
        for parameter in parameters {
            Self::validate_parameter(&parameter.name)?;
            let initializer_scope = *current;
            let default = parameter
                .default
                .as_ref()
                .map(|form| self.lower_expr(form, initializer_scope))
                .transpose()?;
            let binding = self.push_parameter_scope(current, &parameter.name)?;
            aux.push(LambdaAux {
                binding,
                default,
                initializer_scope,
            });
        }
        Ok(aux)
    }

    fn push_parameter_scope(
        &mut self,
        current: &mut ncl_hir::ScopeId,
        parameter: &crate::lambda_list::ParamName,
    ) -> Result<ncl_hir::BindingId, HirBuildError> {
        Self::validate_parameter(parameter)?;
        *current = self.form.bindings_mut().push_scope(*current)?;
        self.bind_parameter(*current, parameter)
    }

    fn supplied_parameter(
        &mut self,
        scope: ncl_hir::ScopeId,
        parameter: Option<&crate::lambda_list::ParamName>,
    ) -> Result<Option<ncl_hir::BindingId>, HirBuildError> {
        parameter
            .map(|parameter| self.bind_parameter(scope, parameter))
            .transpose()
    }

    fn bind_parameter(
        &mut self,
        scope: ncl_hir::ScopeId,
        parameter: &crate::lambda_list::ParamName,
    ) -> Result<ncl_hir::BindingId, HirBuildError> {
        let Some(name) = parameter.symbol() else {
            return Err(HirBuildError::Unrepresentable(
                UnsupportedHirForm::LambdaPattern,
            ));
        };
        let symbol = self.intern(name)?;
        self.form
            .bindings_mut()
            .bind_variable(scope, symbol)
            .map_err(HirBuildError::Binding)
    }

    const fn validate_parameter(
        parameter: &crate::lambda_list::ParamName,
    ) -> Result<(), HirBuildError> {
        if parameter.symbol().is_some() {
            Ok(())
        } else {
            Err(HirBuildError::Unrepresentable(
                UnsupportedHirForm::LambdaPattern,
            ))
        }
    }

    pub(super) fn lower_local_functions(
        &mut self,
        scope: ncl_hir::ScopeId,
        definitions: &[LocalFunction],
        declarations: &[Declaration],
        body: &[Expr],
        recursive: bool,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let kind = if recursive {
            LocalFunctionKind::Labels
        } else {
            LocalFunctionKind::Flet
        };
        let mut names = Vec::with_capacity(definitions.len());
        for definition in definitions {
            names.push(self.intern(&definition.name)?);
        }
        let group = self
            .form
            .bindings_mut()
            .bind_local_functions(scope, kind, &names)?;
        let mut hir_definitions = Vec::with_capacity(definitions.len());
        let mut signatures = Vec::with_capacity(definitions.len());
        for (definition, function_binding) in definitions.iter().zip(&group.definitions) {
            let lambda = &definition.lambda;
            if lambda.docstring.is_some() {
                return Err(HirBuildError::Unrepresentable(
                    UnsupportedHirForm::Docstring,
                ));
            }
            let (body_scope, signature) =
                self.lambda_signature(function_binding.definition_scope, &lambda.lambda_list)?;
            self.push_scope_maps();
            self.attach_declarations(body_scope, &lambda.declarations)?;
            let body_nodes = lambda
                .body
                .iter()
                .map(|form| self.lower_expr(form, body_scope))
                .collect::<Result<Vec<_>, _>>()?;
            self.pop_scope_maps();
            hir_definitions.push(LocalFunctionDefinition {
                binding: function_binding.binding,
                definition_scope: function_binding.definition_scope,
                parameters: Vec::new(),
                body_scope,
                body: body_nodes,
            });
            signatures.push((function_binding.binding, signature));
        }
        self.push_scope_maps();
        self.attach_declarations(group.body_scope, declarations)?;
        let body_nodes = body
            .iter()
            .map(|form| self.lower_expr(form, group.body_scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.pop_scope_maps();
        let node = self.node(
            scope,
            NodeKind::LocalFunctions {
                kind,
                body_scope: group.body_scope,
                definitions: hir_definitions,
                body: body_nodes,
            },
        )?;
        for (binding, signature) in signatures {
            self.form
                .set_lambda_signature_for_local_function(binding, signature)?;
        }
        Ok(node)
    }

    pub(super) fn resolve_function_target(
        &mut self,
        scope: ncl_hir::ScopeId,
        symbol: &SymbolRef,
    ) -> Result<CallTarget, HirBuildError> {
        let symbol_id = self.intern(symbol)?;
        self.form
            .bindings_mut()
            .resolve_function(scope, symbol_id)?
            .map_or(Ok(CallTarget::Global(symbol_id)), |binding| {
                Ok(CallTarget::Local(binding))
            })
    }
}
