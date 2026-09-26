#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]
//! The single unsafe boundary of the NCL runtime.

mod code;
mod codegen;
mod heap;
mod heap_state;
mod heap_types;
mod invoke;
pub mod os;
mod stw;
mod sync;
mod thread;
mod word;

pub use code::{
    CodeError, CodeObjectMetadata, CodePtr, CodeRegistry, FrameHeader, Safepoint, SafepointMap,
    SourceLocation, alloc_code, free_code, publish_code, scan_frame, scan_frame_chain,
    scan_frame_chain_with_registry, scan_frame_with_registers, walk_frame_headers, write_code,
};
pub use codegen::{set_tlab, tlab_bump};
pub use heap::{
    Finalizer, Heap, HeapConfig, LayoutError, PageKind, ReferenceLayout, StorageCondition, TypeTag,
    Weakness,
};
pub use invoke::{invoke_entry, invoke_entry_with_function, invoke_entry_with_function_address};
pub use sync::{Condvar, Mutex, Semaphore, WaitQueue};
pub use thread::{NativeState, RootToken, SafepointState, Thread, ThreadLayout, thread_layout};
pub use word::{LowTag, Word};

/// A borrowed precise-root slot whose value the collector may rewrite in place.
///
/// A mutator reads a root through this handle after an allocation or collection.
/// Because the slot is interior-mutable, the optimizer cannot reuse a value the
/// mutator observed before the collection that forwarded it.
#[derive(Clone, Copy, Debug)]
pub struct RootSlot<'a> {
    cell: &'a core::cell::Cell<Word>,
}

impl<'a> RootSlot<'a> {
    /// Wrap a mutator-owned root cell.
    #[must_use]
    pub const fn new(cell: &'a core::cell::Cell<Word>) -> Self {
        Self { cell }
    }
}

impl core::ops::Deref for RootSlot<'_> {
    type Target = Word;

    fn deref(&self) -> &Word {
        // SAFETY: the cell outlives this borrow, and the collector rewrites it
        // only while no reference derived here is live.
        unsafe { &*self.cell.as_ptr() }
    }
}

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

/// Read a payload word from a live object.
pub fn read_word(heap: &Heap, object: Word, slot: usize) -> Option<Word> {
    heap.read_word(object, slot + 1)
}

/// Write a payload word in a live object.
pub fn write_word(heap: &Heap, object: Word, slot: usize, value: Word) -> bool {
    heap.write_word(object, slot + 1, value)
}

/// Read the header widetag of a live object.
pub fn widetag(heap: &Heap, object: Word) -> Option<u8> {
    heap.widetag(object)
}

/// Read the widetag of a live object through a registered thread.
#[must_use]
pub fn object_widetag(thread: &Thread, object: Word) -> Option<u8> {
    let heap = thread.heap()?;
    heap.widetag(object)
}

/// Read a cons payload word through a registered thread.
#[must_use]
pub fn read_cons_word(thread: &Thread, object: Word, slot: usize) -> Option<Word> {
    let heap = thread.heap()?;
    heap.read_word(object, slot)
}

/// Read a header-object payload word through a registered thread.
#[must_use]
pub fn read_object_word(thread: &Thread, object: Word, slot: usize) -> Option<Word> {
    let heap = thread.heap()?;
    heap.read_word(object, slot + 1)
}

/// Write a header-object payload word through a registered thread.
pub fn write_object_word(thread: &mut Thread, object: Word, slot: usize, value: Word) -> bool {
    let Some(heap) = thread.heap() else {
        return false;
    };
    heap.write_word(object, slot, value)
}

/// Write a cons payload word through a registered thread.
pub fn write_cons_word(thread: &mut Thread, object: Word, slot: usize, value: Word) -> bool {
    let Some(heap) = thread.heap() else {
        return false;
    };
    heap.write_word_at(object, slot, value)
}

/// Register a mutator with a heap.
///
/// # Errors
///
/// Returns `ThreadNotRegistered` when the thread is already registered with this heap.
pub fn register_thread(heap: &Heap, thread: &mut Thread) -> Result<(), StorageCondition> {
    heap.register_thread(thread)
}

/// Register a mutator with the heap already associated with another thread.
///
/// # Errors
///
/// Returns `ThreadNotRegistered` when the reference thread has no heap.
pub fn register_thread_with_thread(
    reference: &Thread,
    thread: &mut Thread,
) -> Result<(), StorageCondition> {
    reference
        .heap()
        .ok_or(StorageCondition::ThreadNotRegistered)?
        .register_thread(thread)
}

/// Register published code metadata through a registered thread.
///
/// # Errors
///
/// Returns `CodeError::NotRegistered` when the thread has no heap.
pub fn register_code(
    thread: &Thread,
    code: &CodePtr,
    metadata: CodeObjectMetadata,
) -> Result<(), CodeError> {
    thread
        .heap()
        .ok_or(CodeError::NotRegistered)?
        .register_code(code, metadata)
}

/// Configure strict stale-word checking for a registered thread's heap.
pub fn set_strict_forwarding(thread: &Thread, on: bool) {
    if let Some(heap) = thread.heap() {
        heap.set_strict_forwarding(on);
    }
}

/// Remove a mutator from a heap.
pub fn unregister_thread(thread: &Thread) {
    if let Some(heap) = thread.heap() {
        heap.unregister_thread(thread);
    }
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

/// Register a precise root slot owned by a heap-level registry.
pub fn push_heap_root(heap: &Heap, value: &mut Word) -> RootToken {
    heap.push_root(value)
}

/// Remove the most recently registered heap-level root.
pub fn pop_heap_root(heap: &Heap, token: RootToken) -> bool {
    heap.pop_root(token)
}

/// Enter a foreign/native section.
pub const fn enter_native(thread: &mut Thread) {
    thread.enter_native();
}

/// Leave a foreign/native section.
pub fn leave_native(thread: &mut Thread) {
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

/// Return the collection epoch of the thread's heap.
#[must_use]
pub fn heap_epoch(thread: &Thread) -> u64 {
    thread.heap().map_or(0, Heap::gc_epoch)
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
