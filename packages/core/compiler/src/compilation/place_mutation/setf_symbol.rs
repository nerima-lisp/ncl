use super::super::*;

impl CompileState {
    pub(super) fn compile_setf_symbol_cell_place(
        &mut self,
        function: FunctionId,
        place: &Form,
        value_form: &Form,
    ) -> Result<bool, CompileError> {
        let symbol_cell_place = match &place.kind {
            FormKind::List(items) if items.len() == 2 => {
                Self::symbol_name_info(&items[0], "setf symbol cell place operator")
                    .ok()
                    .filter(|(name, _)| {
                        matches!(name.as_str(), "SYMBOL-VALUE" | "SYMBOL-FUNCTION")
                    })
                    .map(|(operator, _)| (operator, &items[1]))
            }
            _ => None,
        };
        let Some((operator, target)) = symbol_cell_place else {
            return Ok(false);
        };
        self.compile_expression(function, target)?;
        self.compile_expression(function, value_form)?;
        self.emit(
            function,
            Instruction::SetfSymbolCellDynamic { operator },
            place.span,
        )?;
        Ok(true)
    }
}
