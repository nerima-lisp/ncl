#![allow(clippy::wildcard_imports)]
use crate::*;

impl CompileState {
    pub(super) fn compile_destructuring_default(
        &mut self,
        form: &Form,
    ) -> Result<FunctionId, CompileError> {
        let default_function = self.reserve_function(None, Vec::new());
        self.compile_expression(default_function, form)?;
        self.emit(default_function, Instruction::Return, form.span)?;
        Ok(default_function)
    }
}
