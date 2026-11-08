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
            } => self.lower_if(scope, test, then, otherwise.as_deref()),
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
            } => self.lower_multiple_value_call(scope, function, arguments),
            Expr::MultipleValueProg1 { first, forms } => {
                self.lower_multiple_value_prog1(scope, first, forms)
            }
            Expr::The {
                type_specifier,
                value,
            } => self.lower_type_assertion(scope, type_specifier, value),
            Expr::EvalWhen { situations, body } => self.lower_eval_when(scope, situations, body),
            Expr::LoadTimeValue { form, read_only } => {
                self.lower_load_time_value(scope, form, *read_only)
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
                    return Err(HirBuildError::Unrepresentable(
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
                    return Err(HirBuildError::Unrepresentable(
                        super::UnsupportedHirForm::SymbolMacrolet,
                    ));
                }
                self.lower_declared_sequence(scope, declarations, body)
            }
        }
    }

    fn lower_if(
        &mut self,
        scope: ScopeId,
        test: &Expr,
        then_branch: &Expr,
        else_branch: Option<&Expr>,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let test = self.lower_expr(test, scope)?;
        let then_branch = self.lower_expr(then_branch, scope)?;
        let else_branch = else_branch
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

    fn lower_multiple_value_call(
        &mut self,
        scope: ScopeId,
        function: &Expr,
        arguments: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let function = self.lower_expr(function, scope)?;
        let arguments = arguments
            .iter()
            .map(|argument| self.lower_expr(argument, scope))
            .collect::<Result<Vec<_>, _>>()?;
        self.node(
            scope,
            NodeKind::MultipleValues(MultipleValueOperation::Call {
                function,
                arguments,
            }),
        )
    }

    fn lower_multiple_value_prog1(
        &mut self,
        scope: ScopeId,
        first: &Expr,
        forms: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
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

    fn lower_type_assertion(
        &mut self,
        scope: ScopeId,
        type_specifier: &crate::types::TypeSpecifier,
        value: &Expr,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
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

    fn lower_eval_when(
        &mut self,
        scope: ScopeId,
        situations: &[EvalSituation],
        body: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let situations = situations.iter().copied().map(map_situation).collect();
        let form = self.lower_sequence(body, scope)?;
        self.node(scope, NodeKind::CompileTimeForm { situations, form })
    }

    fn lower_load_time_value(
        &mut self,
        scope: ScopeId,
        form: &Expr,
        read_only: bool,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let form = self.lower_expr(form, scope)?;
        self.node(scope, NodeKind::LoadTimeValue { form, read_only })
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

const fn map_situation(situation: EvalSituation) -> CompileTimeSituation {
    match situation {
        EvalSituation::CompileToplevel => CompileTimeSituation::CompileToplevel,
        EvalSituation::LoadToplevel => CompileTimeSituation::LoadToplevel,
        EvalSituation::Execute => CompileTimeSituation::Execute,
        EvalSituation::Eval => CompileTimeSituation::Eval,
    }
}
