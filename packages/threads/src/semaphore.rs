//! Semaphores, wait queues, condition variables, spinlocks, and the foreground lock.
//!
//! The blocking state lives in [`crate::sync::SyncState`]; this module maps the
//! Lisp-visible objects onto it.

use std::time::Duration;

use ncl_object::{Runtime, ThreadContext, Word};

use crate::ThreadError;
use crate::state::{lock, make_named_object, read_handle, read_slot, write_slot};
use crate::sync::{
    SemaphoreRecord, SyncState, WaitQueueRecord, block_until, mutex_handle, next_handle,
    semaphore_handle, sync, waitqueue_handle,
};
use crate::thread::current_id;

/// Slot index of the status word in a semaphore notification object.
const NOTIFICATION_STATUS: usize = 2;
/// Slot index of the held flag in a spinlock object.
const SPINLOCK_HELD: usize = 2;
/// Maximum token count, used to mark a broadcast wait queue.
const BROADCAST: u64 = u64::MAX;

/// Create a counting semaphore with `count` available permits.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_semaphore(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
    count: u64,
) -> Result<Word, ThreadError> {
    let (mutex, _) = sync();
    let handle = next_handle(&mut lock(mutex));
    lock(mutex).semaphores.insert(
        handle,
        SemaphoreRecord {
            count,
            max: count.max(1),
        },
    );
    make_named_object(ctx, runtime, "SEMAPHORE", name, handle, &[])
}

/// Create a wait queue.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_waitqueue(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ThreadError> {
    let (mutex, _) = sync();
    let handle = next_handle(&mut lock(mutex));
    lock(mutex)
        .waitqueues
        .insert(handle, WaitQueueRecord::default());
    make_named_object(ctx, runtime, "WAITQUEUE", name, handle, &[])
}

/// Create a semaphore notification object.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_semaphore_notification(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ThreadError> {
    make_named_object(
        ctx,
        runtime,
        "SEMAPHORE-NOTIFICATION",
        name,
        0,
        &[Word::NIL],
    )
}

/// Create a spinlock.
///
/// # Errors
/// Returns an object-layer error when the object cannot be built.
///
/// # Panics
/// Panics if a root token cannot be removed in stack order.
pub fn make_spinlock(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    name: &str,
) -> Result<Word, ThreadError> {
    let (mutex, _) = sync();
    let handle = next_handle(&mut lock(mutex));
    lock(mutex).spinlocks.insert(handle, false);
    make_named_object(ctx, runtime, "SPINLOCK", name, handle, &[Word::NIL])
}
/// Return a semaphore's name.
///
/// # Errors
/// Returns an object-layer error when the name slot is malformed.
pub fn semaphore_name(ctx: &ThreadContext, semaphore: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, semaphore, 0)
}

/// Return a semaphore's available permit count.
///
/// # Errors
/// Returns [`ThreadError::NotASemaphore`] for an unknown semaphore.
pub fn semaphore_count(ctx: &ThreadContext, semaphore: Word) -> Result<Word, ThreadError> {
    let handle = semaphore_handle(ctx, semaphore)?;
    let (table, _) = sync();
    let state = lock(table);
    let count = state
        .semaphores
        .get(&handle)
        .map_or(0, |record| record.count);
    drop(state);
    Ok(Word::fixnum(
        i64::try_from(count).map_err(|_| ThreadError::Deadlock)?,
    ))
}

fn try_take_permit(state: &mut SyncState, handle: u64) -> Option<()> {
    let record = state.semaphores.get_mut(&handle)?;
    if record.count == 0 {
        return None;
    }
    record.count -= 1;
    Some(())
}

/// Wait for and consume a semaphore permit.
///
/// # Errors
/// Returns [`ThreadError::NotASemaphore`] for an unknown semaphore,
/// [`ThreadError::Timeout`] on expiry, and [`ThreadError::Interrupted`] when a
/// cooperative interrupt is pending.
pub fn wait_on_semaphore(
    ctx: &mut ThreadContext,
    semaphore: Word,
    timeout: Option<Duration>,
) -> Result<Word, ThreadError> {
    let handle = semaphore_handle(ctx, semaphore)?;
    block_until(ctx, timeout, |state| try_take_permit(state, handle))?;
    Ok(Word::TRUE)
}

