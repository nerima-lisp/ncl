#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]
#![allow(missing_docs, dead_code)]

//! The single unsafe boundary of the NCL runtime.

mod code;
mod heap;
mod heap_state;
mod heap_types;
mod os;
mod stw;
mod sync;
mod thread;
mod word;

pub use code::{
    CodePtr, FrameHeader, Safepoint, SafepointMap, alloc_code, free_code, publish_code,
    walk_frame_headers,
};
pub use heap::{
    Finalizer, Heap, HeapConfig, LayoutError, PageKind, ReferenceLayout, StorageCondition, TypeTag,
    Weakness,
};
pub use sync::{Condvar, Mutex, Semaphore, WaitQueue};
pub use thread::{NativeState, RootToken, SafepointState, Thread};
pub use word::{LowTag, Word};

#[cfg(target_arch = "aarch64")]
fn snapshot_callee_saved() -> [u64; 16] {
    let mut values = [0_u64; 16];
    // SAFETY: the output array is valid for sixteen u64 stores and the assembly only reads callee-saved registers.
    unsafe {
        core::arch::asm!("stp x19, x20, [{0}, #0]", "stp x21, x22, [{0}, #16]", "stp x23, x24, [{0}, #32]", "stp x25, x26, [{0}, #48]", "stp x27, x28, [{0}, #64]", "stp x29, x30, [{0}, #80]", in(reg) values.as_mut_ptr(), options(nostack, preserves_flags));
    }
    values
}
#[cfg(not(target_arch = "aarch64"))]
#[cfg(target_arch = "x86_64")]
fn snapshot_callee_saved() -> [u64; 16] {
    let mut values = [0_u64; 16];
    // SAFETY: each output is a scalar register snapshot and no stack or flags are modified.
    unsafe {
        core::arch::asm!(
            "mov {0}, rbx", "mov {1}, rbp", "mov {2}, r12",
            "mov {3}, r13", "mov {4}, r14", "mov {5}, r15",
            out(reg) values[0], out(reg) values[1], out(reg) values[2],
            out(reg) values[3], out(reg) values[4], out(reg) values[5],
            options(nostack, preserves_flags)
        );
    }
    values
}
#[cfg(not(any(target_arch = "aarch64", target_arch = "x86_64")))]
fn snapshot_callee_saved() -> [u64; 16] {
    [0; 16]
}

/// Allocate a normal header object.
///
/// # Errors
///
/// Returns a storage condition when the size is invalid, the thread is not registered, or capacity is exhausted.
pub fn alloc(
    thread: &mut Thread,
    heap: &Heap,
    tag: TypeTag,
    words: usize,
) -> Result<Word, StorageCondition> {
    heap.alloc(thread, tag, words)
}

/// Allocate an object in the large-object space.
///
/// # Errors
///
/// Returns a storage condition when the size is invalid, the thread is not registered, or capacity is exhausted.
pub fn alloc_large(
    thread: &mut Thread,
    heap: &Heap,
    tag: TypeTag,
    words: usize,
) -> Result<Word, StorageCondition> {
    heap.alloc_large(thread, tag, words)
}

/// Allocate a two-word cons cell.
///
/// # Errors
///
/// Returns a storage condition when the thread is not registered or capacity is exhausted.
pub fn alloc_cons(
    thread: &mut Thread,
    heap: &Heap,
    car: Word,
    cdr: Word,
) -> Result<Word, StorageCondition> {
    heap.alloc_cons(thread, car, cdr)
}

/// Register a mutator with a heap.
///
/// # Errors
///
/// Returns `ThreadNotRegistered` when the thread is already registered with this heap.
pub fn register_thread(heap: &Heap, thread: &mut Thread) -> Result<(), StorageCondition> {
    heap.register_thread(thread)
}

/// Remove a mutator from a heap.
pub fn unregister_thread(heap: &Heap, thread: &Thread) {
    heap.unregister_thread(thread);
}

/// Request and publish a safepoint if one is pending.
pub fn poll_safepoint(thread: &mut Thread) {
    thread.poll_safepoint();
}

/// Request a cooperative safepoint on the next poll.
pub fn request_safepoint(thread: &mut Thread) {
    thread.request_safepoint();
}

/// Publish the current stack boundary and callee-saved register snapshot.
pub fn publish_safepoint(thread: &mut Thread) {
    thread.publish_snapshot();
    thread.poll_safepoint();
}

/// Publish a conservative candidate discovered in a native stack or register.
pub fn publish_conservative_root(thread: &mut Thread, value: Word) {
    thread.publish_conservative_root(value);
}

/// Register a contiguous set of precise root slots.
pub fn register_root_set(thread: &mut Thread, values: &mut [Word]) -> RootToken {
    let token = RootToken {
        index: thread.roots.len(),
        count: values.len(),
    };
    thread
        .roots
        .extend(values.iter_mut().map(std::ptr::from_mut));
    token
}

/// Enter a foreign/native section.
pub const fn enter_native(thread: &mut Thread) {
    thread.enter_native();
}

/// Leave a foreign/native section.
pub const fn leave_native(thread: &mut Thread) {
    thread.leave_native();
}

/// Add a mutable slot to the precise root set.
pub fn push_root(thread: &mut Thread, value: &mut Word) -> RootToken {
    thread.push_root(value)
}

/// Remove the most recent precise root.
pub fn pop_root(thread: &mut Thread, token: RootToken) -> bool {
    thread.pop_root(token)
}

/// Record a reference store for the generational collector.
pub fn write_barrier(thread: &mut Thread, object: Word, slot: usize) {
    if let Some(heap) = thread.heap {
        // SAFETY: the heap pointer is installed only by register_thread.
        unsafe {
            (*heap).barrier(object, slot);
        }
    }
}

/// Register the payload reference layout for a widetag.
///
/// # Errors
///
/// Returns `LayoutError` when the widetag already has a layout.
pub fn register_layout(
    heap: &Heap,
    widetag: u8,
    layout: ReferenceLayout,
) -> Result<(), LayoutError> {
    heap.register_layout(widetag, layout)
}

/// Register a callback run after a collection releases its roots.
pub fn register_after_gc_hook(heap: &Heap, hook: fn()) {
    heap.register_after_gc_hook(hook);
}

/// Start a collection and update registered precise roots.
pub fn collect(thread: &mut Thread, full: bool) {
    thread.heap_collect(full);
}

/// Mark an object as weak with the specified weakness policy.
#[must_use]
pub fn make_weak(thread: &Thread, value: Word, weakness: Weakness) -> Word {
    thread.heap.map_or(value, |heap| {
        // SAFETY: the heap pointer is installed only by register_thread.
        unsafe { (*heap).make_weak(value, weakness) }
    })
}
/// Read the value slot of a weak object, or NIL when it is cleared.
#[must_use]
pub fn weak_value(thread: &Thread, value: Word) -> Word {
    thread.heap.map_or(Word::NIL, |heap| {
        // SAFETY: the heap pointer is installed only by register_thread.
        unsafe { (*heap).weak_value(value) }
    })
}
/// Register a one-shot finalizer callback.
pub fn register_finalizer(thread: &Thread, object: Word, callback: Finalizer) {
    if let Some(heap) = thread.heap {
        // SAFETY: the heap pointer is installed only by register_thread.
        unsafe {
            (*heap).register_finalizer(object, callback);
        }
    }
}

/// Run finalizers queued by completed collections.
pub fn run_pending_finalizers(thread: &Thread) {
    if let Some(heap) = thread.heap {
        // SAFETY: the heap pointer is installed only by register_thread.
        unsafe {
            (*heap).run_pending_finalizers();
        }
    }
}
