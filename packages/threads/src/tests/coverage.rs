#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on runtime results"
)]

use std::{sync::Arc, time::Duration};

use ncl_object::{
    Runtime, ThreadContext, Word, make_cons, make_string, simple_vector_length, simple_vector_ref,
    string_length,
};

use crate::{
    MonotonicDeadline, MutexKind, ThreadError, ThreadId, call_with_timing,
    clear_semaphore_notification, condition_broadcast, condition_notify, condition_wait,
    current_thread, deadline_timeout_condition, decode_timeout, defer_deadline, enable_interrupt,
    finished_state, get_foreground, get_mutex, get_spinlock, holding_mutex_p, list_all_threads,
    list_all_timers, main_thread_p, make_mutex, make_process, make_rwlock, make_semaphore,
    make_semaphore_notification, make_spinlock, make_thread, make_timer, make_waitqueue,
    mutex_name, mutex_owner, mutex_value, object_id, pop_deadline, process_alive_p, process_close,
    process_core_dumped, process_error, process_exit_code, process_input, process_kill,
    process_output, process_p, process_pid, process_plist, process_pty, process_status,
    process_wait, push_deadline, release_foreground, release_mutex, release_spinlock,
    run_expired_timers, rwlock_rdlock, rwlock_unlock, rwlock_wrlock, schedule_timer,
    semaphore_count, semaphore_name, semaphore_notification_status, signal_deadline,
    signal_semaphore, spinlock_held_p, terminate_thread, thread_alive_p, thread_error_thread,
    thread_name_of, thread_os_tid_of, thread_state_of, timer_name, timer_scheduled_p,
    try_semaphore, unschedule_timer, wait_on_semaphore, waitqueue_name, with_deadline,
    with_interrupts, with_mutex, with_recursive_lock, with_timeout, without_interrupts,
};

fn fixture() -> (Runtime, ThreadContext) {
    let runtime = Runtime::new().unwrap();
    crate::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    (runtime, ctx)
}

