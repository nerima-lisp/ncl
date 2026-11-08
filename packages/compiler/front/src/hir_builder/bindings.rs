use ncl_hir::binding::BindingUseKind;
use ncl_hir::{
    BindingKind, CallTarget, LambdaAux, LambdaKey, LambdaOptional, LambdaSignature,
    LetBinding as HirLetBinding, LetKind, LocalFunctionDefinition, LocalFunctionKind, NodeKind,
    VariableTarget,
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
        if assignments.len() == 1 {
            if let [assignment] = assignments.as_slice() {
                return Ok(*assignment);
            }
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
            return Err(HirBuildError::Unsupported(UnsupportedHirForm::Docstring));
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
            return Err(HirBuildError::Unsupported(
                UnsupportedHirForm::LambdaPattern,
            ));
        }

        let mut current = scope;
        let mut required = Vec::with_capacity(lambda_list.required.len());
        for name in &lambda_list.required {
            let Some(name) = name.symbol() else {
                return Err(HirBuildError::Unsupported(
                    UnsupportedHirForm::LambdaPattern,
                ));
            };
            current = self.form.bindings_mut().push_scope(current)?;
            let symbol = self.intern(name)?;
            required.push(self.form.bindings_mut().bind_variable(current, symbol)?);
        }

        let mut optional = Vec::with_capacity(lambda_list.optional.len());
        for parameter in &lambda_list.optional {
            let Some(name) = parameter.name.symbol() else {
                return Err(HirBuildError::Unsupported(
                    UnsupportedHirForm::LambdaPattern,
                ));
            };
            let initializer_scope = current;
            let default = parameter
                .default
                .as_ref()
                .map(|form| self.lower_expr(form, initializer_scope))
                .transpose()?;
            current = self.form.bindings_mut().push_scope(current)?;
            let binding_symbol = self.intern(name)?;
            let binding = self
                .form
                .bindings_mut()
                .bind_variable(current, binding_symbol)?;
            let supplied_p = parameter
                .supplied_p
                .as_ref()
                .map(|parameter| {
                    let Some(name) = parameter.symbol() else {
                        return Err(HirBuildError::Unsupported(
                            UnsupportedHirForm::LambdaPattern,
                        ));
                    };
                    let symbol = self.intern(name)?;
                    self.form
                        .bindings_mut()
                        .bind_variable(current, symbol)
                        .map_err(HirBuildError::Binding)
                })
                .transpose()?;
            optional.push(LambdaOptional {
                binding,
                default,
                initializer_scope,
                supplied_p,
            });
        }

        let rest = if let Some(parameter) = &lambda_list.rest {
            let Some(name) = parameter.symbol() else {
                return Err(HirBuildError::Unsupported(
                    UnsupportedHirForm::LambdaPattern,
                ));
            };
            current = self.form.bindings_mut().push_scope(current)?;
            let symbol = self.intern(name)?;
            Some(self.form.bindings_mut().bind_variable(current, symbol)?)
        } else {
            None
        };

        let mut keys = Vec::with_capacity(lambda_list.keys.len());
        for parameter in &lambda_list.keys {
            let Some(name) = parameter.name.symbol() else {
                return Err(HirBuildError::Unsupported(
                    UnsupportedHirForm::LambdaPattern,
                ));
            };
            let initializer_scope = current;
            let default = parameter
                .default
                .as_ref()
                .map(|form| self.lower_expr(form, initializer_scope))
                .transpose()?;
            current = self.form.bindings_mut().push_scope(current)?;
            let keyword = self.intern(&parameter.keyword)?;
            let binding_symbol = self.intern(name)?;
            let binding = self
                .form
                .bindings_mut()
                .bind_variable(current, binding_symbol)?;
            let supplied_p = parameter
                .supplied_p
                .as_ref()
                .map(|parameter| {
                    let Some(name) = parameter.symbol() else {
                        return Err(HirBuildError::Unsupported(
                            UnsupportedHirForm::LambdaPattern,
                        ));
                    };
                    let symbol = self.intern(name)?;
                    self.form
                        .bindings_mut()
                        .bind_variable(current, symbol)
                        .map_err(HirBuildError::Binding)
                })
                .transpose()?;
            keys.push(LambdaKey {
                keyword,
                binding,
                default,
                initializer_scope,
                supplied_p,
            });
        }

        let mut aux = Vec::with_capacity(lambda_list.aux.len());
        for parameter in &lambda_list.aux {
            let Some(name) = parameter.name.symbol() else {
                return Err(HirBuildError::Unsupported(
                    UnsupportedHirForm::LambdaPattern,
                ));
            };
            let initializer_scope = current;
            let default = parameter
                .default
                .as_ref()
                .map(|form| self.lower_expr(form, initializer_scope))
                .transpose()?;
            current = self.form.bindings_mut().push_scope(current)?;
            let symbol = self.intern(name)?;
            let binding = self.form.bindings_mut().bind_variable(current, symbol)?;
            aux.push(LambdaAux {
                binding,
                default,
                initializer_scope,
            });
        }

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
                return Err(HirBuildError::Unsupported(UnsupportedHirForm::Docstring));
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
        match self
            .form
            .bindings_mut()
            .resolve_function(scope, symbol_id)?
        {
            Some(binding) => {
                if self
                    .form
                    .bindings_mut()
                    .binding(binding)
                    .map(|item| item.kind())
                    != Some(BindingKind::LocalFunction)
                {
                    return Err(HirBuildError::Unsupported(
                        UnsupportedHirForm::FunctionBinding,
                    ));
                }
                Ok(CallTarget::Local(binding))
            }
            None => Ok(CallTarget::Global(symbol_id)),
        }
    }
}
