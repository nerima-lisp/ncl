//! Integration tests for hash-table equality, weak entries, and regressions.

use ncl_object::{Package, Runtime, ThreadContext, allocate, make_cons};
use ncl_sys::{HeapConfig, StorageCondition, Word};

#[test]
fn moved_registered_context_remains_usable() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let mut ctx = Box::new(ctx);
    assert!(ctx.collect(true).is_ok());
}

#[test]
fn unregistered_context_rejects_collection_and_allocation() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    let error = ncl_object::ObjectError::Storage(StorageCondition::ThreadNotRegistered);
    assert_eq!(ctx.collect(true), Err(error));
    assert_eq!(
        make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL),
        Err(error)
    );
    assert_eq!(allocate(&mut ctx, &runtime, 0x7f, 1), Err(error));
}

#[test]
fn package_failure_releases_all_roots_before_collection() {
    let heap_config = HeapConfig {
        dynamic_space_size: 16 * 1024 * 1024,
        bytes_considered_between_gcs: 4 * 1024 * 1024,
    };
    let root_count = heap_config.dynamic_space_size / (64 * std::mem::size_of::<Word>()) + 128;
    let runtime = Runtime::with_config(heap_config)
        .unwrap_or_else(|error| panic!("Runtime::with_config failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let mut sentinel = Word::NIL;
    let sentinel_token = ncl_object::push_root(&mut ctx, &mut sentinel);
    let roots = vec![Word::NIL; root_count];
    let result = ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let mut allocated = 0;
        for words in [64, 16, 4, 1] {
            loop {
                match allocate(ctx, &runtime, 0x7f, words) {
                    Ok(value) => {
                        roots[allocated] = value;
                        allocated += 1;
                    }
                    Err(ncl_object::ObjectError::Storage(StorageCondition::CapacityExceeded)) => {
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
        }
        assert!(allocated > 0);
        assert!(allocated < roots.len());
        ctx.collect(true)?;
        Ok(Package::new(ctx, &runtime, "ROOT-FAILURE"))
    });
    assert!(matches!(
        result,
        Ok(Err(ncl_object::ObjectError::Storage(
            StorageCondition::CapacityExceeded
        )))
    ));
    assert!(ncl_object::pop_root(&mut ctx, sentinel_token));
    assert!(ctx.collect(true).is_ok());
}

#[test]
fn moved_registered_context_allows_allocation() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let mut ctx = Box::new(ctx);
    assert!(make_cons(&mut ctx, &runtime, Word::NIL, Word::NIL).is_ok());
}

#[test]
fn moved_registered_context_allows_try_push_root() {
    let runtime = Runtime::new().unwrap_or_else(|error| panic!("Runtime::new failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let mut ctx = Box::new(ctx);
    let mut value = Word::NIL;
    let token = ncl_object::try_push_root(&mut ctx, &mut value)
        .unwrap_or_else(|error| panic!("push failed: {error:?}"));
    assert!(ncl_object::try_pop_root(&mut ctx, token).is_ok());
}
