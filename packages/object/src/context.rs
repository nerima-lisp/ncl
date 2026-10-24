//! Per-mutator thread registration and control state.

use crate::{LispError, ObjectError, Runtime};
use core::cell::Cell;
use ncl_sys::{
    Heap, RootToken, RuntimeErrorPayloadKind, RuntimeErrorPayloadSlot, RuntimeErrorSource,
    StorageCondition, Thread, Word,
};
use std::sync::Arc;

impl RuntimeErrorPayloadKind for ObjectError {
    const SOURCE: RuntimeErrorSource = RuntimeErrorSource::Object;
}

impl RuntimeErrorPayloadKind for LispError {
    const SOURCE: RuntimeErrorSource = RuntimeErrorSource::Lisp;
}

struct RuntimeErrorPayloadCapability<'a, T: RuntimeErrorPayloadKind> {
    thread: &'a mut Thread,
    payload: &'a mut RuntimeErrorPayloadSlot<T>,
}

impl<T: RuntimeErrorPayloadKind> RuntimeErrorPayloadCapability<'_, T> {
    const fn record(self, payload: T) {
        self.thread
            .record_runtime_error_payload(self.payload, payload);
    }

    const fn take(self) -> Option<T> {
        self.thread.take_runtime_error_payload(self.payload)
    }
}

/// Typed runtime errors taken when generated code returns to its Rust caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingRuntimeErrors {
    /// Object-layer runtime error.
    pub object: Option<ObjectError>,
    /// Typed Lisp condition.
    pub lisp: Option<LispError>,
    /// Direct native runtime error.
    pub native: Option<ncl_sys::NativeError>,
}

pub type ConditionHandlerInvoker =
    fn(std::ptr::NonNull<()>, &mut ThreadContext, Word, &[Word]) -> Result<Word, ObjectError>;

#[derive(Debug)]
pub struct ThreadContext {
    pub(crate) thread: Box<Thread>,
    pub(crate) registered: bool,
    heap: Option<Arc<Heap>>,
    bindings: Vec<(u32, Word)>,
    values: Vec<Word>,
    pending: RuntimeErrorPayloadSlot<ObjectError>,
    pending_lisp_error: RuntimeErrorPayloadSlot<LispError>,
    pending_condition: Option<Word>,
    pub(crate) handler: Option<usize>,
    pub(crate) cleanup: Option<usize>,
    pub(crate) catch: Option<usize>,
    condition_handler_head: Box<Cell<Word>>,
    condition_handler_root: Option<RootToken>,
    condition_handler_invoker: Option<ConditionHandlerInvoker>,
    pub(crate) gc_stress: bool,
    /// Active `catch`/`unwind-protect`/`progv` dynamic-extent frames,
    /// innermost last. See [`crate::nonlocal`].
    pub(crate) frames: Vec<crate::nonlocal::DynamicFrame>,
    pub(crate) pending_unwind: Vec<crate::nonlocal::PendingExit>,
    evaluator_runtime: Option<std::ptr::NonNull<()>>,
}
impl ThreadContext {
    /// Create an unregistered context.
    #[must_use]
    pub fn new() -> Self {
        Self {
            thread: Box::new(Thread::new()),
            registered: false,
            heap: None,
            bindings: Vec::new(),
            values: Vec::new(),
            pending: RuntimeErrorPayloadSlot::new(),
            pending_lisp_error: RuntimeErrorPayloadSlot::new(),
            pending_condition: None,
            handler: None,
            cleanup: None,
            catch: None,
            condition_handler_head: Box::new(Cell::new(Word::NIL)),
            condition_handler_root: None,
            condition_handler_invoker: None,
            gc_stress: false,
            frames: Vec::new(),
            pending_unwind: Vec::new(),
            evaluator_runtime: None,
        }
    }

    /// Associate this context with its owning evaluator for the duration of an evaluation.
    pub const fn set_evaluator_runtime(&mut self, runtime: *mut ()) {
        self.evaluator_runtime = std::ptr::NonNull::new(runtime);
    }

    /// Clear the evaluator association after an outer evaluation returns.
    pub const fn clear_evaluator_runtime(&mut self) {
        self.evaluator_runtime = None;
    }

