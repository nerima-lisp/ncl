use ncl_hir::{
    CompileTimeSituation, Literal as HirLiteral, MultipleValueOperation, NodeKind, ScopeId,
};

use crate::ast::{EvalSituation, Expr};

use super::HirBuildError;
use super::context::BuildContext;

impl BuildContext<'_> {
    pub(super) fn lower_expr(
        &mut self,
        expr: &Expr,
        scope: ScopeId,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        match expr {
            Expr::Constant(literal) => {
                let literal = self.literal(literal)?;
                self.node(scope, NodeKind::Literal(literal))
            }
            Expr::Variable(symbol) => self.lower_variable(scope, symbol),
            Expr::Call {
                operator,
                arguments,
            } => self.lower_call(scope, operator, arguments),
            Expr::Function(designator) => self.lower_function_designator(scope, designator),
            Expr::Lambda(lambda) => self.lower_lambda(scope, lambda),
            Expr::If {
                test,
                then,
                otherwise,
            } => {
                let test = self.lower_expr(test, scope)?;
                let then_branch = self.lower_expr(then, scope)?;
                let else_branch = otherwise
                    .as_deref()
                    .map(|form| self.lower_expr(form, scope))
                    .transpose()?;
                self.node(
                    scope,
                    NodeKind::If {
                        test,
                        then_branch,
                        else_branch,
                    },
                )
            }
            Expr::Progn(forms) => self.lower_sequence(forms, scope),
            Expr::Block { name, body } => self.lower_block(scope, name, body),
            Expr::ReturnFrom { name, value } => {
                self.lower_return_from(scope, name, value.as_deref())
            }
            Expr::Tagbody(items) => self.lower_tagbody(scope, items),
            Expr::Go { tag } => self.lower_go(scope, tag),
            Expr::Catch { tag, body } => self.lower_catch(scope, tag, body),
            Expr::Throw { tag, value } => self.lower_throw(scope, tag, value),
            Expr::UnwindProtect { protected, cleanup } => {
                self.lower_unwind_protect(scope, protected, cleanup)
            }
            Expr::Let {
                sequential,
                bindings,
                declarations,
                body,
            } => self.lower_let(scope, *sequential, bindings, declarations, body),
            Expr::Progv {
                symbols,
                values,
                body,
            } => self.lower_progv(scope, symbols, values, body),
            Expr::Setq(pairs) => self.lower_setq(scope, pairs),
            Expr::MultipleValueCall {
                function,
                arguments,
            } => {
                let function = self.lower_expr(function, scope)?;
                let arguments = arguments
                    .iter()
                    .map(|item| self.lower_expr(item, scope))
                    .collect::<Result<Vec<_>, _>>()?;
                self.node(
                    scope,
                    NodeKind::MultipleValues(MultipleValueOperation::Call {
                        function,
                        arguments,
                    }),
                )
            }
            Expr::MultipleValueProg1 { first, forms } => {
                let first = self.lower_expr(first, scope)?;
                let rest = forms
                    .iter()
                    .map(|form| self.lower_expr(form, scope))
                    .collect::<Result<Vec<_>, _>>()?;
                self.node(
                    scope,
                    NodeKind::MultipleValues(MultipleValueOperation::Prog1 { first, rest }),
                )
            }
            Expr::The {
                type_specifier,
                value,
            } => {
                let value = self.lower_expr(value, scope)?;
                let (asserted_type, expected_type) = self.type_specifier(type_specifier)?;
                self.node(
                    scope,
                    NodeKind::TypeAssertion {
                        value,
                        asserted_type,
                        expected_type,
                    },
                )
            }
            Expr::EvalWhen { situations, body } => {
                let situations = situations.iter().map(map_situation).collect();
                let form = self.lower_sequence(body, scope)?;
                self.node(scope, NodeKind::CompileTimeForm { situations, form })
            }
            Expr::LoadTimeValue { form, read_only } => {
                let form = self.lower_expr(form, scope)?;
                self.node(
                    scope,
                    NodeKind::LoadTimeValue {
                        form,
                        read_only: *read_only,
                    },
                )
            }
            Expr::Locally { declarations, body } => {
                self.lower_declared_sequence(scope, declarations, body)
            }
            Expr::Flet {
                definitions,
                declarations,
                body,
            } => self.lower_local_functions(scope, definitions, declarations, body, false),
            Expr::Labels {
                definitions,
                declarations,
                body,
            } => self.lower_local_functions(scope, definitions, declarations, body, true),
            Expr::Macrolet {
                definitions,
                declarations,
                body,
            } => {
                if !definitions.is_empty() {
                    return Err(HirBuildError::Unsupported(
                        super::UnsupportedHirForm::Macrolet,
                    ));
                }
                self.lower_declared_sequence(scope, declarations, body)
            }
            Expr::SymbolMacrolet {
                definitions,
                declarations,
                body,
            } => {
                if !definitions.is_empty() {
                    return Err(HirBuildError::Unsupported(
                        super::UnsupportedHirForm::SymbolMacrolet,
                    ));
                }
                self.lower_declared_sequence(scope, declarations, body)
            }
        }
    }

    pub(super) fn lower_sequence(
        &mut self,
        forms: &[Expr],
        scope: ScopeId,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        if forms.is_empty() {
            return self.node(scope, NodeKind::Literal(HirLiteral::Nil));
        }
        let forms = forms
            .iter()
            .map(|form| self.lower_expr(form, scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.node(
            scope,
            NodeKind::Sequence {
                body_scope: scope,
                forms,
            },
        )
    }

    pub(super) fn node(
        &mut self,
        scope: ScopeId,
        kind: NodeKind,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        self.form
            .alloc_node(scope, kind)
            .map_err(HirBuildError::Form)
    }
}

fn map_situation(situation: &EvalSituation) -> CompileTimeSituation {
    match situation {
        EvalSituation::CompileToplevel => CompileTimeSituation::CompileToplevel,
        EvalSituation::LoadToplevel => CompileTimeSituation::LoadToplevel,
        EvalSituation::Execute => CompileTimeSituation::Execute,
        EvalSituation::Eval => CompileTimeSituation::Eval,
    }
}
