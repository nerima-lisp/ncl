use crate::{CompileError, CompileErrorKind, Form, FormKind, SymbolTokenKind, parse_symbol_token};

use super::literals::literal_constant;

pub fn compile_eval_when_executes(form: &Form) -> Result<bool, CompileError> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    #[test]
    fn eval_when_situations_reject_non_symbol_forms() {
        let span = Span::new(0, 1);
        let list = Form::list(vec![Form::atom("execute", span)], span);
        assert_eq!(compile_eval_when_executes(&list), Ok(true));
        assert!(compile_eval_when_executes(&Form::atom("execute", span)).is_err());
    }
}
