#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on thread lifecycle outcomes"
)]

//! Thread lifecycle: spawn, join, terminate, interrupts, and thread objects.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_threads::ThreadError;

const JOIN_TIMEOUT: Duration = Duration::from_secs(30);

fn registry_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn body_ok(_runtime: &Runtime, _ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    Ok(())
}

fn body_fail(_runtime: &Runtime, _ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    Err(ThreadError::Timeout)
}

fn body_allocates(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    for _ in 0..64 {
        let mut word = ncl_object::make_string(ctx, runtime, &['x'; 8])?;
        let token = ncl_object::push_root(ctx, &mut word);
        ncl_sys::poll_safepoint(ctx.thread_mut());
        let _ = ncl_object::pop_root(ctx, token);
    }
    Ok(())
}

fn runtime() -> Arc<Runtime> {
    Arc::new(Runtime::new().unwrap())
}

#[test]
fn join_reports_success_for_a_finished_body() {
    let _guard = registry_guard();
    let runtime = runtime();
    let id = ncl_threads::spawn(&runtime, "ok", body_ok).unwrap();
    ncl_threads::join(id, Some(JOIN_TIMEOUT)).unwrap();
    assert!(!ncl_threads::alive_p(id));
    assert_eq!(ncl_threads::thread_name(id).as_deref(), Some("ok"));
}

#[test]
fn join_reports_the_body_error() {
    let _guard = registry_guard();
    let runtime = runtime();
    let id = ncl_threads::spawn(&runtime, "fail", body_fail).unwrap();
    assert_eq!(
        ncl_threads::join(id, Some(JOIN_TIMEOUT)),
        Err(ThreadError::Timeout)
    );
}

#[test]
fn a_spawned_body_registers_its_own_context() {
    let _guard = registry_guard();
    let runtime = runtime();
    let id = ncl_threads::spawn(&runtime, "alloc", body_allocates).unwrap();
    ncl_threads::join(id, Some(JOIN_TIMEOUT)).unwrap();
    assert!(ncl_threads::os_tid(id).is_some());
}

#[test]
fn join_unknown_thread_reports_not_running() {
    let _guard = registry_guard();
    let runtime = runtime();
    let id = ncl_threads::spawn(&runtime, "gone", body_ok).unwrap();
    ncl_threads::join(id, Some(JOIN_TIMEOUT)).unwrap();
    let _ = ncl_threads::dispose_finished();
    assert_eq!(
        ncl_threads::join(id, Some(JOIN_TIMEOUT)),
        Err(ThreadError::NotRunning)
    );
}

#[test]
fn joining_the_main_thread_is_a_deadlock() {
    let id = ncl_threads::ThreadId::from_raw(ncl_threads::MAIN_THREAD_ID);
    assert_eq!(ncl_threads::join(id, None), Err(ThreadError::Deadlock));
    assert!(ncl_threads::alive_p(id));
    assert_eq!(ncl_threads::thread_name(id).as_deref(), Some("main thread"));
}

#[test]
fn interrupts_stay_pending_until_taken() {
    let _guard = registry_guard();
    let runtime = runtime();
    let id = ncl_threads::spawn(&runtime, "parked", ncl_threads::idle_body).unwrap();
    ncl_threads::interrupt(id).unwrap();
    assert!(ncl_threads::take_interrupt(id));
    assert!(!ncl_threads::take_interrupt(id));
    ncl_threads::terminate(id).unwrap();
    ncl_threads::join(id, Some(JOIN_TIMEOUT)).unwrap();
}

#[test]
fn terminate_releases_a_parked_thread() {
    let _guard = registry_guard();
    let runtime = runtime();
    let id = ncl_threads::spawn(&runtime, "idle", ncl_threads::idle_body).unwrap();
    assert!(ncl_threads::should_terminate(id) == false);
    ncl_threads::terminate(id).unwrap();
    assert!(ncl_threads::should_terminate(id));
    ncl_threads::join(id, Some(JOIN_TIMEOUT)).unwrap();
}

#[test]
fn all_ids_is_sorted_and_contains_spawned_threads() {
    let _guard = registry_guard();
    let runtime = runtime();
    let first = ncl_threads::spawn(&runtime, "first", body_ok).unwrap();
    let second = ncl_threads::spawn(&runtime, "second", body_ok).unwrap();
    ncl_threads::join(first, Some(JOIN_TIMEOUT)).unwrap();
    ncl_threads::join(second, Some(JOIN_TIMEOUT)).unwrap();
    let ids = ncl_threads::all_ids();
    assert!(ids.contains(&first));
    assert!(ids.contains(&second));
    assert!(ids.windows(2).all(|pair| pair[0].get() < pair[1].get()));
    assert!(ncl_threads::dispose_finished() >= 2);
}

#[test]
fn make_thread_records_name_function_and_state() {
    let runtime = runtime();
    ncl_threads::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let thread = ncl_threads::make_thread(&mut ctx, &runtime, "worker", Word::TRUE).unwrap();
    assert_eq!(
        ncl_threads::thread_alive_p(&ctx, thread).unwrap(),
        Word::TRUE
    );
    assert_eq!(ncl_threads::main_thread_p(&ctx, thread).unwrap(), Word::NIL);
    assert_eq!(ncl_threads::thread_state_of(&ctx, thread).unwrap(), 0);
    assert!(
        ncl_threads::thread_os_tid_of(&ctx, thread)
            .unwrap()
            .is_fixnum()
    );

    ncl_threads::terminate_thread(&mut ctx, thread).unwrap();
    assert_eq!(
        ncl_threads::thread_state_of(&ctx, thread).unwrap(),
        ncl_threads::STATE_TERMINATED
    );
    ncl_threads::join_thread(&ctx, thread).unwrap();

    let name = ncl_threads::thread_name_of(&ctx, thread).unwrap();
    assert_eq!(ncl_object::string_length(&ctx, name).unwrap(), 6);
}

#[test]
fn current_thread_is_the_main_thread() {
    let runtime = runtime();
    ncl_threads::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();

    let main = ncl_threads::current_thread(&mut ctx, &runtime).unwrap();
    assert_eq!(ncl_threads::main_thread_p(&ctx, main).unwrap(), Word::TRUE);
    assert_eq!(
        ncl_threads::current_thread(&mut ctx, &runtime).unwrap(),
        main
    );
    assert_ne!(
        ncl_threads::list_all_threads(&mut ctx, &runtime).unwrap(),
        Word::NIL
    );
}

#[test]
fn thread_yield_and_finished_state_are_available() {
    ncl_threads::thread_yield();
    assert_eq!(ncl_threads::finished_state(), ncl_threads::STATE_FINISHED);
}

#[test]
fn thread_error_thread_reports_the_condition_boundary() {
    assert!(matches!(
        ncl_threads::thread_error_thread(Word::NIL),
        Err(ThreadError::Unsupported(_))
    ));
}

#[test]
fn object_id_rejects_non_thread_objects() {
    let runtime = runtime();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    assert_eq!(
        ncl_threads::object_id(&ctx, Word::fixnum(1)),
        Err(ThreadError::NotAThread)
    );
}
