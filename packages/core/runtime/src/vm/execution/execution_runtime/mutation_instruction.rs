use ncl_syntax::FormKind;

use crate::{Environment, Runtime, RuntimeError, Value};

pub(super) fn execute(
    runtime: &Runtime,
    form: &ncl_syntax::Form,
    stack: &mut Vec<Value>,
    environment: &Environment,
) -> Result<(), RuntimeError> {
    let FormKind::List(items) = &form.kind else {
        return Err(RuntimeError::InvalidForm {
            message: "runtime mutation instruction requires a list".to_string(),
            span: Some(form.span),
        });
    };
    let Some(FormKind::Atom(operator)) = items.first().map(|item| &item.kind) else {
        return Err(RuntimeError::InvalidForm {
            message: "runtime mutation instruction requires an operator".to_string(),
            span: Some(form.span),
        });
    };
    let value = if operator.eq_ignore_ascii_case("PUSH") {
        runtime.special_push(items, environment)?
    } else if operator.eq_ignore_ascii_case("POP") {
        runtime.special_pop(items, environment)?
    } else if operator.eq_ignore_ascii_case("PUSHNEW") {
        runtime.special_pushnew(items, environment)?
    } else if operator.eq_ignore_ascii_case("ROTATEF") {
        runtime.special_rotatef(items, environment)?
    } else if operator.eq_ignore_ascii_case("SHIFTF") {
        runtime.special_shiftf(items, environment)?
    } else {
        return Err(RuntimeError::InvalidForm {
            message: format!("unsupported runtime mutation operator: {operator}"),
            span: Some(form.span),
        });
    };
    stack.push(value);
    Ok(())
}
