#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]
#![allow(missing_docs, dead_code)]

//! The single unsafe boundary of the NCL runtime.

mod code;
mod heap;
mod os;
mod thread;
mod word;

pub use code::{CodePtr, Safepoint, SafepointMap, alloc_code, free_code, publish_code};
pub use heap::{
    Finalizer, Heap, HeapConfig, LayoutError, PageKind, ReferenceLayout, StorageCondition, TypeTag,
    Weakness,
};
pub use thread::{NativeState, RootToken, SafepointState, Thread};
pub use word::{LowTag, Word};

/// Allocate a normal header object.
pub fn alloc(
    thread: &mut Thread,
    heap: &Heap,
    tag: TypeTag,
    words: usize,
) -> Result<Word, StorageCondition> {
    heap.alloc(thread, tag, words)
}

/// Allocate an object in the large-object space.
pub fn alloc_large(
    thread: &mut Thread,
    heap: &Heap,
    tag: TypeTag,
    words: usize,
) -> Result<Word, StorageCondition> {
    heap.alloc_large(thread, tag, words)
}

/// Allocate a two-word cons cell.
pub fn alloc_cons(
    thread: &mut Thread,
    heap: &Heap,
    car: Word,
    cdr: Word,
) -> Result<Word, StorageCondition> {
    heap.alloc_cons(thread, car, cdr)
}

/// Register a mutator with a heap.
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

/// Enter a foreign/native section.
pub fn enter_native(thread: &mut Thread) {
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
pub fn register_layout(
    heap: &Heap,
    widetag: u8,
    layout: ReferenceLayout,
) -> Result<(), LayoutError> {
    heap.register_layout(widetag, layout)
}

/// Start a collection and update registered precise roots.
pub fn collect(thread: &mut Thread, full: bool) {
    thread.heap_collect(full);
}

/// Mark an object as weak with the specified weakness policy.
pub fn make_weak(thread: &Thread, value: Word, weakness: Weakness) -> Word {
    thread
        .heap
        .map_or(value, |heap| unsafe { (*heap).make_weak(value, weakness) })
}
/// Read the value slot of a weak object, or NIL when it is cleared.
pub fn weak_value(thread: &Thread, value: Word) -> Word {
    thread
        .heap
        .map_or(Word::NIL, |heap| unsafe { (*heap).weak_value(value) })
}
/// Register a one-shot finalizer callback.
pub fn register_finalizer(thread: &Thread, object: Word, callback: Finalizer) {
    if let Some(heap) = thread.heap {
        unsafe {
            (*heap).register_finalizer(object, callback);
        }
    }
}
