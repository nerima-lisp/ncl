#![allow(clippy::wildcard_imports)]
use super::super::*;

impl CompileState {
    pub(super) fn compile_setf_property_place(
        &mut self,
        function: FunctionId,
        place: &Form,
        value_form: &Form,
    ) -> Result<bool, CompileError> {
        let FormKind::List(items) = &place.kind else {
            return Ok(false);
        };
        if items.len() != 3 {
            return Ok(false);
        }
        let Some((operator, _)) = Self::symbol_name_info(&items[0], "setf place operator").ok()
        else {
            return Ok(false);
        };
        match operator.as_str() {
            "GET" => {
                self.compile_expression(function, &items[1])?;
                self.compile_expression(function, &items[2])?;
                self.compile_expression(function, value_form)?;
                self.emit(function, Instruction::SetfGetDynamic, place.span)?;
                Ok(true)
            }
            "GETHASH" => {
                self.compile_expression(function, &items[1])?;
                self.compile_expression(function, &items[2])?;
                self.compile_expression(function, value_form)?;
                self.emit(function, Instruction::SetfGethashDynamic, place.span)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}
