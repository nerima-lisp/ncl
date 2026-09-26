//! Per-mutator thread registration and control state.

use crate::{LispError, ObjectError, Runtime};
use ncl_sys::{StorageCondition, Thread, Word};

#[derive(Debug)]
pub struct ThreadContext {
    pub(crate) thread: Box<Thread>,
    pub(crate) registered: bool,
    bindings: Vec<(u32, Word)>,
    values: Vec<Word>,
    pending: Option<ObjectError>,
    pending_lisp_error: Option<LispError>,
    pending_condition: Option<Word>,
    pub(crate) non_local_exit: bool,
    pub(crate) handler: Option<usize>,
    pub(crate) cleanup: Option<usize>,
    pub(crate) catch: Option<usize>,
    pub(crate) gc_stress: bool,
}
impl ThreadContext {
    /// Create an unregistered context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            thread: Box::new(Thread::new()),
            registered: false,
            bindings: Vec::new(),
            values: Vec::new(),
            pending: None,
            pending_lisp_error: None,
            pending_condition: None,
            non_local_exit: false,
            handler: None,
            cleanup: None,
            catch: None,
            gc_stress: false,
        }
    }
    /// Register this context with a runtime.
    ///
    /// The thread state is heap allocated, so moving this context after registration is safe.
    /// The `runtime` must outlive every registered context: dropping it while a context is
    /// still registered would leave the thread's heap reference dangling.
    ///
    /// # Errors
    ///
    /// Returns the storage condition reported by the heap.
    pub fn register(&mut self, runtime: &Runtime) -> Result<(), ObjectError> {
        ncl_sys::register_thread(&runtime.heap, &mut self.thread).map_err(ObjectError::from)?;
        self.registered = true;
        self.ensure_standard_packages(runtime)
    }
    pub(crate) fn ensure_standard_packages(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), ObjectError> {
        for name in ["COMMON-LISP", "COMMON-LISP-USER", "KEYWORD", "NCL"] {
            runtime.ensure_package(self, name)?;
        }
        let common_lisp = runtime
            .find_package(self, "COMMON-LISP")
            .ok_or(ObjectError::Layout)?;
        let common_lisp_user = runtime
            .find_package(self, "COMMON-LISP-USER")
            .ok_or(ObjectError::Layout)?;
        crate::Package::from_word(common_lisp_user).use_package(self, runtime, common_lisp)?;
        Ok(())
    }
    /// Bind a special variable, preserving stack order.
    pub fn bind(&mut self, index: u32, value: Word) {
        self.bindings.push((index, value));
    }
    /// Remove the latest binding for an index.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectError::Unbound`] when no binding exists.
    pub fn unbind(&mut self, index: u32) -> Result<Word, ObjectError> {
        if let Some(position) = self.bindings.iter().rposition(|(key, _)| *key == index) {
            Ok(self.bindings.remove(position).1)
        } else {
            Err(ObjectError::Unbound)
        }
    }
    /// Set multiple values.
    pub fn set_values(&mut self, values: &[Word]) {
        self.values.clear();
        self.values.extend_from_slice(values);
    }
    /// Read multiple values.
    #[must_use]
    pub fn values(&self) -> &[Word] {
        &self.values
    }
    /// Record a pending condition.
    pub const fn set_pending(&mut self, error: ObjectError) {
        self.pending = Some(error);
    }
    /// Take the pending condition.
    pub const fn take_pending(&mut self) -> Option<ObjectError> {
        self.pending.take()
    }
    pub const fn set_pending_lisp_error(&mut self, error: LispError) {
        self.pending_lisp_error = Some(error);
    }
    /// Take the pending typed Lisp condition, if one was recorded.
    pub const fn take_pending_lisp_error(&mut self) -> Option<LispError> {
        self.pending_lisp_error.take()
    }
    /// Store the condition object produced for the latest typed builtin error.
    pub const fn set_pending_condition(&mut self, condition: Word) {
        self.pending_condition = Some(condition);
    }
    /// Take the pending condition object, if one was produced at the builtin boundary.
    pub const fn take_pending_condition(&mut self) -> Option<Word> {
        self.pending_condition.take()
    }
    /// Run a collection for this registered context.
    ///
    /// # Errors
    /// Returns a storage error if this context is not registered.
    pub fn collect(&mut self, full: bool) -> Result<(), ObjectError> {
        self.require_registered()?;
        ncl_sys::collect(&mut self.thread, full);
        Ok(())
    }
    /// Force a full collection before every object allocation when enabled.
    pub const fn set_gc_stress(&mut self, on: bool) {
        self.gc_stress = on;
    }
    /// Configure strict stale-word checking for this context's heap.
    pub fn set_strict_forwarding(&self, on: bool) {
        ncl_sys::set_strict_forwarding(&self.thread, on);
    }
    /// Return the stable thread pointer used by generated code.
    pub fn thread_mut(&mut self) -> &mut Thread {
        &mut self.thread
    }
    /// Mark an object as weak with the requested policy.
    ///
    /// This low-level operation does not validate registration or context movement.
    #[must_use]
    pub fn make_weak(&self, value: Word, weakness: ncl_sys::Weakness) -> Word {
        ncl_sys::make_weak(&self.thread, value, weakness)
    }
    /// Read the value slot of a weak object.
    ///
    /// This low-level operation does not validate registration or context movement.
    #[must_use]
    pub fn weak_value(&self, value: Word) -> Word {
        ncl_sys::weak_value(&self.thread, value)
    }
    pub(crate) const fn require_registered(&self) -> Result<(), ObjectError> {
        if !self.registered {
            return Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered));
        }
        Ok(())
    }
}
/// Unregister the context from its heap.
///
/// `ncl_sys::unregister_thread` resolves the heap through the `Thread`'s stored
/// heap reference, so the `Runtime` that owns the heap must outlive every
/// registered `ThreadContext`. Dropping a `Runtime` while a registered context
/// is still alive would dereference freed heap.
impl Drop for ThreadContext {
    fn drop(&mut self) {
        if self.registered {
            ncl_sys::unregister_thread(&self.thread);
            self.registered = false;
        }
    }
}
impl Default for ThreadContext {
    fn default() -> Self {
        Self::new()
    }
}
