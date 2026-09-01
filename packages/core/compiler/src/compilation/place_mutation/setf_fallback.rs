#![allow(clippy::wildcard_imports)]
use super::super::*;

mod assignment;
mod list;

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
        assignment::compile_assignment_setf(self, function, place, value_form)
    }
}
