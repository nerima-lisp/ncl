#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    #[allow(clippy::too_many_lines)]
    pub(crate) fn compile_dolist(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if items.len() < 2 {
            return Err(Self::arity_error(items, "DOLIST", "at least one", span));
        }
        let Some(spec_form) = items.get(1) else {
            return Err(Self::internal_error(
                span,
                "missing DOLIST binding after arity check",
            ));
        };
        let FormKind::List(spec) = &spec_form.kind else {
            return Err(CompileError::new(
                CompileErrorKind::ExpectedList {
                    context: "DOLIST binding".to_string(),
                },
                spec_form.span,
            ));
        };
        if !(spec.len() == 2 || spec.len() == 3) {
            return Err(CompileError::new(
                CompileErrorKind::InvalidForm {
                    message: "DOLIST binding needs a variable, list, and optional result"
                        .to_string(),
                },
                spec_form.span,
            ));
        }
        let Some(variable_form) = spec.first() else {
            return Err(Self::internal_error(
                spec_form.span,
                "missing DOLIST variable",
            ));
        };
        let variable = Self::symbol_name(variable_form, "DOLIST variable")?;
        let Some(list) = spec.get(1) else {
            return Err(Self::internal_error(spec_form.span, "missing DOLIST list"));
        };
        let result = spec.get(2);
        let tail = self.fresh_name("DOLIST_TAIL");

        self.emit(function, Instruction::EnterScope, spec_form.span)?;
        self.compile_expression(function, list)?;
        self.emit(function, Instruction::Define(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Nil),
            spec_form.span,
        )?;
        self.emit(
            function,
            Instruction::Define(variable.clone()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let loop_start = self.instruction_count(function, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("ENDP".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        let body_jump = self.emit(
            function,
            Instruction::JumpIfFalse(usize::MAX),
            spec_form.span,
        )?;
        let exit_jump = self.emit(function, Instruction::Jump(usize::MAX), spec_form.span)?;

        let body_start = self.instruction_count(function, span)?;
        self.patch_jump(function, body_jump, body_start, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("CAR".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        self.emit(function, Instruction::Set(variable.clone()), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;

        let body = items.get(2..).unwrap_or(&[]);
        self.compile_sequence(function, body)?;
        self.emit(function, Instruction::Pop, span)?;
        self.emit(
            function,
            Instruction::FunctionLoad("CDR".to_string()),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Load(tail.clone()), spec_form.span)?;
        self.emit(function, Instruction::Call(1), spec_form.span)?;
        self.emit(function, Instruction::Set(tail), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        self.emit(function, Instruction::Jump(loop_start), spec_form.span)?;

        let end = self.instruction_count(function, span)?;
        self.patch_jump(function, exit_jump, end, span)?;
        self.emit(
            function,
            Instruction::Constant(Constant::Nil),
            spec_form.span,
        )?;
        self.emit(function, Instruction::Set(variable), spec_form.span)?;
        self.emit(function, Instruction::Pop, spec_form.span)?;
        if let Some(result) = result {
            self.compile_expression(function, result)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(function, Instruction::ExitScope, span)?;
        Ok(())
    }
}
