#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on the process object model"
)]

//! Process object accessors.

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_threads::ThreadError;

struct Fixture {
    ctx: ThreadContext,
    runtime: Runtime,
}

fn fixture() -> Fixture {
    let runtime = Runtime::new().unwrap();
    ncl_threads::register(&runtime).unwrap();
    let mut ctx = ThreadContext::new();
    ctx.register(&runtime).unwrap();
    Fixture { runtime, ctx }
}

#[test]
fn a_process_records_its_identifier_and_status() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let process = ncl_threads::make_process(ctx, runtime, 4242, Word::NIL).unwrap();

    assert_eq!(ncl_threads::process_p(ctx, process).unwrap(), Word::TRUE);
    assert_eq!(
        ncl_threads::process_pid(ctx, process).unwrap().as_fixnum(),
        Some(4242)
    );
    assert_eq!(
        ncl_threads::process_status(ctx, process).unwrap(),
        Word::NIL
    );
    assert_eq!(
        ncl_threads::process_alive_p(ctx, process).unwrap(),
        Word::TRUE
    );
    assert_eq!(
        ncl_threads::process_exit_code(ctx, process).unwrap(),
        Word::NIL
    );
}

#[test]
fn kill_marks_a_process_dead_and_records_a_status() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let process = ncl_threads::make_process(ctx, runtime, 7, Word::NIL).unwrap();

    assert_eq!(ncl_threads::process_kill(ctx, process).unwrap(), Word::TRUE);
    assert_eq!(
        ncl_threads::process_alive_p(ctx, process).unwrap(),
        Word::NIL
    );
    let status = ncl_threads::process_status(ctx, process).unwrap();
    assert_eq!(status.as_fixnum(), Some(-15));
    assert_eq!(
        ncl_threads::process_exit_code(ctx, process).unwrap(),
        status
    );
    assert_eq!(
        ncl_threads::process_wait(ctx, process, None).unwrap(),
        status
    );
}

#[test]
fn waiting_for_a_live_process_reports_a_timeout() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let process = ncl_threads::make_process(ctx, runtime, 8, Word::NIL).unwrap();
    assert_eq!(
        ncl_threads::process_wait(ctx, process, None),
        Err(ThreadError::Timeout)
    );
}

#[test]
fn close_clears_the_stream_slots() {
    let mut fixture = fixture();
    let Fixture { runtime, ctx } = &mut fixture;
    let process = ncl_threads::make_process(ctx, runtime, 9, Word::NIL).unwrap();
    assert_eq!(
        ncl_threads::process_close(ctx, process).unwrap(),
        Word::TRUE
    );
    for slot in [
        ncl_threads::process_input(ctx, process).unwrap(),
        ncl_threads::process_output(ctx, process).unwrap(),
        ncl_threads::process_error(ctx, process).unwrap(),
    ] {
        assert_eq!(slot, Word::NIL);
    }
    assert_eq!(
        ncl_threads::process_core_dumped(ctx, process).unwrap(),
        Word::NIL
    );
    assert_eq!(ncl_threads::process_pty(ctx, process).unwrap(), Word::NIL);
    assert_eq!(ncl_threads::process_plist(ctx, process).unwrap(), Word::NIL);
}

#[test]
fn process_p_rejects_non_processes() {
    let mut fixture = fixture();
    let Fixture { ctx, .. } = &mut fixture;
    assert_eq!(
        ncl_threads::process_p(ctx, Word::fixnum(3)).unwrap(),
        Word::NIL
    );
    assert!(ncl_threads::process_pid(ctx, Word::fixnum(3)).is_err());
}
