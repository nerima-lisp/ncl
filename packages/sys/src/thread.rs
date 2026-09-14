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
    index: usize,
}

#[derive(Debug)]
pub struct Thread {
    pub(crate) roots: Vec<*mut Word>,
    pub(crate) heap: Option<*const crate::heap::Heap>,
    state: SafepointState,
    native: NativeState,
    pub(crate) tlab: Vec<u64>,
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
        }
    }
    /// Push a precise root. The referenced slot must outlive the token.
    pub fn push_root(&mut self, value: &mut Word) -> RootToken {
        let token = RootToken {
            index: self.roots.len(),
        };
        self.roots.push(ptr::from_mut(value));
        token
    }
    /// Pop the most recently pushed root.
    pub fn pop_root(&mut self, token: RootToken) -> bool {
        token.index + 1 == self.roots.len() && self.roots.pop().is_some()
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
    pub(crate) fn enter_native(&mut self) {
        self.native = NativeState::Native;
        self.state = SafepointState::Safe;
    }
    pub(crate) fn leave_native(&mut self) {
        self.native = NativeState::Lisp;
        self.state = SafepointState::Running;
    }
    pub(crate) fn heap_collect(&mut self, _full: bool) {
        self.state = SafepointState::Collecting;
        self.state = SafepointState::Running;
    }
}
