use super::Runtime;
use crate::{ObjectError, ThreadContext};
use std::sync::atomic::Ordering;

impl Runtime {
    pub(crate) fn generic_builtin_entry(&self) -> usize {
        self.generic_builtin_entry.load(Ordering::Relaxed)
    }

    /// Install the native entry used by generic builtins.
    ///
    /// # Errors
    /// Returns an object error when an existing function cannot be patched.
    pub fn install_generic_builtin_entry(
        &self,
        ctx: &mut ThreadContext,
        address: usize,
    ) -> Result<(), ObjectError> {
        self.generic_builtin_entry.store(address, Ordering::Relaxed);
        let pending = self
            .builtins
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter(|entry| entry.implementation.entry == 0)
            .map(|entry| *entry.function)
            .collect::<Vec<_>>();
        let entry_word = crate::object_access::fix(address)?;
        for function_word in pending {
            crate::object_access::put(
                ctx,
                function_word,
                crate::function_offset::ENTRY,
                entry_word,
            )?;
        }
        Ok(())
    }
}
