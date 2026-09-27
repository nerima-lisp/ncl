//! Non-local return analysis for IR v2 control lowering.

use crate::ast::{Expr, FunctionDesignator, Operator, TagbodyItem};
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
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            mentions_return_from(test, name)
                || mentions_return_from(then, name)
                || otherwise
                    .as_deref()
                    .is_some_and(|form| mentions_return_from(form, name))
        }
        Expr::Block { body, .. } => body.iter().any(|form| mentions_return_from(form, name)),
        Expr::Tagbody(items) => items.iter().any(|item| match item {
            TagbodyItem::Tag(_) => false,
            TagbodyItem::Form(form) => mentions_return_from(form, name),
        }),
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
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Lambda(_)
        | Expr::Function(_)
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::Setq(_)
        | Expr::MultipleValueCall { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. }
        | Expr::Flet { .. }
        | Expr::Labels { .. } => false,
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
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            has_unwind_protect(test)
                || has_unwind_protect(then)
                || otherwise.as_deref().is_some_and(has_unwind_protect)
        }
        Expr::Block { body, .. } => body.iter().any(has_unwind_protect),
        Expr::Tagbody(items) => items.iter().any(|item| match item {
            TagbodyItem::Tag(_) => false,
            TagbodyItem::Form(form) => has_unwind_protect(form),
        }),
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
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Function(FunctionDesignator::Name(_))
        | Expr::ReturnFrom { .. }
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::Setq(_)
        | Expr::MultipleValueCall { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. }
        | Expr::Flet { .. }
        | Expr::Labels { .. } => false,
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
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            nested_return(test, name)
                || nested_return(then, name)
                || otherwise
                    .as_deref()
                    .is_some_and(|form| nested_return(form, name))
        }
        Expr::Block { body, .. } => body.iter().any(|form| nested_return(form, name)),
        Expr::Tagbody(items) => items.iter().any(|item| match item {
            TagbodyItem::Tag(_) => false,
            TagbodyItem::Form(form) => nested_return(form, name),
        }),
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Function(_)
        | Expr::ReturnFrom { .. }
        | Expr::Go { .. }
        | Expr::Catch { .. }
        | Expr::Throw { .. }
        | Expr::UnwindProtect { .. }
        | Expr::Progv { .. }
        | Expr::Setq(_)
        | Expr::MultipleValueCall { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. }
        | Expr::Flet { .. }
        | Expr::Labels { .. } => false,
    }
}

fn contains_return(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::ReturnFrom {
            name: target,
            value,
        } => {
            target == name
                || value
                    .as_deref()
                    .is_some_and(|form| contains_return(form, name))
        }
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            forms.iter().any(|form| contains_return(form, name))
        }
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(|form| contains_return(form, name))
                || body.iter().any(|form| contains_return(form, name))
        }
        Expr::UnwindProtect { protected, cleanup } => {
            contains_return(protected, name)
                || cleanup.iter().any(|form| contains_return(form, name))
        }
        Expr::Catch { tag, body } => {
            contains_return(tag, name) || body.iter().any(|form| contains_return(form, name))
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            contains_return(symbols, name)
                || contains_return(values, name)
                || body.iter().any(|form| contains_return(form, name))
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            contains_return(test, name)
                || contains_return(then, name)
                || otherwise
                    .as_deref()
                    .is_some_and(|form| contains_return(form, name))
        }
        Expr::Block { body, .. } => body.iter().any(|form| contains_return(form, name)),
        Expr::Tagbody(items) => items.iter().any(|item| match item {
            TagbodyItem::Tag(_) => false,
            TagbodyItem::Form(form) => contains_return(form, name),
        }),
        Expr::Call {
            operator,
            arguments,
        } => {
            let in_lambda = matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| contains_return(form, name)));
            in_lambda || arguments.iter().any(|form| contains_return(form, name))
        }
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Lambda(_)
        | Expr::Function(_)
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::Setq(_)
        | Expr::MultipleValueCall { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. }
        | Expr::Flet { .. }
        | Expr::Labels { .. } => false,
    }
}