#[test]
fn thread_errors_display_and_source_are_stable() {
    let errors = [
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
    for error in errors {
        assert!(!error.to_string().is_empty());
        assert!(std::error::Error::source(&error).is_none());
    }
    let object = ThreadError::from(ncl_object::ObjectError::TypeError);
    assert!(object.to_string().starts_with("object error:"));
    assert!(std::error::Error::source(&object).is_some());
}

#[test]
fn process_accessors_cover_live_and_dead_slots() {
    let (runtime, mut ctx) = fixture();
    let process = make_process(&mut ctx, &runtime, 77, Word::fixnum(3)).unwrap();
    assert_eq!(process_p(&ctx, process).unwrap(), Word::TRUE);
    assert_eq!(process_pid(&ctx, process).unwrap().as_fixnum(), Some(77));
    assert_eq!(process_status(&ctx, process).unwrap(), Word::fixnum(3));
    assert_eq!(process_alive_p(&ctx, process).unwrap(), Word::TRUE);
    assert_eq!(
        process_wait(&mut ctx, process, Some(Duration::ZERO)),
        Err(ThreadError::Timeout)
    );
    assert_eq!(process_kill(&mut ctx, process).unwrap(), Word::TRUE);
    assert_eq!(process_alive_p(&ctx, process).unwrap(), Word::NIL);
    assert_eq!(
        process_wait(&mut ctx, process, None).unwrap().as_fixnum(),
        Some(-15)
    );
    assert_eq!(process_core_dumped(&ctx, process).unwrap(), Word::NIL);
    assert_eq!(process_pty(&ctx, process).unwrap(), Word::NIL);
    assert_eq!(process_plist(&ctx, process).unwrap(), Word::NIL);
    assert_eq!(process_input(&ctx, process).unwrap(), Word::NIL);
    assert_eq!(process_output(&ctx, process).unwrap(), Word::NIL);
    assert_eq!(process_error(&ctx, process).unwrap(), Word::NIL);
    assert_eq!(process_close(&mut ctx, process).unwrap(), Word::TRUE);
    assert_eq!(process_p(&ctx, Word::fixnum(1)).unwrap(), Word::NIL);
}

#[test]
fn timer_and_deadline_values_round_trip() {
    let (runtime, mut ctx) = fixture();
    let timer = make_timer(&mut ctx, &runtime, "coverage").unwrap();
    assert_eq!(timer_scheduled_p(&ctx, timer).unwrap(), Word::NIL);
    schedule_timer(&mut ctx, timer, MonotonicDeadline::from_nanos(10)).unwrap();
    assert_eq!(timer_scheduled_p(&ctx, timer).unwrap(), Word::TRUE);
    assert!(run_expired_timers(MonotonicDeadline::from_nanos(9)).is_empty());
    let expired = run_expired_timers(MonotonicDeadline::from_nanos(10));
    assert_eq!(expired.len(), 1);
    assert_eq!(timer_scheduled_p(&ctx, timer).unwrap(), Word::NIL);
    unschedule_timer(&mut ctx, timer).unwrap();
    assert_eq!(
        string_length(&ctx, timer_name(&ctx, timer).unwrap()).unwrap(),
        8
    );
    assert_eq!(signal_deadline(&ctx).unwrap(), Word::NIL);
    assert_eq!(push_deadline(&mut ctx, Word::fixnum(1)).unwrap(), Word::NIL);
    let previous = push_deadline(&mut ctx, Word::fixnum(2)).unwrap();
    assert!(previous.is_fixnum());
    let deferred = defer_deadline(&mut ctx, Word::fixnum(3)).unwrap();
    assert!(deferred.is_fixnum());
    assert!(pop_deadline(&mut ctx).unwrap().is_fixnum());
    assert!(pop_deadline(&mut ctx).unwrap().is_fixnum());
    assert_eq!(pop_deadline(&mut ctx).unwrap(), Word::NIL);
}

#[test]
fn timeout_decoder_rejects_negative_and_invalid_nanoseconds() {
    let (runtime, mut ctx) = fixture();
    assert_eq!(decode_timeout(&mut ctx, Word::NIL).unwrap(), None);
    assert_eq!(
        decode_timeout(&mut ctx, Word::fixnum(2)).unwrap(),
        Some(Duration::from_secs(2))
    );
    let pair = make_cons(&mut ctx, &runtime, Word::fixnum(1), Word::fixnum(9)).unwrap();
    assert_eq!(
        decode_timeout(&mut ctx, pair).unwrap(),
        Some(Duration::from_secs(1) + Duration::from_nanos(9))
    );
    assert!(decode_timeout(&mut ctx, Word::fixnum(-1)).is_err());
    let bad = make_cons(
        &mut ctx,
        &runtime,
        Word::fixnum(1),
        Word::fixnum(1_000_000_000),
    )
    .unwrap();
    assert!(decode_timeout(&mut ctx, bad).is_err());
    assert_eq!(ThreadId::from_raw(4).get(), 4);
    let _ = make_string;
}

#[test]
#[allow(clippy::too_many_lines)]
fn synchronization_objects_return_concrete_states() {
    let (runtime, mut ctx) = fixture();

    let mutex = make_mutex(&mut ctx, &runtime, "mutex", MutexKind::NonRecursive).unwrap();
    assert_eq!(
        mutex_name(&ctx, mutex)
            .map(|word| string_length(&ctx, word).unwrap())
            .unwrap(),
        5
    );
    assert_eq!(mutex_owner(&ctx, mutex).unwrap(), Word::NIL);
    assert_eq!(mutex_value(&ctx, mutex).unwrap(), Word::fixnum(0));
    assert_eq!(get_mutex(&mut ctx, mutex, false, None).unwrap(), Word::TRUE);
    assert_eq!(holding_mutex_p(&ctx, mutex).unwrap(), Word::TRUE);
    assert_eq!(get_mutex(&mut ctx, mutex, false, None).unwrap(), Word::NIL);
    assert_eq!(mutex_value(&ctx, mutex).unwrap(), Word::fixnum(1));
    assert_eq!(release_mutex(&mut ctx, mutex), Ok(()));
    assert_eq!(holding_mutex_p(&ctx, mutex).unwrap(), Word::NIL);

    let recursive = make_mutex(&mut ctx, &runtime, "recursive", MutexKind::Recursive).unwrap();
    assert_eq!(
        with_recursive_lock(&mut ctx, recursive, |_ctx| Ok(Word::fixnum(9))).unwrap(),
        Word::fixnum(9)
    );
    assert_eq!(
        with_mutex(&mut ctx, recursive, |_ctx| Ok(Word::fixnum(8))).unwrap(),
        Word::fixnum(8)
    );
    assert_eq!(mutex_value(&ctx, recursive).unwrap(), Word::fixnum(0));

    let rwlock = make_rwlock(&mut ctx, &runtime, "rw").unwrap();
    assert_eq!(
        rwlock_rdlock(&mut ctx, rwlock, false, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        rwlock_rdlock(&mut ctx, rwlock, false, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        rwlock_wrlock(&mut ctx, rwlock, false, None).unwrap(),
        Word::NIL
    );
    assert_eq!(rwlock_unlock(&mut ctx, rwlock), Ok(()));
    assert_eq!(rwlock_unlock(&mut ctx, rwlock), Ok(()));
    assert_eq!(
        rwlock_wrlock(&mut ctx, rwlock, true, None).unwrap(),
        Word::TRUE
    );
    assert_eq!(rwlock_unlock(&mut ctx, rwlock), Ok(()));

    let semaphore = make_semaphore(&mut ctx, &runtime, "sem", 1).unwrap();
    assert_eq!(semaphore_count(&ctx, semaphore).unwrap(), Word::fixnum(1));
    assert_eq!(try_semaphore(&mut ctx, semaphore).unwrap(), Word::TRUE);
    assert_eq!(try_semaphore(&mut ctx, semaphore).unwrap(), Word::NIL);
    assert_eq!(
        wait_on_semaphore(&mut ctx, semaphore, Some(Duration::ZERO)),
        Err(ThreadError::Timeout)
    );
    assert_eq!(signal_semaphore(&mut ctx, semaphore).unwrap(), Word::TRUE);
    assert_eq!(
        wait_on_semaphore(&mut ctx, semaphore, Some(Duration::ZERO)).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        semaphore_name(&ctx, semaphore)
            .map(|word| string_length(&ctx, word).unwrap())
            .unwrap(),
        3
    );

    let queue = make_waitqueue(&mut ctx, &runtime, "queue").unwrap();
    assert_eq!(
        waitqueue_name(&ctx, queue)
            .map(|word| string_length(&ctx, word).unwrap())
            .unwrap(),
        5
    );
    assert_eq!(
        condition_wait(&mut ctx, queue, mutex, Some(Duration::ZERO)),
        Err(ThreadError::Deadlock)
    );
    get_mutex(&mut ctx, mutex, true, None).unwrap();
    assert_eq!(
        condition_wait(&mut ctx, queue, mutex, Some(Duration::ZERO)),
        Err(ThreadError::Timeout)
    );
    assert_eq!(condition_notify(&mut ctx, queue).unwrap(), Word::TRUE);
    assert_eq!(
        condition_wait(&mut ctx, queue, mutex, Some(Duration::ZERO)).unwrap(),
        Word::TRUE
    );
    assert_eq!(condition_broadcast(&mut ctx, queue).unwrap(), Word::TRUE);
    assert_eq!(
        condition_wait(&mut ctx, queue, mutex, Some(Duration::ZERO)).unwrap(),
        Word::TRUE
    );
    release_mutex(&mut ctx, mutex).unwrap();

    let notification = make_semaphore_notification(&mut ctx, &runtime, "note").unwrap();
    assert_eq!(
        semaphore_notification_status(&ctx, notification).unwrap(),
        Word::NIL
    );
    assert_eq!(
        clear_semaphore_notification(&mut ctx, notification).unwrap(),
        Word::TRUE
    );
    let spinlock = make_spinlock(&mut ctx, &runtime, "spin").unwrap();
    assert_eq!(spinlock_held_p(&ctx, spinlock).unwrap(), Word::NIL);
    assert_eq!(get_spinlock(&mut ctx, spinlock).unwrap(), Word::TRUE);
    assert_eq!(spinlock_held_p(&ctx, spinlock).unwrap(), Word::TRUE);
    assert_eq!(release_spinlock(&mut ctx, spinlock).unwrap(), Word::TRUE);
    assert_eq!(spinlock_held_p(&ctx, spinlock).unwrap(), Word::NIL);
    assert_eq!(get_foreground(&mut ctx, true, None).unwrap(), Word::TRUE);
    assert_eq!(get_foreground(&mut ctx, false, None).unwrap(), Word::TRUE);
    assert_eq!(release_foreground(&mut ctx).unwrap(), Word::TRUE);
    assert_eq!(release_foreground(&mut ctx), Err(ThreadError::Deadlock));
}

#[test]
fn thread_objects_and_registration_return_concrete_values() {
    let runtime = Arc::new(Runtime::new().unwrap());
    crate::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    let main = current_thread(&mut ctx, &runtime).unwrap();
    assert_eq!(main_thread_p(&ctx, main).unwrap(), Word::TRUE);
    assert_eq!(object_id(&ctx, main).unwrap().get(), 0);
    assert_eq!(thread_alive_p(&ctx, main).unwrap(), Word::TRUE);
    assert_eq!(thread_os_tid_of(&ctx, main).unwrap(), Word::fixnum(0));
    assert_ne!(list_all_threads(&mut ctx, &runtime).unwrap(), Word::NIL);
    assert_eq!(current_thread(&mut ctx, &runtime).unwrap(), main);

    let thread = make_thread(&mut ctx, &runtime, "worker", Word::TRUE).unwrap();
    assert_eq!(thread_alive_p(&ctx, thread).unwrap(), Word::TRUE);
    assert_eq!(main_thread_p(&ctx, thread).unwrap(), Word::NIL);
    assert_eq!(thread_state_of(&ctx, thread).unwrap(), 0);
    assert!(thread_os_tid_of(&ctx, thread).unwrap().is_fixnum());
    terminate_thread(&mut ctx, thread).unwrap();
    assert_eq!(thread_state_of(&ctx, thread).unwrap(), 2);
    crate::join_thread(&ctx, thread).unwrap();
    assert_eq!(
        string_length(&ctx, thread_name_of(&ctx, thread).unwrap()).unwrap(),
        6
    );
    assert_eq!(
        object_id(&ctx, Word::fixnum(1)),
        Err(ThreadError::NotAThread)
    );
    assert_eq!(
        thread_error_thread(Word::NIL),
        Err(ThreadError::MissingClass)
    );
    assert_eq!(finished_state(), 1);
}

#[test]
fn time_and_condition_entry_points_return_concrete_values() {
    let (runtime, mut ctx) = fixture();
    assert!(crate::get_time_of_day() > 0);
    assert_eq!(enable_interrupt(&mut ctx, false).unwrap(), Word::TRUE);
    assert_eq!(enable_interrupt(&mut ctx, true).unwrap(), Word::NIL);
    assert_eq!(
        without_interrupts(&mut ctx, |_ctx| Ok(Word::fixnum(1))).unwrap(),
        Word::fixnum(1)
    );
    assert_eq!(
        with_interrupts(&mut ctx, |_ctx| Ok(Word::fixnum(2))).unwrap(),
        Word::fixnum(2)
    );
    assert_eq!(
        with_deadline(&mut ctx, Word::fixnum(1), |_ctx| Ok(Word::fixnum(3))).unwrap(),
        Word::fixnum(3)
    );
    assert_eq!(
        with_timeout(&mut ctx, &runtime, Word::fixnum(1), |_ctx| Ok(
            Word::fixnum(4)
        ))
        .unwrap(),
        Word::fixnum(4)
    );
    let timing = call_with_timing(&mut ctx, &runtime, |_ctx| Ok(Word::fixnum(5))).unwrap();
    assert_eq!(simple_vector_length(&ctx, timing).unwrap(), 2);
    assert_eq!(simple_vector_ref(&ctx, timing, 1).unwrap(), Word::fixnum(5));
    assert_eq!(
        deadline_timeout_condition(&mut ctx, &runtime),
        Err(ThreadError::MissingClass)
    );
}

#[test]
fn timer_listing_and_process_exit_values_are_concrete() {
    let (runtime, mut ctx) = fixture();
    assert_eq!(list_all_timers(&mut ctx, &runtime), Ok(Word::NIL));
    let timer = make_timer(&mut ctx, &runtime, "timer").unwrap();
    assert_ne!(list_all_timers(&mut ctx, &runtime).unwrap(), Word::NIL);
    assert_eq!(
        timer_name(&ctx, timer)
            .map(|word| string_length(&ctx, word).unwrap())
            .unwrap(),
        5
    );
    schedule_timer(&mut ctx, timer, MonotonicDeadline::from_nanos(12)).unwrap();
    assert_eq!(
        run_expired_timers(MonotonicDeadline::from_nanos(11)).len(),
        0
    );
    assert_eq!(
        run_expired_timers(MonotonicDeadline::from_nanos(12)).len(),
        1
    );
    assert_eq!(unschedule_timer(&mut ctx, timer).unwrap(), Word::TRUE);

    let process = make_process(&mut ctx, &runtime, 99, Word::fixnum(7)).unwrap();
    assert_eq!(process_exit_code(&ctx, process).unwrap(), Word::NIL);
    process_kill(&mut ctx, process).unwrap();
    assert_eq!(process_exit_code(&ctx, process).unwrap(), Word::fixnum(-15));
    process_close(&mut ctx, process).unwrap();
    assert_eq!(process_alive_p(&ctx, process).unwrap(), Word::NIL);
}
