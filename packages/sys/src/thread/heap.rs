use super::Thread;

impl Thread {
    /// Return the heap this thread is registered with.
    #[must_use]
    pub(crate) fn heap(&self) -> Option<&crate::heap::Heap> {
        self.heap.map(|heap| {
            // SAFETY: registration stores this heap pointer for the thread lifetime.
            unsafe { &*heap }
        })
    }
}
