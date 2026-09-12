#![allow(clippy::wildcard_imports)]
use super::super::super::*;
use super::assignment::compile_value_for_setf;

pub(super) fn compile_symbol_setf(
    state: &mut CompileState,
    function: FunctionId,
    place: &Form,
    value_form: &Form,
) -> Result<bool, CompileError> {
    let Ok((name, escaped)) = CompileState::symbol_name_info(place, "setf place") else {
        return Ok(false);
    };
    compile_value_for_setf(state, function, value_form)?;
    let instruction = if escaped {
        Instruction::SetExact(name)
    } else {
        Instruction::Set(name)
    };
    state.emit(function, instruction, place.span)?;
    Ok(true)
}
