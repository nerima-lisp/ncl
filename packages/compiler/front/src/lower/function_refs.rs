//! Local function reference analysis for recursive binding groups.

use std::collections::BTreeSet;

use crate::ast::{Expr, FunctionDesignator, LambdaExpr, Operator, TagbodyItem};
use crate::symbols::SymbolRef;

pub(super) fn local_function_references(
    lambda: &LambdaExpr,
    local_names: &BTreeSet<SymbolRef>,
) -> BTreeSet<SymbolRef> {
    let mut references = BTreeSet::new();
    for form in &lambda.body {
        collect(form, local_names, &mut references);
    }
    references
}

#[allow(clippy::too_many_lines, reason = "exhaustive AST traversal")]
fn collect(expr: &Expr, local_names: &BTreeSet<SymbolRef>, references: &mut BTreeSet<SymbolRef>) {
    match expr {
        Expr::Constant(_) | Expr::Variable(_) | Expr::Go { .. } => {}
        Expr::Call {
            operator,
            arguments,
        } => {
            match operator {
                Operator::Name(name) if local_names.contains(name) => {
                    references.insert(name.clone());
                }
                Operator::Lambda(lambda) => {
                    for form in &lambda.body {
                        collect(form, local_names, references);
                    }
                }
                Operator::Name(_) => {}
            }
            for argument in arguments {
                collect(argument, local_names, references);
            }
        }
        Expr::Function(FunctionDesignator::Name(name)) => {
            if local_names.contains(name) {
                references.insert(name.clone());
            }
        }
        Expr::Function(FunctionDesignator::Lambda(lambda)) | Expr::Lambda(lambda) => {
            for form in &lambda.body {
                collect(form, local_names, references);
            }
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            collect(test, local_names, references);
            collect(then, local_names, references);
            if let Some(otherwise) = otherwise {
                collect(otherwise, local_names, references);
            }
        }
        Expr::Progn(forms)
        | Expr::EvalWhen { body: forms, .. }
        | Expr::Block { body: forms, .. }
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            for form in forms {
                collect(form, local_names, references);
            }
        }
        Expr::ReturnFrom { value, .. } => {
            if let Some(value) = value {
                collect(value, local_names, references);
            }
        }
        Expr::Tagbody(items) => {
            for item in items {
                if let TagbodyItem::Form(form) = item {
                    collect(form, local_names, references);
                }
            }
        }
        Expr::Catch { tag, body } => {
            collect(tag, local_names, references);
            for form in body {
                collect(form, local_names, references);
            }
        }
        Expr::Throw { tag, value } => {
            collect(tag, local_names, references);
            collect(value, local_names, references);
        }
        Expr::UnwindProtect { protected, cleanup } => {
            collect(protected, local_names, references);
            for form in cleanup {
                collect(form, local_names, references);
            }
        }
        Expr::Let { bindings, body, .. } => {
            for binding in bindings {
                if let Some(value) = &binding.value {
                    collect(value, local_names, references);
                }
            }
            for form in body {
                collect(form, local_names, references);
            }
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            collect(symbols, local_names, references);
            collect(values, local_names, references);
            for form in body {
                collect(form, local_names, references);
            }
        }
        Expr::Setq(pairs) => {
            for (_, value) in pairs {
                collect(value, local_names, references);
            }
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            collect(function, local_names, references);
            for argument in arguments {
                collect(argument, local_names, references);
            }
        }
        Expr::MultipleValueProg1 { first, forms } => {
            collect(first, local_names, references);
            for form in forms {
                collect(form, local_names, references);
            }
        }
        Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
            collect(value, local_names, references);
        }
        Expr::Flet {
            definitions, body, ..
        } => {
            for definition in definitions {
                for form in &definition.lambda.body {
                    collect(form, local_names, references);
                }
            }
            let visible_names = without_names(local_names, definitions);
            for form in body {
                collect(form, &visible_names, references);
            }
        }
        Expr::Labels {
            definitions, body, ..
        } => {
            let visible_names = without_names(local_names, definitions);
            for definition in definitions {
                for form in &definition.lambda.body {
                    collect(form, &visible_names, references);
                }
            }
            for form in body {
                collect(form, &visible_names, references);
            }
        }
    }
}

fn without_names(
    local_names: &BTreeSet<SymbolRef>,
    definitions: &[crate::ast::LocalFunction],
) -> BTreeSet<SymbolRef> {
    let nested_names = definitions
        .iter()
        .map(|definition| &definition.name)
        .collect::<BTreeSet<_>>();
    local_names
        .iter()
        .filter(|name| !nested_names.contains(name))
        .cloned()
        .collect()
}
