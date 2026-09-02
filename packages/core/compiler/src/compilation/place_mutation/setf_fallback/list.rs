#![allow(clippy::wildcard_imports)]
use super::super::super::*;

pub(super) fn compile_list_setf(
    state: &mut CompileState,
    function: FunctionId,
    place: &Form,
    value_form: &Form,
) -> Result<bool, CompileError> {
    if let Some((index_form, target, accessors, name, escaped)) = crate::helpers::dynamic_nth_list_place(place) {
        state.compile_expression(function, index_form)?;
        state.compile_expression(function, target)?;
        state.compile_expression(function, value_form)?;
        state.emit(function, Instruction::SetfNestedNthDynamic { accessors, name, escaped }, place.span)?;
        return Ok(true);
    }
    let mut accessors = Vec::new();
    let mut target = place;
    while let Some((accessor, next_target)) = crate::helpers::list_accessor_target(target) {
        accessors.push(accessor);
        target = next_target;
    }
    if accessors.len() >= 2
        || accessors
            .first()
            .is_some_and(|accessor| crate::helpers::is_composite_list_accessor(accessor))
    {
        if let Ok((name, escaped)) = CompileState::symbol_name_info(target, "setf list target") {
            accessors.reverse();
            state.compile_expression(function, target)?;
            state.compile_expression(function, value_form)?;
            state.emit(
                function,
                Instruction::SetfNestedList { accessors, name, escaped },
                place.span,
            )?;
            return Ok(true);
        }
    }
    let list_place = match &place.kind {
        FormKind::List(items) if items.len() == 2 => {
            let operator = CompileState::symbol_name_info(&items[0], "setf place operator")
                .ok()
                .map(|(name, _)| name);
            operator.and_then(|operator| {
                matches!(operator.as_str(), "CAR" | "FIRST" | "CDR" | "REST")
                    .then(|| CompileState::symbol_name_info(&items[1], "setf list target").ok())
                    .flatten()
                    .map(|(name, escaped)| (operator, name, escaped, &items[1]))
            })
        }
        _ => None,
    };
    let Some((operator, name, escaped, target)) = list_place else {
        return Ok(false);
    };
    state.compile_expression(function, target)?;
    state.compile_expression(function, value_form)?;
    state.emit(function, Instruction::SetfList { operator, name, escaped }, place.span)?;
    Ok(true)
}
