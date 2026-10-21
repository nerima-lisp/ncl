use crate::ast::{Expr, FunctionDesignator, Operator, TagbodyItem};
use crate::symbols::SymbolRef;

pub(super) fn contains_go(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::Go { tag } => tag == name,
        Expr::Lambda(lambda) | Expr::Function(FunctionDesignator::Lambda(lambda)) => {
            lambda.body.iter().any(|form| contains_go(form, name))
        }
        Expr::Call {
            operator,
            arguments,
        } => {
            matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| contains_go(form, name)))
                || arguments.iter().any(|form| contains_go(form, name))
        }
        Expr::Progn(forms)
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            forms.iter().any(|form| contains_go(form, name))
        }
        Expr::Let { bindings, body, .. } => {
            bindings
                .iter()
                .filter_map(|binding| binding.value.as_ref())
                .any(|form| contains_go(form, name))
                || body.iter().any(|form| contains_go(form, name))
        }
        Expr::Setq(bindings) => bindings.iter().any(|(_, form)| contains_go(form, name)),
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            contains_go(test, name)
                || contains_go(then, name)
                || otherwise
                    .as_deref()
                    .is_some_and(|form| contains_go(form, name))
        }
        Expr::Block { body, .. } => body.iter().any(|form| contains_go(form, name)),
        Expr::Tagbody(items) => {
            !items
                .iter()
                .any(|item| matches!(item, TagbodyItem::Tag(tag) if tag == name))
                && items.iter().any(|item| match item {
                    TagbodyItem::Tag(_) => false,
                    TagbodyItem::Form(form) => contains_go(form, name),
                })
        }
        Expr::Catch { tag, body } => {
            contains_go(tag, name) || body.iter().any(|form| contains_go(form, name))
        }
        Expr::UnwindProtect { protected, cleanup } => {
            contains_go(protected, name) || cleanup.iter().any(|form| contains_go(form, name))
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            contains_go(symbols, name)
                || contains_go(values, name)
                || body.iter().any(|form| contains_go(form, name))
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
                    .any(|form| contains_go(form, name))
            }) || body.iter().any(|form| contains_go(form, name))
        }
        Expr::ReturnFrom { value, .. } => {
            value.as_deref().is_some_and(|form| contains_go(form, name))
        }
        Expr::Throw { tag, value } => contains_go(tag, name) || contains_go(value, name),
        Expr::MultipleValueCall {
            function,
            arguments,
        } => contains_go(function, name) || arguments.iter().any(|form| contains_go(form, name)),
        Expr::MultipleValueProg1 { first, forms } => {
            contains_go(first, name) || forms.iter().any(|form| contains_go(form, name))
        }
        Expr::The { value, .. } => contains_go(value, name),
        Expr::EvalWhen { body, .. } => body.iter().any(|form| contains_go(form, name)),
        Expr::LoadTimeValue { form, .. } => contains_go(form, name),
        Expr::Constant(_) | Expr::Variable(_) | Expr::Function(FunctionDesignator::Name(_)) => {
            false
        }
    }
}
pub(super) fn nested_go(expr: &Expr, name: &SymbolRef) -> bool {
    match expr {
        Expr::Lambda(lambda) | Expr::Function(FunctionDesignator::Lambda(lambda)) => {
            lambda.body.iter().any(|form| contains_go(form, name))
        }
        Expr::Call {
            operator,
            arguments,
        } => {
            matches!(operator, Operator::Lambda(lambda) if lambda.body.iter().any(|form| contains_go(form, name)))
                || arguments.iter().any(|form| nested_go(form, name))
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
                    .any(|form| contains_go(form, name))
            }) || body.iter().any(|form| nested_go(form, name))
        }
        Expr::Progn(_)
        | Expr::Locally { .. }
        | Expr::Macrolet { .. }
        | Expr::SymbolMacrolet { .. }
        | Expr::Let { .. }
        | Expr::If { .. }
        | Expr::Block { .. }
        | Expr::Tagbody(_)
        | Expr::Catch { .. }
        | Expr::UnwindProtect { .. }
        | Expr::Progv { .. }
        | Expr::Constant(_)
        | Expr::Variable(_)
        | Expr::ReturnFrom { .. }
        | Expr::Go { .. }
        | Expr::Throw { .. }
        | Expr::MultipleValueCall { .. }
        | Expr::MultipleValueProg1 { .. }
        | Expr::The { .. }
        | Expr::EvalWhen { .. }
        | Expr::LoadTimeValue { .. }
        | Expr::Function(FunctionDesignator::Name(_)) => false,
        Expr::Setq(bindings) => bindings.iter().any(|(_, form)| nested_go(form, name)),
    }
}
