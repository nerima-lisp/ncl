use crate::word::Word;
use std::ptr;

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
}
/// A LIFO shadow-root handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RootToken {
    pub(crate) index: usize,
    pub(crate) count: usize,
}

/// Machine-visible mutator context.
///
/// This type is `repr(C)` so the offsets returned by [`thread_layout`] are a
/// stable ABI. The `*mut Thread` passed to generated code must remain stable
/// until the thread is unregistered.
#[repr(C)]
#[derive(Debug)]
pub struct Thread {
    pub(crate) roots: Vec<*mut Word>,
    pub(crate) heap: Option<*const crate::heap::Heap>,
    pub(crate) state: SafepointState,
    native: NativeState,
    pub(crate) safepoint_request: u64,
    pub(crate) tlab: Vec<u64>,
    pub(crate) bytes_cons: usize,
    interrupt: bool,
    pub(crate) stack_bounds: Option<(usize, usize)>,
    pub(crate) callee_saved: [u64; 16],
    pub(crate) safepoint_epoch: u64,
    pub(crate) conservative_roots: Vec<Word>,
    pub(crate) tlab_bump: usize,
    pub(crate) tlab_limit: usize,
    pub(crate) mv: Vec<Word>,
    pub(crate) handler: usize,
    pub(crate) cleanup: usize,
    pub(crate) catch: usize,
    pub(crate) pending: u64,
    pub(crate) frame_chain: Vec<Word>,
    pub(crate) frame_registers: Vec<Word>,
    frame_address: Option<usize>,
}

/// Native offsets consumed by the code generator when addressing a thread context.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThreadLayout {
    /// Offset of the TLAB allocation cursor.
    pub tlab_bump: usize,
    /// Offset of the TLAB allocation limit.
    pub tlab_limit: usize,
    /// Offset of the eight-byte safepoint request word read by generated code.
    pub safepoint_request: usize,
    /// Offset of the eight-byte pending interrupt word.
    pub pending: usize,
    /// Offset of the multiple-value return vector.
    pub mv: usize,
    /// Offset of the handler chain.
    pub handler: usize,
    /// Offset of the cleanup chain.
    pub cleanup: usize,
    /// Offset of the catch chain.
    pub catch: usize,
}

