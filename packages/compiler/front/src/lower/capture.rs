//! Free-variable and assignment analysis, used to decide cell boxing.
//!
//! The frozen AST refers to a variable by name, not by binding identity, so this
//! analysis is name-based. It is exact for code without variable shadowing,
//! which is what the lowering tests exercise, and is documented as a known
//! limitation for shadowing-heavy input.

use std::collections::BTreeSet;

use crate::ast::{Expr, FunctionDesignator, LambdaExpr, Operator, TagbodyItem};
use crate::lambda_list::{LambdaList, ParamName};
use crate::symbols::SymbolRef;

/// The assignments and captures a body performs.
#[derive(Clone, Debug, Default)]
pub(super) struct Analysis {
    assigned: BTreeSet<SymbolRef>,
    captured: BTreeSet<SymbolRef>,
}

impl Analysis {
    /// Whether a binding of `name` must be boxed into a cell.
    ///
    /// A binding is boxed when it is assigned somewhere and captured by a
    /// lambda, because two closures must observe each other's writes through
    /// one shared cell.
    pub(super) fn needs_cell(&self, name: &SymbolRef) -> bool {
        self.assigned.contains(name) && self.captured.contains(name)
    }
}

/// Analyze a body for assignments and captures.
pub(super) fn analyze(forms: &[Expr]) -> Analysis {
    let mut analysis = Analysis::default();
    let mut bound = Vec::new();
    for form in forms {
        scan(form, &mut bound, &mut analysis);
    }
    analysis
}

/// Names free in a form, split by Common Lisp's two lexical namespaces.
#[derive(Clone, Debug, Default)]
pub(super) struct FreeNames {
    pub(super) variables: BTreeSet<SymbolRef>,
    pub(super) functions: BTreeSet<SymbolRef>,
}

/// The variables a lambda binds in its own body.
fn lambda_bindings(list: &LambdaList) -> Vec<SymbolRef> {
    let mut names = Vec::new();
    let mut add = |name: &ParamName| {
        if let ParamName::Symbol(symbol) = name {
            names.push(symbol.clone());
        }
    };
    for name in &list.required {
        add(name);
    }
    for optional in &list.optional {
        add(&optional.name);
        if let Some(supplied) = &optional.supplied_p {
            add(supplied);
        }
    }
    if let Some(rest) = &list.rest {
        add(rest);
    }
    for key in &list.keys {
        add(&key.name);
        if let Some(supplied) = &key.supplied_p {
            add(supplied);
        }
    }
    for aux in &list.aux {
        add(&aux.name);
    }
    names
}

/// The free variables of a lambda expression.
pub(super) fn free_names(lambda: &LambdaExpr) -> FreeNames {
    let mut bound_variables = lambda_bindings(&lambda.lambda_list);
    let bound_functions = Vec::new();
    let mut free = FreeNames::default();
    for form in &lambda.body {
        collect_free(form, &mut bound_variables, &bound_functions, &mut free);
    }
    free
}

