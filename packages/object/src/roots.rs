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

/// Push a precise root and return its token.
pub fn try_push_root(ctx: &mut ThreadContext, value: &mut Word) -> Result<RootToken, ObjectError> {
    Ok(push_root(ctx, value))
}

/// Pop a precise root and return whether the token was valid.
pub fn try_pop_root(ctx: &mut ThreadContext, token: RootToken) -> Result<bool, ObjectError> {
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

pub fn with_roots<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &[Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut rooted_values = values.to_vec();
    let mut tokens = Vec::with_capacity(rooted_values.len());
    for value in &mut rooted_values {
        match try_push_root(ctx, value) {
            Ok(token) => tokens.push(token),
            Err(error) => {
                for token in tokens.into_iter().rev() {
                    let _ = try_pop_root(ctx, token);
                }
                return Err(error);
            }
        }
    }
    let result = f(ctx, &rooted_values);
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
            slot_ref(&ctx, crate::Instance::from(instance_word), 0),
            Ok(Word::fixnum(7))
        );
        assert!(pop_root(&mut ctx, token));
    }
}
