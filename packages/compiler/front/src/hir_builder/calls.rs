use ncl_hir::{CallTarget, MultipleValueOperation, NodeKind, ScopeId};

use crate::ast::{Expr, FunctionDesignator, Operator};

use super::HirBuildError;
use super::context::BuildContext;

impl BuildContext<'_> {
    pub(super) fn lower_call(
        &mut self,
        scope: ScopeId,
        operator: &Operator,
        arguments: &[Expr],
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let (target, arguments) = match operator {
            Operator::Name(name) => {
                let target = self.resolve_function_target(scope, name)?;
                if name.is_common_lisp("VALUES") && matches!(target, CallTarget::Global(_)) {
                    let values = arguments
                        .iter()
                        .map(|argument| self.lower_expr(argument, scope))
                        .collect::<Result<Vec<_>, _>>()?;
                    return self.node(
                        scope,
                        NodeKind::MultipleValues(MultipleValueOperation::Produce { values }),
                    );
                }
                if name.is_common_lisp("FUNCALL") && matches!(target, CallTarget::Global(_)) {
                    let Some((function, arguments)) = arguments.split_first() else {
                        return Err(HirBuildError::Unsupported(
                            super::UnsupportedHirForm::MalformedCall,
                        ));
                    };
                    let function = self.lower_expr(function, scope)?;
                    let arguments = arguments
                        .iter()
                        .map(|argument| self.lower_expr(argument, scope))
                        .collect::<Result<Vec<_>, _>>()?;
                    (CallTarget::Value(function), arguments)
                } else {
                    let arguments = arguments
                        .iter()
                        .map(|argument| self.lower_expr(argument, scope))
                        .collect::<Result<Vec<_>, _>>()?;
                    (target, arguments)
                }
            }
            Operator::Lambda(lambda) => {
                let function = self.lower_lambda(scope, lambda)?;
                let arguments = arguments
                    .iter()
                    .map(|argument| self.lower_expr(argument, scope))
                    .collect::<Result<Vec<_>, _>>()?;
                (CallTarget::Value(function), arguments)
            }
        };
        let node = self.node(scope, NodeKind::Call { target, arguments })?;
        self.record_function_use(target, node)?;
        Ok(node)
    }

    pub(super) fn lower_function_designator(
        &mut self,
        scope: ScopeId,
        designator: &FunctionDesignator,
    ) -> Result<ncl_hir::NodeId, HirBuildError> {
        let target = match designator {
            FunctionDesignator::Name(name) => self.resolve_function_target(scope, name)?,
            FunctionDesignator::Lambda(lambda) => {
                let function = self.lower_lambda(scope, lambda)?;
                CallTarget::Value(function)
            }
        };
        let node = self.node(scope, NodeKind::FunctionReference { target })?;
        self.record_function_use(target, node)?;
        Ok(node)
    }

    pub(super) fn record_function_use(
        &mut self,
        target: CallTarget,
        node: ncl_hir::NodeId,
    ) -> Result<(), HirBuildError> {
        if let CallTarget::Local(binding) = target {
            self.form.bindings_mut().record_use(
                binding,
                node,
                ncl_hir::binding::BindingUseKind::Read,
            )?;
        }
        Ok(())
    }
}
