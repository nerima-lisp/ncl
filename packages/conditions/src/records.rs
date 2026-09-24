//! Handler, restart, cleanup, and catch record chains.
//!
//! The design contract (`docs/src/design/native-backend.md`, "Non-local exit
//! records") fixes three current pointers in `ThreadContext`: handler,
//! cleanup, and catch. Phase 1 records are heap-allocated simple vectors
//! chained by a `previous` slot, and the chain heads live in those three
//! pointers. The handler pointer carries both handler and restart records,
//! distinguished by a leading kind tag, because restarts share the dynamic
//! extent of the surrounding handlers.
//!
//! Record layouts (slot index: meaning):
//!
//! * handler: 0 kind, 1 class, 2 function, 3 previous, 4 depth
//! * restart: 0 kind, 1 name, 2 function, 3 report, 4 interactive, 5 test,
//!   6 previous, 7 depth
//! * cleanup: 0 entry, 1 previous, 2 depth
//!
//! Catch records are deferred to the compiler-front lowering lane: `throw`
//! and `catch` are special operators, and their record runtime side lands
//! with L12/L14. Records do not yet live in machine stack frames; that is the
//! L13/L14 unwinder work, and a collection while a chain is active is out of
//! scope for Phase 1.

use ncl_object::{ObjectError, ThreadContext, Word, simple_vector_ref};

/// Index of the leading record-kind tag.
pub const KIND: usize = 0;
/// Kind tag for a handler record.
pub const HANDLER_TAG: Word = Word::fixnum(0);
/// Kind tag for a restart record.
pub const RESTART_TAG: Word = Word::fixnum(1);

/// Index of the handler record's class slot.
pub const HANDLER_CLASS: usize = 1;
/// Index of the handler record's previous slot.
pub const HANDLER_PREVIOUS: usize = 3;
/// Index of the handler record's depth slot.
pub const HANDLER_DEPTH: usize = 4;

/// Index of the restart record's name slot.
pub const RESTART_NAME: usize = 1;
/// Index of the restart record's function slot.
pub const RESTART_FUNCTION: usize = 2;
/// Index of the restart record's previous slot.
pub const RESTART_PREVIOUS: usize = 6;
/// Index of the restart record's depth slot.
pub const RESTART_DEPTH: usize = 7;

/// Index of the cleanup record's previous slot.
pub const CLEANUP_PREVIOUS: usize = 1;
/// Index of the cleanup record's depth slot.
pub const CLEANUP_DEPTH: usize = 2;

/// Read the handler-cluster chain head.
pub fn cluster_head(ctx: &ThreadContext) -> Word {
    let (handler, _, _) = ctx.control_pointers();
    handler.map_or(Word::NIL, from_pointer)
}

/// Write the handler-cluster chain head, preserving the other pointers.
pub const fn set_cluster_head(ctx: &mut ThreadContext, head: Word) {
    let (_, cleanup, catch) = ctx.control_pointers();
    ctx.set_control_pointers(Some(to_pointer(head)), cleanup, catch);
}

/// Read the cleanup chain head.
pub fn cleanup_head(ctx: &ThreadContext) -> Word {
    let (_, cleanup, _) = ctx.control_pointers();
    cleanup.map_or(Word::NIL, from_pointer)
}

/// Write the cleanup chain head, preserving the other pointers.
pub const fn set_cleanup_head(ctx: &mut ThreadContext, head: Word) {
    let (handler, _, catch) = ctx.control_pointers();
    ctx.set_control_pointers(handler, Some(to_pointer(head)), catch);
}

/// Read the `previous` link of a handler-cluster record, dispatching on its
/// kind tag because handler and restart records store `previous` at different
/// indices.
pub fn record_previous(ctx: &ThreadContext, record: Word) -> Result<Word, ObjectError> {
    let kind = simple_vector_ref(ctx, record, KIND)?;
    let slot = if kind == HANDLER_TAG {
        HANDLER_PREVIOUS
    } else {
        RESTART_PREVIOUS
    };
    simple_vector_ref(ctx, record, slot)
}

/// Read the depth of a handler-cluster record, dispatching on its kind tag
/// because handler and restart records store `depth` at different indices.
pub fn cluster_record_depth(ctx: &ThreadContext, record: Word) -> Result<i64, ObjectError> {
    let kind = simple_vector_ref(ctx, record, KIND)?;
    let slot = if kind == HANDLER_TAG {
        HANDLER_DEPTH
    } else {
        RESTART_DEPTH
    };
    Ok(simple_vector_ref(ctx, record, slot)?
        .as_fixnum()
        .unwrap_or(0))
}

/// Compute the dynamic depth of a new handler-cluster record.
pub fn cluster_next_depth(ctx: &ThreadContext, previous: Word) -> Result<Word, ObjectError> {
    if previous == Word::NIL {
        return Ok(Word::fixnum(1));
    }
    Ok(Word::fixnum(cluster_record_depth(ctx, previous)? + 1))
}

/// Compute the dynamic depth of a new cleanup record. The cleanup chain is
/// uniform, so its records share one depth slot.
pub fn cleanup_next_depth(ctx: &ThreadContext, previous: Word) -> Result<Word, ObjectError> {
    if previous == Word::NIL {
        return Ok(Word::fixnum(1));
    }
    let depth = simple_vector_ref(ctx, previous, CLEANUP_DEPTH)?
        .as_fixnum()
        .unwrap_or(0)
        + 1;
    Ok(Word::fixnum(depth))
}

#[allow(
    clippy::cast_possible_truncation,
    reason = "control pointers are usize-sized on supported targets"
)]
const fn to_pointer(word: Word) -> usize {
    word.bits() as usize
}

const fn from_pointer(pointer: usize) -> Word {
    Word::from_bits(pointer as u64)
}
