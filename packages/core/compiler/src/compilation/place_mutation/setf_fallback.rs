#![allow(clippy::wildcard_imports)]
use super::super::*;

mod assignment;
mod evaluator;
mod list;
mod symbol;

impl CompileState {
    pub(super) fn compile_setf_fallback(
        &mut self,
        function: FunctionId,
        place: &Form,
        value_form: &Form,
    ) -> Result<(), CompileError> {
        if list::compile_list_setf(self, function, place, value_form)? {
            return Ok(());
        }
        if symbol::compile_symbol_setf(self, function, place, value_form)? {
            return Ok(());
        }
        evaluator::compile_evaluator_setf(self, function, place, value_form)
    }
}
