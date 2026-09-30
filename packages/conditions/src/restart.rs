//! Restart and cleanup records, and non-local exit.

use ncl_object::{
    FunctionObject, Runtime, ThreadContext, Word, make_cons, make_simple_vector, pop_root,
    push_root,
};

use crate::class::string_words_equal;
use crate::error::ConditionError;
use crate::records;

/// A restart record pushed onto the handler cluster.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartRecord(Word);

impl RestartRecord {
    /// Return the underlying restart record word.
    #[must_use]
    pub const fn as_word(self) -> Word {
        self.0
    }
    /// Wrap a restart record word (for example one read back off the
    /// dynamic restart stack by `FIND-RESTART`/`COMPUTE-RESTARTS`).
    #[must_use]
    pub const fn from_word(word: Word) -> Self {
        Self(word)
    }
}

/// A cleanup record pushed onto the cleanup chain.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupRecord(Word);

/// Push a restart with the given name, function, report, and flags.
///
/// # Errors
/// Returns an object-layer error when the record cannot be allocated.
pub fn push_restart(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: Word,
    function: Word,
    report: Word,
    interactive: Word,
    test: Word,
) -> Result<RestartRecord, ConditionError> {
    let previous = records::cluster_head(ctx);
    let depth = records::cluster_next_depth(ctx, previous)?;
    let record = make_simple_vector(
        ctx,
        runtime,
        &[
            records::RESTART_TAG,
            name,
            function,
            report,
            interactive,
            test,
            previous,
            depth,
        ],
    )
    .map_err(ConditionError::from)?;
    records::set_cluster_head(ctx, record);
    Ok(RestartRecord(record))
}

/// Restore the handler cluster to the head before `record`.
pub fn pop_restart(ctx: &mut ThreadContext, record: RestartRecord) {
    let previous = records::RestartRecord::from_word(record.0)
        .previous(ctx)
        .unwrap_or(Word::NIL);
    records::set_cluster_head(ctx, previous);
}

/// Find the innermost restart whose name string equals `name`.
///
/// # Errors
/// Returns an object-layer error when a record or name is malformed.
pub fn find_restart(ctx: &ThreadContext, name: Word) -> Result<Option<Word>, ConditionError> {
    let mut head = records::cluster_head(ctx);
    while head != Word::NIL {
        let record = records::ClusterRecord::from_word(ctx, head).map_err(ConditionError::from)?;
        if let records::ClusterRecord::Restart(restart) = record {
            let restart_name = restart.name(ctx).map_err(ConditionError::from)?;
            if string_words_equal(ctx, restart_name, name)? {
                return Ok(Some(head));
            }
        }
        head = records::record_previous(ctx, head)?;
    }
    Ok(None)
}

/// Collect every active restart into a Lisp list, innermost first.
///
/// # Errors
/// Returns an object-layer error when a record or allocation fails.
pub fn compute_restarts(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
) -> Result<Word, ConditionError> {
    let mut list = Word::NIL;
    let mut head = records::cluster_head(ctx);
    while head != Word::NIL {
        let record = records::ClusterRecord::from_word(ctx, head).map_err(ConditionError::from)?;
        if matches!(record, records::ClusterRecord::Restart(_)) {
            let token_head = push_root(ctx, &mut head);
            let token_list = push_root(ctx, &mut list);
            list = make_cons(ctx, runtime, head, list).map_err(ConditionError::from)?;
            pop_root(ctx, token_list);
            pop_root(ctx, token_head);
        }
        head = records::record_previous(ctx, head)?;
    }
    Ok(list)
}

/// Invoke a restart with the given arguments.
///
/// When the restart's stored function is a real callable object (as
/// installed by `restart-bind`/`restart-case`), it is called with
/// `arguments` through the generic condition-function invocation hook; its
/// return value (or the transfer performed by a non-local exit inside it) is
/// this call's result, matching `INVOKE-RESTART`.
///
/// Some restarts (for example the `CONTINUE` restart `cerror` installs) use
/// the function slot as an opaque Phase 1 continuation marker rather than a
/// callable object. For those, invocation keeps the original Phase 1
/// contract: the marker word is returned as-is and a pending non-local exit
/// is recorded for the caller to interpret.
///
/// # Errors
/// Returns an object-layer error when `restart` is malformed or the callable
/// function fails.
pub fn invoke_restart(
    ctx: &mut ThreadContext,
    restart: Word,
    arguments: &[Word],
) -> Result<Word, ConditionError> {
    let restart_record = records::RestartRecord::from_word(restart);
    let function = restart_record.function(ctx).map_err(ConditionError::from)?;
    if let Ok(function_object) = FunctionObject::try_from(function) {
        return ctx
            .invoke_condition_handler(function_object.as_word(), arguments)
            .map_err(ConditionError::from);
    }
    ctx.set_non_local_exit(true);
    Ok(function)
}

/// Find a restart by name and invoke it with the given arguments.
///
/// # Errors
/// Returns [`ConditionError::RestartNotFound`] when no restart matches, or the
/// failures of [`find_restart`] and [`invoke_restart`].
pub fn invoke_restart_by_name(
    ctx: &mut ThreadContext,
    name: Word,
    arguments: &[Word],
) -> Result<Word, ConditionError> {
    let restart = find_restart(ctx, name)?.ok_or(ConditionError::RestartNotFound)?;
    invoke_restart(ctx, restart, arguments)
}

/// Read the name of a restart record as a Lisp string, or `NIL` for an
/// anonymous restart.
///
/// # Errors
/// Returns an object-layer error when `restart` is malformed.
pub fn restart_name(ctx: &ThreadContext, restart: Word) -> Result<Word, ConditionError> {
    records::RestartRecord::from_word(restart)
        .name(ctx)
        .map_err(ConditionError::from)
}

/// Push a cleanup record whose entry runs during unwind.
///
/// # Errors
/// Returns an object-layer error when the record cannot be allocated.
pub fn push_cleanup(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    entry: Word,
) -> Result<CleanupRecord, ConditionError> {
    let previous = records::cleanup_head(ctx);
    let depth = records::cleanup_next_depth(ctx, previous)?;
    let record = make_simple_vector(ctx, runtime, &[entry, previous, depth])
        .map_err(ConditionError::from)?;
    records::set_cleanup_head(ctx, record);
    Ok(CleanupRecord(record))
}

/// Restore the cleanup chain to the head before `record`.
pub fn pop_cleanup(ctx: &mut ThreadContext, record: CleanupRecord) {
    let previous = records::CleanupRecord::from_word(record.0)
        .previous(ctx)
        .unwrap_or(Word::NIL);
    records::set_cleanup_head(ctx, previous);
}

/// Run the pending non-local exit: clear the cleanup chain and mark the exit.
///
/// Phase 1 pops cleanup records without invoking their entry functions, since
/// those are generated-code addresses. The machine-side transfer to the
/// selected `target_pc` is the L13/L14 unwinder work.
pub fn unwind(ctx: &mut ThreadContext) {
    records::set_cleanup_head(ctx, Word::NIL);
    ctx.set_non_local_exit(true);
}
