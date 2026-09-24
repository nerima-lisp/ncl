//! Lisp thread entities, the OS-thread registry, and lifecycle operations.
//!
//! Phase 1 models one OS thread as one Lisp thread. Each spawned thread creates
//! and registers its own [`ThreadContext`], participates in cooperative
//! safepoints, and unregisters before its OS thread exits. The Lisp thread
//! object is a heap instance carrying a name, a scalar identifier, a state, and
//! the recorded function object.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use ncl_object::{Runtime, ThreadContext};

use crate::ThreadError;
use crate::state::lock;

/// Slot index of the thread name.
pub const NAME_SLOT: usize = 0;
/// Slot index of the scalar thread identifier.
pub const ID_SLOT: usize = 1;
/// Slot index of the thread state word.
pub const STATE_SLOT: usize = 2;
/// Slot index of the recorded function object.
pub const FUNCTION_SLOT: usize = 3;

/// State word stored in a thread object: the thread is running.
pub const STATE_RUNNING: i64 = 0;
/// State word stored in a thread object: the thread has exited.
pub const STATE_FINISHED: i64 = 1;
/// State word stored in a thread object: the thread was terminated.
pub const STATE_TERMINATED: i64 = 2;

/// The identifier of the Lisp thread that owns the main OS thread.
pub const MAIN_THREAD_ID: u64 = 0;

/// A unique Lisp thread identifier.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ThreadId(u64);

impl ThreadId {
    /// Return the raw identifier.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Wrap a raw identifier.
    #[must_use]
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

/// A Rust entry point run on a newly spawned Lisp thread.
///
/// The body receives the shared runtime and its own registered context. It must
/// return before the context is dropped; the runtime outlives the context
/// because the spawned closure owns an [`Arc`] of it.
pub type ThreadBody = fn(&Runtime, &mut ThreadContext) -> Result<(), ThreadError>;

/// Lifecycle state of a registered Lisp thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Life {
    Running,
    Finished,
    Terminated,
}

#[derive(Debug)]
struct Record {
    name: String,
    life: Life,
    terminate: bool,
    interrupt: bool,
    result: Option<Result<(), ThreadError>>,
}

#[derive(Debug, Default)]
struct Registry {
    next_id: u64,
    live: HashMap<u64, Record>,
}

static REGISTRY: OnceLock<(Mutex<Registry>, Condvar)> = OnceLock::new();

thread_local! {
    static CURRENT: Cell<Option<u64>> = const { Cell::new(None) };
}

fn registry() -> &'static (Mutex<Registry>, Condvar) {
    REGISTRY.get_or_init(|| (Mutex::new(Registry::default()), Condvar::new()))
}

fn lock_registry() -> MutexGuard<'static, Registry> {
    lock(&registry().0)
}

/// Spawn a Lisp thread that runs `body`.
///
/// The spawned thread registers its own [`ThreadContext`], records its
/// identifier, runs `body`, unregisters, and then publishes completion. The
/// `runtime` handle is shared with the child so it outlives the child's
/// context.
///
/// # Errors
/// Returns [`ThreadError::SpawnFailed`] when the OS refuses to create the
/// thread.
pub fn spawn(
    runtime: &Arc<Runtime>,
    name: &str,
    body: ThreadBody,
) -> Result<ThreadId, ThreadError> {
    let id = {
        let mut registry = lock_registry();
        registry.next_id = registry.next_id.saturating_add(1);
        let id = registry.next_id;
        registry.live.insert(
            id,
            Record {
                name: name.to_owned(),
                life: Life::Running,
                terminate: false,
                interrupt: false,
                result: None,
            },
        );
        id
    };
    let child_runtime = Arc::clone(runtime);
    let started = std::thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || run_thread(&child_runtime, id, body));
    let Ok(handle) = started else {
        lock_registry().live.remove(&id);
        return Err(ThreadError::SpawnFailed);
    };
    drop(handle);
    Ok(ThreadId(id))
}

fn run_thread(runtime: &Runtime, id: u64, body: ThreadBody) {
    CURRENT.with(|current| current.set(Some(id)));
    let mut context = ThreadContext::new();
    let outcome = match context.register(runtime) {
        Ok(()) => body(runtime, &mut context),
        Err(error) => Err(ThreadError::from(error)),
    };
    drop(context);
    CURRENT.with(|current| current.set(None));
    let (mutex, condvar) = registry();
    {
        let mut registry = mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(record) = registry.live.get_mut(&id) {
            record.life = if record.terminate {
                Life::Terminated
            } else {
                Life::Finished
            };
            record.result = Some(outcome);
        }
    }
    condvar.notify_all();
}

/// Return the identifier of the calling Lisp thread.
#[must_use]
pub fn current_id() -> ThreadId {
    ThreadId(CURRENT.with(std::cell::Cell::get).unwrap_or(MAIN_THREAD_ID))
}

