use crate::ast::{Expr, FunctionDesignator, Operator, TagbodyItem};
use crate::symbols::SymbolRef;

use super::go::contains_go;

pub(super) fn go_exits_unwind_protect(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::UnwindProtect { protected, cleanup } => {
            contains_go(protected, name) || cleanup.iter().any(|form| contains_go(form, name))
        }
        Expr::Lambda(lambda) | Expr::Function(FunctionDesignator::Lambda(lambda)) => {
            go_exits_in_forms(&lambda.body, name)
        }
        Expr::Call {
            operator,
            arguments,
        } => {
            matches!(operator, Operator::Lambda(lambda) if go_exits_in_forms(&lambda.body, name))
                || go_exits_in_forms(arguments, name)
        }
        Expr::Flet {
            definitions, body, ..
        }
        | Expr::Labels {
            definitions, body, ..
        } => {
            definitions
                .iter()
                .any(|definition| go_exits_in_forms(&definition.lambda.body, name))
                || go_exits_in_forms(body, name)
        }
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => go_exits_in_forms(forms, name),
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(|form| go_exits_unwind_protect(form, name))
                || go_exits_in_forms(body, name)
        }
        Expr::Setq(bindings) => bindings
            .iter()
            .any(|(_, form)| go_exits_unwind_protect(form, name)),
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            go_exits_unwind_protect(test, name)
                || go_exits_unwind_protect(then, name)
                || otherwise
                    .as_deref()
                    .is_some_and(|form| go_exits_unwind_protect(form, name))
        }
        Expr::Block { body, .. } | Expr::EvalWhen { body, .. } => go_exits_in_forms(body, name),
        Expr::Tagbody(items) => {
            !items
                .iter()
                .any(|item| matches!(item, TagbodyItem::Tag(tag) if tag == name))
                && items.iter().any(|item| match item {
                    TagbodyItem::Tag(_) => false,
                    TagbodyItem::Form(form) => go_exits_unwind_protect(form, name),
                })
        }
        Expr::Catch { tag, body } => {
            go_exits_unwind_protect(tag, name) || go_exits_in_forms(body, name)
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            go_exits_unwind_protect(symbols, name)
                || go_exits_unwind_protect(values, name)
                || go_exits_in_forms(body, name)
        }
        Expr::ReturnFrom { value, .. } => value
            .as_deref()
            .is_some_and(|form| go_exits_unwind_protect(form, name)),
        Expr::Throw { tag, value } => {
            go_exits_unwind_protect(tag, name) || go_exits_unwind_protect(value, name)
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => go_exits_unwind_protect(function, name) || go_exits_in_forms(arguments, name),
        Expr::MultipleValueProg1 { first, forms } => {
            go_exits_unwind_protect(first, name) || go_exits_in_forms(forms, name)
        }
        Expr::The { value, .. } => go_exits_unwind_protect(value, name),
        Expr::LoadTimeValue { form, .. } => go_exits_unwind_protect(form, name),
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Go { .. }
        | Expr::Function(FunctionDesignator::Name(_)) => false,
    }
}

fn go_exits_in_forms(forms: &[Expr], name: &SymbolRef) -> bool {
    forms.iter().any(|form| go_exits_unwind_protect(form, name))
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
pub(super) fn mentions_return_from(expr: &Expr, name: &SymbolRef) -> bool {
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
        Expr::Setq(bindings) => bindings
            .iter()
            .any(|(_, form)| mentions_return_from(form, name)),
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
        Expr::Flet {
            definitions, body, ..
        }
        | Expr::Labels {
            definitions, body, ..
        } => {
            definitions.iter().any(|definition| {
                definition
                    .lambda
                    .body
                    .iter()
                    .any(|form| mentions_return_from(form, name))
            }) || body.iter().any(|form| mentions_return_from(form, name))
        }
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Lambda(_)
        | Expr::Function(_)
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::MultipleValueCall { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. } => false,
    }
}
/// Whether `expr` contains, at any depth (including inside nested lambdas),
/// an `unwind-protect`. A conservative over-approximation is fine here: the
/// only effect of a false positive is choosing the (always-correct)
/// escaping lowering when the simpler local-jump one would also have
/// worked.
pub(super) fn has_unwind_protect(expr: &Expr) -> bool {
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
        Expr::Setq(bindings) => bindings.iter().any(|(_, form)| has_unwind_protect(form)),
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
        Expr::Flet {
            definitions, body, ..
        }
        | Expr::Labels {
            definitions, body, ..
        } => {
            definitions
                .iter()
                .any(|definition| definition.lambda.body.iter().any(has_unwind_protect))
                || body.iter().any(has_unwind_protect)
        }
        Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::Function(FunctionDesignator::Name(_))
        | Expr::ReturnFrom { .. }
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::MultipleValueCall { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. } => false,
    }
}
