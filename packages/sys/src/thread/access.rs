use super::{RootToken, Thread};
use crate::{CodeError, CodeObjectMetadata, CodePtr, StorageCondition, Word};
use core::ptr::NonNull;

/// Restricted access to native, heap, and root operations on a thread.
///
/// This view does not expose runtime-error payload mutation or consumption.
#[derive(Debug)]
pub struct ThreadAccess<'a> {
    thread: &'a mut Thread,
}

impl<'a> ThreadAccess<'a> {
    /// Borrow a thread through this restricted API.
    pub const fn new(thread: &'a mut Thread) -> Self {
        Self { thread }
    }

    /// Return the thread's address for synchronous native entry calls.
    #[must_use]
    pub fn as_non_null(&mut self) -> NonNull<Thread> {
        NonNull::from(&mut *self.thread)
    }

    /// Return the address used to identify this thread within the process.
    #[must_use]
    pub fn identity(&self) -> usize {
        core::ptr::from_ref(&*self.thread).addr()
    }

    /// Whether any typed runtime-error source is pending.
    #[must_use]
    pub const fn runtime_error_pending(&self) -> bool {
        self.thread.runtime_error_pending()
    }

    /// Return the machine-visible multiple-value area.
    #[must_use]
    pub const fn multiple_values(&self) -> &[Word] {
        self.thread.multiple_values()
    }

    /// Return the number of live words in the multiple-value area.
    #[must_use]
    pub const fn multiple_value_count(&self) -> usize {
        self.thread.mv_count()
    }

    /// Return the number of active generated-code handler frames.
    #[must_use]
    pub const fn handler_depth(&self) -> usize {
        self.thread.handler_depth()
    }

    /// Return the number of active cleanup frames that `unwind-protect` can run.
    #[must_use]
    pub const fn cleanup_depth(&self) -> usize {
        self.thread.cleanup_depth()
    }

    /// Register another thread with the heap associated with this thread.
    ///
    /// # Errors
    /// Returns [`StorageCondition::ThreadNotRegistered`] when this thread has
    /// no associated heap.
    pub fn register_thread_with(&self, thread: &mut Thread) -> Result<(), StorageCondition> {
        crate::register_thread_with_thread(self.thread, thread)
    }

    /// Register published code metadata through this thread.
    ///
    /// # Errors
    /// Returns [`CodeError::NotRegistered`] when this thread has no heap.
    pub fn register_code(
        &self,
        code: &CodePtr,
        metadata: CodeObjectMetadata,
    ) -> Result<(), CodeError> {
        crate::register_code(self.thread, code, metadata)
    }

    /// Request and publish a safepoint when one is pending.
    pub fn poll_safepoint(&mut self) {
        crate::poll_safepoint(self.thread);
    }

    /// Enter a foreign or native section.
    pub fn enter_native(&mut self) {
        crate::enter_native(self.thread);
    }

    /// Leave a foreign or native section and deliver a pending poll.
    pub fn leave_native(&mut self) {
        crate::leave_native(self.thread);
    }

    /// Add a mutable slot to the precise root set.
    pub fn push_root(&mut self, value: &mut Word) -> RootToken {
        crate::push_root(self.thread, value)
    }

    /// Remove the most recent precise root.
    pub fn pop_root(&mut self, token: RootToken) -> bool {
        crate::pop_root(self.thread, token)
    }

    /// Register a contiguous set of precise root slots.
    pub fn register_root_set(&mut self, values: &mut [Word]) -> RootToken {
        crate::register_root_set(self.thread, values)
    }

    /// Read a header-object payload word through this thread.
    #[must_use]
    pub fn read_object_word(&self, object: Word, slot: usize) -> Option<Word> {
        crate::read_object_word(self.thread, object, slot)
    }

    /// Write a header-object payload word through this thread.
    pub fn write_object_word(&mut self, object: Word, slot: usize, value: Word) -> bool {
        crate::write_object_word(self.thread, object, slot, value)
    }

    /// Record a reference store for the generational collector.
    pub fn write_barrier(&mut self, object: Word, slot: usize) {
        crate::write_barrier(NonNull::from(&mut *self.thread), object, slot);
    }

    /// Return the collection epoch of this thread's heap.
    #[must_use]
    pub fn heap_epoch(&self) -> u64 {
        crate::heap_epoch(self.thread)
    }

    /// Return the heap's cumulative forwarded-object count for GC stress tests.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[must_use]
    pub fn forwarded_object_count_for_test(&self) -> usize {
        self.thread
            .heap()
            .map_or(0, crate::heap::Heap::forwarded_object_count_for_test)
    }

    /// Whether each registered root has its exact forwarded or unchanged live value.
    #[cfg(feature = "test-support")]
    #[doc(hidden)]
    #[must_use]
    pub fn registered_root_slots_are_live_and_forwarded_for_test(&self) -> bool {
        self.thread
            .heap()
            .is_some_and(crate::heap::Heap::registered_root_slots_are_live_and_forwarded_for_test)
    }

    /// Invoke published native code using the architecture entry convention.
    ///
    /// `argc` is the total argument count, and `arguments` carries the first
    /// four words. `rest` is the address, encoded as `u64`, of any remaining
    /// contiguous words, or zero when there are none. Keep those words alive
    /// until this synchronous call returns.
    pub fn invoke_entry(
        &mut self,
        code: &CodePtr,
        entry_offset: usize,
        argc: u64,
        arguments: [u64; 4],
        rest: u64,
    ) -> (u64, u64) {
        crate::invoke_entry(
            code,
            entry_offset,
            self.as_non_null().as_ptr(),
            argc,
            arguments,
            rest,
        )
    }

    /// Invoke a published native entry with an explicit function object.
    ///
    /// The argument layout and lifetime match [`Self::invoke_entry`].
    pub fn invoke_entry_with_function(
        &mut self,
        code: &CodePtr,
        entry_offset: usize,
        function_object: u64,
        argc: u64,
        arguments: [u64; 4],
        rest: u64,
    ) -> (u64, u64) {
        crate::invoke_entry_with_function(
            code,
            entry_offset,
            self.as_non_null().as_ptr(),
            function_object,
            argc,
            arguments,
            rest,
        )
    }

    /// Invoke a published native entry address with an explicit function object.
    ///
    /// The argument layout and lifetime match [`Self::invoke_entry`].
    pub fn invoke_entry_with_function_address(
        &mut self,
        entry: usize,
        function_object: u64,
        argc: u64,
        arguments: [u64; 4],
        rest: u64,
    ) -> (u64, u64) {
        crate::invoke_entry_with_function_address(
            entry,
            self.as_non_null().as_ptr(),
            function_object,
            argc,
            arguments,
            rest,
        )
    }
}
