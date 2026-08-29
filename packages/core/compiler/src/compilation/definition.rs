#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_lambda(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 3 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "lambda needs parameters and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let parameter_form = &items[1];
        let lambda_list = Self::parameters(parameter_form)?;
        let child = self.reserve_function_with_rest(
            None,
            lambda_list.required.clone(),
            lambda_list.required_escaped.clone(),
            lambda_list.rest.clone(),
            lambda_list.rest_escaped,
        );
        let optional = self.compile_optional_parameters(&lambda_list.optional)?;
        self.functions[child].optional = optional;
        let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
        self.functions[child].keywords = keywords;
        self.functions[child].has_keyword_section = lambda_list.has_keyword_section;
        self.functions[child].allow_other_keys = lambda_list.allow_other_keys;
        let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
        self.functions[child].auxiliary = auxiliary;
        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;
        self.emit(function, Instruction::MakeClosure(child), span)?;
        Ok(())
    }

    pub(crate) fn compile_function(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "FUNCTION", "one", 1, span)?;
        let argument = &items[1];
        if matches!(argument.kind, FormKind::Atom(_)) {
            let (name, escaped) = Self::symbol_name_info(argument, "function name")?;
            self.emit(
                function,
                if escaped {
                    Instruction::FunctionLoadExact(name)
                } else {
                    Instruction::FunctionLoad(name)
                },
                argument.span,
            )?;
        } else {
            self.compile_expression(function, argument)?;
        }
        Ok(())
    }

    pub(crate) fn compile_define(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        Self::require_arity(items, "DEFINE", "two", 2, span)?;
        let name_form = &items[1];
        let value_form = &items[2];
        let (name, escaped) = Self::symbol_name_info(name_form, "define name")?;
        self.compile_expression(function, value_form)?;
        let instruction = if escaped {
            Instruction::DefineExact(name)
        } else {
            Instruction::Define(name)
        };
        self.emit(function, instruction, value_form.span)?;
        Ok(())
    }

    pub(crate) fn compile_defun(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 4 {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "defun needs a name, parameters, and a body".to_string(),
                },
                operator_span(items, span),
            ));
        }
        let name_form = &items[1];
        let parameter_form = &items[2];
        let (name, name_escaped) = Self::symbol_name_info(name_form, "defun name")?;
        let lambda_list = Self::parameters(parameter_form)?;
        let child = self.reserve_function_with_rest(
            Some(name.clone()),
            lambda_list.required,
            lambda_list.required_escaped,
            lambda_list.rest,
            lambda_list.rest_escaped,
        );
        let optional = self.compile_optional_parameters(&lambda_list.optional)?;
        self.functions[child].optional = optional;
        let keywords = self.compile_keyword_parameters(&lambda_list.keywords)?;
        self.functions[child].keywords = keywords;
        self.functions[child].has_keyword_section = lambda_list.has_keyword_section;
        self.functions[child].allow_other_keys = lambda_list.allow_other_keys;
        let auxiliary = self.compile_auxiliary_parameters(&lambda_list.auxiliary)?;
        self.functions[child].auxiliary = auxiliary;
        let body = items.get(3..).unwrap_or(&[]);
        self.compile_sequence(child, body)?;
        self.emit(child, Instruction::Return, span)?;

        self.emit(function, Instruction::MakeClosure(child), span)?;
        let define = if name_escaped {
            Instruction::DefineExact(name.clone())
        } else {
            Instruction::Define(name.clone())
        };
        self.emit(function, define, span)?;
        self.emit(function, Instruction::Pop, span)?;
        let constant = if name_escaped {
            Constant::SymbolExact(name)
        } else {
            Constant::Symbol(name)
        };
        self.emit(function, Instruction::Constant(constant), span)?;
        Ok(())
    }
}
