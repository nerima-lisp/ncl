mod model;

pub use model::{
    AuxiliaryParameter, CompileError, CompileErrorKind, Constant, DestructureAuxiliaryParameter,
    DestructureKeywordParameter, DestructureLambdaList, DestructureOptionalParameter,
    DestructurePattern, DestructureSpec, FunctionCode, FunctionId, HandlerBindClause,
    HandlerCaseClause, Instruction, KeywordParameter, OptionalParameter, Program,
    RestartBindClause, RestartCaseClause,
};

use std::collections::HashSet;

use ncl_syntax::{
    Form, FormKind, LambdaListAuxiliaryParameter, LambdaListErrorKind, LambdaListKeywordParameter,
    LambdaListOptionalParameter, OrdinaryLambdaList, Span, SymbolTokenKind,
    parse_ordinary_lambda_list, parse_symbol_token,
};

#[derive(Clone, Copy, Eq, PartialEq)]
enum DestructureLambdaListSection {
    Required,
    Optional,
    Rest,
    Keyword,
    Auxiliary,
}

/// Stateless compiler entry points for syntax forms.
#[derive(Clone, Copy, Debug, Default)]
pub struct Compiler;

impl Compiler {
    /// Compile a sequence of forms into an entry function.
    pub fn compile_forms(forms: &[Form]) -> Result<Program, CompileError> {
        let mut state = CompileState::default();
        state.collect_names(forms);
        let entry = state.reserve_function(None, Vec::new());
        state.compile_sequence(entry, forms)?;
        state.emit(entry, Instruction::Return, Span::new(0, 0))?;
        Ok(Program {
            functions: state.functions,
            entry,
        })
    }

    /// Compile one form as a complete program.
    pub fn compile_form(form: &Form) -> Result<Program, CompileError> {
        Self::compile_forms(std::slice::from_ref(form))
    }
}

#[derive(Default)]
struct CompileState {
    functions: Vec<FunctionCode>,
    local_function_scopes: Vec<HashSet<String>>,
    used_names: HashSet<String>,
    temporary_counter: usize,
}

mod bindings;
mod conditionals;
mod conditions;
mod control_flow;
mod definitions;
mod destructuring;
mod expressions;
mod iteration;
mod state;
mod streams;

fn normalize_name(name: &str) -> String {
    name.to_ascii_uppercase()
}

fn operator_span(items: &[Form], fallback: Span) -> Span {
    items.first().map_or(fallback, |form| form.span)
}

fn symbol_reference(atom: &str) -> Option<(String, bool)> {
    let token = parse_symbol_token(atom).ok()?;
    if token.kind != SymbolTokenKind::Symbol {
        return None;
    }
    if token.escaped {
        return token.package.is_none().then_some((token.name, true));
    }
    Some((normalize_name(atom), false))
}

fn special_operator_name(atom: &str) -> Option<String> {
    let token = parse_symbol_token(atom).ok()?;
    if token.kind == SymbolTokenKind::Symbol && token.package.is_none() && !token.escaped {
        Some(normalize_name(&token.name))
    } else {
        None
    }
}

fn case_default_clause(form: &Form) -> bool {
    let FormKind::Atom(atom) = &form.kind else {
        return false;
    };
    let Ok(token) = parse_symbol_token(atom) else {
        return false;
    };
    token.kind == SymbolTokenKind::Symbol
        && !token.escaped
        && (token.name.eq_ignore_ascii_case("T") || token.name.eq_ignore_ascii_case("OTHERWISE"))
}

fn compile_eval_when_executes(form: &Form) -> Result<bool, CompileError> {
    let FormKind::List(situations) = &form.kind else {
        return Err(CompileError::new(
            CompileErrorKind::ExpectedList {
                context: "EVAL-WHEN situations".to_string(),
            },
            form.span,
        ));
    };
    let mut executes = false;
    for situation in situations {
        let FormKind::Atom(name) = &situation.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        };
        let Ok(token) = parse_symbol_token(name) else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        };
        if token.kind == SymbolTokenKind::Uninterned
            || (token.kind == SymbolTokenKind::Symbol && literal_constant(name).is_some())
        {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedSymbol {
                    context: "EVAL-WHEN situation".to_string(),
                },
                situation.span,
            ));
        }
        if token.package.is_none() && token.name.eq_ignore_ascii_case("execute") {
            executes = true;
        }
    }
    Ok(executes)
}

fn literal_constant(atom: &str) -> Option<Constant> {
    let token = parse_symbol_token(atom).ok()?;
    match token.kind {
        SymbolTokenKind::Keyword => {
            if token.escaped {
                Some(Constant::KeywordExact(token.name))
            } else {
                Some(Constant::Keyword(normalize_name(&token.name)))
            }
        }
        SymbolTokenKind::Symbol if token.package.is_none() && !token.escaped => {
            if token.name.eq_ignore_ascii_case("nil") || token.name.eq_ignore_ascii_case("#f") {
                return Some(Constant::Nil);
            }
            if token.name.eq_ignore_ascii_case("t") || token.name.eq_ignore_ascii_case("#t") {
                return Some(Constant::Boolean(true));
            }
            if let Ok(value) = token.name.parse::<i64>() {
                return Some(Constant::Integer(value));
            }
            if let Some((numerator, denominator)) = rational_literal_parts(&token.name) {
                return if denominator == 1 {
                    Some(Constant::Integer(numerator))
                } else {
                    Some(Constant::Rational {
                        numerator,
                        denominator,
                    })
                };
            }
            token.name.parse::<f64>().ok().map(Constant::Float)
        }
        _ => None,
    }
}

fn rational_literal_parts(name: &str) -> Option<(i64, i64)> {
    let (numerator, denominator) = name.split_once('/')?;
    if numerator.is_empty()
        || denominator.is_empty()
        || numerator.contains('/')
        || denominator.contains('/')
    {
        return None;
    }
    let numerator = numerator.parse::<i128>().ok()?;
    let denominator = denominator.parse::<i128>().ok()?;
    if denominator == 0 {
        return None;
    }
    let (numerator, denominator) = if denominator < 0 {
        (numerator.checked_neg()?, denominator.checked_neg()?)
    } else {
        (numerator, denominator)
    };
    let numerator_abs = if numerator < 0 {
        numerator.checked_neg()? as u128
    } else {
        numerator as u128
    };
    let divisor = gcd(numerator_abs, denominator as u128);
    let numerator = i64::try_from(numerator / divisor as i128).ok()?;
    let denominator = i64::try_from(denominator / divisor as i128).ok()?;
    Some((numerator, denominator))
}

fn gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

fn tag_name(form: &Form) -> Option<String> {
    let FormKind::Atom(name) = &form.kind else {
        return None;
    };
    if name.is_empty() || name == ":" {
        return None;
    }
    if name.starts_with(':') {
        return (name.len() > 1).then(|| normalize_name(name));
    }
    if name.eq_ignore_ascii_case("nil")
        || name.eq_ignore_ascii_case("t")
        || name.parse::<i64>().is_ok()
        || literal_constant(name).is_none()
    {
        Some(normalize_name(name))
    } else {
        None
    }
}
