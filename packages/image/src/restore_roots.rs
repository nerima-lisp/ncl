//! Root registration used while and after restoring an image.

use ncl_object::{ThreadContext, Word, pop_root, push_root};
use ncl_sys::RootToken;

use crate::error::ImageError;

/// Keep every object slot rooted while the reconstruction passes run.
pub(crate) fn with_slots<T>(
    ctx: &mut ThreadContext,
    count: usize,
    restore: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, ImageError>,
) -> Result<T, ImageError> {
    let mut slots = vec![Word::NIL; count];
    let mut tokens = Vec::with_capacity(count);
    for slot in &mut slots { tokens.push(push_root(ctx, slot)); }
    let result = restore(ctx, &mut slots);
    for token in tokens.into_iter().rev() { let _ = pop_root(ctx, token); }
    result
}

/// Register the final root slice owned by a loaded image.
pub(crate) fn register(ctx: &mut ThreadContext, roots: &mut [Word]) -> RootToken {
    ncl_sys::register_root_set(ctx.thread_mut(), roots)
}
