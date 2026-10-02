#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on thread API boundary outcomes"
)]

//! Boundary and lifecycle coverage for the public thread API.

use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use ncl_conditions::ConditionError;
use ncl_object::{ObjectError, Runtime, ThreadContext, Word};
use ncl_threads::{Process, ThreadError};

const SHORT: Duration = Duration::from_millis(50);

struct Fixture {
    ctx: ThreadContext,
    runtime: Arc<Runtime>,
    _guard: MutexGuard<'static, ()>,
}

fn fixture() -> Fixture {
    let guard = test_guard();
    let runtime = Arc::new(Runtime::new().unwrap());
    ncl_threads::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    Fixture {
        ctx,
        runtime,
        _guard: guard,
    }
}

fn test_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn wait_for(flag: &AtomicBool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(1);
    while !flag.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::yield_now();
    }
    flag.load(Ordering::Acquire)
}

#[test]
fn thread_errors_keep_their_display_and_source_contract() {
    let object = ThreadError::from(ObjectError::TypeError);
    assert!(object.to_string().starts_with("object error:"));
    assert!(Error::source(&object).is_some());
    assert_eq!(ThreadError::from(ObjectError::TypeError), object);

    let condition = ThreadError::from(ConditionError::Unhandled);
    assert!(condition.to_string().starts_with("condition error:"));
    assert!(Error::source(&condition).is_some());
    assert_eq!(ThreadError::from(ConditionError::Unhandled), condition);

    let simple = [
        ThreadError::NotAThread,
        ThreadError::NotAMutex,
        ThreadError::NotASemaphore,
        ThreadError::NotAWaitQueue,
        ThreadError::NotATimer,
        ThreadError::NotARwLock,
        ThreadError::NotAProcess,
        ThreadError::NotRunning,
        ThreadError::JoinTimeout,
        ThreadError::Timeout,
        ThreadError::Interrupted,
        ThreadError::Deadlock,
        ThreadError::SpawnFailed,
        ThreadError::MissingClass,
        ThreadError::RootStackCorrupted,
    ];
    for error in simple {
        assert!(!error.to_string().is_empty());
        assert!(Error::source(&error).is_none());
    }
}

#[test]
fn process_handles_round_trip_through_words() {
    let raw = Word::fixnum(42);
    let from_const = Process::from_word(raw);
    let from_trait = Process::from(raw);
    assert_eq!(from_const.as_word(), raw);
    assert_eq!(from_trait.as_word(), raw);
    assert_eq!(Word::from(from_const), raw);
}

static BLOCKED_READY: AtomicBool = AtomicBool::new(false);

fn blocked_on_semaphore(_runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    let semaphore = ncl_threads::make_semaphore(ctx, _runtime, "blocked", 0)?;
    BLOCKED_READY.store(true, Ordering::Release);
    ncl_threads::wait_on_semaphore(ctx, semaphore, Some(Duration::from_secs(1)))?;
    Ok(())
}

static FOREGROUND_READY: AtomicBool = AtomicBool::new(false);
static FOREGROUND_STOP: AtomicBool = AtomicBool::new(false);

fn foreground_holder(_runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    ncl_threads::get_foreground(ctx, true, None)?;
    FOREGROUND_READY.store(true, Ordering::Release);
    while !FOREGROUND_STOP.load(Ordering::Acquire) {
        ncl_threads::thread_yield();
    }
    ncl_threads::release_foreground(ctx)?;
    Ok(())
}

static FOREGROUND_WAITING: AtomicBool = AtomicBool::new(false);

fn foreground_waiter(_runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    FOREGROUND_WAITING.store(true, Ordering::Release);
    ncl_threads::get_foreground(ctx, true, None)?;
    ncl_threads::release_foreground(ctx).map(|_| ())
}

static SPINLOCK_FOR_WORKER: OnceLock<Word> = OnceLock::new();
static SPINLOCK_READY: AtomicBool = AtomicBool::new(false);
static SPINLOCK_ACQUIRED: AtomicBool = AtomicBool::new(false);

fn spinlock_waiter(_runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    let spinlock = *SPINLOCK_FOR_WORKER
        .get()
        .expect("spinlock worker handle is installed before spawning");
    SPINLOCK_READY.store(true, Ordering::Release);
    ncl_threads::get_spinlock(ctx, spinlock)?;
    SPINLOCK_ACQUIRED.store(true, Ordering::Release);
    ncl_threads::release_spinlock(ctx, spinlock).map(|_| ())
}

