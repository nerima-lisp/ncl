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

    /// Borrow the underlying context for non-allocating object operations.
    ///
    /// Allocation should use the typed methods on this scope so that values
    /// which survive the operation remain represented by handles.
    #[must_use]
    pub const fn context(&self) -> &ThreadContext {
        self.ctx
    }

    /// Borrow the underlying context for an object operation that mutates an
    /// existing object without allocating a new rooted value.
    pub const fn context_mut(&mut self) -> &mut ThreadContext {
        self.ctx
    }

    /// Allocate a string and retain the result in this scope.
    ///
    /// # Errors
    /// Returns the allocation or layout error reported by the object layer.
    pub fn make_string(
        &mut self,
        runtime: &crate::Runtime,
        values: &[char],
    ) -> Result<Handle<'ctx>, crate::ObjectError> {
        let word = crate::make_string(self.ctx, runtime, values)?;
        Ok(self.root(Local::from_word(word)))
    }

    /// Allocate a simple vector from handles and retain the result in this
    /// scope.
    ///
    /// # Errors
    /// Returns the allocation or layout error reported by the object layer.
    pub fn make_simple_vector(
        &mut self,
        runtime: &crate::Runtime,
        values: &HandleVec<'ctx>,
    ) -> Result<Handle<'ctx>, crate::ObjectError> {
        let words = values
            .iter()
            .map(|handle| self.get(*handle).as_word())
            .collect::<Vec<_>>();
        let word = crate::make_simple_vector(self.ctx, runtime, &words)?;
        Ok(self.root(Local::from_word(word)))
    }

    /// Read a cons car through a handle.
    ///
    /// # Errors
    /// Returns a type or storage error when the handle does not contain a cons.
    pub fn car<'borrow>(
        &'borrow self,
        value: Handle<'ctx>,
    ) -> Result<Local<'borrow>, crate::ObjectError> {
        crate::car(self.ctx, self.get(value).as_word()).map(Local::from_word)
    }

    /// Read a cons cdr through a handle.
    ///
    /// # Errors
    /// Returns a type or storage error when the handle does not contain a cons.
    pub fn cdr<'borrow>(
        &'borrow self,
        value: Handle<'ctx>,
    ) -> Result<Local<'borrow>, crate::ObjectError> {
        crate::cdr(self.ctx, self.get(value).as_word()).map(Local::from_word)
    }

    /// Call a Lisp function represented by a rooted handle.
    ///
    /// The temporary ABI words are backed by handles for the whole call, and
    /// the returned value is rooted before this method returns.
    ///
    /// # Errors
    /// Returns an object or callback error from function designator conversion
    /// or invocation.
    pub fn call_function<C: crate::FunctionCaller>(
        &mut self,
        runtime: &crate::Runtime,
        designator: Handle<'ctx>,
        args: &HandleVec<'ctx>,
        caller: &mut C,
        values: &mut crate::MultipleValues,
    ) -> Result<Handle<'ctx>, crate::ObjectError> {
        let designator_word = designator.get(self).as_word();
        let function = crate::FunctionDesignator::try_from_word(self.ctx, designator_word)?;
        let words = args
            .iter()
            .map(|handle| handle.get(self).as_word())
            .collect::<Vec<_>>();
        let result = caller.call_function(
            self.ctx,
            runtime,
            function,
            crate::FunctionArguments::new(&words),
            values,
        )?;
        Ok(self.root(Local::from_word(result)))
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

    /// Walk a proper Lisp list and root every element.
    ///
    /// # Errors
    /// Returns `ObjectError::TypeError` when `list` is not a proper list.
    pub fn list_to_handle_vec(
        &mut self,
        list: Local<'_>,
    ) -> Result<HandleVec<'ctx>, crate::ObjectError> {
        let mut words = Vec::new();
        let mut cursor = list.as_word();
        while cursor != Word::NIL {
            if !cursor.is_cons() {
                return Err(crate::ObjectError::TypeError);
            }
            words.push(crate::car(self.ctx, cursor)?);
            cursor = crate::cdr(self.ctx, cursor)?;
        }
        Ok(self.root_many(&words.into_iter().map(Local::from_word).collect::<Vec<_>>()))
    }

    /// Build a proper Lisp list from rooted handles.
    ///
    /// # Errors
    /// Returns the allocation error reported by the object heap.
    pub fn make_list(
        &mut self,
        runtime: &crate::Runtime,
        values: &HandleVec<'ctx>,
    ) -> Result<Handle<'ctx>, crate::ObjectError> {
        let mut result = self.root(Local::from_word(Word::NIL));
        for handle in values.as_slice().iter().rev() {
            result = self.make_cons(runtime, *handle, result)?;
        }
        Ok(result)
    }

    /// Intern a symbol by name in `package` and root the result.
    ///
    /// # Errors
    /// Returns the package or allocation error reported by the object layer.
    pub fn intern(
        &mut self,
        runtime: &crate::Runtime,
        package: &str,
        name: &str,
    ) -> Result<Handle<'ctx>, crate::ObjectError> {
        let package_word = runtime.ensure_package(self.ctx, package)?;
        let (symbol, _) =
            crate::Package::from_word(package_word).intern(self.ctx, runtime, name)?;
        Ok(self.root(Local::from_word(symbol)))
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
    include!("scope_tests.rs");
}