    #[must_use]
    pub const fn evaluator_runtime(&self) -> Option<std::ptr::NonNull<()>> {
        self.evaluator_runtime
    }
    /// Register this context with a runtime.
    ///
    /// The thread state is heap allocated, so moving this context after registration is safe.
    /// The `runtime` must outlive every registered context so its rooted tables remain valid
    /// while the context can collect from the shared heap.
    ///
    /// # Errors
    ///
    /// Returns the storage condition reported by the heap.
    pub fn register(&mut self, runtime: &Runtime) -> Result<(), ObjectError> {
        ncl_sys::register_thread(&runtime.heap, &mut self.thread).map_err(ObjectError::from)?;
        self.registered = true;
        self.heap = Some(Arc::clone(&runtime.heap));
        self.condition_handler_root =
            Some(self.thread.push_root_cell(&self.condition_handler_head));
        self.ensure_standard_packages(runtime)
    }
    pub(crate) fn push_heap_root_cell(&self, value: &Cell<Word>) -> Result<RootToken, ObjectError> {
        let Some(heap) = self.heap.as_ref() else {
            return Err(ObjectError::Layout);
        };
        Ok(ncl_sys::push_heap_root_cell(heap, value))
    }
    pub(crate) const fn heap(&self) -> Option<&Arc<Heap>> {
        self.heap.as_ref()
    }
    pub(crate) fn ensure_standard_packages(
        &mut self,
        runtime: &Runtime,
    ) -> Result<(), ObjectError> {
        // "NCL-NLX" homes the compiler-generated tokens `return-from`/`go`
        // desugar to when they escape into a closure (see
        // `ncl_compiler_front`'s `Context::token`, which interns
        // `NCL-NLX:BLOCK:<name>`/`NCL-NLX:TAG:<name>` symbols compiled code
        // then resolves as ordinary `catch` tags); it must exist before any
        // such form compiles.
        for name in [
            "COMMON-LISP",
            "COMMON-LISP-USER",
            "KEYWORD",
            "NCL",
            "NCL-NLX",
            "SB-EXT",
            "SB-THREAD",
            "SB-ALIEN",
            "SB-SYS",
            "SB-MOP",
            "SB-GRAY",
            "SB-UNICODE",
            "SB-DEBUG",
            "SB-ALIEN-INTERNALS",
            "SB-C",
            "SB-IMPL",
            "SB-INT",
            "SB-KERNEL",
            "SB-PCL",
            "SB-UNIX",
            "SB-VM",
        ] {
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
    /// Publish native return values into the thread's multiple-value area.
    ///
    /// # Errors
    /// Returns [`ObjectError::Layout`] when the result exceeds the native ABI area.
    pub fn set_multiple_value_area(&mut self, values: &[Word]) -> Result<(), ObjectError> {
        if values.len() > ncl_sys::MULTIPLE_VALUE_AREA_WORDS {
            return Err(ObjectError::Layout);
        }
        self.thread.set_multiple_value_area(values);
        Ok(())
    }
    /// Read multiple values.
    #[must_use]
    pub fn values(&self) -> &[Word] {
        &self.values
    }
    /// Record a pending condition.
    pub const fn set_pending(&mut self, error: ObjectError) {
        RuntimeErrorPayloadCapability {
            thread: &mut self.thread,
            payload: &mut self.pending,
        }
        .record(error);
    }
    /// Take the pending condition.
    pub const fn take_pending(&mut self) -> Option<ObjectError> {
        RuntimeErrorPayloadCapability {
            thread: &mut self.thread,
            payload: &mut self.pending,
        }
        .take()
    }
    /// Whether no Rust-side boundary error is pending.
    #[must_use]
    pub const fn pending_is_none(&self) -> bool {
        self.pending.is_none()
    }
    pub const fn set_pending_lisp_error(&mut self, error: LispError) {
        RuntimeErrorPayloadCapability {
            thread: &mut self.thread,
            payload: &mut self.pending_lisp_error,
        }
        .record(error);
    }
    /// Take the pending typed Lisp condition, if one was recorded.
    pub const fn take_pending_lisp_error(&mut self) -> Option<LispError> {
        RuntimeErrorPayloadCapability {
            thread: &mut self.thread,
            payload: &mut self.pending_lisp_error,
        }
        .take()
    }
    /// Take the direct native error, clearing its source flag even when empty.
    pub fn take_pending_native_error(&mut self) -> Option<ncl_sys::NativeError> {
        self.thread.take_native_error()
    }
    /// Take every typed runtime error payload and clear all three source flags.
    pub fn take_pending_runtime_errors(&mut self) -> PendingRuntimeErrors {
        let object = self.take_pending();
        let lisp = self.take_pending_lisp_error();
        let native = self.take_pending_native_error();
        PendingRuntimeErrors {
            object,
            lisp,
            native,
        }
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
    #[must_use]
    pub fn condition_handler_head(&self) -> Word {
        self.condition_handler_head.get()
    }
    pub fn set_condition_handler_head(&mut self, head: Word) {
        self.condition_handler_head.set(head);
    }
    pub const fn root_condition_handler_head(&mut self) -> bool {
        self.condition_handler_root.is_none()
    }
    pub const fn unroot_condition_handler_head(&mut self, _runtime: &Runtime) -> bool {
        true
    }
    pub fn set_condition_handler_invoker(&mut self, invoker: ConditionHandlerInvoker) {
        self.condition_handler_invoker = Some(invoker);
    }
    /// Invoke the runtime callback for a condition handler or restart function.
    ///
    /// `arguments` is the full argument list passed to `handler` (one
    /// condition object for handler-bind handlers, or zero or more
    /// user-supplied values for restart functions).
    ///
    /// # Errors
    /// Returns an object error when no evaluator is active or invocation fails.
    pub fn invoke_condition_handler(
        &mut self,
        handler: Word,
        arguments: &[Word],
    ) -> Result<Word, ObjectError> {
        let Some(invoker) = self.condition_handler_invoker else {
            return Ok(Word::NIL);
        };
        let Some(runtime) = self.evaluator_runtime else {
            return Err(ObjectError::Layout);
        };
        invoker(runtime, self, handler, arguments)
    }
    /// Return a restricted view of the thread's native and heap operations.
    ///
    /// The view does not expose runtime-error payload mutation or consumption.
    ///
    /// ```compile_fail
    /// let mut context = ncl_object::ThreadContext::new();
    /// let mut payload = None::<()>;
    /// context.thread_access().record_runtime_error_payload(
    ///     ncl_sys::RuntimeErrorSource::Object,
    ///     &mut payload,
    ///     (),
    /// );
    /// ```
    pub fn thread_access(&mut self) -> ncl_sys::ThreadAccess<'_> {
        ncl_sys::ThreadAccess::new(self.thread.as_mut())
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
/// The context's `Arc` keeps the heap alive, but root slots owned by `Runtime`
/// must also remain valid until registered contexts stop collecting from it.
/// Therefore the `Runtime` must outlive every registered `ThreadContext`.
impl Drop for ThreadContext {
    fn drop(&mut self) {
        if self.registered {
            let _ = self.condition_handler_root.take();
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

#[cfg(test)]
mod tests {
    use super::{LispError, ObjectError, PendingRuntimeErrors, ThreadContext};
    use ncl_sys::{NativeError, NativeOperation, Word};

    #[test]
    fn pending_error_payloads_keep_source_flags_in_sync() {
        let mut context = ThreadContext::new();
        context.set_pending(ObjectError::ControlError);
        context.set_pending_lisp_error(LispError::EndOfFile);
        assert!(context.thread.runtime_error_pending());

        assert_eq!(context.take_pending(), Some(ObjectError::ControlError));
        assert!(context.thread.runtime_error_pending());
        assert_eq!(
            context.take_pending_lisp_error(),
            Some(LispError::EndOfFile)
        );
        assert!(!context.thread.runtime_error_pending());
    }

    #[test]
    fn multiple_value_area_rejects_results_over_the_native_limit() {
        let mut context = ThreadContext::new();
        let twenty = (0..ncl_sys::MULTIPLE_VALUE_AREA_WORDS)
            .map(|value| Word::fixnum(i64::try_from(value).unwrap_or(0)))
            .collect::<Vec<_>>();
        assert_eq!(context.set_multiple_value_area(&twenty), Ok(()));
        assert_eq!(context.thread_access().multiple_values(), twenty);

        let twenty_one = (0..=ncl_sys::MULTIPLE_VALUE_AREA_WORDS)
            .map(|value| Word::fixnum(i64::try_from(value).unwrap_or(0)))
            .collect::<Vec<_>>();
        assert_eq!(
            context.set_multiple_value_area(&twenty_one),
            Err(ObjectError::Layout)
        );
    }

    #[test]
    fn taking_pending_runtime_errors_drains_all_three_sources() {
        let mut context = ThreadContext::new();
        let native = NativeError::NullThread {
            operation: NativeOperation::Car,
        };
        context.set_pending(ObjectError::ControlError);
        context.set_pending_lisp_error(LispError::EndOfFile);
        context.thread.set_native_error(native);

        assert_eq!(
            context.take_pending_runtime_errors(),
            PendingRuntimeErrors {
                object: Some(ObjectError::ControlError),
                lisp: Some(LispError::EndOfFile),
                native: Some(native),
            }
        );
        assert!(!context.thread.runtime_error_pending());
    }
}