#[test]
fn thread_registry_covers_timeout_disposal_and_unknown_ids() {
    let fixture = fixture();
    let id = ncl_threads::spawn(&fixture.runtime, "boundary", ncl_threads::idle_body).unwrap();

    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        ncl_threads::join(id, Some(Duration::ZERO)),
        Err(ThreadError::JoinTimeout)
    );
    assert!(ncl_threads::alive_p(id));
    assert_eq!(ncl_threads::thread_name(id).as_deref(), Some("boundary"));
    assert_eq!(ncl_threads::os_tid(id), Some(id.get()));
    assert_eq!(
        ncl_threads::os_tid(ncl_threads::ThreadId::from_raw(ncl_threads::MAIN_THREAD_ID)),
        Some(ncl_threads::MAIN_THREAD_ID)
    );

    ncl_threads::terminate(id).unwrap();
    ncl_threads::join(id, Some(SHORT)).unwrap();
    assert!(!ncl_threads::alive_p(id));
    assert!(ncl_threads::should_terminate(id));
    assert!(ncl_threads::dispose_finished() >= 1);

    assert_eq!(ncl_threads::thread_name(id), None);
    assert_eq!(ncl_threads::os_tid(id), None);
    assert!(!ncl_threads::take_interrupt(id));
    assert_eq!(ncl_threads::terminate(id), Err(ThreadError::NotRunning));
    assert_eq!(ncl_threads::interrupt(id), Err(ThreadError::NotRunning));
    assert_eq!(
        ncl_threads::join(id, Some(SHORT)),
        Err(ThreadError::NotRunning)
    );
}

#[test]
fn a_blocking_thread_api_reports_an_interrupt() {
    let fixture = fixture();
    BLOCKED_READY.store(false, Ordering::Release);
    let id = ncl_threads::spawn(&fixture.runtime, "blocked", blocked_on_semaphore).unwrap();
    if !wait_for(&BLOCKED_READY) {
        let _ = ncl_threads::interrupt(id);
        let _ = ncl_threads::join(id, Some(Duration::from_secs(2)));
        panic!("blocking worker did not reach its wait");
    }

    ncl_threads::interrupt(id).unwrap();
    assert_eq!(
        ncl_threads::join(id, Some(Duration::from_secs(2))),
        Err(ThreadError::Interrupted)
    );
    assert!(ncl_threads::dispose_finished() >= 1);
}

#[test]
fn a_foreground_owner_blocks_another_thread_until_released() {
    let mut fixture = fixture();
    FOREGROUND_READY.store(false, Ordering::Release);
    FOREGROUND_STOP.store(false, Ordering::Release);
    let id = ncl_threads::spawn(&fixture.runtime, "foreground", foreground_holder).unwrap();
    if !wait_for(&FOREGROUND_READY) {
        FOREGROUND_STOP.store(true, Ordering::Release);
        let _ = ncl_threads::join(id, Some(Duration::from_secs(2)));
        panic!("foreground worker did not acquire the lock");
    }

    assert_eq!(
        ncl_threads::get_foreground(&mut fixture.ctx, true, Some(SHORT)),
        Err(ThreadError::Timeout)
    );
    assert_eq!(
        ncl_threads::get_foreground(&mut fixture.ctx, false, None),
        Ok(Word::NIL)
    );
    FOREGROUND_STOP.store(true, Ordering::Release);
    ncl_threads::join(id, Some(SHORT + SHORT)).unwrap();
    assert_eq!(
        ncl_threads::get_foreground(&mut fixture.ctx, false, None).unwrap(),
        Word::TRUE
    );
    ncl_threads::release_foreground(&mut fixture.ctx).unwrap();
}

#[test]
fn a_waiting_foreground_thread_is_released_by_the_owner() {
    let mut fixture = fixture();
    FOREGROUND_WAITING.store(false, Ordering::Release);
    ncl_threads::get_foreground(&mut fixture.ctx, false, None).unwrap();
    let id = ncl_threads::spawn(&fixture.runtime, "foreground-waiter", foreground_waiter).unwrap();
    if !wait_for(&FOREGROUND_WAITING) {
        ncl_threads::release_foreground(&mut fixture.ctx).unwrap();
        let _ = ncl_threads::join(id, Some(Duration::from_secs(2)));
        panic!("foreground waiter did not start");
    }

    std::thread::sleep(Duration::from_millis(20));
    ncl_threads::release_foreground(&mut fixture.ctx).unwrap();
    ncl_threads::join(id, Some(Duration::from_secs(2))).unwrap();
}

