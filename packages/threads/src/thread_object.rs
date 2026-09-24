//! Lisp thread objects and the object-level operations over the registry.

use std::sync::Arc;

use ncl_object::{
    Instance, Runtime, ThreadContext, Word, make_cons, make_string, set_symbol_value, slot_ref,
    slot_set, symbol_value,
};

use crate::ThreadError;
use crate::state::{intern_internal, make_object, with_root};
use crate::thread::{
    ID_SLOT, MAIN_THREAD_ID, NAME_SLOT, STATE_FINISHED, STATE_RUNNING, STATE_SLOT,
    STATE_TERMINATED, ThreadId, alive_p, idle_body, interrupt, join, os_tid, spawn, terminate,
};

/// Read the scalar identifier recorded in a thread object.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] when the object has no identifier slot.
pub fn object_id(ctx: &ThreadContext, thread: Word) -> Result<ThreadId, ThreadError> {
    let value =
        slot_ref(ctx, Instance::from(thread), ID_SLOT).map_err(|_| ThreadError::NotAThread)?;
    value
        .as_fixnum()
        .and_then(|id| u64::try_from(id).ok())
        .map(ThreadId::from_raw)
        .ok_or(ThreadError::NotAThread)
}

/// Build the Lisp object that represents the calling (main) OS thread.
///
/// The object carries identifier `0`, is not backed by a registry record, and
/// is always reported alive.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_main_thread_object(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ThreadError> {
    let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    with_root(ctx, &mut name_word, |ctx, name_word| {
        let slots = [
            *name_word,
            Word::fixnum(0),
            Word::fixnum(STATE_RUNNING),
            Word::NIL,
        ];
        let mut object = make_object(ctx, runtime, "THREAD", &slots)?;
        with_root(ctx, &mut object, |ctx, object| {
            push_all_threads(ctx, runtime, *object)?;
            Ok(*object)
        })
    })
}

/// Create a Lisp thread object and start its OS thread.
///
/// The thread registers its own context and then parks in [`idle_body`]: Phase
/// 1 has no Rust-to-Lisp call path, so `function` is recorded on the object but
/// not invoked. Terminate the thread to release it.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built, and
/// [`ThreadError::SpawnFailed`] when the OS thread cannot start.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_thread(
    ctx: &mut ThreadContext,
    runtime: &Arc<Runtime>,
    name: &str,
    function: Word,
) -> Result<Word, ThreadError> {
    let mut name_word = make_string(ctx, runtime, &name.chars().collect::<Vec<_>>())?;
    with_root(ctx, &mut name_word, |ctx, name_word| {
        let mut function = function;
        with_root(ctx, &mut function, |ctx, function| {
            let slots = [
                *name_word,
                Word::fixnum(0),
                Word::fixnum(STATE_RUNNING),
                *function,
            ];
            let mut object = make_object(ctx, runtime, "THREAD", &slots)?;
            with_root(ctx, &mut object, |ctx, object| {
                let id = spawn(runtime, name, idle_body)?;
                let id = i64::try_from(id.get()).map_err(|_| ThreadError::NotAThread)?;
                slot_set(ctx, Instance::from(*object), ID_SLOT, Word::fixnum(id))?;
                push_all_threads(ctx, runtime, *object)?;
                Ok(*object)
            })
        })
    })
}

/// Append a thread object to the `SB-THREAD::*ALL-THREADS*` list.
///
/// The list lives in a symbol value cell, so the heap keeps the thread objects
/// alive without an unregistered Rust container.
fn push_all_threads(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    object: Word,
) -> Result<(), ThreadError> {
    let symbol = intern_internal(ctx, runtime, "SB-THREAD", "*ALL-THREADS*")?;
    let mut object = object;
    with_root(ctx, &mut object, |ctx, object| {
        let mut list = symbol_value(ctx, symbol)?;
        if list == Word::UNBOUND {
            list = Word::NIL;
        }
        with_root(ctx, &mut list, |ctx, list| {
            let cons = make_cons(ctx, runtime, *object, *list)?;
            set_symbol_value(ctx, symbol, cons)?;
            Ok(())
        })
    })
}

