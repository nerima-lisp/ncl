use super::control::{args, bindings, form};
use crate::{elements, fresh_symbol, list};
use ncl_object::{
    BuiltinArgs, MultipleValues, ObjectError, Runtime, ThreadContext, Word, symbol_name,
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
    ncl_object::with_roots(ctx, &values, |ctx, roots| {
        let temporaries = vec![Word::NIL; values.len() / 2];
        ncl_object::with_rooted_slice(ctx, &temporaries, |ctx, temporaries| {
            for temporary in temporaries.iter_mut() {
                *temporary = crate::fresh_symbol(ctx, runtime)?;
            }
            let mut binding_pairs = Vec::with_capacity(values.len() / 2);
            for (index, temporary) in temporaries.iter_mut().enumerate() {
                // check-added-lines: allow(index) index is bounded by the rooted value pairs.
                let variable = **roots.get(index * 2).ok_or(ObjectError::TypeError)?;
                symbol_name(ctx, variable)?;
                // check-added-lines: allow(index) index is bounded by the rooted value pairs.
                let value = **roots.get(index * 2 + 1).ok_or(ObjectError::TypeError)?;
                binding_pairs.push((*temporary, value));
            }
            let binding_list = bindings(ctx, runtime, &binding_pairs)?;
            let mut assignments = Vec::with_capacity(values.len());
            for (index, temporary) in temporaries.iter().enumerate() {
                // check-added-lines: allow(index) index is bounded by the rooted value pairs.
                let variable = **roots.get(index * 2).ok_or(ObjectError::TypeError)?;
                assignments.extend([variable, *temporary]);
            }
            ncl_object::with_roots(ctx, &[binding_list], |ctx, roots| {
                let binding_list = **roots.first().ok_or(ObjectError::TypeError)?;
                let assignment = form(ctx, runtime, "SETQ", &assignments)?;
                form(ctx, runtime, "LET", &[binding_list, assignment])
            })
        })
    })
}

pub fn expand_multiple_value_setq(
    runtime: &Runtime,
    ctx: &mut ThreadContext,
    input: &[Word],
    _values: &mut MultipleValues,
) -> Result {
    let input_form = input.first().copied().ok_or(ObjectError::TypeError)?;
    let values = args(ctx, input_form)?;
    let variables_form = values.first().copied().ok_or(ObjectError::TypeError)?;
    let value_form = values.get(1).copied().ok_or(ObjectError::TypeError)?;
    if values.len() != 2 {
        return Err(ObjectError::TypeError);
    }
    let variables = elements(ctx, variables_form)?;
    for variable in &variables {
        symbol_name(ctx, *variable)?;
    }

    let mut scope = ncl_object::Scope::new(ctx);
    let temporaries = variables
        .iter()
        .map(|_| fresh_symbol(scope.context_mut(), runtime))
        .collect::<Result<Vec<_>>>()?;
    let temporary_handles = scope.root_many(
        &temporaries
            .iter()
            .copied()
            .map(ncl_object::Local::<Word>::from_word)
            .collect::<Vec<_>>(),
    );
    let value_handle = scope.root(ncl_object::Local::<Word>::from_word(value_form));
    let rest = fresh_symbol(scope.context_mut(), runtime)?;
    let rest_handle = scope.root(ncl_object::Local::<Word>::from_word(rest));
    let rest_marker = crate::symbol(scope.context_mut(), runtime, "&REST")?;
    let rest_marker_handle = scope.root(ncl_object::Local::<Word>::from_word(rest_marker));
    let mut lambda_values = temporary_handles
        .iter()
        .map(|handle| scope.get(*handle).as_word())
        .collect::<Vec<_>>();
    lambda_values.push(scope.get(rest_marker_handle).as_word());
    lambda_values.push(scope.get(rest_handle).as_word());
    let lambda_list = list(scope.context_mut(), runtime, &lambda_values)?;
    let lambda_list_handle = scope.root(ncl_object::Local::<Word>::from_word(lambda_list));

    let mut assignments = Vec::with_capacity(variables.len() * 2);
    for (variable, handle) in variables.iter().copied().zip(temporary_handles.iter()) {
        assignments.push(variable);
        assignments.push(scope.get(*handle).as_word());
    }
    let result = variables.first().copied().unwrap_or(ncl_object::Word::NIL);
    let assignment = form(scope.context_mut(), runtime, "SETQ", &assignments)?;
    let body = form(scope.context_mut(), runtime, "PROGN", &[assignment, result])?;
    let lambda_list_word = scope.get(lambda_list_handle).as_word();
    let lambda = form(
        scope.context_mut(),
        runtime,
        "LAMBDA",
        &[lambda_list_word, body],
    )?;
    let value_word = scope.get(value_handle).as_word();
    form(
        scope.context_mut(),
        runtime,
        "MULTIPLE-VALUE-CALL",
        &[lambda, value_word],
    )
}
