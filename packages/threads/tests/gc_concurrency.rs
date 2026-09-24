#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "tests assert on stop-the-world collection under concurrent allocation"
)]

//! A stop-the-world collection runs while two other threads are allocating.
//!
//! Both workers allocate continuously and poll their safepoint until the main
//! thread finishes a fixed number of full collections and clears the stop flag.
//! The handshake makes the overlap deterministic: the workers are still
//! allocating for every collection the main thread runs, so the collector must
//! park them and scan their published roots.
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_threads::ThreadError;

/// Number of full collections the main thread performs while the workers run.
const COLLECTIONS: usize = 16;
/// Characters requested per worker allocation.
const PAYLOAD: usize = 64;

static WORKERS_STARTED: AtomicUsize = AtomicUsize::new(0);
static STOP: AtomicBool = AtomicBool::new(false);
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn allocating_worker(runtime: &Runtime, ctx: &mut ThreadContext) -> Result<(), ThreadError> {
    let mut started = false;
    while !STOP.load(Ordering::SeqCst) {
        let mut cell = Cell::new(ncl_object::make_string(ctx, runtime, &['x'; PAYLOAD])?);
        let token = ncl_sys::push_root(ctx.thread_mut(), cell.get_mut());
        let slot = ncl_sys::RootSlot::new(&cell);
        ncl_sys::poll_safepoint(ctx.thread_mut());
        let length = ncl_object::string_length(ctx, *slot)?;
        assert_eq!(length, PAYLOAD);
        let _ = ncl_sys::pop_root(ctx.thread_mut(), token);
        ALLOCATED.fetch_add(1, Ordering::SeqCst);
        if !started {
            started = true;
            WORKERS_STARTED.fetch_add(1, Ordering::SeqCst);
        }
    }
    Ok(())
}

#[test]
fn collection_parks_two_concurrently_allocating_threads() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    let runtime = Arc::new(Runtime::new().unwrap());
    ncl_threads::register(&runtime).unwrap();
    let mut main_ctx = ThreadContext::new();
    main_ctx.register(&runtime).unwrap();

    WORKERS_STARTED.store(0, Ordering::SeqCst);
    STOP.store(false, Ordering::SeqCst);
    ALLOCATED.store(0, Ordering::SeqCst);

    let first = ncl_threads::spawn(&runtime, "allocator-a", allocating_worker).unwrap();
    let second = ncl_threads::spawn(&runtime, "allocator-b", allocating_worker).unwrap();
    while WORKERS_STARTED.load(Ordering::SeqCst) < 2 {
        std::thread::yield_now();
    }

    for collection in 0..COLLECTIONS {
        while ALLOCATED.load(Ordering::SeqCst) <= collection {
            std::thread::yield_now();
        }
        main_ctx.collect(true).unwrap();
        assert!(ALLOCATED.load(Ordering::Relaxed) > 0);
    }

    STOP.store(true, Ordering::SeqCst);
    ncl_threads::join(first, Some(Duration::from_secs(60))).unwrap();
    ncl_threads::join(second, Some(Duration::from_secs(60))).unwrap();

    assert!(ALLOCATED.load(Ordering::Relaxed) >= COLLECTIONS);
    assert!(ncl_sys::heap_epoch(main_ctx.thread_mut()) >= u64::try_from(COLLECTIONS).unwrap());
}

#[test]
fn a_collection_releases_words_held_by_another_thread() {
    let _test_guard = TEST_LOCK.lock().unwrap();
    let runtime = Arc::new(Runtime::new().unwrap());
    ncl_threads::register(&runtime).unwrap();
    let mut main_ctx = ThreadContext::new();
    main_ctx.register(&runtime).unwrap();

    WORKERS_STARTED.store(0, Ordering::SeqCst);
    STOP.store(false, Ordering::SeqCst);
    ALLOCATED.store(0, Ordering::SeqCst);

    let worker = ncl_threads::spawn(&runtime, "allocator", allocating_worker).unwrap();
    while WORKERS_STARTED.load(Ordering::SeqCst) < 1 {
        std::thread::yield_now();
    }
    for _ in 0..COLLECTIONS {
        main_ctx.collect(false).unwrap();
    }
    STOP.store(true, Ordering::SeqCst);
    ncl_threads::join(worker, Some(Duration::from_secs(60))).unwrap();

    let mut survivor = ncl_object::make_string(&mut main_ctx, &runtime, &['y'; PAYLOAD]).unwrap();
    let token = ncl_object::push_root(&mut main_ctx, &mut survivor);
    main_ctx.collect(true).unwrap();
    assert_eq!(
        ncl_object::string_length(&main_ctx, survivor).unwrap(),
        PAYLOAD
    );
    assert!(ncl_object::pop_root(&mut main_ctx, token));
    assert_ne!(survivor, Word::NIL);
}
