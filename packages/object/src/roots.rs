use crate::{ObjectError, ThreadContext};
use ncl_sys::{RootToken, Word};

/// Push a precise root.
pub fn push_root(ctx: &mut ThreadContext, value: &mut Word) -> RootToken {
    ncl_sys::push_root(&mut ctx.thread, value)
}

/// Pop a precise root.
pub fn pop_root(ctx: &mut ThreadContext, token: RootToken) -> bool {
    ncl_sys::pop_root(&mut ctx.thread, token)
}

/// Push a precise root after validating the context address.
///
/// # Errors
/// Returns [`ObjectError::ContextMoved`] when the context moved after registration.
pub fn try_push_root(ctx: &mut ThreadContext, value: &mut Word) -> Result<RootToken, ObjectError> {
    ctx.check_registered_address()?;
    Ok(push_root(ctx, value))
}

/// Pop a precise root after validating the context address.
///
/// # Errors
/// Returns [`ObjectError::ContextMoved`] when the context moved after registration.
pub fn try_pop_root(ctx: &mut ThreadContext, token: RootToken) -> Result<bool, ObjectError> {
    ctx.check_registered_address()?;
    Ok(pop_root(ctx, token))
}

pub fn with_root<T>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, &mut Word) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let token = try_push_root(ctx, value)?;
    let result = f(ctx, value);
    finish_root(ctx, token, result)
}

/// Pop a root and return the callback result.
///
/// The callback result is discarded if cleanup detects a moved context.
///
/// # Errors
/// Returns [`ObjectError::ContextMoved`] when the context moved after registration.
///
/// # Panics
/// Panics if the root token is not at the top of the root stack.
pub fn finish_root<T>(
    ctx: &mut ThreadContext,
    token: RootToken,
    result: Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    assert!(pop_root(ctx, token), "root token popped out of stack order");
    ctx.check_registered_address()?;
    result
}
