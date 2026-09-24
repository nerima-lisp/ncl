#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on synchronization outcomes"
)]

//! Mutex, read/write lock, semaphore, wait queue, spinlock, and foreground lock.

use std::time::Duration;

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_threads::{MutexKind, ThreadError};

const SHORT: Duration = Duration::from_millis(50);

struct Fixture {
    ctx: ThreadContext,
    runtime: Runtime,
}

fn fixture() -> Fixture {
    let runtime = Runtime::new().unwrap();
    ncl_threads::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    Fixture { ctx, runtime }
}

#[test]
fn a_non_recursive_mutex_is_held_once() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let mutex = ncl_threads::make_mutex(ctx, runtime, "guard", MutexKind::NonRecursive).unwrap();

    assert_eq!(
        ncl_threads::get_mutex(ctx, mutex, false, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::holding_mutex_p(ctx, mutex).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::get_mutex(ctx, mutex, false, None).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::get_mutex(ctx, mutex, true, Some(SHORT)),
        Err(ThreadError::Timeout)
    );
    assert_eq!(
        ncl_threads::mutex_value(ctx, mutex).unwrap().as_fixnum(),
        Some(1)
    );
    ncl_threads::release_mutex(ctx, mutex).unwrap();
    assert_eq!(ncl_threads::holding_mutex_p(ctx, mutex).unwrap(), Word::NIL);
    assert_eq!(ncl_threads::mutex_owner(ctx, mutex).unwrap(), Word::NIL);
}

#[test]
fn a_recursive_mutex_re_enters_for_its_owner() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let mutex = ncl_threads::make_mutex(ctx, runtime, "recursive", MutexKind::Recursive).unwrap();

    assert_eq!(
        ncl_threads::get_mutex(ctx, mutex, true, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::get_mutex(ctx, mutex, true, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::mutex_value(ctx, mutex).unwrap().as_fixnum(),
        Some(2)
    );
    assert_ne!(ncl_threads::mutex_owner(ctx, mutex).unwrap(), Word::NIL);
    ncl_threads::release_mutex(ctx, mutex).unwrap();
    ncl_threads::release_mutex(ctx, mutex).unwrap();
    assert_eq!(
        ncl_threads::mutex_value(ctx, mutex).unwrap().as_fixnum(),
        Some(0)
    );
}

#[test]
fn releasing_an_unheld_mutex_is_a_deadlock() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let mutex = ncl_threads::make_mutex(ctx, runtime, "free", MutexKind::NonRecursive).unwrap();
    assert_eq!(
        ncl_threads::release_mutex(ctx, mutex),
        Err(ThreadError::Deadlock)
    );
}

#[test]
fn with_mutex_releases_on_every_path() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let mutex = ncl_threads::make_mutex(ctx, runtime, "scoped", MutexKind::NonRecursive).unwrap();

    let value = ncl_threads::with_mutex(ctx, mutex, |_ctx| Ok(Word::fixnum(7))).unwrap();
    assert_eq!(value.as_fixnum(), Some(7));
    assert_eq!(ncl_threads::holding_mutex_p(ctx, mutex).unwrap(), Word::NIL);

    let failed: Result<Word, ThreadError> =
        ncl_threads::with_recursive_lock(ctx, mutex, |_ctx| Err(ThreadError::Timeout));
    assert_eq!(failed, Err(ThreadError::Timeout));
    assert_eq!(ncl_threads::holding_mutex_p(ctx, mutex).unwrap(), Word::NIL);
}

#[test]
fn mutex_name_is_recorded() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let mutex = ncl_threads::make_mutex(ctx, runtime, "named", MutexKind::NonRecursive).unwrap();
    let name = ncl_threads::mutex_name(ctx, mutex).unwrap();
    assert_eq!(ncl_object::string_length(ctx, name).unwrap(), 5);
}

#[test]
fn unknown_handles_are_rejected() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let mutex = ncl_threads::make_mutex(ctx, runtime, "x", MutexKind::NonRecursive).unwrap();
    let other = ncl_threads::make_mutex(ctx, runtime, "y", MutexKind::NonRecursive).unwrap();
    assert_ne!(mutex, other);
    assert!(matches!(
        ncl_threads::mutex_owner(ctx, Word::fixnum(0)),
        Err(ThreadError::NotAMutex)
    ));
}