/// Return byte offsets for the machine-visible part of [`Thread`].
#[must_use]
pub const fn thread_layout() -> ThreadLayout {
    ThreadLayout {
        tlab_bump: std::mem::offset_of!(Thread, tlab_bump),
        tlab_limit: std::mem::offset_of!(Thread, tlab_limit),
        safepoint_request: std::mem::offset_of!(Thread, safepoint_request),
        pending: std::mem::offset_of!(Thread, pending),
        mv: std::mem::offset_of!(Thread, mv),
        handler: std::mem::offset_of!(Thread, handler),
        cleanup: std::mem::offset_of!(Thread, cleanup),
        catch: std::mem::offset_of!(Thread, catch),
    }
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
            safepoint_request: 0,
            tlab: Vec::new(),
            bytes_cons: 0,
            interrupt: false,
            stack_bounds: None,
            callee_saved: [0; 16],
            safepoint_epoch: 0,
            conservative_roots: Vec::new(),
            tlab_bump: 0,
            tlab_limit: 0,
            mv: Vec::new(),
            handler: 0,
            cleanup: 0,
            catch: 0,
            pending: 0,
            frame_chain: Vec::new(),
            frame_registers: Vec::new(),
            frame_address: None,
        }
    }

    /// Return the heap this thread is registered with.
    #[must_use]
    pub(crate) fn heap(&self) -> Option<&crate::heap::Heap> {
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
            self.safepoint_request = 0;
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
            self.safepoint_request = 1;
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
    /// Enter the native runtime state.
    pub const fn enter_native(&mut self) {
        self.native = NativeState::Native;
        self.state = SafepointState::Safe;
    }
    /// Leave the native runtime state and deliver a pending poll.
    pub fn leave_native(&mut self) {
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
        self.write_back_frame_snapshot();
    }
    /// Request delivery of an interrupt at the next safepoint.
    pub const fn request_interrupt(&mut self) {
        self.interrupt = true;
        self.state = SafepointState::PollRequested;
        self.safepoint_request = 1;
    }

    /// Request a local poll without starting a stop-the-world epoch.
    pub const fn request_poll(&mut self) {
        self.state = SafepointState::PollRequested;
        self.safepoint_request = 1;
    }
    /// Whether an interrupt is pending, consuming the request.
    pub const fn take_interrupt(&mut self) -> bool {
        let pending = self.interrupt;
        self.interrupt = false;
        pending
    }

    /// Clear a delivered cooperative safepoint request.
    pub const fn clear_safepoint_request(&mut self) {
        self.safepoint_request = 0;
    }
    /// Install a precise native frame and register snapshot for collection.
    pub fn set_frame_snapshot(&mut self, frames: Vec<Word>, registers: Vec<Word>) {
        self.frame_chain = frames;
        self.frame_registers = registers;
        self.frame_address = None;
    }

    /// Capture the current generated frame header for collection.
    pub fn set_native_frame(&mut self, frame_fp: usize, return_pc: usize) {
        let words = frame_fp as *const Word;
        // SAFETY: the generated frame owns four published header words at the supplied frame pointer.
        self.frame_chain = unsafe { std::slice::from_raw_parts(words, 4).to_vec() };
        self.stack_bounds = None;
        self.callee_saved = [0; 16];
        self.frame_chain[1] = Word::from_bits(return_pc as u64);
        self.frame_address = Some(frame_fp);
    }

    /// Capture the generated caller's continuation at a runtime callback entry.
    #[cfg(target_arch = "aarch64")]
    #[inline(always)]
    pub fn capture_return_address() -> usize {
        let address: usize;
        // SAFETY: x30 contains the generated caller's continuation at callback entry.
        unsafe {
            core::arch::asm!("mov {0}, x30", out(reg) address, options(nostack, preserves_flags));
        }
        address
    }

    #[cfg(not(target_arch = "aarch64"))]
    #[inline(always)]
    pub fn capture_return_address() -> usize {
        0
    }

    /// Return the current value of a captured real frame word.
    #[must_use]
    pub fn frame_word(&self, index: usize) -> Option<Word> {
        let address = self.frame_address?;
        // SAFETY: the captured generated frame remains active until its runtime callback returns.
        Some(unsafe { ((address as *const Word).add(index)).read() })
    }

    /// Write collection's forwarded snapshot values back to the generated frame.
    pub fn write_back_frame_snapshot(&mut self) {
        let Some(address) = self.frame_address else {
            return;
        };
        // SAFETY: the captured generated frame remains active during collection and write-back.
        unsafe {
            for (index, word) in self.frame_chain.iter().copied().enumerate() {
                if index != 1 {
                    ((address as *mut Word).add(index)).write(word);
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_matches_c_struct_offsets() {
        let layout = thread_layout();
        assert_eq!(layout.tlab_bump, std::mem::offset_of!(Thread, tlab_bump));
        assert_eq!(layout.tlab_limit, std::mem::offset_of!(Thread, tlab_limit));
        assert_eq!(
            layout.safepoint_request,
            std::mem::offset_of!(Thread, safepoint_request)
        );
        assert_eq!(layout.pending, std::mem::offset_of!(Thread, pending));
        assert_eq!(layout.mv, std::mem::offset_of!(Thread, mv));
        assert_eq!(layout.handler, std::mem::offset_of!(Thread, handler));
        assert_eq!(layout.cleanup, std::mem::offset_of!(Thread, cleanup));
        assert_eq!(layout.catch, std::mem::offset_of!(Thread, catch));
    }

    #[test]
    fn machine_visible_fields_are_aligned_single_words() {
        let thread = Thread::new();
        let layout = thread_layout();
        let fields = [
            (layout.tlab_bump, std::mem::size_of_val(&thread.tlab_bump)),
            (layout.tlab_limit, std::mem::size_of_val(&thread.tlab_limit)),
            (
                layout.safepoint_request,
                std::mem::size_of_val(&thread.safepoint_request),
            ),
            (layout.pending, std::mem::size_of_val(&thread.pending)),
            (layout.handler, std::mem::size_of_val(&thread.handler)),
            (layout.cleanup, std::mem::size_of_val(&thread.cleanup)),
            (layout.catch, std::mem::size_of_val(&thread.catch)),
        ];
        for (offset, size) in fields {
            assert_eq!(offset % 8, 0);
            assert_eq!(size, 8);
        }
        // `mv` is a Vec descriptor, not a generated-code scalar word field.
        assert_eq!(layout.mv % 8, 0);
        assert_ne!(std::mem::size_of::<Vec<Word>>(), 8);
    }
}