fn collect_lambda_free(
    lambda: &LambdaExpr,
    bound_variables: &[SymbolRef],
    bound_functions: &[SymbolRef],
    free: &mut FreeNames,
) {
    for name in free_names(lambda).variables {
        if !bound_variables.contains(&name) {
            free.variables.insert(name);
        }
    }
    for name in free_names(lambda).functions {
        if !bound_functions.contains(&name) {
            free.functions.insert(name);
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per frozen AST variant keeps the walk auditable"
)]
fn collect_free(
    expr: &Expr,
    bound_variables: &mut Vec<SymbolRef>,
    bound_functions: &[SymbolRef],
    free: &mut FreeNames,
) {
    match expr {
        Expr::Constant(_) | Expr::Go { .. } => {}
        Expr::Variable(name) => {
            if !bound_variables.contains(name) {
                free.variables.insert(name.clone());
            }
        }
        Expr::Call {
            operator,
            arguments,
        } => {
            if let Operator::Lambda(lambda) = operator {
                collect_lambda_free(lambda, bound_variables, bound_functions, free);
            } else if let Operator::Name(name) = operator
                && !bound_functions.contains(name)
            {
                free.functions.insert(name.clone());
            }
            for argument in arguments {
                collect_free(argument, bound_variables, bound_functions, free);
            }
        }
        Expr::Function(designator) => match designator {
            FunctionDesignator::Name(name) if !bound_functions.contains(name) => {
                free.functions.insert(name.clone());
            }
            FunctionDesignator::Lambda(lambda) => {
                collect_lambda_free(lambda, bound_variables, bound_functions, free);
            }
            FunctionDesignator::Name(_) => {}
        },
        Expr::Lambda(lambda) => collect_lambda_free(lambda, bound_variables, bound_functions, free),
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            collect_free(test, bound_variables, bound_functions, free);
            collect_free(then, bound_variables, bound_functions, free);
            if let Some(otherwise) = otherwise {
                collect_free(otherwise, bound_variables, bound_functions, free);
            }
        }
        Expr::Progn(forms)
        | Expr::EvalWhen { body: forms, .. }
        | Expr::Block { body: forms, .. }
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            for form in forms {
                collect_free(form, bound_variables, bound_functions, free);
            }
        }
        Expr::ReturnFrom { value, .. } => {
            if let Some(value) = value {
                collect_free(value, bound_variables, bound_functions, free);
            }
        }
        Expr::Tagbody(items) => {
            for item in items {
                if let TagbodyItem::Form(form) = item {
                    collect_free(form, bound_variables, bound_functions, free);
                }
            }
        }
        Expr::Catch { tag, body } => {
            collect_free(tag, bound_variables, bound_functions, free);
            for form in body {
                collect_free(form, bound_variables, bound_functions, free);
            }
        }
        Expr::Throw { tag, value } => {
            collect_free(tag, bound_variables, bound_functions, free);
            collect_free(value, bound_variables, bound_functions, free);
        }
        Expr::UnwindProtect { protected, cleanup } => {
            collect_free(protected, bound_variables, bound_functions, free);
            for form in cleanup {
                collect_free(form, bound_variables, bound_functions, free);
            }
        }
        Expr::Let { bindings, body, .. } => {
            for binding in bindings {
                if let Some(value) = &binding.value {
                    collect_free(value, bound_variables, bound_functions, free);
                }
            }
            let mark = bound_variables.len();
            bound_variables.extend(bindings.iter().map(|binding| binding.name.clone()));
            for form in body {
                collect_free(form, bound_variables, bound_functions, free);
            }
            bound_variables.truncate(mark);
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            collect_free(symbols, bound_variables, bound_functions, free);
            collect_free(values, bound_variables, bound_functions, free);
            for form in body {
                collect_free(form, bound_variables, bound_functions, free);
            }
        }
        Expr::Setq(pairs) => {
            for (name, value) in pairs {
                if !bound_variables.contains(name) {
                    free.variables.insert(name.clone());
                }
                collect_free(value, bound_variables, bound_functions, free);
            }
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            collect_free(function, bound_variables, bound_functions, free);
            for argument in arguments {
                collect_free(argument, bound_variables, bound_functions, free);
            }
        }
        Expr::MultipleValueProg1 { first, forms } => {
            collect_free(first, bound_variables, bound_functions, free);
            for form in forms {
                collect_free(form, bound_variables, bound_functions, free);
            }
        }
        Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
            collect_free(value, bound_variables, bound_functions, free);
        }
        Expr::Flet {
            definitions, body, ..
        } => {
            for definition in definitions {
                collect_lambda_free(&definition.lambda, bound_variables, bound_functions, free);
            }
            let mut names = bound_functions.to_vec();
            names.extend(definitions.iter().map(|definition| definition.name.clone()));
            for form in body {
                collect_free(form, bound_variables, &names, free);
            }
        }
        Expr::Labels {
            definitions, body, ..
        } => {
            let mut names = bound_functions.to_vec();
            names.extend(definitions.iter().map(|definition| definition.name.clone()));
            for definition in definitions {
                collect_lambda_free(&definition.lambda, bound_variables, &names, free);
            }
            for form in body {
                collect_free(form, bound_variables, &names, free);
            }
        }
    }
}

