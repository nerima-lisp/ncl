//! Handler, restart, cleanup, and catch record chains.
//!
//! The record values are Lisp simple vectors, but this module keeps their
//! chain operations behind typed opaque handles. Slot constants remain
//! crate-visible only for current lowering code.

#![allow(
    clippy::redundant_pub_crate,
    reason = "typed records stay private to this crate"
)]

use ncl_object::{ObjectError, ThreadContext, Word, simple_vector_ref};

/// Opaque handler-cluster record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HandlerRecord(Word);

/// Opaque restart record in the handler-cluster chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RestartRecord(Word);

/// Opaque cleanup-chain record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CleanupRecord(Word);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClusterRecord {
    Handler(HandlerRecord),
    Restart(RestartRecord),
}

impl ClusterRecord {
    pub(crate) fn from_word(ctx: &ThreadContext, word: Word) -> Result<Self, ObjectError> {
        let kind = simple_vector_ref(ctx, word, KIND)?;
        if kind == HANDLER_TAG {
            Ok(Self::Handler(HandlerRecord(word)))
        } else {
            Ok(Self::Restart(RestartRecord(word)))
        }
    }
}

impl HandlerRecord {
    pub(crate) fn class(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        simple_vector_ref(ctx, self.0, HANDLER_CLASS)
    }

    /// Read this record's previous handler-cluster record.
    pub(crate) fn previous(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        simple_vector_ref(ctx, self.0, HANDLER_PREVIOUS)
    }

    /// Read this record's dynamic depth.
    pub(crate) fn depth(self, ctx: &ThreadContext) -> Result<i64, ObjectError> {
        Ok(simple_vector_ref(ctx, self.0, HANDLER_DEPTH)?
            .as_fixnum()
            .unwrap_or(0))
    }
}

impl RestartRecord {
    pub(crate) const fn from_word(word: Word) -> Self {
        Self(word)
    }

    pub(crate) fn name(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        simple_vector_ref(ctx, self.0, RESTART_NAME)
    }

    pub(crate) fn function(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        simple_vector_ref(ctx, self.0, RESTART_FUNCTION)
    }

    /// Read this record's previous handler-cluster record.
    pub(crate) fn previous(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        simple_vector_ref(ctx, self.0, RESTART_PREVIOUS)
    }

    /// Read this record's dynamic depth.
    pub(crate) fn depth(self, ctx: &ThreadContext) -> Result<i64, ObjectError> {
        Ok(simple_vector_ref(ctx, self.0, RESTART_DEPTH)?
            .as_fixnum()
            .unwrap_or(0))
    }
}

impl CleanupRecord {
    pub(crate) const fn from_word(word: Word) -> Self {
        Self(word)
    }

    /// Read this record's previous cleanup record.
    pub(crate) fn previous(self, ctx: &ThreadContext) -> Result<Word, ObjectError> {
        simple_vector_ref(ctx, self.0, CLEANUP_PREVIOUS)
    }

    /// Read this record's dynamic depth.
    pub(crate) fn depth(self, ctx: &ThreadContext) -> Result<i64, ObjectError> {
        Ok(simple_vector_ref(ctx, self.0, CLEANUP_DEPTH)?
            .as_fixnum()
            .unwrap_or(0))
    }
}

/// Index of the leading record-kind tag, retained for current crate callers.
pub(crate) const KIND: usize = 0;
/// Kind tag for a handler record.
pub(crate) const HANDLER_TAG: Word = Word::fixnum(0);
/// Kind tag for a restart record.
pub(crate) const RESTART_TAG: Word = Word::fixnum(1);

/// Index of the handler record's class slot.
pub(crate) const HANDLER_CLASS: usize = 1;
/// Index of the handler record's previous slot.
pub(crate) const HANDLER_PREVIOUS: usize = 3;
/// Index of the handler record's depth slot.
pub(crate) const HANDLER_DEPTH: usize = 4;

