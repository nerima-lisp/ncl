#![allow(
    clippy::unwrap_used,
    reason = "coverage tests assert on runtime results"
)]

use std::time::Duration;

use ncl_object::{Runtime, ThreadContext, Word, make_cons, make_string, string_length};

use crate::{
    MonotonicDeadline, ThreadError, ThreadId, decode_timeout, defer_deadline, make_process,
    make_timer, pop_deadline, process_alive_p, process_close, process_core_dumped, process_error,
    process_input, process_kill, process_output, process_p, process_pid, process_plist,
    process_pty, process_status, process_wait, push_deadline, run_expired_timers, schedule_timer,
    signal_deadline, timer_name, timer_scheduled_p, unschedule_timer,
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