fn scan_lambda(lambda: &LambdaExpr, bound: &mut Vec<SymbolRef>, analysis: &mut Analysis) {
    for name in free_names(lambda).variables {
        if !bound.contains(&name) {
            analysis.captured.insert(name);
        }
    }
    let mark = bound.len();
    bound.extend(lambda_bindings(&lambda.lambda_list));
    for form in &lambda.body {
        scan(form, bound, analysis);
    }
    bound.truncate(mark);
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per frozen AST variant keeps the walk auditable"
)]
fn scan(expr: &Expr, bound: &mut Vec<SymbolRef>, analysis: &mut Analysis) {
    match expr {
        Expr::Constant(_) | Expr::Go { .. } | Expr::Variable(_) => {}
        Expr::Setq(pairs) => {
            for (name, value) in pairs {
                analysis.assigned.insert(name.clone());
                scan(value, bound, analysis);
            }
        }
        Expr::Lambda(lambda) => scan_lambda(lambda, bound, analysis),
        Expr::Call {
            operator,
            arguments,
        } => {
            if let Operator::Lambda(lambda) = operator {
                scan_lambda(lambda, bound, analysis);
            }
            for argument in arguments {
                scan(argument, bound, analysis);
            }
        }
        Expr::Function(designator) => {
            if let FunctionDesignator::Lambda(lambda) = designator {
                scan_lambda(lambda, bound, analysis);
            }
        }
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            scan(test, bound, analysis);
            scan(then, bound, analysis);
            if let Some(otherwise) = otherwise {
                scan(otherwise, bound, analysis);
            }
        }
        Expr::Progn(forms)
        | Expr::EvalWhen { body: forms, .. }
        | Expr::Block { body: forms, .. }
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            for form in forms {
                scan(form, bound, analysis);
            }
        }
        Expr::ReturnFrom { value, .. } => {
            if let Some(value) = value {
                scan(value, bound, analysis);
            }
        }
        Expr::Tagbody(items) => {
            for item in items {
                if let TagbodyItem::Form(form) = item {
                    scan(form, bound, analysis);
                }
            }
        }
        Expr::Catch { tag, body } => {
            scan(tag, bound, analysis);
            for form in body {
                scan(form, bound, analysis);
            }
        }
        Expr::Throw { tag, value } => {
            scan(tag, bound, analysis);
            scan(value, bound, analysis);
        }
        Expr::UnwindProtect { protected, cleanup } => {
            scan(protected, bound, analysis);
            for form in cleanup {
                scan(form, bound, analysis);
            }
        }
        Expr::Let { bindings, body, .. } => {
            for binding in bindings {
                if let Some(value) = &binding.value {
                    scan(value, bound, analysis);
                }
            }
            let mark = bound.len();
            bound.extend(bindings.iter().map(|binding| binding.name.clone()));
            for form in body {
                scan(form, bound, analysis);
            }
            bound.truncate(mark);
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            scan(symbols, bound, analysis);
            scan(values, bound, analysis);
            for form in body {
                scan(form, bound, analysis);
            }
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            scan(function, bound, analysis);
            for argument in arguments {
                scan(argument, bound, analysis);
            }
        }
        Expr::MultipleValueProg1 { first, forms } => {
            scan(first, bound, analysis);
            for form in forms {
                scan(form, bound, analysis);
            }
        }
        Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
            scan(value, bound, analysis);
        }
        Expr::Flet {
            definitions, body, ..
        }
        | Expr::Labels {
            definitions, body, ..
        } => {
            for definition in definitions {
                scan(
                    &Expr::Lambda(Box::new(definition.lambda.clone())),
                    bound,
                    analysis,
                );
            }
            for form in body {
                scan(form, bound, analysis);
            }
        }
    }
}
