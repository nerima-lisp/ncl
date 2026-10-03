#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    reason = "tests assert on dynamic scope restoration"
)]

//! Deadline and interrupt dynamic-scope restoration.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use ncl_object::{Package, Runtime, ThreadContext, Word};
use ncl_threads::ThreadError;

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

#[test]
fn deadline_and_interrupt_scopes_restore_after_errors() {
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
        ncl_threads::signal_deadline(&fixture.ctx).unwrap(),
        Word::NIL
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

    let package = Package::from_word(
        fixture
            .runtime
            .find_package(&fixture.ctx, "NCL-THREADS")
            .unwrap(),
    );
    let (timeout_symbol, _) = package
        .intern(&mut fixture.ctx, &fixture.runtime, "*TIMEOUT-EXIT*")
        .unwrap();
    let timeout_value = ncl_object::symbol_value(&fixture.ctx, timeout_symbol).unwrap();
    assert_eq!(
        ncl_threads::with_timeout(
            &mut fixture.ctx,
            &fixture.runtime,
            Word::fixnum(1),
            |_ctx| Err::<Word, ThreadError>(error),
        ),
        Err(error)
    );
    assert_eq!(
        ncl_object::symbol_value(&fixture.ctx, timeout_symbol).unwrap(),
        timeout_value
    );
    assert_eq!(
        ncl_threads::signal_deadline(&fixture.ctx).unwrap(),
        Word::NIL
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
        ncl_threads::enable_interrupt(&mut fixture.ctx, false).unwrap(),
        Word::NIL
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
