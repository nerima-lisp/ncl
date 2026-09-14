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
    state: SafepointState,
    native: NativeState,
    pub(crate) tlab: Vec<u64>,
    pub(crate) bytes_cons: usize,
    interrupt: bool,
    pub(crate) stack_bounds: Option<(usize, usize)>,
    pub(crate) callee_saved: [u64; 16],
}

impl Default for Thread {
    fn default() -> Self {
        Self::new()
    }
}
impl Thread {
    /// Create an unregistered thread context.
    pub fn new() -> Self {
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
        }
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
    /// Return the current safepoint state.
    pub const fn safepoint_state(&self) -> SafepointState {
        self.state
    }
    /// Return the current native state.
    pub const fn native_state(&self) -> NativeState {
        self.native
    }
    pub(crate) fn poll_safepoint(&mut self) {
        if self.state == SafepointState::PollRequested {
            self.state = SafepointState::Published;
            self.state = SafepointState::Running;
        }
    }
    pub(crate) fn request_safepoint(&mut self) {
        if self.state == SafepointState::Running {
            self.state = SafepointState::PollRequested;
        }
    }
    pub(crate) fn publish_snapshot(&mut self) {
        let marker = 0_u8;
        let address = std::ptr::from_ref(&marker) as usize;
        self.stack_bounds = Some((address, address + 1));
        self.callee_saved = crate::snapshot_callee_saved();
    }
    pub(crate) fn enter_native(&mut self) {
        self.native = NativeState::Native;
        self.state = SafepointState::Safe;
    }
    pub(crate) fn leave_native(&mut self) {
        self.native = NativeState::Lisp;
        self.state = SafepointState::Running;
    }
    pub(crate) fn heap_collect(&mut self, _full: bool) {
        if let Some(heap) = self.heap {
            // SAFETY: registration stores this thread's heap pointer for its lifetime.
            unsafe {
                (*heap).collect(_full);
            }
        }
        self.state = SafepointState::Collecting;
        self.state = SafepointState::Running;
    }
    /// Request delivery of an interrupt at the next safepoint.
    pub fn request_interrupt(&mut self) {
        self.interrupt = true;
        self.state = SafepointState::PollRequested;
    }
    /// Whether an interrupt is pending, consuming the request.
    pub fn take_interrupt(&mut self) -> bool {
        let pending = self.interrupt;
        self.interrupt = false;
        pending
    }
}
