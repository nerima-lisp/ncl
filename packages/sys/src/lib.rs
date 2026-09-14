#![deny(unsafe_op_in_unsafe_fn, clippy::undocumented_unsafe_blocks)]
#![allow(missing_docs, dead_code)]

//! The single unsafe boundary of the NCL runtime.

mod heap;
mod os;
mod thread;
mod word;

pub use heap::{
    Heap, HeapConfig, LayoutError, PageKind, ReferenceLayout, StorageCondition, TypeTag,
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
pub fn object_widetag(thread: &Thread, object: Word) -> Option<u8> {
    let heap = thread.heap_ref()?;
    heap.widetag(object)
}

/// Read a cons payload word through a registered thread.
pub fn read_cons_word(thread: &Thread, object: Word, slot: usize) -> Option<Word> {
    let heap = thread.heap_ref()?;
    heap.read_cons_word(object, slot)
}

/// Read a header-object payload word through a registered thread.
pub fn read_object_word(thread: &Thread, object: Word, slot: usize) -> Option<Word> {
    let heap = thread.heap_ref()?;
    heap.read_word(object, slot + 1)
}

/// Write a header-object payload word through a registered thread.
pub fn write_object_word(thread: &mut Thread, object: Word, slot: usize, value: Word) -> bool {
    let Some(heap) = thread.heap_ref() else {
        return false;
    };
    heap.write_word(object, slot, value)
}

/// Write a cons payload word through a registered thread.
pub fn write_cons_word(thread: &mut Thread, object: Word, slot: usize, value: Word) -> bool {
    let Some(heap) = thread.heap_ref() else {
        return false;
    };
    heap.write_cons_word(object, slot, value)
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
pub fn write_barrier(_thread: &mut Thread, _object: Word, _slot: usize) {}

/// Register the payload reference layout for a widetag.
pub fn register_layout(
    heap: &Heap,
    widetag: u8,
    layout: ReferenceLayout,
) -> Result<(), LayoutError> {
    heap.register_layout(widetag, layout)
}

/// Start a collection. The moving collector is completed in the next phase.
pub fn collect(thread: &mut Thread, full: bool) {
    thread.heap_collect(full);
}
