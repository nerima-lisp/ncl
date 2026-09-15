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
        self.stack_bounds = current_stack_bounds();
        self.callee_saved = crate::snapshot_callee_saved();
    }
    pub(crate) fn conservative_snapshot(&self) -> Vec<Word> {
        let mut values = self.conservative_roots.clone();
        values.extend(self.callee_saved.iter().copied().map(Word::from_bits));
        if let Some((start, end)) = self.stack_bounds {
            let mut address = start.next_multiple_of(8);
            while address.checked_add(8).is_some_and(|next| next <= end) {
                // SAFETY: the collector reads only a published, stopped stack interval at word alignment.
                values.push(unsafe { Word::from_bits((address as *const u64).read()) });
                address += 8;
            }
        }
        values
    }
    pub(crate) const fn enter_native(&mut self) {
        self.native = NativeState::Native;
        self.state = SafepointState::Safe;
    }
    pub(crate) fn leave_native(&mut self) {
        self.native = NativeState::Lisp;
        self.state = SafepointState::Running;
        self.poll_safepoint();
    }
    pub(crate) fn heap_collect(&mut self, full: bool) {
        if let Some(heap) = self.heap {
            // SAFETY: registration stores this thread's heap pointer for its lifetime.
            unsafe {
                (*heap).collect_with_thread(self, full);
            }
        }
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

#[cfg(target_os = "macos")]
fn current_stack_bounds() -> Option<(usize, usize)> {
    // SAFETY: pthread_self identifies the calling thread and both APIs return its live stack extent.
    unsafe {
        let thread = crate::os::declarations::pthread_self();
        let top = crate::os::declarations::pthread_get_stackaddr_np(thread) as usize;
        let size = crate::os::declarations::pthread_get_stacksize_np(thread);
        top.checked_sub(size).map(|start| (start, top))
    }
}

#[cfg(target_os = "linux")]
fn current_stack_bounds() -> Option<(usize, usize)> {
    let mut attr = [0_u8; 128];
    let mut start = ptr::null_mut();
    let mut size = 0;
    // SAFETY: pthread attributes are written to platform ABI storage and destroyed after use.
    let result = unsafe {
        let thread = crate::os::declarations::pthread_self();
        let result = crate::os::declarations::pthread_getattr_np(thread, attr.as_mut_ptr().cast());
        if result == 0 {
            let result = crate::os::declarations::pthread_attr_getstack(
                attr.as_ptr().cast(),
                &mut start,
                &mut size,
            );
            let _ = crate::os::declarations::pthread_attr_destroy(attr.as_mut_ptr().cast());
            result
        } else {
            result
        }
    };
    (result == 0).then_some((start as usize, (start as usize).saturating_add(size)))
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const fn current_stack_bounds() -> Option<(usize, usize)> {
    None
}
