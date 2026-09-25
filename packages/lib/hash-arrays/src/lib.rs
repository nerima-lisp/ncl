//! Builtin registration for arrays and hash tables.

pub mod array;
pub mod hash;

use ncl_object::{ObjectError, Runtime, ThreadContext};

/// Register the currently implemented hash-table and array builtins.
///
/// # Errors
///
/// Returns the first registration error from the object runtime.
pub fn register(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<(), ObjectError> {
    hash::register(ctx, runtime)?;
    array::register(ctx, runtime)
}