#[test]
fn a_spinlock_waiter_retries_after_the_owner_releases() {
    let mut fixture = fixture();
    let spinlock =
        ncl_threads::make_spinlock(&mut fixture.ctx, &fixture.runtime, "spin-boundary").unwrap();
    SPINLOCK_FOR_WORKER
        .set(spinlock)
        .expect("spinlock worker test runs once");
    SPINLOCK_READY.store(false, Ordering::Release);
    SPINLOCK_ACQUIRED.store(false, Ordering::Release);
    ncl_threads::get_spinlock(&mut fixture.ctx, spinlock).unwrap();

    let id = ncl_threads::spawn(&fixture.runtime, "spin-waiter", spinlock_waiter).unwrap();
    if !wait_for(&SPINLOCK_READY) {
        ncl_threads::release_spinlock(&mut fixture.ctx, spinlock).unwrap();
        let _ = ncl_threads::join(id, Some(Duration::from_secs(2)));
        panic!("spinlock worker did not start");
    }
    std::thread::sleep(Duration::from_millis(20));
    assert_eq!(
        ncl_threads::spinlock_held_p(&fixture.ctx, spinlock).unwrap(),
        Word::TRUE
    );
    ncl_threads::release_spinlock(&mut fixture.ctx, spinlock).unwrap();
    ncl_threads::join(id, Some(Duration::from_secs(2))).unwrap();
    assert!(SPINLOCK_ACQUIRED.load(Ordering::Acquire));
    assert_eq!(
        ncl_threads::spinlock_held_p(&fixture.ctx, spinlock).unwrap(),
        Word::NIL
    );
}

#[test]
fn synchronization_handles_reject_non_objects() {
    let mut fixture = fixture();
    let bad = Word::fixnum(0);
    let mutex = ncl_threads::make_mutex(
        &mut fixture.ctx,
        &fixture.runtime,
        "cross-mutex",
        ncl_threads::MutexKind::NonRecursive,
    )
    .unwrap();
    let rwlock = ncl_threads::make_rwlock(&mut fixture.ctx, &fixture.runtime, "cross-rw").unwrap();
    let semaphore =
        ncl_threads::make_semaphore(&mut fixture.ctx, &fixture.runtime, "cross-sem", 0).unwrap();
    let waitqueue =
        ncl_threads::make_waitqueue(&mut fixture.ctx, &fixture.runtime, "cross-wq").unwrap();

    assert_eq!(
        ncl_threads::get_mutex(&mut fixture.ctx, bad, false, None),
        Err(ThreadError::NotAMutex)
    );
    assert_eq!(
        ncl_threads::rwlock_rdlock(&mut fixture.ctx, mutex, false, None),
        Err(ThreadError::NotARwLock)
    );
    assert_eq!(
        ncl_threads::get_mutex(&mut fixture.ctx, rwlock, false, None),
        Err(ThreadError::NotAMutex)
    );
    assert_eq!(
        ncl_threads::rwlock_rdlock(&mut fixture.ctx, bad, false, None),
        Err(ThreadError::NotARwLock)
    );
    assert_eq!(
        ncl_threads::rwlock_wrlock(&mut fixture.ctx, bad, false, None),
        Err(ThreadError::NotARwLock)
    );
    assert_eq!(
        ncl_threads::wait_on_semaphore(&mut fixture.ctx, bad, Some(SHORT)),
        Err(ThreadError::NotASemaphore)
    );
    assert_eq!(
        ncl_threads::semaphore_count(&fixture.ctx, mutex),
        Err(ThreadError::NotASemaphore)
    );
    assert_eq!(
        ncl_threads::try_semaphore(&mut fixture.ctx, bad),
        Err(ThreadError::NotASemaphore)
    );
    assert_eq!(
        ncl_threads::signal_semaphore(&mut fixture.ctx, bad),
        Err(ThreadError::NotASemaphore)
    );
    assert_eq!(
        ncl_threads::condition_notify(&mut fixture.ctx, bad),
        Err(ThreadError::NotAWaitQueue)
    );
    assert_eq!(
        ncl_threads::condition_notify(&mut fixture.ctx, mutex),
        Err(ThreadError::NotAWaitQueue)
    );
    assert_eq!(
        ncl_threads::condition_notify(&mut fixture.ctx, semaphore),
        Err(ThreadError::NotAWaitQueue)
    );
    assert_eq!(
        ncl_threads::condition_notify(&mut fixture.ctx, waitqueue),
        Ok(Word::TRUE)
    );
    assert_eq!(
        ncl_threads::condition_broadcast(&mut fixture.ctx, bad),
        Err(ThreadError::NotAWaitQueue)
    );
    assert_eq!(
        ncl_threads::condition_broadcast(&mut fixture.ctx, waitqueue),
        Ok(Word::TRUE)
    );
    assert_eq!(
        ncl_threads::condition_notify(&mut fixture.ctx, waitqueue),
        Ok(Word::TRUE)
    );
    assert_eq!(
        ncl_threads::get_spinlock(&mut fixture.ctx, bad),
        Err(ThreadError::NotAMutex)
    );
    assert_eq!(
        ncl_threads::release_spinlock(&mut fixture.ctx, bad),
        Err(ThreadError::NotAMutex)
    );
    assert_eq!(
        ncl_threads::spinlock_held_p(&fixture.ctx, bad),
        Err(ThreadError::NotAMutex)
    );
    assert_eq!(
        ncl_threads::get_spinlock(&mut fixture.ctx, mutex),
        Err(ThreadError::NotAMutex)
    );
    assert_eq!(
        ncl_threads::timer_scheduled_p(&fixture.ctx, mutex),
        Err(ThreadError::NotATimer)
    );
}

