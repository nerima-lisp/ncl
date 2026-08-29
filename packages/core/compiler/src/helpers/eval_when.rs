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

    #[test]
    fn eval_when_situations_reject_a_non_atom_situation() {
        let span = Span::new(0, 1);
        let list = Form::list(vec![Form::list(Vec::new(), span)], span);
        let error = compile_eval_when_executes(&list).map_or_else(
            |error| error,
            |value| panic!("nested list situation, got {value:?}"),
        );
        assert!(matches!(
            error.kind,
            CompileErrorKind::ExpectedSymbol { context } if context == "EVAL-WHEN situation"
        ));
    }

    #[test]
    fn eval_when_situations_reject_an_unparsable_symbol_token() {
        let span = Span::new(0, 1);
        let list = Form::list(vec![Form::atom("", span)], span);
        let error = compile_eval_when_executes(&list).map_or_else(
            |error| error,
            |value| panic!("empty situation token, got {value:?}"),
        );
        assert!(matches!(
            error.kind,
            CompileErrorKind::ExpectedSymbol { context } if context == "EVAL-WHEN situation"
        ));
    }

    #[test]
    fn eval_when_situations_reject_uninterned_and_literal_symbols() {
        let span = Span::new(0, 1);
        for source in ["#:generated", "nil"] {
            let list = Form::list(vec![Form::atom(source, span)], span);
            let error = compile_eval_when_executes(&list).map_or_else(
                |error| error,
                |value| panic!("{source} should be rejected, got {value:?}"),
            );
            assert!(
                matches!(
                    error.kind,
                    CompileErrorKind::ExpectedSymbol { ref context } if context == "EVAL-WHEN situation"
                ),
                "source={source:?} error={error:?}"
            );
        }
    }
}
