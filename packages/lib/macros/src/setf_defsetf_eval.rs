use super::MacroEnvironment;
use ncl_object::{
    BuiltinFunctionCaller, Handle, Local, MultipleValues, ObjectError, ObjectRef, Runtime, Scope,
    Word, classify_object,
};

fn lookup_macro_variable<'scope>(
    scope: &Scope<'scope>,
    environment: &MacroEnvironment<'scope>,
    name: Word,
) -> Option<Handle<'scope, Word>> {
    environment
        .iter()
        .rev()
        .find(|(variable, _)| scope.get(*variable).as_word() == name)
        .map(|(_, value)| *value)
}

pub(in crate::setf) fn eval_macro_form<'scope>(
    scope: &mut Scope<'scope>,
    runtime: &Runtime,
    form: Handle<'scope, Word>,
    environment: &MacroEnvironment<'scope>,
) -> Result<Handle<'scope, Word>, ObjectError> {
    let word = scope.get(form).as_word();
    match classify_object(scope.context(), word) {
        ObjectRef::Symbol(_) => {
            if word == Word::NIL {
                return Ok(form);
            }
            if super::special_form(scope, runtime, word, "T")? {
                return Ok(form);
            }
            if let Some(package) = runtime.find_package(scope.context(), "KEYWORD")
                && ncl_object::symbol_package(scope.context(), word)? == package
            {
                return Ok(form);
            }
            let name = scope.get(form).as_word();
            lookup_macro_variable(scope, environment, name).ok_or(ObjectError::Unbound)
        }
        ObjectRef::Cons(_) => {
            let parts = scope.list_to_handle_vec(Local::from_word(word))?;
            let operator = parts
                .as_slice()
                .first()
                .copied()
                .ok_or(ObjectError::TypeError)?;
            let operator_word = scope.get(operator).as_word();
            if super::special_form(scope, runtime, operator_word, "QUOTE")? {
                if parts.len() != 2 {
                    return Err(ObjectError::TypeError);
                }
                return parts
                    .as_slice()
                    .get(1)
                    .copied()
                    .ok_or(ObjectError::TypeError);
            }
            let operator_word = scope.get(operator).as_word();
            if super::special_form(scope, runtime, operator_word, "PROGN")? {
                let mut result = scope.root(Local::from_word(Word::NIL));
                for form in parts.as_slice().iter().skip(1) {
                    result = eval_macro_form(scope, runtime, *form, environment)?;
                }
                return Ok(result);
            }
            let operator_word = scope.get(operator).as_word();
            if super::special_form(scope, runtime, operator_word, "IF")? {
                if !(3..=4).contains(&parts.len()) {
                    return Err(ObjectError::TypeError);
                }
                let test = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
                let test = eval_macro_form(scope, runtime, test, environment)?;
                let branch = if scope.get(test).as_word() == Word::NIL {
                    parts.as_slice().get(3).copied()
                } else {
                    parts.as_slice().get(2).copied()
                };
                return match branch {
                    Some(form) => eval_macro_form(scope, runtime, form, environment),
                    None => Ok(scope.root(Local::<Word>::from_word(Word::NIL))),
                };
            }
            let operator_word = scope.get(operator).as_word();
            if super::special_form(scope, runtime, operator_word, "QUASIQUOTE")? {
                if parts.len() != 2 {
                    return Err(ObjectError::TypeError);
                }
                let datum = *parts.as_slice().get(1).ok_or(ObjectError::TypeError)?;
                return super::super::eval_quasiquote(scope, runtime, datum, environment, 1);
            }

            let mut arguments = scope.root_many(&[]);
            for argument in parts.as_slice().iter().skip(1) {
                let value = eval_macro_form(scope, runtime, *argument, environment)?;
                let value = scope.get(value).as_word();
                arguments.push(scope, Local::from_word(value));
            }
            let mut caller = BuiltinFunctionCaller;
            let mut values = MultipleValues::default();
            scope.call_function(runtime, operator, &arguments, &mut caller, &mut values)
        }
        _ => Ok(form), // check-added-lines: allow(wildcard) non-call forms pass through unchanged.
    }
}
