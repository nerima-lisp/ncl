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
fn hash_table_default_threshold_failure_releases_all_roots() {
    let heap_config = HeapConfig {
        dynamic_space_size: 16 * 1024 * 1024,
        bytes_considered_between_gcs: 4 * 1024 * 1024,
    };
    let large_object_words = 127;
    let logical_page_size = 32 * 1024;
    // `allocate` adds a header word, making each object 1 KiB.
    let large_object_bytes = (large_object_words + 1) * std::mem::size_of::<Word>();
    let large_objects_per_page = logical_page_size / large_object_bytes;
    let large_root_count = heap_config.dynamic_space_size / large_object_bytes + 1024;
    let runtime = Runtime::with_config(heap_config)
        .unwrap_or_else(|error| panic!("Runtime::with_config failed: {error:?}"));
    let mut ctx = ThreadContext::new();
    assert!(ctx.register(&runtime).is_ok());
    let mut sentinel = Word::NIL;
    let sentinel_token = ncl_object::push_root(&mut ctx, &mut sentinel);
    let roots = vec![Word::NIL; large_root_count];
    let result = ncl_object::with_rooted_slice(&mut ctx, &roots, |ctx, roots| {
        let mut allocated = 0;
        for _ in 0..large_objects_per_page {
            roots[allocated] = allocate(ctx, &runtime, 0x7f, large_object_words)?;
            allocated += 1;
        }
        ctx.collect(true)?;

        let mut doubles = 0;
        while doubles < large_objects_per_page * 2 {
            roots[allocated] = ncl_object::make_double(ctx, &runtime, 2.0)?.as_word();
            allocated += 1;
            doubles += 1;
        }
        assert!(doubles > 0);

        while allocated < roots.len() {
            match allocate(ctx, &runtime, 0x7f, large_object_words) {
                Ok(value) => {
                    roots[allocated] = value;
                    allocated += 1;
                }
                Err(ncl_object::ObjectError::Storage(StorageCondition::CapacityExceeded)) => break,
                Err(error) => return Err(error),
            }
        }
        assert!(allocated < roots.len());
        ctx.collect(true)?;

        Ok(HashTable::new_with_options(
            ctx,
            &runtime,
            HashTest::Eql,
            Weakness::None,
            Word::fixnum(8),
            None,
            None,
        ))
    });
    let result_shape = result.as_ref().map(|inner| inner.as_ref().map(|_| ()));
    assert!(
        matches!(
            result,
            Ok(Err(ncl_object::ObjectError::Storage(
                StorageCondition::CapacityExceeded
            )))
        ),
        "unexpected result: {result_shape:?}"
    );
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
