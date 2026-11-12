use super::{Options, with_scope, word};
use ncl_object::typed::FunctionDesignator;
use ncl_object::{
    BuiltinFunctionCaller, FunctionArguments, FunctionCaller, Local, MultipleValues, ObjectError,
    Runtime, ThreadContext, Word,
};

fn call_predicate(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    args: &[Word],
) -> Result<Word, ObjectError> {
    let mut values = Vec::with_capacity(args.len() + 1);
    values.push(function);
    values.extend_from_slice(args);
    with_scope(ctx, &values, |scope, handles| {
        let designator = FunctionDesignator::try_from_word(
            scope.context(),
            word(scope, *handles.iter().next().ok_or(ObjectError::Layout)?),
        )?;
        let mut caller = BuiltinFunctionCaller;
        let mut values = MultipleValues::new();
        let args = handles
            .iter()
            .skip(1)
            .map(|handle| word(scope, *handle))
            .collect::<Vec<_>>();
        caller.call_function(
            scope.context_mut(),
            runtime,
            designator,
            FunctionArguments::new(&args),
            &mut values,
        )
    })
}
pub(super) fn key(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    key: Word,
    value: Word,
) -> Result<Word, ObjectError> {
    if key == Word::NIL {
        Ok(value)
    } else {
        call_predicate(ctx, runtime, key, &[value])
    }
}
pub fn matches(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    a: Word,
    b: Word,
    opts: Options,
) -> Result<bool, ObjectError> {
    let root_values = [a, b, opts.key, opts.test, opts.test_not];
    with_scope(ctx, &root_values, |scope, handles| {
        let first_handle = *handles.iter().next().ok_or(ObjectError::Layout)?;
        let second_handle = *handles.iter().nth(1).ok_or(ObjectError::Layout)?;
        let key_handle = *handles.iter().nth(2).ok_or(ObjectError::Layout)?;
        let key_fn = word(scope, key_handle);
        let first = word(scope, first_handle);
        let keyed_a_word = key(scope.context_mut(), runtime, key_fn, first)?;
        let keyed_a = scope.root(Local::from_word(keyed_a_word));
        let key_fn = word(scope, key_handle);
        let second = word(scope, second_handle);
        let keyed_second_word = key(scope.context_mut(), runtime, key_fn, second)?;
        let keyed_b = scope.root(Local::from_word(keyed_second_word));
        let left = word(scope, keyed_a);
        let right = word(scope, keyed_b);
        let test = word(scope, *handles.iter().nth(3).ok_or(ObjectError::Layout)?);
        let test_not = word(scope, *handles.iter().nth(4).ok_or(ObjectError::Layout)?);
        if test != Word::NIL {
            Ok(call_predicate(scope.context_mut(), runtime, test, &[left, right])? != Word::NIL)
        } else if test_not != Word::NIL {
            Ok(
                call_predicate(scope.context_mut(), runtime, test_not, &[left, right])?
                    == Word::NIL,
            )
        } else {
            Ok(left == right)
        }
    })
}
pub(super) fn compare(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    function: Word,
    a: Word,
    b: Word,
) -> Result<bool, ObjectError> {
    Ok(call_predicate(ctx, runtime, function, &[a, b])? != Word::NIL)
}
