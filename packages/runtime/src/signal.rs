use std::ptr::NonNull;

use ncl_object::typed::ConditionErrorData;
use ncl_object::{HeapRootedWord, LispError, ObjectError, Runtime, ThreadContext, Word};
use ncl_sys::Thread;

/// Signal a `TYPE-ERROR` for a compiled type check.
///
/// Source-origin ids are not resolved by the current runtime metadata.
pub extern "C" fn ncl_signal_type_error(
    thread: NonNull<Thread>,
    datum: Word,
    expected_type: Word,
    _source_origin: i64,
) -> Word {
    ncl_sys::with_native_context(
        thread,
        |invocation: &mut crate::support::NativeInvocation<'_>| match signal_type_error(
            invocation.context,
            invocation.object,
            datum,
            expected_type,
        ) {
            Ok(value) => value,
            Err(error) => {
                invocation.context.set_pending(error);
                Word::NIL
            }
        },
    )
    .unwrap_or(Word::NIL)
}

pub fn signal_type_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    datum: Word,
    expected_type: Word,
) -> Result<Word, ObjectError> {
    let condition = ncl_object::with_roots(ctx, &[datum, expected_type], |ctx, roots| {
        let datum = **roots.first().ok_or(ObjectError::Layout)?;
        let expected_type = **roots.get(1).ok_or(ObjectError::Layout)?;
        condition_from_lisp_error(
            ctx,
            runtime,
            LispError::Condition(ConditionErrorData::TypeError {
                datum,
                expected_type,
            }),
        )
    })?;
    signal_condition(ctx, condition)?;
    Ok(Word::NIL)
}

pub fn signal_lisp_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: LispError,
) -> Result<(), ObjectError> {
    let condition = condition_from_lisp_error(ctx, runtime, error)?;
    signal_condition(ctx, condition)
}

fn condition_from_lisp_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    error: LispError,
) -> Result<Word, ObjectError> {
    let Some(converter) = runtime.lisp_error_converter() else {
        return Err(ObjectError::Layout);
    };
    converter(ctx, runtime, error)
}

fn signal_condition(ctx: &mut ThreadContext, condition: Word) -> Result<(), ObjectError> {
    let condition = HeapRootedWord::new(ctx, condition)?;
    match ncl_conditions::error(ctx, condition.get()) {
        Ok(()) | Err(ncl_conditions::ConditionError::Unhandled) => Ok(()),
        Err(ncl_conditions::ConditionError::Object(error)) => Err(error),
        Err(_) => Err(ObjectError::Layout),
    }
}
