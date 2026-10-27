use super::MULTIPLE_VALUE_AREA_WORDS;
use crate::word::Word;
use std::ptr;
use std::sync::{Arc, Mutex};

/// Native transition state.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeState {
    /// The mutator is executing Lisp code.
    Lisp,
    /// The mutator is executing outside the managed heap.
    Native,
}

/// Cooperative safepoint state.
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointState {
    /// The mutator may continue running.
    Running,
    /// A collector has requested a safepoint.
    PollRequested,
    /// The mutator has published its root snapshot.
    Published,
    /// The collector is scanning this mutator.
    Collecting,
    /// The mutator is outside the managed heap and safe to stop.
    Safe,
    /// This thread owns the heap slow-path collection gate.
    Collector = 5,
    /// This thread has published roots and is waiting for the collection gate.
    Parked = 6,
}

#[derive(Debug)]
pub struct PublishedRootSlot(pub ptr::NonNull<Word>);

impl PublishedRootSlot {
    /// # Safety
    /// `slot` must be non-null. Callers and consumers own its lifetime and synchronization.
    pub const unsafe fn new(slot: *mut Word) -> Self {
        // SAFETY: the caller guarantees `slot` is non-null.
        Self(unsafe { ptr::NonNull::new_unchecked(slot) })
    }
}

impl AsRef<ptr::NonNull<Word>> for PublishedRootSlot {
    fn as_ref(&self) -> &ptr::NonNull<Word> {
        &self.0
    }
}

// SAFETY: pointer values may cross threads; dereferencing them requires the stop-world token.
unsafe impl Send for PublishedRootSlot {}

#[derive(Debug)]
pub struct PublishedThreadGcState {
    pub(crate) is_native: bool,
    pub(crate) roots: Vec<PublishedRootSlot>,
    pub(crate) multiple_values: [Word; MULTIPLE_VALUE_AREA_WORDS],
    pub(crate) mv_count: usize,
    pub(crate) conservative_roots: Vec<Word>,
    pub(crate) stack_bounds: Option<(usize, usize)>,
    pub(crate) stack_scan_start: Option<usize>,
    pub(crate) callee_saved: [u64; 16],
    pub(crate) frame_chain: Vec<Word>,
    pub(crate) frame_registers: Vec<Word>,
    pub(crate) frame_address: Option<usize>,
    pub(crate) frame_snapshot_failed: bool,
    pub(crate) published_epoch: u64,
}

#[derive(Clone, Debug)]
pub struct PublishedThreadGcStateHandle(Arc<Mutex<PublishedThreadGcState>>);

impl PublishedThreadGcStateHandle {
    pub fn lock(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, PublishedThreadGcState>> {
        self.0.lock()
    }

    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(PublishedThreadGcState {
            is_native: false,
            roots: Vec::new(),
            multiple_values: [Word::NIL; MULTIPLE_VALUE_AREA_WORDS],
            mv_count: 0,
            conservative_roots: Vec::new(),
            stack_bounds: None,
            stack_scan_start: None,
            callee_saved: [0; 16],
            frame_chain: Vec::new(),
            frame_registers: Vec::new(),
            frame_address: None,
            frame_snapshot_failed: false,
            published_epoch: 0,
        })))
    }
}

pub const fn valid_stack_interval(bounds: (usize, usize), stack_pointer: usize) -> bool {
    bounds.0 < bounds.1 && stack_pointer >= bounds.0 && stack_pointer < bounds.1
}

#[cfg(test)]
mod tests {
    use super::valid_stack_interval;

    #[test]
    fn stack_pointer_must_be_inside_nonempty_bounds() {
        assert!(!valid_stack_interval((7, 7), 7));
        assert!(!valid_stack_interval((9, 7), 8));
        assert!(!valid_stack_interval((7, 9), 6));
        assert!(valid_stack_interval((7, 9), 7));
        assert!(valid_stack_interval((7, 9), 8));
        assert!(!valid_stack_interval((7, 9), 9));
    }
}
