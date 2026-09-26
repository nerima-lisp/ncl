use std::path::Path;

use crate::{Runtime, RuntimeError};
use ncl_compiler_front::{Declaration, Expr, FunctionDesignator, LambdaExpr, Operator, Quality};
use ncl_object::Word;
use ncl_object::{Runtime as ObjectRuntime, ThreadContext};
use ncl_reader::{ReadOptions, StringSource, read};

/// The optimization policy selected for one expanded form.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OptimizationLevel {
    /// Prefer the existing safe optimization pipeline for execution speed.
    Speed,
    /// Keep the default safety-first pipeline.
    Safety,
    /// Preserve the unoptimized IR for better debugging behavior.
    Debug,
}

#[derive(Clone, Copy, Debug)]
struct OptimizationProfile {
    speed: u8,
    safety: u8,
    debug: u8,
    unsupported: bool,
}

impl Default for OptimizationProfile {
    fn default() -> Self {
        Self {
            speed: 1,
            safety: 3,
            debug: 1,
            unsupported: false,
        }
    }
}

impl OptimizationProfile {
    fn apply(&mut self, declarations: &[Declaration]) {
        for declaration in declarations {
            let Declaration::Optimize(qualities) = declaration else {
                continue;
            };
            for quality in qualities {
                match quality.quality {
                    Quality::Speed => self.speed = quality.value,
                    Quality::Safety => self.safety = quality.value,
                    Quality::Debug => self.debug = quality.value,
                    Quality::Space | Quality::CompilationSpeed | Quality::Unknown => {
                        self.unsupported = true;
                    }
                    _ => self.unsupported = true,
                }
            }
        }
    }

    fn level(self) -> OptimizationLevel {
        if self.unsupported {
            return OptimizationLevel::Safety;
        }
        if self.debug >= self.speed && self.debug >= self.safety && self.debug > 1 {
            return OptimizationLevel::Debug;
        }
        if self.speed > self.safety {
            return OptimizationLevel::Speed;
        }
        OptimizationLevel::Safety
    }
}

/// Resolve optimize declarations in an expanded form without making unknown
/// or unsupported declarations increase optimization beyond the safe default.
pub(crate) fn optimization_level(expr: &Expr) -> OptimizationLevel {
    let mut profile = OptimizationProfile::default();
    visit_expr(expr, &mut profile);
    profile.level()
}

fn visit_declarations(declarations: &[Declaration], profile: &mut OptimizationProfile) {
    profile.apply(declarations);
}

fn visit_lambda(lambda: &LambdaExpr, profile: &mut OptimizationProfile) {
    visit_declarations(&lambda.declarations, profile);
    for form in &lambda.body {
        visit_expr(form, profile);
    }
}

fn visit_operator(operator: &Operator, profile: &mut OptimizationProfile) {
    if let Operator::Lambda(lambda) = operator {
        visit_lambda(lambda, profile);
    }
}

fn visit_function(function: &FunctionDesignator, profile: &mut OptimizationProfile) {
    if let FunctionDesignator::Lambda(lambda) = function {
        visit_lambda(lambda, profile);
    }
}

fn visit_expr(expr: &Expr, profile: &mut OptimizationProfile) {
    match expr {
        Expr::Constant(_) | Expr::Variable(_) | Expr::Go { .. } => {}
        Expr::Call {
            operator,
            arguments,
        } => {
            visit_operator(operator, profile);
            for argument in arguments {
                visit_expr(argument, profile);
            }
        }
        Expr::Function(function) => visit_function(function, profile),
        Expr::Lambda(lambda) => visit_lambda(lambda, profile),
        Expr::If {
            test,
            then,
            otherwise,
        } => {
            visit_expr(test, profile);
            visit_expr(then, profile);
            if let Some(otherwise) = otherwise {
                visit_expr(otherwise, profile);
            }
        }
        Expr::Progn(forms)
        | Expr::Block { body: forms, .. }
        | Expr::EvalWhen { body: forms, .. }
        | Expr::Locally { body: forms, .. } => {
            if let Expr::Locally { declarations, .. } = expr {
                visit_declarations(declarations, profile);
            }
            for form in forms {
                visit_expr(form, profile);
            }
        }
        Expr::ReturnFrom { value, .. } => {
            if let Some(value) = value {
                visit_expr(value, profile);
            }
        }
        Expr::Tagbody(items) => {
            for item in items {
                if let ncl_compiler_front::TagbodyItem::Form(form) = item {
                    visit_expr(form, profile);
                }
            }
        }
        Expr::Catch { tag, body } => {
            visit_expr(tag, profile);
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Throw { tag, value } => {
            visit_expr(tag, profile);
            visit_expr(value, profile);
        }
        Expr::UnwindProtect { protected, cleanup } => {
            visit_expr(protected, profile);
            for form in cleanup {
                visit_expr(form, profile);
            }
        }
        Expr::Let {
            bindings,
            declarations,
            body,
            ..
        } => {
            visit_declarations(declarations, profile);
            for binding in bindings {
                if let Some(value) = &binding.value {
                    visit_expr(value, profile);
                }
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Progv {
            symbols,
            values,
            body,
        } => {
            visit_expr(symbols, profile);
            visit_expr(values, profile);
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Setq(assignments) => {
            for (_, value) in assignments {
                visit_expr(value, profile);
            }
        }
        Expr::MultipleValueCall {
            function,
            arguments,
        } => {
            visit_expr(function, profile);
            for argument in arguments {
                visit_expr(argument, profile);
            }
        }
        Expr::MultipleValueProg1 { first, forms } => {
            visit_expr(first, profile);
            for form in forms {
                visit_expr(form, profile);
            }
        }
        Expr::The { value, .. } | Expr::LoadTimeValue { form: value, .. } => {
            visit_expr(value, profile);
        }
        Expr::Flet {
            definitions,
            declarations,
            body,
        }
        | Expr::Labels {
            definitions,
            declarations,
            body,
        } => {
            visit_declarations(declarations, profile);
            for definition in definitions {
                visit_lambda(&definition.lambda, profile);
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::Macrolet {
            definitions,
            declarations,
            body,
        } => {
            visit_declarations(declarations, profile);
            for definition in definitions {
                visit_declarations(&definition.declarations, profile);
                for form in &definition.body {
                    visit_expr(form, profile);
                }
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        Expr::SymbolMacrolet {
            declarations,
            body,
            definitions,
        } => {
            visit_declarations(declarations, profile);
            for definition in definitions {
                visit_expr(&definition.expansion, profile);
            }
            for form in body {
                visit_expr(form, profile);
            }
        }
        _ => {}
    }
}

pub fn source(runtime: &mut Runtime, source: &str) -> Result<Word, RuntimeError> {
    runtime.eval(source)
}

pub fn file(runtime: &mut Runtime, path: &Path) -> Result<Word, RuntimeError> {
    let contents = std::fs::read_to_string(path).map_err(|error| RuntimeError::Io {
        path: path.display().to_string(),
        error,
    })?;
    source(runtime, &contents)
}

pub fn read_forms(
    context: &mut ThreadContext,
    object: &ObjectRuntime,
    source: &str,
) -> Result<Vec<Word>, RuntimeError> {
    let options = ReadOptions::standard(context, object)?;
    let mut input = StringSource::new(source);
    let mut forms = Vec::new();
    while let Some(form) = read(context, object, &mut input, &options)? {
        forms.push(form);
    }
    Ok(forms)
}
