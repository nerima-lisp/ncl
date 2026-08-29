#[allow(clippy::wildcard_imports)]
use super::*;

mod parse;

impl CompileState {
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

    fn expect_catch_arity(error: &CompileError) {
        match &error.kind {
            CompileErrorKind::Arity { operator, .. } => assert_eq!(operator, "CATCH"),
            other => panic!("expected the nested CATCH arity error to propagate, got {other:?}"),
        }
    }

    #[test]
    fn compile_optional_parameters_propagates_malformed_default_value() -> Result<(), String> {
        let form = read("(&optional (value (catch)))")
            .map_err(|error| error.to_string())?
            .remove(0);
        let lambda_list = CompileState::parameters(&form).map_err(|error| error.to_string())?;
        let mut state = CompileState::default();

        let error = state
            .compile_optional_parameters(&lambda_list.optional)
            .map_or_else(|error| error, |value| panic!("a malformed &optional default value must propagate its own compile error, got {value:?}"));
        expect_catch_arity(&error);
        Ok(())
    }

    #[test]
    fn compile_keyword_parameters_propagates_malformed_default_value() -> Result<(), String> {
        let form = read("(&key (value (catch)))")
            .map_err(|error| error.to_string())?
            .remove(0);
        let lambda_list = CompileState::parameters(&form).map_err(|error| error.to_string())?;
        let mut state = CompileState::default();

        let error = state
            .compile_keyword_parameters(&lambda_list.keywords)
            .map_or_else(|error| error, |value| panic!("a malformed &key default value must propagate its own compile error, got {value:?}"));
        expect_catch_arity(&error);
        Ok(())
    }

    #[test]
    fn compile_auxiliary_parameters_propagates_malformed_default_value() -> Result<(), String> {
        let form = read("(&aux (value (catch)))")
            .map_err(|error| error.to_string())?
            .remove(0);
        let lambda_list = CompileState::parameters(&form).map_err(|error| error.to_string())?;
        let mut state = CompileState::default();

        let error = state
            .compile_auxiliary_parameters(&lambda_list.auxiliary)
            .map_or_else(|error| error, |value| panic!("a malformed &aux default value must propagate its own compile error, got {value:?}"));
        expect_catch_arity(&error);
        Ok(())
    }
}
