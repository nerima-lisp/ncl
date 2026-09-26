//! Scoped, typed names for precise roots.
//!
//! A [`Handle`] is an index into a [`Scope`].  The scope owns the root slots,
//! so handles remain cheap to copy while the scope is alive.  [`Local`] is the
//! corresponding unrooted value used at API boundaries; it must not be kept
//! across an allocation unless it is first put in a scope.

use core::marker::PhantomData;
use ncl_sys::{RootToken, Word};

use crate::ThreadContext;

type Brand<'scope, T> = PhantomData<(core::cell::Cell<&'scope ()>, fn() -> T)>;

/// A value which is not registered as a GC root.
#[derive(Debug, Eq, PartialEq)]
pub struct Local<'scope, T = Word> {
    word: Word,
    marker: Brand<'scope, T>,
}

impl<T> Copy for Local<'_, T> {}

impl<T> Clone for Local<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Local<'_, T> {
    /// Construct a local from its ABI word.
    #[must_use]
    pub const fn from_word(word: Word) -> Self {
        Self {
            word,
            marker: PhantomData,
        }
    }

    /// Return the ABI word represented by this local.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.word
    }
}

impl<T> From<Word> for Local<'static, T> {
    fn from(word: Word) -> Self {
        Self::from_word(word)
    }
}

/// A typed, copyable reference to a slot owned by a [`Scope`].
#[derive(Debug, Eq, PartialEq)]
pub struct Handle<'scope, T = Word> {
    index: usize,
    marker: Brand<'scope, T>,
}

impl<T> Copy for Handle<'_, T> {}

impl<T> Clone for Handle<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Handle<'_, T> {
    /// Return the slot index.  This is intended for diagnostics and tests.
    #[must_use]
    pub const fn index(self) -> usize {
        self.index
    }
}

/// A collection of handles belonging to one scope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HandleVec<'scope, T = Word> {
    handles: Vec<Handle<'scope, T>>,
}

impl<'scope, T> HandleVec<'scope, T> {
    /// Return the number of handles.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.handles.len()
    }

    /// Return whether this collection has no handles.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.handles.is_empty()
    }

    /// Borrow the handles as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[Handle<'scope, T>] {
        &self.handles
    }

    /// Iterate over the handles.
    pub fn iter(&self) -> core::slice::Iter<'_, Handle<'scope, T>> {
        self.handles.iter()
    }

    /// Iterate over the handles by shared reference.
    pub fn into_iter(&self) -> core::slice::Iter<'_, Handle<'scope, T>> {
        self.iter()
    }
}

impl<'scope, T> HandleVec<'scope, T> {
    /// Append a newly rooted local value.
    pub fn push(&mut self, scope: &mut Scope<'scope>, value: Local<'_, T>) {
        self.handles.push(scope.root(value));
    }
}

/// Owns precise root slots for the duration of a Rust operation.
pub struct Scope<'ctx> {
    ctx: &'ctx mut ThreadContext,
    #[allow(clippy::vec_box)]
    slots: Vec<Box<core::cell::Cell<Word>>>,
    tokens: Vec<RootToken>,
}

impl core::fmt::Debug for Scope<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Scope")
            .field("root_count", &self.slots.len())
            .finish_non_exhaustive()
    }
}

impl<'ctx> Scope<'ctx> {
    /// Create an empty scope attached to a thread context.
    pub const fn new(ctx: &'ctx mut ThreadContext) -> Self {
        Self {
            ctx,
            slots: Vec::new(),
            tokens: Vec::new(),
        }
    }

    /// Collect the current thread while retaining every handle in this scope.
    ///
    /// # Errors
    /// Returns an error when the thread is not registered.
    pub fn collect(&mut self, full: bool) -> Result<(), crate::ObjectError> {
        self.ctx.collect(full)
    }

