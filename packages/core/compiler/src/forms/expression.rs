use crate::{
    CompileError, CompileErrorKind, CompileState, Constant, Form, FormKind, FunctionId,
    Instruction, Span, literal_constant, normalize_name, symbol_reference,
};

impl CompileState {
    pub(crate) fn compile_sequence(
        &mut self,
        function: FunctionId,
        forms: &[Form],
    ) -> Result<(), CompileError> {
        if forms.is_empty() {
            self.emit(
                function,
                Instruction::Constant(Constant::Nil),
                Span::new(0, 0),
            )?;
            return Ok(());
        }

        for (index, form) in forms.iter().enumerate() {
            self.compile_expression(function, form)?;
            if index + 1 < forms.len() {
                self.emit(function, Instruction::Pop, form.span)?;
            }
        }
        Ok(())
    }

    pub(crate) fn compile_expression(
        &mut self,
        function: FunctionId,
        form: &Form,
    ) -> Result<(), CompileError> {
        match &form.kind {
            FormKind::Atom(atom) => {
                if let Some(constant) = literal_constant(atom) {
                    self.emit(function, Instruction::Constant(constant), form.span)?;
                } else if let Some((name, escaped)) = symbol_reference(atom) {
                    let instruction = if escaped {
                        Instruction::LoadExact(name)
                    } else {
                        Instruction::Load(name)
                    };
                    self.emit(function, instruction, form.span)?;
                } else {
                    self.emit(function, Instruction::Load(normalize_name(atom)), form.span)?;
                }
            }
            FormKind::String(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::String(value.clone())),
                    form.span,
                )?;
            }
            FormKind::Character(value) => {
                self.emit(
                    function,
                    Instruction::Constant(Constant::Character(*value)),
                    form.span,
                )?;
            }
            FormKind::Vector(_) => {
                self.emit(function, Instruction::Quote(form.clone()), form.span)?;
            }
            FormKind::DottedList { .. } => {
                return Err(CompileError::new(
                    CompileErrorKind::UnsupportedForm {
                        message: "dotted lists cannot be evaluated".to_string(),
                    },
                    form.span,
                ));
            }
            FormKind::List(items) => self.compile_list(function, form.span, items)?,
        }
        Ok(())
    }
}
