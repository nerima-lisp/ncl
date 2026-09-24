//! Signalling a foreign error as a Lisp condition.

use ncl_object::{Runtime, ThreadContext, make_string};

use crate::FfiError;
use crate::roots::with_root;

/// Signal a `simple-error` condition carrying `message`.
///
/// A foreign call failure is delivered through the standard condition system
/// (`ncl-conditions`), so a Lisp `handler-case` sees it like any other error.
///
/// # Errors
/// Returns [`FfiError::MissingConditionClass`] when `SIMPLE-ERROR` is not
/// registered, and [`FfiError::Condition`] when signalling fails.
pub fn signal_ffi_error(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    message: &str,
) -> Result<(), FfiError> {
    let Some(class) = ncl_conditions::condition_class(ctx, runtime, "SIMPLE-ERROR") else {
        return Err(FfiError::MissingConditionClass("SIMPLE-ERROR"));
    };
    let mut message_word = make_string(ctx, runtime, &message.chars().collect::<Vec<_>>())?;
    with_root(ctx, &mut message_word, |ctx, message_word| {
        let condition = ncl_conditions::make_condition(ctx, runtime, class, &[*message_word])?;
        ncl_conditions::error(ctx, condition)
    })
}
