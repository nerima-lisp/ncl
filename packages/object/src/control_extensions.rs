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
        self.check_registered_address()?;
        if ncl_sys::write_object_word(&mut self.thread, object, slot, value) {
            ncl_sys::write_barrier(&mut self.thread, object, slot);
            Ok(())
        } else {
            Err(ObjectError::Storage(StorageCondition::ThreadNotRegistered))
        }
    }

    pub const fn set_non_local_exit(&mut self, pending: bool) {
        self.non_local_exit = pending;
    }

    pub const fn take_non_local_exit(&mut self) -> bool {
        let pending = self.non_local_exit;
        self.non_local_exit = false;
        pending
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
