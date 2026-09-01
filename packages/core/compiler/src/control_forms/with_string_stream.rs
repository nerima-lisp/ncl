#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(crate) fn compile_with_input_from_string(&mut self, function: FunctionId, span: Span, items: &[Form]) -> Result<(), CompileError> {
        self.compile_with_string_stream(function, span, items, true, "WITH-INPUT-FROM-STRING")
    }

    pub(crate) fn compile_with_output_to_string(&mut self, function: FunctionId, span: Span, items: &[Form]) -> Result<(), CompileError> {
        self.compile_with_string_stream(function, span, items, false, "WITH-OUTPUT-TO-STRING")
    }

    fn compile_with_string_stream(&mut self, function: FunctionId, span: Span, items: &[Form], input: bool, operator: &str) -> Result<(), CompileError> {
        if items.len() < 2 { return Err(Self::arity_error(items, operator, "at least one", span)); }
        let FormKind::List(binding) = &items[1].kind else { return Err(CompileError::new(CompileErrorKind::ExpectedList { context: format!("{operator} binding") }, items[1].span)); };
        if binding.is_empty() || (input && binding.len() < 2) { return Err(CompileError::new(CompileErrorKind::InvalidForm { message: format!("{operator} binding needs a variable{}", if input { " and string form" } else { "" }) }, items[1].span)); }
        let variable = Self::symbol_name(&binding[0], &format!("{operator} variable"))?;
        let stream = self.reserve_function(None, Vec::new());
        let mut stream_form = vec![Form::atom(if input { "MAKE-STRING-INPUT-STREAM" } else { "MAKE-STRING-OUTPUT-STREAM" }, span)];
        if input { stream_form.extend(binding[1..].iter().cloned()); }
        self.compile_expression(stream, &Form::list(stream_form, span))?;
        self.emit(stream, Instruction::Return, span)?;
        let body = self.reserve_function(None, Vec::new());
        self.compile_sequence(body, &items[2..])?;
        self.emit(body, Instruction::Return, span)?;
        self.emit(function, Instruction::StandardStreamBind { input, stream, variable, body }, span)?;
        Ok(())
    }
}
