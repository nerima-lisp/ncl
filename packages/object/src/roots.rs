use crate::{ObjectError, Runtime, ThreadContext};
use core::cell::Cell;
use ncl_sys::{Heap, RootSlot, RootToken, Word};
use std::sync::Arc;

/// Push a precise root.
pub fn push_root(ctx: &mut ThreadContext, value: &mut Word) -> RootToken {
    ncl_sys::push_root(&mut ctx.thread, value)
}

/// Pop a precise root.
pub fn pop_root(ctx: &mut ThreadContext, token: RootToken) -> bool {
    ncl_sys::pop_root(&mut ctx.thread, token)
}

/// Push a precise root owned by the runtime's heap-level registry, independent
/// of the per-thread root stack used by [`push_root`]/[`pop_root`].
///
/// Compiling a form (e.g. `ncl-runtime`'s `Runtime::eval_form`) grows the
/// per-thread root stack: each compiled function's code object and
/// constants table stay rooted for the runtime's lifetime, via `push_root`
/// calls that are never popped. A caller that needs to keep a `Word` alive
/// *across* such a call therefore cannot use a `push_root`/`pop_root` pair on
/// the thread stack: compilation leaves newer roots above that token, so
/// `pop_root` cannot remove it.
/// Ordinary compilation does not touch the heap-level registry, so this API
/// can be used around a call that grows the thread root stack.
pub fn push_heap_root(runtime: &Runtime, value: &mut Word) -> RootToken {
    ncl_sys::push_heap_root(&runtime.heap, value)
}

/// Pop a precise root from the runtime's heap-level registry.
pub fn pop_heap_root(runtime: &Runtime, token: RootToken) -> bool {
    ncl_sys::pop_heap_root(&runtime.heap, token)
}

/// A stable slot registered as a root in the runtime's heap-level registry.
///
/// The registry holds a pointer to the slot until `release` or `Drop` pops its
/// root token.
#[derive(Debug)]
pub struct HeapRootedWord {
    heap: Arc<Heap>,
    slot: Box<Cell<Word>>,
    token: Option<RootToken>,
}

impl HeapRootedWord {
    pub fn new(ctx: &ThreadContext, value: Word) -> Result<Self, ObjectError> {
        let heap = Arc::clone(ctx.heap().ok_or(ObjectError::Layout)?);
        let mut slot = Box::new(Cell::new(value));
        let token = ctx.push_heap_root(slot.get_mut())?;
        Ok(Self {
            heap,
            slot,
            token: Some(token),
        })
    }

    pub fn get(&self) -> Word {
        self.slot.get()
    }

    pub fn release(mut self) -> Result<(), ObjectError> {
        let Some(token) = self.token else {
            return Err(ObjectError::RootStackCorrupted);
        };
        if ncl_sys::pop_heap_root(&self.heap, token) {
            self.token = None;
            Ok(())
        } else {
            Err(ObjectError::RootStackCorrupted)
        }
    }
}

impl Drop for HeapRootedWord {
    fn drop(&mut self) {
        if let Some(token) = self.token.take() {
            let _ = ncl_sys::pop_heap_root(&self.heap, token);
        }
    }
}

/// Push a precise root and return its token.
///
/// # Errors
/// This wrapper preserves the fallible root API for callers that use it.
pub fn try_push_root(ctx: &mut ThreadContext, value: &mut Word) -> Result<RootToken, ObjectError> {
    Ok(push_root(ctx, value))
}

/// Pop a precise root and return whether the token was valid.
///
/// # Errors
/// This wrapper preserves the fallible root API for callers that use it.
///
/// An invalid token is returned as `Ok(false)`.
pub fn try_pop_root(ctx: &mut ThreadContext, token: RootToken) -> Result<bool, ObjectError> {
    Ok(pop_root(ctx, token))
}

/// Run a callback while keeping one word precisely rooted on the thread root stack.
///
/// # Errors
/// Returns the callback's object-layer error or a root-stack error.
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

