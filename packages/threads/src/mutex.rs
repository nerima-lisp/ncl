//! Mutex and read/write lock objects.
//!
//! The blocking state lives in [`crate::sync::SyncState`]; this module maps the
//! Lisp-visible mutex and read/write lock objects onto it.

use std::time::Duration;

use ncl_object::{Runtime, ThreadContext, Word};

use crate::ThreadError;
use crate::state::{lock, make_named_object, read_handle, read_slot};
use crate::sync::{
    MutexKind, MutexRecord, RwLockRecord, SyncState, block_until, mutex_handle, next_handle, sync,
};
use crate::thread::current_id;

/// Create a mutex.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_mutex(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    kind: MutexKind,
) -> Result<Word, ThreadError> {
    let recursive = i64::from(kind == MutexKind::Recursive);
    let (mutex, _) = sync();
    let handle = next_handle(&mut lock(mutex));
    lock(mutex).mutexes.insert(
        handle,
        MutexRecord {
            owner: None,
            depth: 0,
            recursive: kind == MutexKind::Recursive,
        },
    );
    make_named_object(
        ctx,
        runtime,
        "MUTEX",
        name,
        handle,
        &[Word::fixnum(recursive)],
    )
}

/// Create a read/write lock.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_rwlock(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ThreadError> {
    let (mutex, _) = sync();
    let handle = next_handle(&mut lock(mutex));
    lock(mutex).rwlocks.insert(handle, RwLockRecord::default());
    make_named_object(ctx, runtime, "RWLOCK", name, handle, &[])
}

/// Return a mutex's name.
///
/// # Errors
/// Returns an object-layer error when the name slot is malformed.
pub fn mutex_name(ctx: &ThreadContext, mutex: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, mutex, 0)
}

/// Return the identifier of the thread holding a mutex, or NIL.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown mutex.
pub fn mutex_owner(ctx: &ThreadContext, mutex: Word) -> Result<Word, ThreadError> {
    let handle = mutex_handle(ctx, mutex)?;
    let (table, _) = sync();
    let state = lock(table);
    let owner = state.mutexes.get(&handle).and_then(|record| record.owner);
    drop(state);
    Ok(fixnum_or_nil(owner))
}

/// Return a mutex's recursion depth.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown mutex.
pub fn mutex_value(ctx: &ThreadContext, mutex: Word) -> Result<Word, ThreadError> {
    let handle = mutex_handle(ctx, mutex)?;
    let (table, _) = sync();
    let state = lock(table);
    let depth = state.mutexes.get(&handle).map_or(0, |record| record.depth);
    drop(state);
    Ok(Word::fixnum(
        i64::try_from(depth).map_err(|_| ThreadError::Deadlock)?,
    ))
}

/// Return whether the calling thread holds a mutex.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown mutex.
pub fn holding_mutex_p(ctx: &ThreadContext, mutex: Word) -> Result<Word, ThreadError> {
    let handle = mutex_handle(ctx, mutex)?;
    let me = current_id().get();
    let (table, _) = sync();
    let state = lock(table);
    let held = state
        .mutexes
        .get(&handle)
        .is_some_and(|record| record.owner == Some(me));
    drop(state);
    Ok(if held { Word::TRUE } else { Word::NIL })
}

fn fixnum_or_nil(value: Option<u64>) -> Word {
    value
        .and_then(|id| i64::try_from(id).ok())
        .map_or(Word::NIL, Word::fixnum)
}

fn try_acquire(state: &mut SyncState, handle: u64, me: u64) -> Option<()> {
    let record = state.mutexes.get_mut(&handle)?;
    match record.owner {
        None => {
            record.owner = Some(me);
            record.depth = 1;
            Some(())
        }
        Some(owner) if owner == me && record.recursive => {
            record.depth = record.depth.saturating_add(1);
            Some(())
        }
        Some(_) => None,
    }
}

/// Acquire a mutex.
///
/// Returns `T` on success, `NIL` when `waitp` is false and the mutex is held,
/// and [`ThreadError::Timeout`] when a timeout expires.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown mutex,
/// [`ThreadError::Timeout`] on expiry, and [`ThreadError::Interrupted`] when a
/// cooperative interrupt is pending.
pub fn get_mutex(
    ctx: &mut ThreadContext,
    mutex: Word,
    waitp: bool,
    timeout: Option<Duration>,
) -> Result<Word, ThreadError> {
    let handle = mutex_handle(ctx, mutex)?;
    let me = current_id().get();
    if !waitp {
        let (table, _) = sync();
        let mut state = lock(table);
        return Ok(match try_acquire(&mut state, handle, me) {
            Some(()) => Word::TRUE,
            None => Word::NIL,
        });
    }
    block_until(ctx, timeout, |state| try_acquire(state, handle, me))?;
    Ok(Word::TRUE)
}

