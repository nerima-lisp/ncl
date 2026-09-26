use crate::{ObjectError, ThreadContext};
use core::cell::Cell;
use ncl_sys::{RootSlot, RootToken, Word};

/// Push a precise root.
pub fn push_root(ctx: &mut ThreadContext, value: &mut Word) -> RootToken {
    ncl_sys::push_root(&mut ctx.thread, value)
}

/// Pop a precise root.
pub fn pop_root(ctx: &mut ThreadContext, token: RootToken) -> bool {
    ncl_sys::pop_root(&mut ctx.thread, token)
}

/// Push a precise root and return its token.
///
/// # Errors
/// This compatibility wrapper currently always returns a token.
pub fn try_push_root(ctx: &mut ThreadContext, value: &mut Word) -> Result<RootToken, ObjectError> {
    Ok(push_root(ctx, value))
}

/// Pop a precise root and return whether the token was valid.
///
/// # Errors
/// This compatibility wrapper currently always returns the pop result.
pub fn try_pop_root(ctx: &mut ThreadContext, token: RootToken) -> Result<bool, ObjectError> {
    Ok(pop_root(ctx, token))
}

/// Push one precise root while running a callback.
///
/// The callback receives a handle that reads the collector-updated value after
/// each allocation.
///
/// # Errors
/// Returns an object error when root registration or the callback fails.
pub fn with_root<T>(
    ctx: &mut ThreadContext,
    value: &mut Word,
    f: impl FnOnce(&mut ThreadContext, RootSlot<'_>) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    // The root slot is interior-mutable so the optimizer cannot assume the
    // collection preserves the value: the collector rewrites registered slots
    // in place, and a plain `Word` behind `&mut` is otherwise treated as
    // unmodified across the call that performs the collection.
    let mut slot = Cell::new(*value);
    let token = try_push_root(ctx, slot.get_mut())?;
    let result = f(ctx, RootSlot::new(&slot));
    *value = slot.get();
    finish_root(ctx, token, result)
}

/// Push several precise roots while running a callback.
///
/// The callback receives handles that read the collector-updated values after
/// each allocation.
///
/// # Errors
/// Returns an object error when a root cannot be registered or the callback
/// returns one.
///
/// # Panics
/// Panics if root cleanup detects a corrupted root stack.
pub fn with_roots<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &[RootSlot<'_>]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut cells: Vec<Cell<Word>> = values.iter().copied().map(Cell::new).collect();
    let mut tokens = Vec::with_capacity(cells.len());
    for cell in &mut cells {
        match try_push_root(ctx, cell.get_mut()) {
            Ok(token) => tokens.push(token),
            Err(error) => {
                for token in tokens.into_iter().rev() {
                    assert!(try_pop_root(ctx, token).is_ok_and(|popped| popped));
                }
                return Err(error);
            }
        }
    }
    let slots: Vec<RootSlot<'_>> = cells.iter().map(RootSlot::new).collect();
    let result = f(ctx, &slots);
    for token in tokens.into_iter().rev() {
        assert!(try_pop_root(ctx, token).unwrap_or(false));
    }
    result
}

/// Pop a root and return the callback result.
///
/// # Panics
/// Panics if the root token is not at the top of the root stack.
pub fn finish_root<T>(
    ctx: &mut ThreadContext,
    token: RootToken,
    result: Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    assert!(pop_root(ctx, token), "root token popped out of stack order");
    result
}

#[cfg(test)]
mod tests {
    use super::{pop_root, push_root, with_root};
    use crate::{Runtime, ThreadContext, make_instance, make_string, slot_ref};
    use ncl_sys::Word;

    #[test]
    fn with_root_reads_the_moved_word_after_collection() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let mut word = make_string(&mut ctx, &runtime, &['x'; 64])
            .unwrap_or_else(|error| panic!("string allocation failed: {error:?}"));
        let before = word;

        let returned = with_root(&mut ctx, &mut word, |ctx, word| {
            ctx.collect(true)?;
            Ok(*word)
        })
        .unwrap_or_else(|error| panic!("collection failed: {error:?}"));

        assert_eq!(returned, word);
        assert_ne!(before, word);
    }

    #[test]
    fn instance_slot_vector_survives_collection() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let instance = make_instance(&mut ctx, &runtime, Word::NIL, &[Word::fixnum(7)])
            .unwrap_or_else(|error| panic!("instance allocation failed: {error:?}"));
        let mut instance_word = instance.as_word();
        let token = push_root(&mut ctx, &mut instance_word);
        ctx.collect(true)
            .unwrap_or_else(|error| panic!("collection failed: {error:?}"));
        assert_eq!(
            slot_ref(&ctx, crate::Instance::from_word(instance_word), 0),
            Ok(Word::fixnum(7))
        );
        assert!(pop_root(&mut ctx, token));
    }
}