/// Index of the restart record's name slot.
pub(crate) const RESTART_NAME: usize = 1;
/// Index of the restart record's function slot.
pub(crate) const RESTART_FUNCTION: usize = 2;
/// Index of the restart record's previous slot.
pub(crate) const RESTART_PREVIOUS: usize = 6;
/// Index of the restart record's depth slot.
pub(crate) const RESTART_DEPTH: usize = 7;

/// Index of the cleanup record's previous slot.
pub(crate) const CLEANUP_PREVIOUS: usize = 1;
/// Index of the cleanup record's depth slot.
pub(crate) const CLEANUP_DEPTH: usize = 2;

/// Read the handler-cluster chain head.
pub fn cluster_head(ctx: &ThreadContext) -> Word {
    let (handler, _, _) = ctx.control_pointers();
    handler.map_or(Word::NIL, word_from_pointer)
}

/// Write the handler-cluster chain head, preserving the other pointers.
pub const fn set_cluster_head(ctx: &mut ThreadContext, head: Word) {
    let (_, cleanup, catch) = ctx.control_pointers();
    ctx.set_control_pointers(pointer_from_word(head), cleanup, catch);
}

/// Read the cleanup chain head.
pub fn cleanup_head(ctx: &ThreadContext) -> Word {
    let (_, cleanup, _) = ctx.control_pointers();
    cleanup.map_or(Word::NIL, word_from_pointer)
}

/// Write the cleanup chain head, preserving the other pointers.
pub const fn set_cleanup_head(ctx: &mut ThreadContext, head: Word) {
    let (handler, _, catch) = ctx.control_pointers();
    ctx.set_control_pointers(handler, pointer_from_word(head), catch);
}

/// Read the `previous` link of a handler-cluster record.
pub fn record_previous(ctx: &ThreadContext, record: Word) -> Result<Word, ObjectError> {
    match ClusterRecord::from_word(ctx, record)? {
        ClusterRecord::Handler(value) => value.previous(ctx),
        ClusterRecord::Restart(value) => value.previous(ctx),
    }
}

/// Read the depth of a handler-cluster record.
pub fn cluster_record_depth(ctx: &ThreadContext, record: Word) -> Result<i64, ObjectError> {
    match ClusterRecord::from_word(ctx, record)? {
        ClusterRecord::Handler(value) => value.depth(ctx),
        ClusterRecord::Restart(value) => value.depth(ctx),
    }
}

/// Compute the dynamic depth of a new handler-cluster record.
pub fn cluster_next_depth(ctx: &ThreadContext, previous: Word) -> Result<Word, ObjectError> {
    if previous == Word::NIL {
        return Ok(Word::fixnum(1));
    }
    Ok(Word::fixnum(cluster_record_depth(ctx, previous)? + 1))
}

/// Compute the dynamic depth of a new cleanup record.
pub fn cleanup_next_depth(ctx: &ThreadContext, previous: Word) -> Result<Word, ObjectError> {
    if previous == Word::NIL {
        return Ok(Word::fixnum(1));
    }
    Ok(Word::fixnum(
        CleanupRecord::from_word(previous).depth(ctx)? + 1,
    ))
}

fn word_from_pointer(pointer: usize) -> Word {
    u64::try_from(pointer).map_or(Word::NIL, Word::from_bits)
}

#[cfg(target_pointer_width = "64")]
#[allow(
    clippy::unnecessary_wraps,
    reason = "ThreadContext stores optional control pointers"
)]
const fn pointer_from_word(word: Word) -> Option<usize> {
    Some(usize::from_ne_bytes(word.bits().to_ne_bytes()))
}

#[cfg(target_pointer_width = "32")]
#[allow(
    clippy::cast_possible_truncation,
    clippy::unnecessary_wraps,
    reason = "control pointers are usize-sized on supported targets"
)]
const fn pointer_from_word(word: Word) -> Option<usize> {
    Some(word.bits() as usize)
}
