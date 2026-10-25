use super::control::{args, bindings, form};
use crate::{elements, fresh_symbol};
use ncl_object::{
    BuiltinArgs, Local, MultipleValues, ObjectError, Runtime, Scope, ThreadContext, Word,
    symbol_name,
};

type Result<T = Word> = std::result::Result<T, ObjectError>;

pub fn expand(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let input_form = input.get(0).ok_or(ObjectError::TypeError)?;
    let values = args(ctx, input_form)?;
    if !values.len().is_multiple_of(2) {
        return Err(ObjectError::TypeError);
    }
    let mut scope = Scope::new(ctx);
    let mut temporaries = Vec::with_capacity(values.len() / 2);
    let mut variables = Vec::with_capacity(values.len() / 2);
    let mut expressions = Vec::with_capacity(values.len() / 2);
    for index in 0..values.len() / 2 {
        // check-added-lines: allow(index) index is bounded by the rooted value pairs.
        let variable = values.get(index * 2).ok_or(ObjectError::TypeError)?.get();
        symbol_name(scope.context_mut(), variable)?;
        variables.push(scope.root(Local::<Word>::from_word(variable)));
        // check-added-lines: allow(index) index is bounded by the rooted value pairs.
        let expression = values
            .get(index * 2 + 1)
            .ok_or(ObjectError::TypeError)?
            .get();
        expressions.push(scope.root(Local::<Word>::from_word(expression)));
        let temporary = crate::fresh_symbol(scope.context_mut(), runtime)?;
        temporaries.push(scope.root(Local::<Word>::from_word(temporary)));
    }

    let binding_pairs = temporaries
        .iter()
        .zip(expressions.iter())
        .map(|(temporary, expression)| {
            (
                scope.get(*temporary).as_word(),
                scope.get(*expression).as_word(),
            )
        })
        .collect::<Vec<_>>();
    let binding_list = bindings(scope.context_mut(), runtime, &binding_pairs)?;
    let binding_list = scope.root(Local::<Word>::from_word(binding_list));

    let mut assignments = Vec::with_capacity(values.len());
    for (variable, temporary) in variables.iter().zip(temporaries.iter()) {
        assignments.extend([
            scope.get(*variable).as_word(),
            scope.get(*temporary).as_word(),
        ]);
    }
    let assignment = form(scope.context_mut(), runtime, "SETQ", &assignments)?;
    let assignment = scope.root(Local::<Word>::from_word(assignment));
    let binding_list = scope.get(binding_list).as_word();
    let assignment = scope.get(assignment).as_word();
    form(
        scope.context_mut(),
        runtime,
        "LET",
        &[binding_list, assignment],
    )
}

pub fn expand_multiple_value_setq(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    input: &BuiltinArgs<'_>,
    _values: &mut MultipleValues,
) -> Result {
    let input_form = input.get(0).ok_or(ObjectError::TypeError)?;
    let arguments = args(ctx, input_form)?;
    // check-added-lines: allow(index) exact-shape destructuring
    let [variable_list, value_form] = arguments.as_slice() else {
        return Err(ObjectError::TypeError);
    };
    let variables = elements(ctx, variable_list.get())?;
    let mut scope = Scope::new(ctx);
    let variable_handles = scope.root_many(
        &variables
            .iter()
            .copied()
            .map(Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let value_form = scope.root(Local::<Word>::from_word(value_form.get()));
    for variable in variable_handles.iter() {
        let variable = scope.get(*variable).as_word();
        symbol_name(scope.context_mut(), variable)?;
    }

    let value_list = fresh_symbol(scope.context_mut(), runtime)?;
    let value_list = scope.root(Local::<Word>::from_word(value_list));
    let value_form = scope.get(value_form).as_word();
    let value_form = form(
        scope.context_mut(),
        runtime,
        "MULTIPLE-VALUE-LIST",
        &[value_form],
    )?;
    let value_form = scope.root(Local::<Word>::from_word(value_form));
    let value_list_word = scope.get(value_list).as_word();
    let value_form_word = scope.get(value_form).as_word();
    let binding_list = bindings(
        scope.context_mut(),
        runtime,
        &[(value_list_word, value_form_word)],
    )?;
    let binding_list = scope.root(Local::<Word>::from_word(binding_list));

    let value_list_word = scope.get(value_list).as_word();
    let primary = form(
        scope.context_mut(),
        runtime,
        "NTH",
        &[Word::fixnum(0), value_list_word],
    )?;
    let primary = scope.root(Local::<Word>::from_word(primary));
    let mut assignments = Vec::new();
    for (index, variable) in variable_handles.iter().enumerate() {
        let index = i64::try_from(index).map_err(|_| ObjectError::TypeError)?;
        let value_list_word = scope.get(value_list).as_word();
        let value = form(
            scope.context_mut(),
            runtime,
            "NTH",
            &[Word::fixnum(index), value_list_word],
        )?;
        let value = scope.root(Local::<Word>::from_word(value));
        let variable_word = scope.get(*variable).as_word();
        assignments.push(scope.root(Local::<Word>::from_word(variable_word)));
        let value_word = scope.get(value).as_word();
        assignments.push(scope.root(Local::<Word>::from_word(value_word)));
    }
    let primary_word = scope.get(primary).as_word();
    let primary = scope.root(Local::<Word>::from_word(primary_word));
    let mut body_values = vec![primary];
    if !assignments.is_empty() {
        let assignment_values = assignments
            .iter()
            .map(|assignment| scope.get(*assignment).as_word())
            .collect::<Vec<_>>();
        let assignment = form(scope.context_mut(), runtime, "SETQ", &assignment_values)?;
        let assignment = scope.root(Local::<Word>::from_word(assignment));
        let assignment_word = scope.get(assignment).as_word();
        body_values.push(scope.root(Local::<Word>::from_word(assignment_word)));
    }
    let body_values = body_values
        .iter()
        .map(|value| scope.get(*value).as_word())
        .collect::<Vec<_>>();
    let body = form(scope.context_mut(), runtime, "PROG1", &body_values)?;
    let body = scope.root(Local::<Word>::from_word(body));
    let body = scope.get(body).as_word();
    let binding_list = scope.get(binding_list).as_word();
    form(scope.context_mut(), runtime, "LET", &[binding_list, body])
}

#[cfg(test)]
#[path = "psetq/tests.rs"]
mod tests;
