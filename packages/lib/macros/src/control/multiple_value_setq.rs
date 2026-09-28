//! Structural expansion of `MULTIPLE-VALUE-SETQ`.

use super::{form, multiple_value_bind};
use crate::{elements, fresh_symbol, list};
use ncl_object::{
    BuiltinArgs, Local, MultipleValues, ObjectError, ObjectRef, Runtime, Scope, ThreadContext,
    Word, classify_object,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

/// Expand a multiple-value assignment while retaining the primary value.
pub(crate) fn expand_multiple_value_setq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    values: &[Word],
) -> Result {
    if values.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    let variables_word = values.first().ok_or(ObjectError::TypeError)?;
    let value_word = values.get(1).ok_or(ObjectError::TypeError)?;

    let mut scope = Scope::new(ctx);
    let variables_handle = scope.root(Local::<Word>::from_word(*variables_word));
    let value_handle = scope.root(Local::<Word>::from_word(*value_word));
    let variables_word = scope.get(variables_handle).as_word();
    let variables = elements(scope.context_mut(), variables_word)?;
    let variable_handles = scope.root_many(
        &variables
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );

    for variable_handle in variable_handles.iter() {
        let variable = scope.get(*variable_handle).as_word();
        if !matches!(
            classify_object(scope.context_mut(), variable),
            ObjectRef::Symbol(_)
        ) {
            return Err(ObjectError::TypeError);
        }
    }

    let value = scope.get(value_handle).as_word();
    if variable_handles.is_empty() {
        return form(scope.context_mut(), runtime, "VALUES", &[value]);
    }

    let mut temporary_handles = Vec::with_capacity(variable_handles.len());
    for _ in variable_handles.iter() {
        let temporary = fresh_symbol(scope.context_mut(), runtime)?;
        temporary_handles.push(scope.root(Local::<Word>::from_word(temporary)));
    }

    let temporary_words = temporary_handles
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    let lambda_list = list(scope.context_mut(), runtime, &temporary_words)?;
    let lambda_list_handle = scope.root(Local::<Word>::from_word(lambda_list));

    let mut setq_arguments = Vec::with_capacity(variable_handles.len() * 2);
    for (variable_handle, temporary_handle) in variable_handles.iter().zip(temporary_handles.iter())
    {
        setq_arguments.push(scope.get(*variable_handle).as_word());
        setq_arguments.push(scope.get(*temporary_handle).as_word());
    }
    let setq = form(scope.context_mut(), runtime, "SETQ", &setq_arguments)?;
    let setq_handle = scope.root(Local::<Word>::from_word(setq));
    let setq_word = scope.get(setq_handle).as_word();
    let primary_word = scope
        .get(*temporary_handles.first().ok_or(ObjectError::TypeError)?)
        .as_word();
    let body = form(
        scope.context_mut(),
        runtime,
        "PROGN",
        &[setq_word, primary_word],
    )?;
    let body_handle = scope.root(Local::<Word>::from_word(body));

    let expansion_arguments = [
        scope.get(lambda_list_handle).as_word(),
        value,
        scope.get(body_handle).as_word(),
    ];
    multiple_value_bind::expand(scope.context_mut(), runtime, &expansion_arguments)
}

/// Adapt the macro-call ABI to [`expand_multiple_value_setq`].
pub(crate) fn expand_multiple_value_setq_adapter(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let form = input.get(0).ok_or(ObjectError::TypeError)?;
    let arguments = super::args(ctx, form)?;
    expand_multiple_value_setq(ctx, runtime, &arguments)
}
