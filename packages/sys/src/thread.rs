use crate::word::Word;
use std::ptr;

/// Native transition state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeState {
    Lisp,
    Native,
}
/// Cooperative safepoint state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafepointState {
    Running,
    PollRequested,
    Published,
    Collecting,
    Safe,
}
/// A LIFO shadow-root handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootToken {
    pub(crate) index: usize,
    pub(crate) count: usize,
}

#[derive(Debug)]
pub struct Thread {
    pub(crate) roots: Vec<*mut Word>,
    pub(crate) heap: Option<*const crate::heap::Heap>,
    pub(crate) state: SafepointState,
    native: NativeState,
    pub(crate) tlab: Vec<u64>,
    pub(crate) bytes_cons: usize,
    interrupt: bool,
    pub(crate) stack_bounds: Option<(usize, usize)>,
    pub(crate) callee_saved: [u64; 16],
    pub(crate) safepoint_epoch: u64,
    pub(crate) conservative_roots: Vec<Word>,
}

impl Default for Thread {
    fn default() -> Self {
        Self::new()
    }
}
impl Thread {
    /// Create an unregistered thread context.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            roots: Vec::new(),
            heap: None,
            state: SafepointState::Running,
            native: NativeState::Lisp,
            tlab: Vec::new(),
            bytes_cons: 0,
            interrupt: false,
            stack_bounds: None,
            callee_saved: [0; 16],
            safepoint_epoch: 0,
            conservative_roots: Vec::new(),
        }
    }
    pub(crate) fn heap_ref(&self) -> Option<&crate::heap::Heap> {
        self.heap.map(|heap| {
            // SAFETY: registration stores this heap pointer for the thread lifetime.
            unsafe { &*heap }
        })
    }
    /// Push a precise root. The referenced slot must outlive the token.
    pub fn push_root(&mut self, value: &mut Word) -> RootToken {
        let token = RootToken {
            index: self.roots.len(),
            count: 1,
        };
        self.roots.push(ptr::from_mut(value));
        token
    }
    /// Pop the most recently pushed root.
    pub fn pop_root(&mut self, token: RootToken) -> bool {
        token.index + token.count == self.roots.len()
            && (0..token.count).all(|_| self.roots.pop().is_some())
    }
    /// Publish a word found by conservative stack or register scanning.
    pub fn publish_conservative_root(&mut self, value: Word) {
        self.conservative_roots.push(value);
    }
    /// Return the current safepoint state.
    #[must_use]
    pub const fn safepoint_state(&self) -> SafepointState {
        self.state
    }
    /// Return the current native state.
    #[must_use]
    pub const fn native_state(&self) -> NativeState {
        self.native
    }
    pub(crate) fn poll_safepoint(&mut self) {
        if self.native == NativeState::Native {
            return;
        }
        if self.state == SafepointState::PollRequested || self.heap.is_some() {
            self.publish_snapshot();
            if let Some(heap) = self.heap {
                // SAFETY: the heap pointer is installed by register_thread and remains valid while registered.
                unsafe { (*heap).poll_thread(self) };
            } else {
                self.state = SafepointState::Running;
            }
        }
    }
    pub(crate) fn request_safepoint(&mut self) {
        if self.state == SafepointState::Running {
            self.state = SafepointState::PollRequested;
            if let Some(heap) = self.heap {
                // SAFETY: the heap pointer is installed by register_thread and remains valid while registered.
                unsafe { (*heap).request_epoch() };
            }
        }
    }
    pub(crate) fn publish_snapshot(&mut self) {
        let marker = 0_u8;
        let address = std::ptr::from_ref(&marker) as usize;
        self.stack_bounds = Some((address, address + 1));
        self.callee_saved = crate::snapshot_callee_saved();
    }
    pub(crate) const fn enter_native(&mut self) {
        self.native = NativeState::Native;
        self.state = SafepointState::Safe;
    }
    pub(crate) const fn leave_native(&mut self) {
        self.native = NativeState::Lisp;
        self.state = SafepointState::Running;
    }
    pub(crate) fn heap_collect(&mut self, full: bool) {
        if let Some(heap) = self.heap {
            // SAFETY: registration stores this thread's heap pointer for its lifetime.
            unsafe {
                (*heap).collect_with_thread(self, full);
            }
        }
        self.state = SafepointState::Collecting;
        self.state = SafepointState::Running;
    }
    /// Request delivery of an interrupt at the next safepoint.
    pub const fn request_interrupt(&mut self) {
        self.interrupt = true;
        self.state = SafepointState::PollRequested;
    }
    /// Whether an interrupt is pending, consuming the request.
    pub const fn take_interrupt(&mut self) -> bool {
        let pending = self.interrupt;
        self.interrupt = false;
        pending
    }
}

// SAFETY: a Thread is an owner-local mutator context and is transferred to one OS thread at a time.
unsafe impl Send for Thread {}
