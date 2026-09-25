//! Shared blocking state for mutexes, read/write locks, semaphores, wait
//! queues, condition variables, spinlocks, and the foreground lock.
//!
//! Every synchronization object is a heap instance whose scalar handle indexes a
//! process-wide table of blocking state. The table is guarded by one
//! `Mutex`/`Condvar` pair, so acquire, release, and wake are serialized while the
//! Lisp-visible object stays a movable heap value. Blocking waits bracket
//! themselves with `enter_native`/`leave_native` so a stop-the-world collection
//! does not wait for a parked mutator, and they poll the cooperative interrupt
//! flag before parking.
//!
//! The operation-specific entry points live in [`crate::mutex`] and
//! [`crate::semaphore`].

use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use ncl_object::{ThreadContext, Word};

use crate::ThreadError;
use crate::state::{lock, read_handle};
use crate::thread::{current_id, take_interrupt};

/// Whether a mutex re-enters for its owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MutexKind {
    /// A second acquisition by the owner blocks.
    NonRecursive,
    /// A second acquisition by the owner succeeds and increments the depth.
    Recursive,
}

/// A handle allocated for a synchronization resource.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SyncHandle(u64);

impl SyncHandle {
    pub const fn from_raw(value: u64) -> Self {
        Self(value)
    }
    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Debug)]
pub struct MutexRecord {
    pub(crate) owner: Option<u64>,
    pub(crate) depth: u64,
    pub(crate) recursive: bool,
}

#[derive(Debug, Default)]
pub struct RwLockRecord {
    pub(crate) readers: u64,
    pub(crate) writer: Option<u64>,
}

#[derive(Debug)]
pub struct SemaphoreRecord {
    pub(crate) count: u64,
    pub(crate) max: u64,
}

#[derive(Debug, Default)]
pub struct WaitQueueRecord {
    pub(crate) tokens: u64,
}

#[derive(Debug, Default)]
pub struct SyncState {
    pub(crate) next: u64,
    pub(crate) mutexes: HashMap<SyncHandle, MutexRecord>,
    pub(crate) rwlocks: HashMap<SyncHandle, RwLockRecord>,
    pub(crate) semaphores: HashMap<SyncHandle, SemaphoreRecord>,
    pub(crate) waitqueues: HashMap<SyncHandle, WaitQueueRecord>,
    pub(crate) spinlocks: HashMap<SyncHandle, bool>,
    pub(crate) foreground: Option<u64>,
}

static SYNC: OnceLock<(Mutex<SyncState>, Condvar)> = OnceLock::new();

pub fn sync() -> &'static (Mutex<SyncState>, Condvar) {
    SYNC.get_or_init(|| (Mutex::new(SyncState::default()), Condvar::new()))
}

pub const fn next_handle(state: &mut SyncState) -> SyncHandle {
    state.next = state.next.saturating_add(1);
    SyncHandle(state.next)
}

/// Block until `try_take` yields a value, the caller is interrupted, or
/// `timeout` expires.
///
/// # Errors
/// Returns [`ThreadError::Timeout`] on expiry and [`ThreadError::Interrupted`]
/// when a cooperative interrupt is pending.
pub fn block_until<T>(
    ctx: &mut ThreadContext,
    timeout: Option<Duration>,
    mut try_take: impl FnMut(&mut SyncState) -> Option<T>,
) -> Result<T, ThreadError> {
    let deadline = timeout.map(|span| Instant::now() + span);
    ncl_sys::enter_native(ctx.thread_mut());
    let (mutex, condvar) = sync();
    let mut state = lock(mutex);
    let outcome = loop {
        if let Some(value) = try_take(&mut state) {
            break Ok(value);
        }
        if take_interrupt(current_id()) {
            break Err(ThreadError::Interrupted);
        }
        match deadline {
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    break Err(ThreadError::Timeout);
                }
                let (guard, _) = condvar
                    .wait_timeout(state, deadline - now)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state = guard;
            }
            None => {
                state = condvar
                    .wait(state)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    };
    drop(state);
    ncl_sys::leave_native(ctx.thread_mut());
    outcome
}

pub fn mutex_handle(ctx: &ThreadContext, mutex: Word) -> Result<SyncHandle, ThreadError> {
    let handle =
        SyncHandle::from_raw(read_handle(ctx, mutex, 1).map_err(|_| ThreadError::NotAMutex)?);
    let (mutex, _) = sync();
    if lock(mutex).mutexes.contains_key(&handle) {
        Ok(handle)
    } else {
        Err(ThreadError::NotAMutex)
    }
}

pub fn semaphore_handle(ctx: &ThreadContext, semaphore: Word) -> Result<SyncHandle, ThreadError> {
    let handle = SyncHandle::from_raw(
        read_handle(ctx, semaphore, 1).map_err(|_| ThreadError::NotASemaphore)?,
    );
    let (mutex, _) = sync();
    if lock(mutex).semaphores.contains_key(&handle) {
        Ok(handle)
    } else {
        Err(ThreadError::NotASemaphore)
    }
}

pub fn waitqueue_handle(ctx: &ThreadContext, waitqueue: Word) -> Result<SyncHandle, ThreadError> {
    let handle = SyncHandle::from_raw(
        read_handle(ctx, waitqueue, 1).map_err(|_| ThreadError::NotAWaitQueue)?,
    );
    let (mutex, _) = sync();
    if lock(mutex).waitqueues.contains_key(&handle) {
        Ok(handle)
    } else {
        Err(ThreadError::NotAWaitQueue)
    }
}
