use crate::{
    CompileError, CompileErrorKind, CompileState, Form, LambdaListErrorKind, OrdinaryLambdaList,
    parse_ordinary_lambda_list,
};

impl CompileState {
    pub(crate) fn parameters(form: &Form) -> Result<OrdinaryLambdaList, CompileError> {
        parse_ordinary_lambda_list(form).map_err(|error| {
            let span = error.span;
            let kind = match error.kind {
                LambdaListErrorKind::ExpectedList => CompileErrorKind::ExpectedList {
                    context: "parameters".to_string(),
                },
                LambdaListErrorKind::ExpectedSymbol { context } => {
                    CompileErrorKind::ExpectedSymbol {
                        context: context.to_string(),
                    }
                }
                LambdaListErrorKind::InvalidForm { message } => {
                    CompileErrorKind::InvalidForm { message }
                }
            };
            CompileError::new(kind, span)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Span;

    #[test]
    fn maps_lambda_list_errors_to_typed_compile_errors() {
        let span = Span::new(3, 8);
        let cases = [
            (
                "non-list parameters",
                Form::atom("value", span),
                CompileErrorKind::ExpectedList {
                    context: "parameters".to_string(),
                },
            ),
            (
                "non-symbol parameter",
                Form::list(vec![Form::list(Vec::new(), span)], span),
                CompileErrorKind::ExpectedSymbol {
                    context: "parameter".to_string(),
                },
            ),
            (
                "invalid lambda-list marker",
                Form::list(vec![Form::atom("&mystery", span)], span),
                CompileErrorKind::InvalidForm {
                    message: "unsupported lambda-list marker &MYSTERY".to_string(),
                },
            ),
        ];

        for (name, form, expected_kind) in cases {
            let Err(error) = CompileState::parameters(&form) else {
                panic!("{name} should be rejected");
            };
            assert_eq!(error.kind, expected_kind, "{name}");
            assert_eq!(error.span, span, "{name}");
        }
    }
}
