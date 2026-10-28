use super::{
    FunctionObject, NativeCallResult, NativeInvocation, ObjectRuntime, ThreadContext, Word,
    error_result, unregister_heap_roots,
};
use ncl_object::MultipleValues;
use ncl_sys::Thread;
use std::ptr::NonNull;

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

/// Registers available to the generic trampoline stub, in the order the
/// shared `extern "C"` dispatcher receives them. `argc`/`a0..a3`/`rest` mirror
/// the native Lisp calling convention's own argument-count/argument-list
/// registers; `function_object`/`thread_ptr` are the two pinned registers the
/// ISA stub moves into ordinary argument slots. The stub also passes the
/// caller's frame pointer and continuation PC so this boundary can publish a
/// generated frame before dispatching the builtin.
#[allow(clippy::similar_names, clippy::too_many_arguments)]
pub(super) extern "C" fn dispatch(
    argc: u64,
    a0: u64,
    a1: u64,
    a2: u64,
    a3: u64,
    rest: u64,
    function_object: u64,
    thread_ptr: u64,
    frame_fp: u64,
    return_pc: u64,
) -> NativeCallResult {
    let Ok(address) = usize::try_from(thread_ptr) else {
        return error_result();
    };
    let Some(thread) = NonNull::new(std::ptr::without_provenance_mut::<Thread>(address)) else {
        return error_result();
    };
    let Ok(frame_fp) = usize::try_from(frame_fp) else {
        return error_result();
    };
    let Ok(return_pc) = usize::try_from(return_pc) else {
        return error_result();
    };
    // `unwrap_or_else` is banned project-wide by `check_added_lines.py`
    // (its one exemption is the `PoisonError::into_inner` idiom), so this
    // stays a `match` rather than the `Option::map_or_else`/`unwrap_or_else`
    // form clippy would otherwise prefer.
    #[allow(clippy::option_if_let_else)]
    match ncl_sys::with_native_context(thread, |invocation: &mut NativeInvocation<'_>| {
        super::boundary_stub::with_generated_frame(invocation, frame_fp, return_pc, |invocation| {
            dispatch_with_context(invocation, argc, [a0, a1, a2, a3], rest, function_object)
        })
    }) {
        Some(result) => result,
        None => error_result(),
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
    let mut values = MultipleValues::new();
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
        call_with_heap_rooted_arguments(
            context,
            object,
            function.as_word(),
            arguments,
            root_tokens,
            &mut values,
        )
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
            &mut values,
        )
    };
    match result {
        Ok(value) => publish_result(context, value, &values),
        Err(ncl_object::ObjectError::NonLocalExit) => {
            record_boundary_error(context, ncl_object::ObjectError::NonLocalExit);
            error_result()
        }
        Err(error) => {
            if let Some(condition) = context.take_pending_condition() {
                return signal_condition_error(context, condition, error);
            }
            record_boundary_error(context, error);
            error_result()
        }
    }
}

pub(super) fn signal_condition_error(
    context: &mut ThreadContext,
    condition: Word,
    original_error: ncl_object::ObjectError,
) -> NativeCallResult {
    match ncl_conditions::error(context, condition) {
        Ok(()) => {
            let values = context.values().to_vec();
            let value = values.first().copied().unwrap_or(Word::NIL);
            publish_values(context, value, &values)
        }
        Err(ncl_conditions::ConditionError::Unhandled) => {
            context.set_pending_condition(condition);
            record_boundary_error(context, original_error);
            error_result()
        }
        Err(condition_error) => {
            let error = match condition_error {
                ncl_conditions::ConditionError::Object(error) => error,
                // check-added-lines: allow(wildcard) preserve the original error.
                _ => original_error,
            };
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
    values: &mut MultipleValues,
) -> Result<Word, ncl_object::ObjectError> {
    if arguments.len() != root_tokens.len() {
        return Err(ncl_object::ObjectError::Layout);
    }
    let mut rooted_function = function;
    let function_token = ncl_object::push_heap_root(object, &mut rooted_function);
    for (argument, token) in arguments.iter_mut().zip(root_tokens.iter_mut()) {
        *token = Some(ncl_object::push_heap_root(object, argument));
    }
    let result = FunctionObject::try_from(rooted_function).and_then(|function| {
        object
            .call_builtin_if_registered(context, function, arguments, values)
            .ok_or(ncl_object::ObjectError::Unbound)?
    });
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

fn publish_result(
    context: &mut ThreadContext,
    value: Word,
    values: &MultipleValues,
) -> NativeCallResult {
    if !values.is_explicit() {
        return publish_values(context, value, &[value]);
    }
    publish_values(context, value, values.as_slice())
}

fn publish_values(context: &mut ThreadContext, value: Word, words: &[Word]) -> NativeCallResult {
    if context.is_unwinding() {
        return error_result();
    }
    if let Err(error) = context.set_multiple_value_area(words) {
        context.set_pending(error);
        return error_result();
    }
    NativeCallResult {
        value: value.bits(),
        count: u64::try_from(words.len()).unwrap_or(0),
    }
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
