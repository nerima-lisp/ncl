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
    let temporaries = vec![Word::NIL; values.len() / 2];
    ncl_object::with_rooted_slice(ctx, &temporaries, |ctx, temporaries| {
        for temporary in temporaries.iter_mut() {
            *temporary = crate::fresh_symbol(ctx, runtime)?;
        }
        let mut binding_pairs = Vec::with_capacity(values.len() / 2);
        for (index, temporary) in temporaries.iter_mut().enumerate() {
            // check-added-lines: allow(index) index is bounded by the rooted value pairs.
            let variable = values.get(index * 2).ok_or(ObjectError::TypeError)?.get();
            symbol_name(ctx, variable)?;
            // check-added-lines: allow(index) index is bounded by the rooted value pairs.
            let value = values
                .get(index * 2 + 1)
                .ok_or(ObjectError::TypeError)?
                .get();
            binding_pairs.push((*temporary, value));
        }
        let binding_list = bindings(ctx, runtime, &binding_pairs)?;
        let mut assignments = Vec::with_capacity(values.len());
        for (index, temporary) in temporaries.iter().enumerate() {
            // check-added-lines: allow(index) index is bounded by the rooted value pairs.
            let variable = values.get(index * 2).ok_or(ObjectError::TypeError)?.get();
            assignments.extend([variable, *temporary]);
        }
        ncl_object::with_roots(ctx, &[binding_list], |ctx, roots| {
            let binding_list = **roots.first().ok_or(ObjectError::TypeError)?;
            let assignment = form(ctx, runtime, "SETQ", &assignments)?;
            form(ctx, runtime, "LET", &[binding_list, assignment])
        })
    })
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
    let mut assignments = Vec::with_capacity(variable_handles.len().saturating_mul(2));
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
        assignments.extend([scope.get(*variable).as_word(), scope.get(value).as_word()]);
    }
    let mut body_values = vec![scope.get(primary).as_word()];
    if !assignments.is_empty() {
        let assignment = form(scope.context_mut(), runtime, "SETQ", &assignments)?;
        let assignment = scope.root(Local::<Word>::from_word(assignment));
        body_values.push(scope.get(assignment).as_word());
    }
    let body = form(scope.context_mut(), runtime, "PROG1", &body_values)?;
    let body = scope.root(Local::<Word>::from_word(body));
    let body = scope.get(body).as_word();
    let binding_list = scope.get(binding_list).as_word();
    form(scope.context_mut(), runtime, "LET", &[binding_list, body])
}