/// Return the `SB-THREAD::*ALL-THREADS*` list of thread objects.
///
/// # Errors
/// Returns an object-layer error when the internal symbol cannot be interned.
pub fn list_all_threads(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ThreadError> {
    let symbol = intern_internal(ctx, runtime, "SB-THREAD", "*ALL-THREADS*")?;
    let list = symbol_value(ctx, symbol)?;
    Ok(if list == Word::UNBOUND {
        Word::NIL
    } else {
        list
    })
}

/// Join a thread object.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] for a non-thread object and
/// [`ThreadError::NotRunning`] when its record has already been disposed.
pub fn join_thread(ctx: &ThreadContext, thread: Word) -> Result<(), ThreadError> {
    join(object_id(ctx, thread)?, None)
}

/// Return whether a thread object is still running.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] for a non-thread object.
pub fn thread_alive_p(ctx: &ThreadContext, thread: Word) -> Result<Word, ThreadError> {
    Ok(if alive_p(object_id(ctx, thread)?) {
        Word::TRUE
    } else {
        Word::NIL
    })
}

/// Return a thread object's recorded name.
///
/// # Errors
/// Returns an object-layer error when the name slot is malformed.
pub fn thread_name_of(ctx: &ThreadContext, thread: Word) -> Result<Word, ThreadError> {
    Ok(slot_ref(ctx, Instance::from(thread), NAME_SLOT)?)
}

/// Return a thread object's recorded OS thread identifier.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] for a non-thread object.
pub fn thread_os_tid_of(ctx: &ThreadContext, thread: Word) -> Result<Word, ThreadError> {
    let id = object_id(ctx, thread)?;
    let tid = i64::try_from(os_tid(id).unwrap_or_else(|| id.get()))
        .map_err(|_| ThreadError::NotAThread)?;
    Ok(Word::fixnum(tid))
}

/// Terminate a thread object at its next cooperative check.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] for a non-thread object.
pub fn terminate_thread(ctx: &mut ThreadContext, thread: Word) -> Result<(), ThreadError> {
    terminate(object_id(ctx, thread)?)?;
    slot_set(
        ctx,
        Instance::from(thread),
        STATE_SLOT,
        Word::fixnum(STATE_TERMINATED),
    )?;
    Ok(())
}

/// Request an interrupt on a thread object.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] for a non-thread object.
pub fn interrupt_thread(ctx: &ThreadContext, thread: Word) -> Result<(), ThreadError> {
    interrupt(object_id(ctx, thread)?)
}

/// Return the Lisp thread object for the calling thread.
///
/// The object is cached in `SB-THREAD::*CURRENT-THREAD*`. Per-thread
/// `*current-thread*` overrides are not wired because Phase 1 symbol value
/// cells are global defaults.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn current_thread(ctx: &mut ThreadContext, runtime: &Runtime) -> Result<Word, ThreadError> {
    let symbol = intern_internal(ctx, runtime, "SB-THREAD", "*CURRENT-THREAD*")?;
    let existing = symbol_value(ctx, symbol)?;
    if existing != Word::UNBOUND && existing != Word::NIL {
        return Ok(existing);
    }
    let mut object = make_main_thread_object(ctx, runtime, "main thread")?;
    with_root(ctx, &mut object, |ctx, object| {
        set_symbol_value(ctx, symbol, *object)?;
        Ok(*object)
    })
}

/// Return whether a thread object denotes the main thread.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] for a non-thread object.
pub fn main_thread_p(ctx: &ThreadContext, thread: Word) -> Result<Word, ThreadError> {
    Ok(if object_id(ctx, thread)?.get() == MAIN_THREAD_ID {
        Word::TRUE
    } else {
        Word::NIL
    })
}

/// Report that a condition's thread slot is not readable from this layer.
///
/// The condition classes that carry a thread slot belong to `ncl-conditions`,
/// which owns their slot layout.
///
/// # Errors
/// Always returns [`ThreadError::Unsupported`].
pub const fn thread_error_thread(_condition: Word) -> Result<Word, ThreadError> {
    Err(ThreadError::Unsupported(
        "condition slot access belongs to ncl-conditions",
    ))
}

/// Return the state word recorded in a thread object.
///
/// # Errors
/// Returns [`ThreadError::NotAThread`] for a non-thread object.
pub fn thread_state_of(ctx: &ThreadContext, thread: Word) -> Result<i64, ThreadError> {
    slot_ref(ctx, Instance::from(thread), STATE_SLOT)
        .map_err(|_| ThreadError::NotAThread)?
        .as_fixnum()
        .ok_or(ThreadError::NotAThread)
}

/// State word of a thread that has exited.
#[must_use]
pub const fn finished_state() -> i64 {
    STATE_FINISHED
}
