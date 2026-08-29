use crate::{
    CompileError, CompileState, Constant, Form, FormKind, FunctionId, Instruction, Span,
    normalize_name, special_operator_name, symbol_reference,
};

impl CompileState {
    pub(super) fn compile_list(
        &mut self,
        function: FunctionId,
        span: Span,
        items: &[Form],
    ) -> Result<(), CompileError> {
        let Some(operator) = items.first() else {
            self.emit(function, Instruction::Constant(Constant::Nil), span)?;
            return Ok(());
        };

        let operator_name = match &operator.kind {
            FormKind::Atom(name) => special_operator_name(name),
            _ => None,
        };
        if let Some(name) = operator_name.as_deref() {
            if let Some(result) = self.dispatch_core_and_control_forms(name, function, span, items)
            {
                return result;
            }
            if let Some(result) = self.dispatch_logic_and_binding_forms(name, function, span, items)
            {
                return result;
            }
        }

        self.compile_call(function, span, operator, items)
    }

    fn compile_call(
        &mut self,
        function: FunctionId,
        span: Span,
        operator: &Form,
        items: &[Form],
    ) -> Result<(), CompileError> {
        if let FormKind::Atom(name) = &operator.kind {
            let (reference_name, escaped) =
                symbol_reference(name).unwrap_or_else(|| (normalize_name(name), false));
            self.emit(
                function,
                if escaped {
                    Instruction::FunctionLoadExact(reference_name)
                } else {
                    Instruction::FunctionLoad(reference_name)
                },
                operator.span,
            )?;
            for item in items.iter().skip(1) {
                self.compile_expression(function, item)?;
            }
        } else {
            for item in items {
                self.compile_expression(function, item)?;
            }
        }
        self.emit(
            function,
            Instruction::Call(items.len().saturating_sub(1)),
            span,
        )?;
        Ok(())
    }
}