#[test]
fn a_read_write_lock_admits_readers_then_a_writer() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let lock = ncl_threads::make_rwlock(ctx, runtime, "rw").unwrap();

    assert_eq!(
        ncl_threads::rwlock_rdlock(ctx, lock, false, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::rwlock_rdlock(ctx, lock, false, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::rwlock_wrlock(ctx, lock, false, None).unwrap(),
        Word::NIL
    );
    ncl_threads::rwlock_unlock(ctx, lock).unwrap();
    ncl_threads::rwlock_unlock(ctx, lock).unwrap();
    assert_eq!(
        ncl_threads::rwlock_wrlock(ctx, lock, true, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::rwlock_rdlock(ctx, lock, false, None).unwrap(),
        Word::NIL
    );
    ncl_threads::rwlock_unlock(ctx, lock).unwrap();
    assert_eq!(
        ncl_threads::rwlock_unlock(ctx, lock),
        Err(ThreadError::Deadlock)
    );
}

#[test]
fn a_semaphore_hands_out_and_returns_permits() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let semaphore = ncl_threads::make_semaphore(ctx, runtime, "sem", 1).unwrap();

    assert_eq!(
        ncl_threads::semaphore_count(ctx, semaphore)
            .unwrap()
            .as_fixnum(),
        Some(1)
    );
    assert_eq!(
        ncl_threads::try_semaphore(ctx, semaphore).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::try_semaphore(ctx, semaphore).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::wait_on_semaphore(ctx, semaphore, Some(SHORT)),
        Err(ThreadError::Timeout)
    );
    ncl_threads::signal_semaphore(ctx, semaphore).unwrap();
    assert_eq!(
        ncl_threads::wait_on_semaphore(ctx, semaphore, Some(SHORT)).unwrap(),
        Word::TRUE
    );
    let name = ncl_threads::semaphore_name(ctx, semaphore).unwrap();
    assert_eq!(ncl_object::string_length(ctx, name).unwrap(), 3);
}

#[test]
fn a_wait_queue_delivers_notifications() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let queue = ncl_threads::make_waitqueue(ctx, runtime, "wq").unwrap();
    let mutex = ncl_threads::make_mutex(ctx, runtime, "m", MutexKind::NonRecursive).unwrap();

    assert_eq!(
        ncl_threads::condition_wait(ctx, queue, mutex, Some(SHORT)),
        Err(ThreadError::Deadlock)
    );
    ncl_threads::get_mutex(ctx, mutex, true, None).unwrap();
    assert_eq!(
        ncl_threads::condition_wait(ctx, queue, mutex, Some(SHORT)),
        Err(ThreadError::Timeout)
    );
    assert_eq!(
        ncl_threads::holding_mutex_p(ctx, mutex).unwrap(),
        Word::TRUE
    );
    ncl_threads::condition_notify(ctx, queue).unwrap();
    assert_eq!(
        ncl_threads::condition_wait(ctx, queue, mutex, Some(SHORT)).unwrap(),
        Word::TRUE
    );
    ncl_threads::condition_broadcast(ctx, queue).unwrap();
    assert_eq!(
        ncl_threads::condition_wait(ctx, queue, mutex, Some(SHORT)).unwrap(),
        Word::TRUE
    );
    ncl_threads::release_mutex(ctx, mutex).unwrap();
    let name = ncl_threads::waitqueue_name(ctx, queue).unwrap();
    assert_eq!(ncl_object::string_length(ctx, name).unwrap(), 2);
}

#[test]
fn a_semaphore_notification_clears_its_status() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let notification = ncl_threads::make_semaphore_notification(ctx, runtime, "note").unwrap();
    assert_eq!(
        ncl_threads::semaphore_notification_status(ctx, notification).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::clear_semaphore_notification(ctx, notification).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::semaphore_notification_status(ctx, notification).unwrap(),
        Word::NIL
    );
}

#[test]
fn a_spinlock_is_held_until_released() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let spinlock = ncl_threads::make_spinlock(ctx, runtime, "spin").unwrap();
    assert_eq!(
        ncl_threads::spinlock_held_p(ctx, spinlock).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::get_spinlock(ctx, spinlock).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::spinlock_held_p(ctx, spinlock).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::release_spinlock(ctx, spinlock).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::spinlock_held_p(ctx, spinlock).unwrap(),
        Word::NIL
    );
}

#[test]
fn the_foreground_lock_is_reentrant_for_its_owner() {
    let mut fixture = fixture();
    let Fixture { ctx, .. } = &mut fixture;
    assert_eq!(
        ncl_threads::get_foreground(ctx, true, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::get_foreground(ctx, false, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(ncl_threads::release_foreground(ctx).unwrap(), Word::TRUE);
    assert_eq!(
        ncl_threads::release_foreground(ctx),
        Err(ThreadError::Deadlock)
    );
}
