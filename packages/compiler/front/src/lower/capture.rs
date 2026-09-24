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
pub(super) fn free_variables(lambda: &LambdaExpr) -> BTreeSet<SymbolRef> {
    let mut bound = lambda_bindings(&lambda.lambda_list);
    let mut free = BTreeSet::new();
    for form in &lambda.body {
        collect_free(form, &mut bound, &mut free);
    }
    free
}

fn collect_lambda_free(lambda: &LambdaExpr, bound: &[SymbolRef], free: &mut BTreeSet<SymbolRef>) {
    for name in free_variables(lambda) {
        if !bound.contains(&name) {
            free.insert(name);
        }
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one arm per frozen AST variant keeps the walk auditable"
)]
fn collect_free(expr: &Expr, bound: &mut Vec<SymbolRef>, free: &mut BTreeSet<SymbolRef>) {
    match expr {
        Expr::Constant(_) | Expr::Go { .. } => {}
        Expr::Variable(name) => {
            if !bound.contains(name) {
                free.insert(name.clone());
            }
        }
        Expr::Call {
            operator,
            arguments,
        } => {
            if let Operator::Lambda(lambda) = operator {
                collect_lambda_free(lambda, bound, free);
            }
            for argument in arguments {
                collect_free(argument, bound, free);
            }
        }
        Expr::Function(designator) => {
            if let FunctionDesignator::Lambda(lambda) = designator {
                collect_lambda_free(lambda, bound, free);
            }
        }
        Expr::Lambda(lambda) => collect_lambda_free(lambda, bound, free),
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            collect_free(test, bound, free);
            collect_free(then, bound, free);
            if let Some(otherwise) = otherwise {
                collect_free(otherwise, bound, free);
            }
        }
        Expr::Progn(forms)
        | Expr::EvalWhen { body: forms, .. }
        | Expr::Block { body: forms, .. }
        | Expr::Locally { body: forms, .. }
        | Expr::Macrolet { body: forms, .. }
        | Expr::SymbolMacrolet { body: forms, .. } => {
            for form in forms {
                collect_free(form, bound, free);
            }
        }
        Expr::ReturnFrom { value, .. } => {
            if let Some(value) = value {
                collect_free(value, bound, free);
            }
        }
        Expr::Tagbody(items) => {
            for item in items {
                if let TagbodyItem::Form(form) = item {
                    collect_free(form, bound, free);
                }
            }
        }
        Expr::Catch { tag, body } => {
            collect_free(tag, bound, free);
            for form in body {
                collect_free(form, bound, free);
            }
        }
        Expr::Throw { tag, value } => {
            collect_free(tag, bound, free);
            collect_free(value, bound, free);
        }
        Expr::UnwindProtect { protected, cleanup } => {
            collect_free(protected, bound, free);
            for form in cleanup {
                collect_free(form, bound, free);
            }
        }
        Expr::Let { bindings, body, .. } => {
            for binding in bindings {
                if let Some(value) = &binding.value {
                    collect_free(value, bound, free);
                }
            }
            let mark = bound.len();
            bound.extend(bindings.iter().map(|binding| binding.name.clone()));
            for form in body {
                collect_free(form, bound, free);
            }
            bound.truncate(mark);
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            collect_free(symbols, bound, free);
            collect_free(values, bound, free);
            for form in body {
                collect_free(form, bound, free);
            }
        }
        Expr::Setq(pairs) => {
            for (name, value) in pairs {
                if !bound.contains(name) {
                    free.insert(name.clone());
                }
                collect_free(value, bound, free);
            }
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            collect_free(function, bound, free);
            for argument in arguments {
                collect_free(argument, bound, free);
            }
        }
        Expr::MultipleValueProg1 { first, forms } => {
            collect_free(first, bound, free);
            for form in forms {
                collect_free(form, bound, free);
            }
        }
        Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
            collect_free(value, bound, free);
        }
        Expr::Flet {
            definitions, body, ..
        }
        | Expr::Labels {
            definitions, body, ..
        } => {
            for definition in definitions {
                collect_lambda_free(&definition.lambda, bound, free);
            }
            let mark = bound.len();
            bound.extend(definitions.iter().map(|definition| definition.name.clone()));
            for form in body {
                collect_free(form, bound, free);
            }
            bound.truncate(mark);
        }
    }
}

fn scan_lambda(lambda: &LambdaExpr, bound: &mut Vec<SymbolRef>, analysis: &mut Analysis) {
    for name in free_variables(lambda) {
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
            let mark = bound.len();
            bound.extend(definitions.iter().map(|definition| definition.name.clone()));
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
            bound.truncate(mark);
        }
    }
}
