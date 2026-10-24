use crate::{StorageCondition, Thread};

/// Run a minor collection when `full` is `false`, or a full collection when it is `true`.
///
/// A minor collection scans young objects and follows old-to-young references
/// recorded by write barriers.
/// Set `full` to `true` when dead objects from every generation need collection.
///
/// # Errors
/// Returns the [`StorageCondition`] reported by the thread's heap, or
/// [`StorageCondition::ThreadNotRegistered`] when the thread has no heap.
pub fn collect(thread: &mut Thread, full: bool) -> Result<(), StorageCondition> {
    thread.heap_collect(full)
}