/// Run a callback while keeping the given words rooted on the thread root stack.
///
/// # Errors
/// Returns a root-stack or callback error.
///
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
                    if !try_pop_root(ctx, token).is_ok_and(|popped| popped) {
                        return Err(ObjectError::RootStackCorrupted);
                    }
                }
                return Err(error);
            }
        }
    }
    let slots: Vec<RootSlot<'_>> = cells.iter().map(RootSlot::new).collect();
    let result = f(ctx, &slots);
    for token in tokens.into_iter().rev() {
        if !try_pop_root(ctx, token).is_ok_and(|popped| popped) {
            return Err(ObjectError::RootStackCorrupted);
        }
    }
    result
}

/// Run a callback with a contiguous mutable word slice registered as roots.
///
/// The slice allocation is kept alive and its elements remain mutable for the
/// complete callback, so a native caller may safely pass a pointer into it as
/// the rest-argument area.
///
/// # Errors
/// Returns an object-layer error from the callback or a root-stack error.
///
pub fn with_rooted_slice<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut rooted = values.to_vec();
    let token = ncl_sys::register_root_set(&mut ctx.thread, &mut rooted);
    let result = f(ctx, &mut rooted);
    if !ncl_sys::pop_root(&mut ctx.thread, token) {
        return Err(ObjectError::RootStackCorrupted);
    }
    result
}

/// Keep a mutable word slice rooted independently of the per-thread root stack.
///
/// Use this when the callback can compile code and grow that stack.
///
/// # Errors
/// Returns a callback or root-registry error.
pub fn with_heap_rooted_slice<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &mut [Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut rooted = values.to_vec();
    let mut tokens = Vec::with_capacity(rooted.len());
    for value in &mut rooted {
        match ctx.push_heap_root(value) {
            Ok(token) => tokens.push(token),
            Err(error) => {
                return match release_heap_roots(ctx, tokens) {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        }
    }
    let result = f(ctx, &mut rooted);
    release_heap_roots(ctx, tokens)?;
    result
}

fn release_heap_roots(ctx: &ThreadContext, tokens: Vec<RootToken>) -> Result<(), ObjectError> {
    for token in tokens.into_iter().rev() {
        if !ctx.pop_heap_root(token) {
            return Err(ObjectError::RootStackCorrupted);
        }
    }
    Ok(())
}

/// Pop a thread root and return the callback result.
///
pub fn finish_root<T>(
    ctx: &mut ThreadContext,
    token: RootToken,
    result: Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    if pop_root(ctx, token) {
        result
    } else {
        Err(ObjectError::RootStackCorrupted)
    }
}

#[cfg(test)]
mod tests {
    use super::{pop_root, push_root, with_heap_rooted_slice, with_root};
    use crate::{
        ObjectRef, Runtime, ThreadContext, classify_object, make_instance, make_string, slot_ref,
        string_length, string_ref,
    };
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

        let returned = with_root(&mut ctx, &mut word, |ctx, word| {
            ctx.collect(true)?;
            Ok(*word)
        })
        .unwrap_or_else(|error| panic!("collection failed: {error:?}"));

        assert!(matches!(
            classify_object(&ctx, returned),
            ObjectRef::String(_)
        ));
        assert_eq!(string_length(&ctx, returned), Ok(64));
        assert!((0..64).all(|index| { string_ref(&ctx, returned, index) == Ok('x') }));
    }

    #[test]
    fn with_heap_rooted_slice_survives_thread_root_growth_and_collection() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let value = make_string(&mut ctx, &runtime, &['x'; 64])
            .unwrap_or_else(|error| panic!("string allocation failed: {error:?}"));
        let mut stack_value = Word::fixnum(1);
        let mut stack_token = None;

        let returned = with_heap_rooted_slice(&mut ctx, &[value], |ctx, rooted| {
            stack_token = Some(ncl_sys::push_root(&mut ctx.thread, &mut stack_value));
            ctx.collect(true)?;
            Ok(rooted[0])
        })
        .unwrap_or_else(|error| panic!("collection failed: {error:?}"));

        assert_eq!(string_length(&ctx, returned), Ok(64));
        let stack_token = stack_token.unwrap_or_else(|| panic!("thread root was not pushed"));
        assert!(pop_root(&mut ctx, stack_token));
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