/// Consume a semaphore permit without waiting.
///
/// # Errors
/// Returns [`ThreadError::NotASemaphore`] for an unknown semaphore.
pub fn try_semaphore(ctx: &mut ThreadContext, semaphore: Word) -> Result<Word, ThreadError> {
    let handle = semaphore_handle(ctx, semaphore)?;
    let (table, _) = sync();
    let mut state = lock(table);
    Ok(match try_take_permit(&mut state, handle) {
        Some(()) => Word::TRUE,
        None => Word::NIL,
    })
}

/// Return a semaphore permit and wake one waiter.
///
/// # Errors
/// Returns [`ThreadError::NotASemaphore`] for an unknown semaphore.
pub fn signal_semaphore(ctx: &mut ThreadContext, semaphore: Word) -> Result<Word, ThreadError> {
    let handle = semaphore_handle(ctx, semaphore)?;
    let (table, condvar) = sync();
    let mut state = lock(table);
    let record = state
        .semaphores
        .get_mut(&handle)
        .ok_or(ThreadError::NotASemaphore)?;
    record.count = record.count.saturating_add(1).min(record.max);
    drop(state);
    condvar.notify_all();
    Ok(Word::TRUE)
}

/// Return a wait queue's name.
///
/// # Errors
/// Returns an object-layer error when the name slot is malformed.
pub fn waitqueue_name(ctx: &ThreadContext, waitqueue: Word) -> Result<Word, ThreadError> {
    read_slot(ctx, waitqueue, 0)
}

/// Wait on a wait queue, releasing and re-acquiring `mutex` around the wait.
///
/// # Errors
/// Returns [`ThreadError::NotAWaitQueue`] or [`ThreadError::NotAMutex`] for
/// unknown objects, [`ThreadError::Timeout`] on expiry, and
/// [`ThreadError::Interrupted`] when a cooperative interrupt is pending.
pub fn condition_wait(
    ctx: &mut ThreadContext,
    waitqueue: Word,
    mutex: Word,
    timeout: Option<Duration>,
) -> Result<Word, ThreadError> {
    let queue = waitqueue_handle(ctx, waitqueue)?;
    let handle = mutex_handle(ctx, mutex)?;
    let me = current_id().get();
    let depth = {
        let (table, condvar) = sync();
        let mut state = lock(table);
        let record = state
            .mutexes
            .get_mut(&handle)
            .ok_or(ThreadError::NotAMutex)?;
        if record.owner != Some(me) {
            return Err(ThreadError::Deadlock);
        }
        let depth = record.depth;
        record.depth = 0;
        record.owner = None;
        drop(state);
        condvar.notify_all();
        depth
    };
    let waited = block_until(ctx, timeout, |state| {
        let record = state.waitqueues.get_mut(&queue)?;
        if record.tokens == 0 {
            return None;
        }
        if record.tokens != BROADCAST {
            record.tokens -= 1;
        }
        Some(())
    });
    {
        let (table, _) = sync();
        let mut state = lock(table);
        if let Some(record) = state.mutexes.get_mut(&handle) {
            record.owner = Some(me);
            record.depth = depth;
        }
    }
    waited?;
    Ok(Word::TRUE)
}

/// Wake one waiter on a wait queue.
///
/// # Errors
/// Returns [`ThreadError::NotAWaitQueue`] for an unknown queue.
pub fn condition_notify(ctx: &mut ThreadContext, waitqueue: Word) -> Result<Word, ThreadError> {
    let handle = waitqueue_handle(ctx, waitqueue)?;
    let (table, condvar) = sync();
    let mut state = lock(table);
    let record = state
        .waitqueues
        .get_mut(&handle)
        .ok_or(ThreadError::NotAWaitQueue)?;
    if record.tokens != BROADCAST {
        record.tokens = record.tokens.saturating_add(1);
    }
    drop(state);
    condvar.notify_all();
    Ok(Word::TRUE)
}

/// Wake every current and future waiter on a wait queue.
///
/// # Errors
/// Returns [`ThreadError::NotAWaitQueue`] for an unknown queue.
pub fn condition_broadcast(ctx: &mut ThreadContext, waitqueue: Word) -> Result<Word, ThreadError> {
    let handle = waitqueue_handle(ctx, waitqueue)?;
    let (table, condvar) = sync();
    let mut state = lock(table);
    let record = state
        .waitqueues
        .get_mut(&handle)
        .ok_or(ThreadError::NotAWaitQueue)?;
    record.tokens = BROADCAST;
    drop(state);
    condvar.notify_all();
    Ok(Word::TRUE)
}