    /// Build a cons cell while keeping its arguments rooted in this scope.
    ///
    /// # Errors
    /// Returns the allocation error reported by the object heap.
    pub fn make_cons(
        &mut self,
        runtime: &crate::Runtime,
        car: Handle<'ctx>,
        cdr: Handle<'ctx>,
    ) -> Result<Handle<'ctx>, crate::ObjectError> {
        let first = car.get(self).as_word();
        let second = cdr.get(self).as_word();
        let result = crate::make_cons(self.ctx, runtime, first, second)?;
        Ok(self.root(Local::from_word(result)))
    }

    /// Root a word and return its typed handle.
    ///
    /// ```compile_fail
    /// # use ncl_object::{Runtime, Scope, ThreadContext, Word};
    /// # let runtime = match Runtime::new() { Ok(value) => value, Err(_) => return };
    /// # let mut ctx = ThreadContext::new();
    /// # if ctx.register(&runtime).is_err() { return; }
    /// # let mut scope = Scope::new(&mut ctx);
    /// let temporary = Word::NIL;
    /// scope.root(temporary); // a raw temporary is not a rooted Local
    /// ```
    pub fn root<T>(&mut self, value: Local<'_, T>) -> Handle<'ctx, T> {
        let local: Local<'_, T> = value;
        let slot = Box::new(core::cell::Cell::new(local.as_word()));
        let token = self.ctx.thread.push_root_cell(&slot);
        let index = self.slots.len();
        self.slots.push(slot);
        self.tokens.push(token);
        Handle {
            index,
            marker: PhantomData,
        }
    }

    /// Root every word in a slice.
    pub fn root_many<T>(&mut self, values: &[Local<'_, T>]) -> HandleVec<'ctx, T> {
        let handles = values
            .iter()
            .copied()
            .map(|value| self.root(value))
            .collect();
        HandleVec { handles }
    }

    /// Read a rooted value after an allocation or collection.
    #[must_use]
    pub fn get<T>(&self, handle: Handle<'ctx, T>) -> Local<'_, T> {
        let word = self
            .slots
            .get(handle.index)
            .map_or(Word::NIL, |slot| slot.get());
        Local::from_word(word)
    }

    /// Update a rooted value in its slot.
    pub fn set<T>(&mut self, handle: Handle<'ctx, T>, value: Local<'_, T>) {
        if let Some(slot) = self.slots.get(handle.index) {
            slot.set(value.as_word());
        }
    }

    /// Read all values represented by a handle vector.
    #[must_use]
    pub fn get_many<'borrow, T>(
        &'borrow self,
        handles: &HandleVec<'ctx, T>,
    ) -> Vec<Local<'borrow, T>> {
        handles.iter().map(|handle| self.get(*handle)).collect()
    }
}

impl<'scope, T> Handle<'scope, T> {
    /// Read the current value, borrowing the scope for the returned local.
    ///
    /// ```compile_fail
    /// # use ncl_object::{Local, Runtime, Scope, ThreadContext, Word};
    /// # let runtime = match Runtime::new() { Ok(value) => value, Err(_) => return };
    /// # let mut ctx = ThreadContext::new();
    /// # if ctx.register(&runtime).is_err() { return; }
    /// # let mut scope = Scope::new(&mut ctx);
    /// # let handle = scope.root(Local::from_word(Word::NIL));
    /// let local = handle.get(&scope);
    /// let _collection = scope.collect(true);
    /// let _ = local.as_word(); // the scope is still immutably borrowed
    /// ```
    #[must_use]
    pub fn get<'borrow>(self, scope: &'borrow Scope<'scope>) -> Local<'borrow, T> {
        scope.get(self)
    }
}

/// A child scope cannot coexist with use of its parent scope.
///
/// ```compile_fail
/// # use ncl_object::{Local, Runtime, Scope, ThreadContext, Word};
/// # let runtime = match Runtime::new() { Ok(value) => value, Err(_) => return };
/// # let mut ctx = ThreadContext::new();
/// # if ctx.register(&runtime).is_err() { return; }
/// # let mut parent = Scope::new(&mut ctx);
/// let child = parent.reborrow();
/// let _collection = parent.collect(true); // parent is borrowed by child
/// drop(child);
/// ```
impl Scope<'_> {
    /// Reborrow this scope as a nested child scope.
    pub const fn reborrow(&mut self) -> Scope<'_> {
        Scope::new(self.ctx)
    }
}

impl<'scope, T> IntoIterator for &'scope HandleVec<'scope, T> {
    type Item = &'scope Handle<'scope, T>;
    type IntoIter = core::slice::Iter<'scope, Handle<'scope, T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.handles.iter()
    }
}

impl Drop for Scope<'_> {
    fn drop(&mut self) {
        for token in self.tokens.drain(..).rev() {
            match ncl_sys::pop_root(&mut self.ctx.thread, token) {
                true | false => {}
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{HandleVec, Local, Scope};
    use crate::{Runtime, ThreadContext, Word, make_string};

    #[test]
    fn handles_read_the_forwarded_value_after_collection() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        let word = make_string(&mut ctx, &runtime, &['x'; 64]).expect("string");
        let mut scope = Scope::new(&mut ctx);
        let handle = scope.root::<crate::StringObject>(Local::from_word(word));
        scope.collect(true).expect("collection");
        assert_ne!(word, scope.get(handle).as_word());
    }

    #[test]
    fn vectors_keep_order_and_can_be_updated() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        let mut scope = Scope::new(&mut ctx);
        let handles = scope.root_many::<Word>(&[
            Local::from_word(Word::fixnum(1)),
            Local::from_word(Word::fixnum(2)),
        ]);
        assert_eq!(
            scope
                .get_many(&handles)
                .into_iter()
                .map(Local::as_word)
                .collect::<Vec<_>>(),
            vec![Word::fixnum(1), Word::fixnum(2)]
        );
        let second = handles.iter().nth(1).copied().expect("second handle");
        scope.set(second, Local::from_word(Word::fixnum(3)));
        assert_eq!(scope.get(second).as_word(), Word::fixnum(3));
    }

    #[test]
    fn handle_vec_accumulates_under_gc_stress_and_strict_forwarding() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);
        let mut scope = Scope::new(&mut ctx);
        let mut values: HandleVec<'_, Word> = HandleVec {
            handles: Vec::new(),
        };
        for _ in 0..40 {
            let word = crate::make_string(scope.ctx, &runtime, &['x'; 8]).expect("string");
            values.push(&mut scope, Local::from_word(word));
        }
        scope.collect(true).expect("collection");
        assert_eq!(values.len(), 40);
        assert!(values.iter().all(|handle| {
            let word = scope.get(*handle).as_word();
            matches!(
                crate::classify_object(scope.ctx, word),
                crate::ObjectRef::String(_)
            )
        }));
    }

    #[test]
    fn list_construction_keeps_handle_arguments_rooted() {
        let runtime = Runtime::new().expect("runtime");
        let mut ctx = ThreadContext::new();
        ctx.register(&runtime).expect("register");
        ctx.set_gc_stress(true);
        ctx.set_strict_forwarding(true);
        let mut scope = Scope::new(&mut ctx);
        let car = scope.root(Local::from_word(Word::fixnum(1)));
        let cdr = scope.root(Local::from_word(Word::NIL));
        let cons = scope.make_cons(&runtime, car, cdr).expect("cons");
        scope.collect(true).expect("collection");
        let cons_word = cons.get(&scope).as_word();
        assert_eq!(crate::car(scope.ctx, cons_word), Ok(Word::fixnum(1)));
    }
}
