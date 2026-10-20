use super::{
    FunctionObject, NativeCallResult, NativeInvocation, ObjectRuntime, ThreadContext, Word,
    error_result, ok_result, unregister_heap_roots,
};

/// Record an error at the native boundary without retaining a transient exit.
pub const fn record_boundary_error(context: &mut ThreadContext, error: ncl_object::ObjectError) {
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
    let result = if count <= registers.len() {
        let mut arguments = [Word::NIL; 4];
        for (target, source) in arguments.iter_mut().zip(registers).take(count) {
            *target = Word::from_bits(source);
        }
        let mut root_tokens = [None; 4];
        let Some(arguments) = arguments.get_mut(..count) else {
            context.set_pending(ncl_object::ObjectError::Layout);
            return error_result();
        };
        let Some(root_tokens) = root_tokens.get_mut(..count) else {
            context.set_pending(ncl_object::ObjectError::Layout);
            return error_result();
        };
        call_with_heap_rooted_arguments(context, object, function.as_word(), arguments, root_tokens)
    } else {
        let Ok(mut arguments) = copy_extended_arguments(registers, rest, count) else {
            context.set_pending(ncl_object::ObjectError::Layout);
            return error_result();
        };
        let mut root_tokens = vec![None; count];
        call_with_heap_rooted_arguments(
            context,
            object,
            function.as_word(),
            &mut arguments,
            &mut root_tokens,
        )
    };
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

fn copy_extended_arguments(
    registers: [u64; 4],
    rest: u64,
    count: usize,
) -> Result<Vec<Word>, ncl_object::ObjectError> {
    let rest_count = count
        .checked_sub(registers.len())
        .ok_or(ncl_object::ObjectError::Layout)?;
    let rest_words = ncl_sys::copy_native_words(rest, rest_count)
        .map_err(|_| ncl_object::ObjectError::Layout)?;
    let mut arguments = Vec::with_capacity(count);
    arguments.extend(registers.into_iter().map(Word::from_bits));
    arguments.extend(rest_words);
    Ok(arguments)
}

fn call_with_heap_rooted_arguments(
    context: &mut ThreadContext,
    object: &ObjectRuntime,
    function: Word,
    arguments: &mut [Word],
    root_tokens: &mut [Option<ncl_sys::RootToken>],
) -> Result<Word, ncl_object::ObjectError> {
    if arguments.len() != root_tokens.len() {
        return Err(ncl_object::ObjectError::Layout);
    }
    let mut rooted_function = function;
    let function_token = ncl_object::push_heap_root(object, &mut rooted_function);
    for (argument, token) in arguments.iter_mut().zip(root_tokens.iter_mut()) {
        *token = Some(ncl_object::push_heap_root(object, argument));
    }
    let result = FunctionObject::try_from(rooted_function)
        .and_then(|function| object.call_builtin(context, function, arguments));
    let arguments_popped = unregister_heap_roots(
        object,
        root_tokens.iter_mut().rev().filter_map(Option::take),
    );
    let function_popped = ncl_object::unregister_heap_root(object, function_token);
    if arguments_popped.is_err() || function_popped.is_err() {
        return Err(ncl_object::ObjectError::RootStackCorrupted);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::copy_extended_arguments;
    use ncl_object::ObjectError;

    #[test]
    fn rejects_invalid_large_rest_area_before_reserving_arguments() {
        let result = copy_extended_arguments([0; 4], 0, ncl_sys::CALL_ARGUMENTS_LIMIT);
        assert_eq!(result, Err(ObjectError::Layout));
    }
}