/// Return a semaphore notification's status.
///
/// # Errors
/// Returns an object-layer error when the status slot is malformed.
pub fn semaphore_notification_status(
    ctx: &ThreadContext,
    notification: Word,
) -> Result<Word, ThreadError> {
    read_slot(ctx, notification, NOTIFICATION_STATUS)
}

/// Clear a semaphore notification's status.
///
/// # Errors
/// Returns an object-layer error when the status slot cannot be written.
pub fn clear_semaphore_notification(
    ctx: &mut ThreadContext,
    notification: Word,
) -> Result<Word, ThreadError> {
    write_slot(ctx, notification, NOTIFICATION_STATUS, Word::NIL)?;
    Ok(Word::TRUE)
}

fn spinlock_handle(ctx: &ThreadContext, spinlock: Word) -> Result<u64, ThreadError> {
    let handle = read_handle(ctx, spinlock, 1).map_err(|_| ThreadError::NotAMutex)?;
    let (table, _) = sync();
    if lock(table).spinlocks.contains_key(&handle) {
        Ok(handle)
    } else {
        Err(ThreadError::NotAMutex)
    }
}

/// Acquire a spinlock, blocking until it is free.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown spinlock,
/// [`ThreadError::Timeout`] on expiry, and [`ThreadError::Interrupted`] when a
/// cooperative interrupt is pending.
pub fn get_spinlock(ctx: &mut ThreadContext, spinlock: Word) -> Result<Word, ThreadError> {
    let handle = spinlock_handle(ctx, spinlock)?;
    block_until(ctx, None, |state| {
        let held = state.spinlocks.get_mut(&handle)?;
        if *held {
            return None;
        }
        *held = true;
        Some(())
    })?;
    write_slot(ctx, spinlock, SPINLOCK_HELD, Word::TRUE)?;
    Ok(Word::TRUE)
}

/// Release a spinlock.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown spinlock.
pub fn release_spinlock(ctx: &mut ThreadContext, spinlock: Word) -> Result<Word, ThreadError> {
    let handle = spinlock_handle(ctx, spinlock)?;
    let (table, condvar) = sync();
    let mut state = lock(table);
    if let Some(held) = state.spinlocks.get_mut(&handle) {
        *held = false;
    }
    drop(state);
    condvar.notify_all();
    write_slot(ctx, spinlock, SPINLOCK_HELD, Word::NIL)?;
    Ok(Word::TRUE)
}

/// Return whether a spinlock is held.
///
/// # Errors
/// Returns [`ThreadError::NotAMutex`] for an unknown spinlock.
pub fn spinlock_held_p(ctx: &ThreadContext, spinlock: Word) -> Result<Word, ThreadError> {
    let handle = spinlock_handle(ctx, spinlock)?;
    let (table, _) = sync();
    let state = lock(table);
    let held = state.spinlocks.get(&handle).copied().unwrap_or(false);
    drop(state);
    Ok(if held { Word::TRUE } else { Word::NIL })
}

/// Acquire the foreground lock.
///
/// # Errors
/// Returns [`ThreadError::Timeout`] on expiry and
/// [`ThreadError::Interrupted`] when a cooperative interrupt is pending.
pub fn get_foreground(
    ctx: &mut ThreadContext,
    waitp: bool,
    timeout: Option<Duration>,
) -> Result<Word, ThreadError> {
    let me = current_id().get();
    let take = |state: &mut SyncState| match state.foreground {
        None => {
            state.foreground = Some(me);
            Some(())
        }
        Some(owner) if owner == me => Some(()),
        Some(_) => None,
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

/// Release the foreground lock held by the calling thread.
///
/// # Errors
/// Returns [`ThreadError::Deadlock`] when the caller does not hold it.
pub fn release_foreground(_ctx: &mut ThreadContext) -> Result<Word, ThreadError> {
    let me = current_id().get();
    let (table, condvar) = sync();
    let mut state = lock(table);
    if state.foreground != Some(me) {
        return Err(ThreadError::Deadlock);
    }
    state.foreground = None;
    drop(state);
    condvar.notify_all();
    Ok(Word::TRUE)
}
