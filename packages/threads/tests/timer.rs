#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on timer and deadline outcomes"
)]

//! Timers, wall-clock time, timeout decoding, deadlines, and interrupt enablement.

use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_threads::ThreadError;

struct Fixture {
    ctx: ThreadContext,
    runtime: Runtime,
    _guard: MutexGuard<'static, ()>,
}

fn timer_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn fixture() -> Fixture {
    let guard = timer_guard();
    let runtime = Runtime::new().unwrap();
    ncl_threads::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    Fixture {
        ctx,
        runtime,
        _guard: guard,
    }
}

#[test]
fn a_timer_schedules_and_expires() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx, .. } = &mut fixture;
    let timer = ncl_threads::make_timer(ctx, runtime, "tick").unwrap();

    assert_eq!(
        ncl_threads::timer_scheduled_p(ctx, timer).unwrap(),
        Word::NIL
    );
    ncl_threads::schedule_timer(ctx, timer, 100).unwrap();
    assert_eq!(
        ncl_threads::timer_scheduled_p(ctx, timer).unwrap(),
        Word::TRUE
    );
    assert!(ncl_threads::run_expired_timers(50).is_empty());
    let expired = ncl_threads::run_expired_timers(100);
    assert_eq!(expired.len(), 1);
    assert_eq!(
        ncl_threads::timer_scheduled_p(ctx, timer).unwrap(),
        Word::NIL
    );
    assert!(ncl_threads::run_expired_timers(200).is_empty());
}

#[test]
fn unscheduling_clears_the_deadline() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx, .. } = &mut fixture;
    let timer = ncl_threads::make_timer(ctx, runtime, "cancel").unwrap();
    ncl_threads::schedule_timer(ctx, timer, 10).unwrap();
    ncl_threads::unschedule_timer(ctx, timer).unwrap();
    assert!(ncl_threads::run_expired_timers(u64::MAX).is_empty());
    let name = ncl_threads::timer_name(ctx, timer).unwrap();
    assert_eq!(ncl_object::string_length(ctx, name).unwrap(), 6);
}

#[test]
fn list_all_timers_contains_created_timers() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx, .. } = &mut fixture;
    assert_eq!(
        ncl_threads::list_all_timers(ctx, runtime).unwrap(),
        Word::NIL
    );
    let timer = ncl_threads::make_timer(ctx, runtime, "listed").unwrap();
    let list = ncl_threads::list_all_timers(ctx, runtime).unwrap();
    assert_eq!(ncl_object::car(ctx, list).unwrap(), timer);
}

#[test]
fn unknown_timers_are_rejected() {
    let mut fixture = fixture();
    let Fixture { ctx, .. } = &mut fixture;
    assert_eq!(
        ncl_threads::timer_scheduled_p(ctx, Word::fixnum(0)),
        Err(ThreadError::NotATimer)
    );
    assert_eq!(
        ncl_threads::unschedule_timer(ctx, Word::fixnum(0)),
        Err(ThreadError::NotATimer)
    );
}

#[test]
fn get_time_of_day_is_nonzero() {
    assert!(ncl_threads::get_time_of_day() > 1_600_000_000);
}

#[test]
fn decode_timeout_accepts_nil_fixnum_and_cons() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx, .. } = &mut fixture;
    assert_eq!(ncl_threads::decode_timeout(ctx, Word::NIL).unwrap(), None);
    assert_eq!(
        ncl_threads::decode_timeout(ctx, Word::fixnum(3)).unwrap(),
        Some(Duration::from_secs(3))
    );
    let pair = ncl_object::make_cons(ctx, runtime, Word::fixnum(2), Word::fixnum(500)).unwrap();
    assert_eq!(
        ncl_threads::decode_timeout(ctx, pair).unwrap(),
        Some(Duration::from_secs(2) + Duration::from_nanos(500))
    );
    let bad =
        ncl_object::make_cons(ctx, runtime, Word::fixnum(1), Word::fixnum(2_000_000_000)).unwrap();
    assert!(ncl_threads::decode_timeout(ctx, bad).is_err());
    assert!(ncl_threads::decode_timeout(ctx, Word::TRUE).is_err());
}

