use crate::{ObjectError, ThreadContext};
use ncl_sys::{StorageCondition, Word};

impl ThreadContext {
    /// Write a payload slot through the write barrier.
    ///
    /// # Errors
    /// Returns a storage error when the thread is not registered.
    pub fn write_object_slot(
        &mut self,
        object: Word,
        slot: usize,
        value: Word,
    ) -> Result<(), ObjectError> {
        if ncl_sys::write_object_word(&mut self.thread, object, slot, value) {
            ncl_sys::write_barrier(&mut self.thread, object, slot);
            Ok(())
        } else {
            Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
        }
    }

    /// Record whether a non-local exit is currently propagating through
    /// native code. Backed by [`ncl_sys::Thread::set_pending`] so both
    /// generated code and Rust runtime helpers observe the same flag.
    pub fn set_non_local_exit(&mut self, pending: bool) {
        self.thread.set_pending(pending);
    }

    /// Read and clear the pending non-local exit flag.
    pub fn take_non_local_exit(&mut self) -> bool {
        let pending = self.thread.pending();
        self.thread.set_pending(false);
        pending
    }

    /// Whether a non-local exit is currently propagating.
    #[must_use]
    pub const fn is_unwinding(&self) -> bool {
        self.thread.pending()
    }

    pub const fn set_control_pointers(
        &mut self,
        handler: Option<usize>,
        cleanup: Option<usize>,
        catch: Option<usize>,
    ) {
        self.handler = handler;
        self.cleanup = cleanup;
        self.catch = catch;
    }

    #[must_use]
    pub const fn control_pointers(&self) -> (Option<usize>, Option<usize>, Option<usize>) {
        (self.handler, self.cleanup, self.catch)
    }
}
