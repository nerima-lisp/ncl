use super::{
    FunctionObject, NativeCallResult, NativeInvocation, ObjectRuntime, ThreadContext, Word,
    error_result, ok_result,
};

/// Record an error at the native boundary without retaining a transient exit.
pub(super) const fn record_boundary_error(
    context: &mut ThreadContext,
    error: ncl_object::ObjectError,
) {
    match error {
        ncl_object::ObjectError::NonLocalExit if context.is_unwinding() => {}
        ncl_object::ObjectError::NonLocalExit => {
            context.set_pending(ncl_object::ObjectError::ControlError);
        }
        other => context.set_pending(other),
    }
}

pub(super) fn dispatch_with_context(
    invocation: &mut NativeInvocation<'_>,
    argc: u64,
    registers: [u64; 4],
    rest: u64,
    function_object: u64,
) -> NativeCallResult {
    let context: &mut ThreadContext = invocation.context;
    let object: &ObjectRuntime = invocation.object;
    let Some(count) = Word::from_bits(argc)
        .as_fixnum()
        .and_then(|value| usize::try_from(value).ok())
        .filter(|count| *count <= ncl_sys::CALL_ARGUMENTS_LIMIT)
    else {
        context.set_pending(ncl_object::ObjectError::Layout);
        return error_result();
    };
    let Ok(function) = FunctionObject::try_from(Word::from_bits(function_object)) else {
        context.set_pending(ncl_object::ObjectError::Unbound);
        return error_result();
    };
    let mut call_words: Vec<Word> = registers
        .iter()
        .take(count)
        .copied()
        .map(Word::from_bits)
        .collect();
    if count > registers.len() {
        let Ok(rest_words) = ncl_sys::copy_native_words(rest, count - registers.len()) else {
            context.set_pending(ncl_object::ObjectError::Layout);
            return error_result();
        };
        call_words.extend(rest_words);
    }
    let mut rooted_function = function.as_word();
    let function_token = ncl_object::push_heap_root(object, &mut rooted_function);
    let mut rooted_words = call_words;
    let argument_tokens = rooted_words
        .iter_mut()
        .map(|word| ncl_object::push_heap_root(object, word))
        .collect::<Vec<_>>();
    let result = FunctionObject::try_from(rooted_function)
        .and_then(|function| object.call_builtin(context, function, &rooted_words));
    let arguments_popped = argument_tokens
        .into_iter()
        .rev()
        .all(|token| ncl_object::pop_heap_root(object, token));
    let function_popped = ncl_object::pop_heap_root(object, function_token);
    if !arguments_popped || !function_popped {
        context.set_pending(ncl_object::ObjectError::RootStackCorrupted);
        return error_result();
    }
    match result {
        Ok(value) => ok_result(value, context.values().len()),
        Err(ncl_object::ObjectError::NonLocalExit) => {
            record_boundary_error(context, ncl_object::ObjectError::NonLocalExit);
            error_result()
        }
        Err(error) => {
            if let Some(condition) = context.take_pending_condition() {
                match ncl_conditions::error(context, condition) {
                    Ok(()) => return ok_result(Word::NIL, context.values().len()),
                    Err(condition_error) => {
                        let error = match condition_error {
                            ncl_conditions::ConditionError::Object(error) => error,
                            // check-added-lines: allow(wildcard) preserve the original error.
                            _ => error,
                        };
                        record_boundary_error(context, error);
                        return error_result();
                    }
                }
            }
            record_boundary_error(context, error);
            error_result()
        }
    }
}
