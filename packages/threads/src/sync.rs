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

#[cfg(test)]
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use ncl_object::{ThreadContext, Word};

use crate::ThreadError;
use crate::state::{lock, read_handle};
use crate::thread::{current_id, take_interrupt};

#[cfg(test)]
thread_local! {
    static BEFORE_BLOCK_UNTIL_WAIT_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = const { RefCell::new(None) };
}

#[cfg(test)]
fn set_before_block_until_wait_hook(hook: impl FnOnce() + 'static) {
    BEFORE_BLOCK_UNTIL_WAIT_HOOK.with(|slot| *slot.borrow_mut() = Some(Box::new(hook)));
}

#[cfg(test)]
fn run_before_block_until_wait_hook() {
    BEFORE_BLOCK_UNTIL_WAIT_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook();
        }
    });
}

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
    ctx.thread_access().enter_native();
    let (mutex, condvar) = sync();
    let mut state = lock(mutex);
    let outcome = loop {
        if let Some(value) = try_take(&mut state) {
            break Ok(value);
        }
        #[cfg(test)]
        run_before_block_until_wait_hook();
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
    ctx.thread_access().leave_native();
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod spinlock_gc_tests {
    use crate::semaphore::{get_spinlock, make_spinlock, release_spinlock};
    use ncl_object::{Runtime, ThreadContext, Word};
    use ncl_sys::RootToken;
    use std::sync::Arc;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

    #[test]
    fn a_blocked_spinlock_waiter_acquires_after_collection() {
        let runtime = Arc::new(Runtime::new().unwrap());
        crate::register(&runtime).unwrap();

        let (spinlock_sender, spinlock_receiver) = mpsc::channel();
        let (owner_native_sender, owner_native_receiver) = mpsc::channel();
        let (owner_continue_sender, owner_continue_receiver) = mpsc::channel();
        let (owner_released_sender, owner_released_receiver) = mpsc::channel();
        let (owner_release_waiter_sender, owner_release_waiter_receiver) = mpsc::channel();
        let (owner_release_waiter_done_sender, owner_release_waiter_done_receiver) =
            mpsc::channel();
        let (waiter_waiting_sender, waiter_waiting_receiver) = mpsc::channel();
        let (waiter_continue_sender, waiter_continue_receiver) = mpsc::channel();
        let (waiter_acquired_sender, waiter_acquired_receiver) = mpsc::channel();
        let (waiter_finish_sender, waiter_finish_receiver) = mpsc::channel();
        let (collector_ready_sender, collector_ready_receiver) = mpsc::channel();
        let (collect_sender, collect_receiver) = mpsc::channel();
        let (collected_sender, collected_receiver) = mpsc::channel();
        let (collector_finish_sender, collector_finish_receiver) = mpsc::channel();

        let owner = spawn_spinlock_owner(
            Arc::clone(&runtime),
            spinlock_sender,
            owner_native_sender,
            owner_continue_receiver,
            owner_released_sender,
            owner_release_waiter_receiver,
            owner_release_waiter_done_sender,
        );
        let waiter = spawn_spinlock_waiter(
            Arc::clone(&runtime),
            spinlock_receiver,
            waiter_waiting_sender,
            waiter_continue_receiver,
            waiter_acquired_sender,
            waiter_finish_receiver,
        );
        let collector = spawn_collector(
            runtime,
            collector_ready_sender,
            collect_receiver,
            collected_sender,
            collector_finish_receiver,
        );

        assert!(
            owner_native_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok()
        );
        assert!(
            collector_ready_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok()
        );
        assert!(
            waiter_waiting_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok()
        );

        assert!(collect_sender.send(()).is_ok());
        assert!(
            collected_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok()
        );

        // The waiter's native stack may conservatively pin the lock.
        assert!(waiter_continue_sender.send(()).is_ok());
        assert!(owner_continue_sender.send(()).is_ok());
        assert!(
            owner_released_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok()
        );
        assert!(
            waiter_acquired_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok()
        );

        assert!(owner_release_waiter_sender.send(()).is_ok());
        assert!(
            owner_release_waiter_done_receiver
                .recv_timeout(Duration::from_secs(5))
                .is_ok()
        );
        assert!(waiter_finish_sender.send(()).is_ok());
        assert!(collector_finish_sender.send(()).is_ok());
        assert!(owner.join().is_ok());
        assert!(waiter.join().is_ok());
        assert!(collector.join().is_ok());
    }

    #[inline(never)]
    fn make_held_spinlock(ctx: &mut ThreadContext, runtime: &Runtime) -> (Box<Word>, RootToken) {
        let mut spinlock = Box::new(Word::NIL);
        let root = ctx.thread_access().push_root(spinlock.as_mut());
        *spinlock = make_spinlock(ctx, runtime, "rooted-spinlock").unwrap();
        assert_eq!(get_spinlock(ctx, *spinlock).unwrap(), Word::TRUE);
        (spinlock, root)
    }

    #[inline(never)]
    fn send_spinlock(sender: &Sender<Word>, spinlock: Word) {
        sender.send(spinlock).unwrap();
    }

    #[inline(never)]
    fn spawn_spinlock_owner(
        runtime: Arc<Runtime>,
        spinlock_sender: Sender<Word>,
        native_sender: Sender<()>,
        continue_receiver: Receiver<()>,
        released_sender: Sender<()>,
        release_waiter_receiver: Receiver<()>,
        release_waiter_done_sender: Sender<()>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut ctx = ThreadContext::new();
            ctx.register(&runtime).unwrap();
            let (spinlock, root) = make_held_spinlock(&mut ctx, &runtime);
            send_spinlock(&spinlock_sender, *spinlock);

            ctx.thread_access().enter_native();
            assert!(native_sender.send(()).is_ok());
            assert!(
                continue_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .is_ok()
            );
            ctx.thread_access().leave_native();

            release_spinlock(&mut ctx, *spinlock).unwrap();
            assert!(released_sender.send(()).is_ok());
            assert!(
                release_waiter_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .is_ok()
            );
            release_spinlock(&mut ctx, *spinlock).unwrap();
            assert!(release_waiter_done_sender.send(()).is_ok());
            assert!(ctx.thread_access().pop_root(root));
        })
    }

    #[inline(never)]
    fn spawn_spinlock_waiter(
        runtime: Arc<Runtime>,
        spinlock_receiver: Receiver<Word>,
        waiting_sender: Sender<()>,
        continue_receiver: Receiver<()>,
        acquired_sender: Sender<()>,
        finish_receiver: Receiver<()>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut ctx = ThreadContext::new();
            ctx.register(&runtime).unwrap();
            let spinlock = spinlock_receiver
                .recv_timeout(Duration::from_secs(5))
                .unwrap();
            ctx.set_strict_forwarding(true);
            super::set_before_block_until_wait_hook(move || {
                assert!(waiting_sender.send(()).is_ok());
                assert!(
                    continue_receiver
                        .recv_timeout(Duration::from_secs(5))
                        .is_ok()
                );
            });
            assert_eq!(get_spinlock(&mut ctx, spinlock).unwrap(), Word::TRUE);
            assert!(acquired_sender.send(()).is_ok());
            assert!(finish_receiver.recv_timeout(Duration::from_secs(5)).is_ok());
        })
    }

    #[inline(never)]
    fn spawn_collector(
        runtime: Arc<Runtime>,
        ready_sender: Sender<()>,
        collect_receiver: Receiver<()>,
        collected_sender: Sender<()>,
        finish_receiver: Receiver<()>,
    ) -> JoinHandle<()> {
        thread::spawn(move || {
            let mut ctx = ThreadContext::new();
            ctx.register(&runtime).unwrap();
            assert!(ready_sender.send(()).is_ok());
            assert!(
                collect_receiver
                    .recv_timeout(Duration::from_secs(5))
                    .is_ok()
            );
            ctx.collect(true).unwrap();
            assert!(collected_sender.send(()).is_ok());
            assert!(finish_receiver.recv_timeout(Duration::from_secs(5)).is_ok());
        })
    }
}
