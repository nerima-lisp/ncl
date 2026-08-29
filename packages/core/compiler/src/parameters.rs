#![allow(clippy::redundant_pub_crate)]

#[allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(super) fn parameters(form: &Form) -> Result<OrdinaryLambdaList, CompileError> {
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

    pub(super) fn compile_optional_parameters(
        &mut self,
        specifications: &[LambdaListOptionalParameter],
    ) -> Result<Vec<OptionalParameter>, CompileError> {
        let mut optional = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            optional.push(OptionalParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(optional)
    }

    pub(super) fn compile_auxiliary_parameters(
        &mut self,
        specifications: &[LambdaListAuxiliaryParameter],
    ) -> Result<Vec<AuxiliaryParameter>, CompileError> {
        let mut auxiliary = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            auxiliary.push(AuxiliaryParameter {
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
            });
        }
        Ok(auxiliary)
    }

    pub(super) fn compile_keyword_parameters(
        &mut self,
        specifications: &[LambdaListKeywordParameter],
    ) -> Result<Vec<KeywordParameter>, CompileError> {
        let mut keywords = Vec::with_capacity(specifications.len());
        for specification in specifications {
            let default_function = self.reserve_function(None, Vec::new());
            self.compile_expression(default_function, &specification.init_form)?;
            self.emit(
                default_function,
                Instruction::Return,
                specification.init_form.span,
            )?;
            keywords.push(KeywordParameter {
                keyword_name: specification.keyword_name.clone(),
                keyword_name_escaped: specification.keyword_name_escaped,
                name: specification.name.clone(),
                name_escaped: specification.name_escaped,
                default_function,
                supplied_p: specification.supplied_p.clone(),
                supplied_p_escaped: specification.supplied_p_escaped,
            });
        }
        Ok(keywords)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ncl_syntax::read;

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

    #[test]
    fn compiles_default_parameter_functions_into_separate_code() -> Result<(), String> {
        let form = read("(&optional (value 10) &key (limit 20) &aux (state 30))")
            .map_err(|error| error.to_string())?
            .remove(0);
        let lambda_list = CompileState::parameters(&form).map_err(|error| error.to_string())?;
        let mut state = CompileState::default();

        let optional = state
            .compile_optional_parameters(&lambda_list.optional)
            .map_err(|error| error.to_string())?;
        let keywords = state
            .compile_keyword_parameters(&lambda_list.keywords)
            .map_err(|error| error.to_string())?;
        let auxiliary = state
            .compile_auxiliary_parameters(&lambda_list.auxiliary)
            .map_err(|error| error.to_string())?;

        assert_eq!(optional.len(), 1);
        assert_eq!(keywords.len(), 1);
        assert_eq!(auxiliary.len(), 1);
        assert_eq!(state.functions.len(), 3);
        assert!(state.functions.iter().all(|function| matches!(
            function.instructions.as_slice(),
            [Instruction::Constant(_), Instruction::Return]
        )));
        Ok(())
    }
}
