use super::{
    BuiltinArgs, BuiltinFunctionCaller, MultipleValues, ObjectError, Runtime, Scope, Sequence,
    ThreadContext, Word, domain, keep_result, root_args, words,
};
use ncl_object::Local;

fn sequence_from_values(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    sequence: Sequence,
    values: &mut [Word], // check-added-lines: allow(index) slice type
) -> Result<Word, ObjectError> {
    match sequence {
        Sequence::List(_) => domain::selection::list_from_values(ctx, runtime, values),
        Sequence::Vector(_) => {
            let mut scope = Scope::new(ctx);
            let locals = values
                .iter()
                .copied()
                .map(Local::from_word)
                .collect::<Vec<_>>();
            let handles = scope.root_many(&locals);
            let result = scope.make_simple_vector(runtime, &handles)?;
            Ok(scope.get(result).as_word())
        }
        Sequence::String(_) => {
            let characters = values
                .iter()
                .map(|value| {
                    value
                        .as_character()
                        .and_then(char::from_u32)
                        .ok_or(ObjectError::TypeError)
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut scope = Scope::new(ctx);
            let result = scope.make_string(runtime, &characters)?;
            Ok(scope.get(result).as_word())
        }
    }
}

pub fn remove_duplicates_entry(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    args: &BuiltinArgs<'_>,
    values: &mut MultipleValues,
) -> Result<Word, ObjectError> {
    let mut scope = Scope::new(ctx);
    let handles = root_args(&mut scope, args)?;
    let rooted = words(&scope, &handles);
    let (positional, options) =
        domain::selection::parse_options_single(scope.context_mut(), runtime, &rooted)?;
    if positional.len() != 1 {
        return Err(ObjectError::TypeError);
    }
    let sequence = *positional.first().ok_or(ObjectError::TypeError)?;
    let sequence_type = domain::selection::object_sequence(scope.context(), sequence)?;
    let items = domain::selection::sequence_values(scope.context_mut(), sequence_type)?;
    let mut caller = BuiltinFunctionCaller;
    let mut result = domain::selection::remove_duplicates(
        scope.context_mut(),
        runtime,
        &mut caller,
        &items,
        options,
    )?;
    let result = sequence_from_values(scope.context_mut(), runtime, sequence_type, &mut result)?;
    values.clear();
    keep_result(&mut scope, Ok(result))
}
