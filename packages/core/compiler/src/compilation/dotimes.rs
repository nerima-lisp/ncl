#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_dotimes(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "DOTIMES", "at least one", span));
        }
        let (spec_span, variable, count, result) = Self::parse_dotimes_spec(items, span)?;
        let limit = self.fresh_name("DOTIMES_LIMIT");

        self.emit(function, Instruction::EnterScope, spec_span)?;
        self.compile_expression(function, &count)?;
        self.emit(function, Instruction::Define(limit.clone()), spec_span)?;
        self.emit(function, Instruction::Pop, spec_span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(0)),
            spec_span,
        )?;
        self.emit(function, Instruction::Define(variable.clone()), spec_span)?;
        self.emit(function, Instruction::Pop, spec_span)?;

        let loop_start = self.instruction_count(function, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("<".to_string()),
            spec_span,
        )?;
        self.emit(function, Instruction::Load(variable.clone()), spec_span)?;
        self.emit(function, Instruction::Load(limit), spec_span)?;
        self.emit(function, Instruction::Call(2), spec_span)?;
        let exit_jump = self.emit(function, Instruction::JumpIfFalse(usize::MAX), spec_span)?;

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::Pop, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("+".to_string()),
            spec_span,
        )?;
        self.emit(function, Instruction::Load(variable.clone()), spec_span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Integer(1)),
            spec_span,
        )?;
        self.emit(function, Instruction::Call(2), spec_span)?;
        self.emit(function, Instruction::Set(variable), spec_span)?;
        self.emit(function, Instruction::Pop, spec_span)?;
        self.emit(function, Instruction::Jump(loop_start), spec_span)?;

        let end = self.instruction_count(function, span)?;
        self.patch_jump(function, exit_jump, end, span)?;
        if let Some(result) = result {
            self.compile_expression(function, &result)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }

    fn parse_dotimes_spec(
        items: &[Form],
        span: Span,
    ) -> Result<(Span, String, Form, Option<Form>), CompileError> {
        let Some(spec_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing DOTIMES binding after arity check",
            ));
        };
        let FormKind::List(spec) = &spec_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DOTIMES binding".to_string(),
                },
                spec_form.span,
            ));
        };
        if !(spec.len() == 2 || spec.len() == 3) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DOTIMES binding needs a variable, count, and optional result"
                        .to_string(),
                },
                spec_form.span,
            ));
        }
        let Some(variable_form) = spec.first() else {
            return Err(Self::internal_error(
                spec_form.span,
                "missing DOTIMES variable",
            ));
        };
        let variable = Self::symbol_name(variable_form, "DOTIMES variable")?;
        let Some(count) = spec.get(1) else {
            return Err(Self::internal_error(
                spec_form.span,
                "missing DOTIMES count",
            ));
        };
        Ok((
            spec_form.span,
            variable,
            count.clone(),
            spec.get(2).cloned(),
        ))
    }
}