/// Release a mutex held by the calling thread.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown mutex and
/// [`ThreadError::Deadlock`] when the caller is not the owner.
pub fn release_mutex(ctx: &mut ThreadContext, mutex: Word) -> Result<(), ThreadError> {
    let handle = mutex_handle(ctx, mutex)?;
    let me = current_id().get();
    let (table, condvar) = sync();
    let mut state = lock(table);
    let record = state
        .mutexes
        .get_mut(&handle)
        .ok_or(ThreadError::NotAMutex)?;
    if record.owner != Some(me) {
        return Err(ThreadError::Deadlock);
    }
    record.depth = record.depth.saturating_sub(1);
    if record.depth == 0 {
        record.owner = None;
    }
    drop(state);
    condvar.notify_all();
    Ok(())
}

/// Run `f` while holding `mutex`, releasing it on every exit path.
///
/// # Errors
/// Returns the acquisition error or `f`'s error.
pub fn with_mutex<T>(
    ctx: &mut ThreadContext,
    mutex: Word,
    f: impl FnOnce(&mut ThreadContext) -> Result<T, ThreadError>,
) -> Result<T, ThreadError> {
    get_mutex(ctx, mutex, true, None)?;
    let result = f(ctx);
    let released = release_mutex(ctx, mutex);
    match (result, released) {
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// Run `f` while holding `mutex`, which must be a recursive mutex.
///
/// # Errors
/// Returns the acquisition error or `f`'s error.
pub fn with_recursive_lock<T>(
    ctx: &mut ThreadContext,
    mutex: Word,
    f: impl FnOnce(&mut ThreadContext) -> Result<T, ThreadError>,
) -> Result<T, ThreadError> {
    with_mutex(ctx, mutex, f)
}

fn rwlock_handle(ctx: &ThreadContext, lock_word: Word) -> Result<u64, ThreadError> {
    let handle = read_handle(ctx, lock_word, 1).map_err(|_| ThreadError::NotARwLock)?;
    let (table, _) = sync();
    if lock(table).rwlocks.contains_key(&handle) {
        Ok(handle)
    } else {
        Err(ThreadError::NotARwLock)
    }
}

/// Acquire a read/write lock for reading.
///
/// # Errors
/// Returns [`ThreadError::NotARwLock`] for an unknown lock,
/// [`ThreadError::Timeout`] on expiry, and [`ThreadError::Interrupted`] when a
/// cooperative interrupt is pending.
pub fn rwlock_rdlock(
    ctx: &mut ThreadContext,
    lock_word: Word,
    waitp: bool,
    timeout: Option<Duration>,
) -> Result<Word, ThreadError> {
    let handle = rwlock_handle(ctx, lock_word)?;
    let take = |state: &mut SyncState| {
        let record = state.rwlocks.get_mut(&handle)?;
        if record.writer.is_some() {
            return None;
        }
        record.readers = record.readers.saturating_add(1);
        Some(())
    };
    if !waitp {
        let (table, _) = sync();
        let mut state = lock(table);
        return Ok(match take(&mut state) {
            Some(()) => Word::TRUE,
            None => Word::NIL,
        });
    }
    block_until(ctx, timeout, take)?;
    Ok(Word::TRUE)
}

/// Acquire a read/write lock for writing.
///
/// # Errors
/// Returns [`ThreadError::NotARwLock`] for an unknown lock,
/// [`ThreadError::Timeout`] on expiry, and [`ThreadError::Interrupted`] when a
/// cooperative interrupt is pending.
pub fn rwlock_wrlock(
    ctx: &mut ThreadContext,
    lock_word: Word,
    waitp: bool,
    timeout: Option<Duration>,
) -> Result<Word, ThreadError> {
    let handle = rwlock_handle(ctx, lock_word)?;
    let me = current_id().get();
    let take = |state: &mut SyncState| {
        let record = state.rwlocks.get_mut(&handle)?;
        if record.writer.is_some() || record.readers != 0 {
            return None;
        }
        record.writer = Some(me);
        Some(())
    };
    if !waitp {
        let (table, _) = sync();
        let mut state = lock(table);
        return Ok(match take(&mut state) {
            Some(()) => Word::TRUE,
            None => Word::NIL,
        });
    }
    block_until(ctx, timeout, take)?;
    Ok(Word::TRUE)
}

/// Release a read/write lock held by the calling thread.
///
/// # Errors
/// Returns [`ThreadError::NotARwLock`] for an unknown lock and
/// [`ThreadError::Deadlock`] when nothing is held.
pub fn rwlock_unlock(ctx: &mut ThreadContext, lock_word: Word) -> Result<(), ThreadError> {
    let handle = rwlock_handle(ctx, lock_word)?;
    let me = current_id().get();
    let (table, condvar) = sync();
    let mut state = lock(table);
    let record = state
        .rwlocks
        .get_mut(&handle)
        .ok_or(ThreadError::NotARwLock)?;
    if record.writer == Some(me) {
        record.writer = None;
    } else if record.readers != 0 {
        record.readers -= 1;
    } else {
        return Err(ThreadError::Deadlock);
    }
    drop(state);
    condvar.notify_all();
    Ok(())
}
