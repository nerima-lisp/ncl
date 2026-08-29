#![allow(clippy::wildcard_imports)]
use super::*;

impl CompileState {
    pub(crate) fn compile_defvar(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
        force: bool,
    ) -> Result<(), CompileError> {
        let operator = if force { "DEFPARAMETER" } else { "DEFVAR" };
        if !(items.len() == 2 || items.len() == 3) {
            return Err(Self::arity_error(items, operator, "one or two", span));
        }
        let name_form = items
            .get(1)
            .ok_or_else(|| Self::internal_error(span, "missing defvar name"))?;
        let (name, escaped) = Self::symbol_name_info(
            name_form,
            if force {
                "defparameter name"
            } else {
                "defvar name"
            },
        )?;
        if force {
            if let Some(initializer) = items.get(2) {
                self.compile_expression(function, initializer)?;
            } else {
                self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            }
            self.emit(
                function,
                if escaped {
                    Instruction::DefineSpecialExact { name, force: true }
                } else {
                    Instruction::DefineSpecial { name, force: true }
                },
                span,
            )?;
            return Ok(());
        }

        self.emit(
            function,
            if escaped {
                Instruction::IsBoundExact(name.clone())
            } else {
                Instruction::IsBound(name.clone())
            },
            name_form.span,
        )?;
        let initialize_jump = self.emit(function, Instruction::JumpIfFalse(usize::MAX), span)?;
        self.emit(
            function,
            if escaped {
                Instruction::LoadExact(name.clone())
            } else {
                Instruction::Load(name.clone())
            },
            name_form.span,
        )?;
        let end_jump = self.emit(function, Instruction::Jump(usize::MAX), span)?;
        let initialize_target = self.instruction_count(function, span)?;
        if let Some(initializer) = items.get(2) {
            self.compile_expression(function, initializer)?;
        } else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
        }
        self.emit(
            function,
            if escaped {
                Instruction::DefineSpecialExact { name, force: false }
            } else {
                Instruction::DefineSpecial { name, force: false }
            },
            span,
        )?;
        let end_target = self.instruction_count(function, span)?;
        self.patch_jump(function, initialize_jump, initialize_target, span)?;
        self.patch_jump(function, end_jump, end_target, span)?;
        Ok(())
    }
}
