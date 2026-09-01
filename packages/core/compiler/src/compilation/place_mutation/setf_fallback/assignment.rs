#![allow(clippy::wildcard_imports)]
use super::super::super::*;

pub(super) fn compile_value_for_setf(
    state: &mut CompileState,
    function: FunctionId,
    value_form: &Form,
) -> Result<(), CompileError> {
    state.compile_expression(function, value_form)?;
    state.compile_expression(function, value_form)?;
    Ok(())
}
