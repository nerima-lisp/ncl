#![allow(clippy::wildcard_imports)]
use super::super::super::*;

pub(super) fn compile_assignment_setf(
    state: &mut CompileState,
    function: FunctionId,
    place: &Form,
    value_form: &Form,
) -> Result<(), CompileError> {
    state.compile_expression(function, value_form)?;
    let instruction = match CompileState::symbol_name_info(place, "setf place") {
        Ok((name, escaped)) if escaped => Instruction::SetExact(name),
        Ok((name, _)) => Instruction::Set(name),
        Err(_) => Instruction::Setf(place.clone()),
    };
    state.emit(function, instruction, place.span)?;
    Ok(())
}
