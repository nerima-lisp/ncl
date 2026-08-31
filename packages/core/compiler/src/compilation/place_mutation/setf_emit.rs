use super::super::*;

pub(super) fn emit_pop_if_needed(
    state: &mut CompileState,
    function: FunctionId,
    pair_index: usize,
    pair_count: usize,
    value_span: Span,
) -> Result<(), CompileError> {
    if pair_index + 1 < pair_count {
        state.emit(function, Instruction::Pop, value_span)?;
    }
    Ok(())
}
