//! Non-local return analysis for IR v2 control lowering.

use crate::ast::{Expr, FunctionDesignator, Operator};
use crate::symbols::SymbolRef;

pub(super) fn body_has_nested_return(forms: &[Expr], name: &SymbolRef) -> bool {
    forms.iter().any(|form| nested_return(form, name))
        || (forms.iter().any(|form| mentions_return_from(form, name))
            && forms.iter().any(has_unwind_protect))
}

/// Whether `expr` contains, at any depth, a `(return-from name ...)`.
///
/// Unlike [`nested_return`]/[`contains_return`] (which only look inside a
/// nested lambda, since that is the only case that forces a native
/// non-local exit when nothing else is involved), this also looks inside
/// ordinary control forms so it can be combined with [`has_unwind_protect`]:
/// a `return-from` that must unwind past an `unwind-protect`'s cleanup needs
/// the escaping (`catch`/`throw`-backed) lowering even when it never crosses
/// a closure boundary, because the fast, same-function jump used otherwise
/// does not run cleanup forms.
fn mentions_return_from(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::ReturnFrom { name: target, .. } => target == name,
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            forms.iter().any(|form| mentions_return_from(form, name))
        }
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(|form| mentions_return_from(form, name))
                || body.iter().any(|form| mentions_return_from(form, name))
        }
        Expr::UnwindProtect { protected, cleanup } => {
            mentions_return_from(protected, name)
                || cleanup.iter().any(|form| mentions_return_from(form, name))
        }
        Expr::Catch { tag, body } => {
            mentions_return_from(tag, name)
                || body.iter().any(|form| mentions_return_from(form, name))
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            mentions_return_from(symbols, name)
                || mentions_return_from(values, name)
                || body.iter().any(|form| mentions_return_from(form, name))
        }
        Expr::Call {
            operator,
            arguments,
        } => {
            let in_lambda = matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| mentions_return_from(form, name)));
            in_lambda
                || arguments
                    .iter()
                    .any(|form| mentions_return_from(form, name))
        }
        _ => false,
    }
}

/// Whether `expr` contains, at any depth (including inside nested lambdas),
/// an `unwind-protect`. A conservative over-approximation is fine here: the
/// only effect of a false positive is choosing the (always-correct)
/// escaping lowering when the simpler local-jump one would also have
/// worked.
fn has_unwind_protect(expr: &Expr) -> bool {
    match expr {
        Expr::UnwindProtect { .. } => true,
        Expr::Lambda(lambda) => lambda.body.iter().any(has_unwind_protect),
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => forms.iter().any(has_unwind_protect),
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(has_unwind_protect)
                || body.iter().any(has_unwind_protect)
        }
        Expr::Catch { tag, body } => has_unwind_protect(tag) || body.iter().any(has_unwind_protect),
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            has_unwind_protect(symbols)
                || has_unwind_protect(values)
                || body.iter().any(has_unwind_protect)
        }
        Expr::Call {
            operator,
            arguments,
        } => {
            let in_lambda = matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(has_unwind_protect));
            in_lambda || arguments.iter().any(has_unwind_protect)
        }
        Expr::Function(FunctionDesignator::Lambda(lambda)) => {
            lambda.body.iter().any(has_unwind_protect)
        }
        _ => false,
    }
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
