use crate::{ObjectError, Runtime, ThreadContext};
use core::cell::Cell;
use ncl_sys::{Heap, RootSlot, RootToken, Word};
use std::rc::Rc;
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

/// Remove a precise root from the runtime's heap-level registry by its token.
///
/// # Errors
/// Returns [`ObjectError::RootStackCorrupted`] when the token is invalid.
pub fn unregister_heap_root(runtime: &Runtime, token: RootToken) -> Result<(), ObjectError> {
    ncl_sys::unregister_heap_root(&runtime.heap, token).map_err(|_| ObjectError::RootStackCorrupted)
}

/// A stable slot registered as a root in the runtime's heap-level registry.
///
/// The registry holds a pointer to the slot until `release` or `Drop` removes
/// its root token. `Drop` works even when newer roots remain.
#[derive(Debug)]
pub struct HeapRootedWord {
    heap: Arc<Heap>,
    slot: Rc<Cell<Word>>,
    token: Option<RootToken>,
}

impl HeapRootedWord {
    /// Register a word in the heap-level root registry.
    ///
    /// # Errors
    /// Returns `ObjectError::Layout` when the context is not registered.
    pub fn new(ctx: &ThreadContext, value: Word) -> Result<Self, ObjectError> {
        let heap = Arc::clone(ctx.heap().ok_or(ObjectError::Layout)?);
        let slot = Rc::new(Cell::new(value));
        let token = ctx.push_heap_root_cell(&slot)?;
        Ok(Self {
            heap,
            slot,
            token: Some(token),
        })
    }

    #[must_use]
    pub fn get(&self) -> Word {
        self.slot.get()
    }

    pub fn set(&self, value: Word) {
        self.slot.set(value);
    }

    /// Remove this word from the heap-level root registry.
    ///
    /// # Errors
    /// Returns `ObjectError::RootStackCorrupted` when the token is invalid.
    pub fn release(mut self) -> Result<(), ObjectError> {
        let Some(token) = self.token else {
            return Err(ObjectError::RootStackCorrupted);
        };
        self.heap
            .unregister_root(token)
            .map_err(|_| ObjectError::RootStackCorrupted)?;
        self.token = None;
        Ok(())
    }
}

impl Drop for HeapRootedWord {
    fn drop(&mut self) {
        if let Some(token) = self.token.take()
            && self.heap.unregister_root(token).is_err()
        {
            // Keep the slot alive if the registry still holds its pointer.
            std::mem::forget(Rc::clone(&self.slot));
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

/// Keep words rooted independently of the per-thread root stack.
///
/// The callback receives handles whose values it can read and update while
/// compilation or collection moves the underlying objects.
///
/// # Errors
/// Returns a callback or root-registry error.
pub fn with_heap_rooted_slice<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &[HeapRootedWord]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let rooted = values
        .iter()
        .copied()
        .map(|value| HeapRootedWord::new(ctx, value))
        .collect::<Result<Vec<_>, _>>()?;
    let result = f(ctx, &rooted);
    for root in rooted.into_iter().rev() {
        root.release()?;
    }
    result
}

/// Run a callback with a contiguous mutable slice held in the heap root registry.
///
/// # Errors
/// Returns the callback error or `ObjectError::RootStackCorrupted` if a root cannot be removed.
pub fn with_heap_rooted_mut_slice<T>(
    runtime: &crate::Runtime,
    values: &[Word],
    f: impl FnOnce(&mut [Word]) -> Result<T, ObjectError>,
) -> Result<T, ObjectError> {
    let mut rooted = HeapRootedMutSlice {
        heap: Arc::clone(&runtime.heap),
        values: values.to_vec(),
        tokens: Vec::with_capacity(values.len()),
    };
    for value in &mut rooted.values {
        rooted
            .tokens
            .push(ncl_sys::push_heap_root(&rooted.heap, value));
    }
    let result = f(&mut rooted.values);
    if !rooted.unregister_all() {
        rooted.preserve_failed_roots();
        return Err(ObjectError::RootStackCorrupted);
    }
    result
}

struct HeapRootedMutSlice {
    heap: Arc<Heap>,
    values: Vec<Word>,
    tokens: Vec<RootToken>,
}

impl HeapRootedMutSlice {
    fn unregister_all(&mut self) -> bool {
        let mut valid = true;
        for token in self.tokens.drain(..).rev() {
            if self.heap.unregister_root(token).is_err() {
                valid = false;
            }
        }
        valid
    }

    fn preserve_failed_roots(&mut self) {
        std::mem::forget(std::mem::take(&mut self.values));
    }
}

impl Drop for HeapRootedMutSlice {
    fn drop(&mut self) {
        if !self.unregister_all() {
            self.preserve_failed_roots();
        }
    }
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
    use super::{HeapRootedWord, pop_root, push_root, with_heap_rooted_slice, with_root};
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
            Ok(rooted[0].get())
        })
        .unwrap_or_else(|error| panic!("collection failed: {error:?}"));

        assert_eq!(string_length(&ctx, returned), Ok(64));
        let stack_token = stack_token.unwrap_or_else(|| panic!("thread root was not pushed"));
        assert!(pop_root(&mut ctx, stack_token));
    }

    #[test]
    fn with_heap_rooted_slice_releases_its_roots_out_of_order() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let mut later_root = None;

        let result = with_heap_rooted_slice(&mut ctx, &[Word::fixnum(1)], |ctx, rooted| {
            later_root = Some(HeapRootedWord::new(ctx, Word::fixnum(2))?);
            Ok(rooted[0].get())
        })
        .unwrap_or_else(|error| panic!("rooted slice cleanup failed: {error:?}"));

        assert_eq!(result, Word::fixnum(1));
        assert_eq!(
            later_root.as_ref().map(HeapRootedWord::get),
            Some(Word::fixnum(2))
        );
        drop(later_root);
    }

    #[test]
    fn with_heap_rooted_slice_releases_roots_during_unwind() {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));

        let result = catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), crate::ObjectError> =
                with_heap_rooted_slice(&mut ctx, &[Word::TRUE], |_, _| panic!("callback panic"));
        }));

        assert!(result.is_err());
        ctx.collect(true)
            .unwrap_or_else(|error| panic!("collection after unwind failed: {error:?}"));
    }

    #[test]
    fn out_of_order_heap_root_drop_unregisters_slot_before_collection() {
        let runtime =
            Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime)
            .unwrap_or_else(|error| panic!("register failed: {error:?}"));
        let value = make_string(&mut ctx, &runtime, &['x'; 64])
            .unwrap_or_else(|error| panic!("string allocation failed: {error:?}"));
        let first = HeapRootedWord::new(&ctx, value)
            .unwrap_or_else(|error| panic!("heap root failed: {error:?}"));
        let second = HeapRootedWord::new(&ctx, Word::fixnum(2))
            .unwrap_or_else(|error| panic!("second heap root failed: {error:?}"));

        drop(first);

        ctx.collect(true)
            .unwrap_or_else(|error| panic!("collection failed: {error:?}"));
        assert_eq!(second.get(), Word::fixnum(2));
        drop(second);
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
