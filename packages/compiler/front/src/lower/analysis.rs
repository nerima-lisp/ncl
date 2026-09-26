//! Non-local return analysis for IR v2 control lowering.

use crate::ast::{Expr, FunctionDesignator, Operator};
use crate::symbols::SymbolRef;

pub(super) fn body_has_nested_return(forms: &[Expr], name: &SymbolRef) -> bool {
    forms.iter().any(|form| nested_return(form, name))
}

fn nested_return(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::Lambda(lambda) => lambda.body.iter().any(|form| contains_return(form, name)),
        Expr::Call {
            operator,
            arguments,
        } => {
            matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| contains_return(form, name)))
                || arguments.iter().any(|form| nested_return(form, name))
        }
        Expr::Function(FunctionDesignator::Lambda(lambda)) => {
            lambda.body.iter().any(|form| contains_return(form, name))
        }
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(|form| nested_return(form, name))
                || body.iter().any(|form| nested_return(form, name))
        }
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            forms.iter().any(|form| nested_return(form, name))
        }
        _ => false,
    }
}

fn contains_return(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::ReturnFrom { name: target, .. } => target == name,
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            forms.iter().any(|form| contains_return(form, name))
        }
        Expr::Call { arguments, .. } => arguments.iter().any(|form| contains_return(form, name)),
        _ => false,
    }
}