#[test]
fn thread_objects_report_lists_and_finished_state() {
    let mut fixture = fixture();
    assert_eq!(
        ncl_threads::list_all_threads(&mut fixture.ctx, &fixture.runtime).unwrap(),
        Word::NIL
    );

    let object = ncl_threads::make_thread(
        &mut fixture.ctx,
        &fixture.runtime,
        "object-thread",
        Word::TRUE,
    )
    .unwrap();
    assert_ne!(
        ncl_threads::list_all_threads(&mut fixture.ctx, &fixture.runtime).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::thread_alive_p(&fixture.ctx, object).unwrap(),
        Word::TRUE
    );
    ncl_threads::interrupt_thread(&fixture.ctx, object).unwrap();
    ncl_threads::terminate_thread(&mut fixture.ctx, object).unwrap();
    ncl_threads::join_thread(&fixture.ctx, object).unwrap();
    assert_eq!(
        ncl_threads::thread_alive_p(&fixture.ctx, object).unwrap(),
        Word::NIL
    );
    let _ = ncl_threads::dispose_finished();
    assert!(
        ncl_threads::thread_os_tid_of(&fixture.ctx, object)
            .unwrap()
            .is_fixnum()
    );
}

#[test]
fn thread_object_operations_reject_non_threads() {
    let mut fixture = fixture();
    let bad = Word::fixnum(0);
    assert_eq!(
        ncl_threads::object_id(&fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
    assert_eq!(
        ncl_threads::join_thread(&fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
    assert_eq!(
        ncl_threads::thread_alive_p(&fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
    assert!(ncl_threads::thread_name_of(&fixture.ctx, bad).is_err());
    assert_eq!(
        ncl_threads::thread_os_tid_of(&fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
    assert_eq!(
        ncl_threads::terminate_thread(&mut fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
    assert_eq!(
        ncl_threads::interrupt_thread(&fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
    assert_eq!(
        ncl_threads::main_thread_p(&fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
    assert_eq!(
        ncl_threads::thread_state_of(&fixture.ctx, bad),
        Err(ThreadError::NotAThread)
    );
}

#[test]
fn read_write_locks_timeout_while_a_writer_is_held() {
    let mut fixture = fixture();
    let lock = ncl_threads::make_rwlock(&mut fixture.ctx, &fixture.runtime, "rw").unwrap();
    ncl_threads::rwlock_wrlock(&mut fixture.ctx, lock, false, None).unwrap();

    assert_eq!(
        ncl_threads::rwlock_rdlock(&mut fixture.ctx, lock, true, Some(SHORT)),
        Err(ThreadError::Timeout)
    );
    assert_eq!(
        ncl_threads::rwlock_wrlock(&mut fixture.ctx, lock, true, Some(SHORT)),
        Err(ThreadError::Timeout)
    );
    ncl_threads::rwlock_unlock(&mut fixture.ctx, lock).unwrap();
    assert_eq!(
        ncl_threads::rwlock_rdlock(&mut fixture.ctx, lock, true, None).unwrap(),
        Word::TRUE
    );
    ncl_threads::rwlock_unlock(&mut fixture.ctx, lock).unwrap();
}

#[test]
fn a_zero_semaphore_is_bounded_by_its_signal_capacity() {
    let mut fixture = fixture();
    let semaphore =
        ncl_threads::make_semaphore(&mut fixture.ctx, &fixture.runtime, "zero", 0).unwrap();

    assert_eq!(
        ncl_threads::semaphore_count(&fixture.ctx, semaphore)
            .unwrap()
            .as_fixnum(),
        Some(0)
    );
    ncl_threads::signal_semaphore(&mut fixture.ctx, semaphore).unwrap();
    ncl_threads::signal_semaphore(&mut fixture.ctx, semaphore).unwrap();
    assert_eq!(
        ncl_threads::semaphore_count(&fixture.ctx, semaphore)
            .unwrap()
            .as_fixnum(),
        Some(1)
    );
    ncl_threads::wait_on_semaphore(&mut fixture.ctx, semaphore, Some(SHORT)).unwrap();
    assert_eq!(
        ncl_threads::semaphore_count(&fixture.ctx, semaphore)
            .unwrap()
            .as_fixnum(),
        Some(0)
    );
}

#[test]
fn timeout_and_interrupt_scopes_restore_on_errors() {
    let mut fixture = fixture();
    let error = ThreadError::Timeout;

    assert!(ncl_threads::decode_timeout(&mut fixture.ctx, Word::fixnum(-1)).is_err());
    let negative_seconds = ncl_object::make_cons(
        &mut fixture.ctx,
        &fixture.runtime,
        Word::fixnum(-1),
        Word::fixnum(0),
    )
    .unwrap();
    assert!(ncl_threads::decode_timeout(&mut fixture.ctx, negative_seconds).is_err());
    let negative_nanos = ncl_object::make_cons(
        &mut fixture.ctx,
        &fixture.runtime,
        Word::fixnum(1),
        Word::fixnum(-1),
    )
    .unwrap();
    assert!(ncl_threads::decode_timeout(&mut fixture.ctx, negative_nanos).is_err());
    assert!(ncl_threads::push_deadline(&mut fixture.ctx, Word::fixnum(-1)).is_err());
    assert!(ncl_threads::defer_deadline(&mut fixture.ctx, Word::fixnum(-1)).is_err());
    assert!(ncl_threads::defer_deadline(&mut fixture.ctx, Word::fixnum(i64::MAX)).is_err());
    assert!(
        ncl_threads::defer_deadline(&mut fixture.ctx, Word::fixnum(1))
            .unwrap()
            .is_fixnum()
    );

    assert_eq!(
        ncl_threads::with_deadline(&mut fixture.ctx, Word::fixnum(1), |_ctx| {
            Err::<Word, ThreadError>(error)
        }),
        Err(error)
    );
    assert_eq!(
        ncl_threads::signal_deadline(&fixture.ctx).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::with_timeout(
            &mut fixture.ctx,
            &fixture.runtime,
            Word::fixnum(1),
            |_ctx| { Err::<Word, ThreadError>(error) }
        ),
        Err(error)
    );

    assert_eq!(
        ncl_threads::enable_interrupt(&mut fixture.ctx, false).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::with_interrupts(&mut fixture.ctx, |_ctx| { Err::<Word, ThreadError>(error) }),
        Err(error)
    );
    assert_eq!(
        ncl_threads::enable_interrupt(&mut fixture.ctx, true).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::without_interrupts(&mut fixture.ctx, |_ctx| {
            Err::<Word, ThreadError>(error)
        }),
        Err(error)
    );
    assert_eq!(
        ncl_threads::enable_interrupt(&mut fixture.ctx, true).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::call_with_timing(&mut fixture.ctx, &fixture.runtime, |_ctx| Err(error)),
        Err(error)
    );
}

#[test]
fn expired_timers_are_sorted_and_clear_their_schedules() {
    let mut fixture = fixture();
    assert_eq!(
        ncl_threads::MonotonicDeadline::from_nanos(42).as_nanos(),
        42
    );
    let first = ncl_threads::make_timer(&mut fixture.ctx, &fixture.runtime, "first").unwrap();
    let second = ncl_threads::make_timer(&mut fixture.ctx, &fixture.runtime, "second").unwrap();
    ncl_threads::schedule_timer(
        &mut fixture.ctx,
        first,
        ncl_threads::MonotonicDeadline::from_nanos(100),
    )
    .unwrap();
    ncl_threads::schedule_timer(
        &mut fixture.ctx,
        second,
        ncl_threads::MonotonicDeadline::from_nanos(100),
    )
    .unwrap();

    let expired = ncl_threads::run_expired_timers(ncl_threads::MonotonicDeadline::from_nanos(100));
    assert_eq!(expired.len(), 2);
    assert!(
        expired
            .windows(2)
            .all(|pair| pair[0].get() <= pair[1].get())
    );
    assert_eq!(
        ncl_threads::timer_scheduled_p(&fixture.ctx, first).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::timer_scheduled_p(&fixture.ctx, second).unwrap(),
        Word::NIL
    );
}
