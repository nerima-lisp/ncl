#![allow(clippy::wildcard_imports)]
use super::super::super::*;
use super::assignment::compile_value_for_setf;

pub(super) fn compile_evaluator_setf(
    state: &mut CompileState,
    function: FunctionId,
    place: &Form,
    value_form: &Form,
) -> Result<(), CompileError> {
    compile_value_for_setf(state, function, value_form)?;
    state.emit(function, Instruction::Setf(place.clone()), place.span)?;
    Ok(())
}