#[test]
fn the_deadline_stack_pushes_pops_and_defers() {
    let mut fixture = fixture();
    let Fixture { ctx, .. } = &mut fixture;
    assert_eq!(ncl_threads::signal_deadline(ctx).unwrap(), Word::NIL);
    let first = ncl_threads::push_deadline(ctx, Word::fixnum(10)).unwrap();
    assert_eq!(first, Word::NIL);
    let second = ncl_threads::push_deadline(ctx, Word::fixnum(20)).unwrap();
    assert_ne!(second, Word::NIL);
    assert_ne!(ncl_threads::signal_deadline(ctx).unwrap(), Word::NIL);
    let deferred = ncl_threads::defer_deadline(ctx, Word::fixnum(30)).unwrap();
    assert!(deferred.as_fixnum().unwrap() > second.as_fixnum().unwrap());
    let popped = ncl_threads::pop_deadline(ctx).unwrap();
    assert!(popped.as_fixnum().unwrap() > second.as_fixnum().unwrap());
    assert!(ncl_threads::pop_deadline(ctx).unwrap().is_fixnum());
    assert_eq!(ncl_threads::pop_deadline(ctx).unwrap(), Word::NIL);
}

#[test]
fn with_deadline_reports_expiry_after_the_body_returns() {
    let mut fixture = fixture();
    let Fixture { ctx, .. } = &mut fixture;
    let expired = ncl_threads::with_deadline(ctx, Word::fixnum(0), |_ctx| {
        std::thread::sleep(Duration::from_millis(2));
        Ok(Word::fixnum(1))
    });
    assert_eq!(expired, Err(ThreadError::Timeout));

    let fresh = ncl_threads::with_deadline(ctx, Word::fixnum(60), |_ctx| Ok(Word::fixnum(2)));
    assert_eq!(fresh.unwrap().as_fixnum(), Some(2));
    assert_eq!(ncl_threads::signal_deadline(ctx).unwrap(), Word::NIL);
}

#[test]
fn with_timeout_binds_and_restores_exit_timeout() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx, .. } = &mut fixture;
    let value =
        ncl_threads::with_timeout(ctx, runtime, Word::fixnum(60), |_ctx| Ok(Word::fixnum(9)))
            .unwrap();
    assert_eq!(value.as_fixnum(), Some(9));
    assert_eq!(ncl_threads::signal_deadline(ctx).unwrap(), Word::NIL);
}

#[test]
fn interrupt_enablement_toggles_and_restores() {
    let mut fixture = fixture();
    let Fixture { ctx, .. } = &mut fixture;
    assert_eq!(
        ncl_threads::enable_interrupt(ctx, false).unwrap(),
        Word::TRUE
    );
    assert_eq!(ncl_threads::enable_interrupt(ctx, true).unwrap(), Word::NIL);
    let inside = ncl_threads::without_interrupts(ctx, |_ctx| Ok(Word::fixnum(4))).unwrap();
    assert_eq!(inside.as_fixnum(), Some(4));
    let inside = ncl_threads::with_interrupts(ctx, |_ctx| Ok(Word::fixnum(5))).unwrap();
    assert_eq!(inside.as_fixnum(), Some(5));
}

#[test]
fn call_with_timing_returns_millis_and_the_value() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx, .. } = &mut fixture;
    let result = ncl_threads::call_with_timing(ctx, runtime, |_ctx| Ok(Word::fixnum(11))).unwrap();
    assert_eq!(ncl_object::simple_vector_length(ctx, result).unwrap(), 2);
    assert_eq!(
        ncl_object::simple_vector_ref(ctx, result, 1)
            .unwrap()
            .as_fixnum(),
        Some(11)
    );
    assert!(
        ncl_object::simple_vector_ref(ctx, result, 0)
            .unwrap()
            .is_fixnum()
    );
}

#[test]
fn the_deadline_condition_requires_the_condition_system() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx, .. } = &mut fixture;
    assert!(matches!(
        ncl_threads::deadline_timeout_condition(ctx, runtime),
        Err(ThreadError::Unsupported(_))
    ));
    ncl_conditions::register(runtime).unwrap();
    let condition = ncl_threads::deadline_timeout_condition(ctx, runtime).unwrap();
    assert_ne!(condition, Word::NIL);
}