/// Wait for a thread to exit and report the body's outcome.
///
/// # Errors
/// Returns [`ThreadError::NotRunning`] when the identifier is unknown,
/// [`ThreadError::Deadlock`] when asked to join the main thread, and
/// [`ThreadError::JoinTimeout`] when `timeout` elapses first.
pub fn join(id: ThreadId, timeout: Option<Duration>) -> Result<(), ThreadError> {
    if id.0 == MAIN_THREAD_ID {
        return Err(ThreadError::Deadlock);
    }
    let (mutex, condvar) = registry();
    let mut registry = mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let deadline = timeout.map(|span| Instant::now() + span);
    loop {
        let record = registry.live.get(&id.0).ok_or(ThreadError::NotRunning)?;
        if record.life != Life::Running {
            return record.result.unwrap_or(Ok(()));
        }
        match deadline {
            Some(deadline) => {
                let now = Instant::now();
                if now >= deadline {
                    return Err(ThreadError::JoinTimeout);
                }
                let (guard, _) = condvar
                    .wait_timeout(registry, deadline - now)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                registry = guard;
            }
            None => {
                registry = condvar
                    .wait(registry)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
            }
        }
    }
}

/// Whether a registered thread is still running.
#[must_use]
pub fn alive_p(id: ThreadId) -> bool {
    if id.0 == MAIN_THREAD_ID {
        return true;
    }
    lock_registry()
        .live
        .get(&id.0)
        .is_some_and(|record| record.life == Life::Running)
}

/// Return a thread's registered name.
#[must_use]
pub fn thread_name(id: ThreadId) -> Option<String> {
    if id.0 == MAIN_THREAD_ID {
        return Some("main thread".to_owned());
    }
    lock_registry()
        .live
        .get(&id.0)
        .map(|record| record.name.clone())
}

/// Return the recorded OS thread identifier for a thread.
///
/// Phase 1 reports the Lisp thread identifier because `ncl-sys` exposes
/// `pthread_self` only as a raw low-level declaration.
#[must_use]
pub fn os_tid(id: ThreadId) -> Option<u64> {
    if id.0 == MAIN_THREAD_ID {
        return Some(id.0);
    }
    lock_registry().live.contains_key(&id.0).then_some(id.0)
}

/// Return every registered thread identifier, in creation order.
#[must_use]
pub fn all_ids() -> Vec<ThreadId> {
    let mut ids: Vec<u64> = lock_registry().live.keys().copied().collect();
    ids.sort_unstable();
    ids.into_iter().map(ThreadId).collect()
}

/// Ask a running thread to stop at its next cooperative check.
///
/// # Errors
/// Returns [`ThreadError::NotRunning`] when the identifier is unknown.
pub fn terminate(id: ThreadId) -> Result<(), ThreadError> {
    let (mutex, condvar) = registry();
    let mut registry = mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let record = registry
        .live
        .get_mut(&id.0)
        .ok_or(ThreadError::NotRunning)?;
    record.terminate = true;
    drop(registry);
    condvar.notify_all();
    Ok(())
}

/// Whether a termination request is pending for a thread.
#[must_use]
pub fn should_terminate(id: ThreadId) -> bool {
    lock_registry()
        .live
        .get(&id.0)
        .is_some_and(|record| record.terminate)
}

/// Request delivery of an interrupt on a thread's next poll.
///
/// # Errors
/// Returns [`ThreadError::NotRunning`] when the identifier is unknown.
pub fn interrupt(id: ThreadId) -> Result<(), ThreadError> {
    let (mutex, condvar) = registry();
    let mut registry = mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let record = registry
        .live
        .get_mut(&id.0)
        .ok_or(ThreadError::NotRunning)?;
    record.interrupt = true;
    drop(registry);
    condvar.notify_all();
    Ok(())
}

/// Whether an interrupt is pending for a thread, clearing the request.
#[must_use]
pub fn take_interrupt(id: ThreadId) -> bool {
    let mut registry = lock_registry();
    registry.live.get_mut(&id.0).is_some_and(|record| {
        let pending = record.interrupt;
        record.interrupt = false;
        pending
    })
}

/// Drop every record for a thread that has exited.
#[must_use]
pub fn dispose_finished() -> usize {
    let mut registry = lock_registry();
    let before = registry.live.len();
    registry
        .live
        .retain(|_, record| record.life == Life::Running);
    before - registry.live.len()
}

/// Yield the processor to another runnable OS thread.
pub fn thread_yield() {
    std::thread::yield_now();
}

/// Park the calling thread until it is terminated.
///
/// This is the body of a Phase 1 thread created by `make-thread`: there is no
/// Rust-to-Lisp call path yet, so the thread registers, participates in
/// cooperative safepoints, and waits until it is terminated.
///
/// # Errors
/// This body does not fail.
pub fn idle_body(_runtime: &Runtime, context: &mut ThreadContext) -> Result<(), ThreadError> {
    let id = current_id();
    loop {
        ncl_sys::poll_safepoint(context.thread_mut());
        let _interrupted = take_interrupt(id);
        if should_terminate(id) {
            return Ok(());
        }
        let (mutex, condvar) = registry();
        let registry = mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if registry
            .live
            .get(&id.0)
            .is_none_or(|record| !record.terminate)
        {
            let _ = condvar
                .wait_timeout(registry, Duration::from_millis(5))
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }
}
