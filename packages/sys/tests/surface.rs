//! Public-surface tests for the platform and heap boundary.

use ncl_sys::{
    Condvar, Heap, HeapConfig, LowTag, Mutex, NativeState, ReferenceLayout, SafepointState,
    Semaphore, Thread, TypeTag, WaitQueue, Weakness, Word, alloc, alloc_cons, alloc_large, collect,
    enter_native, heap_epoch, leave_native, make_weak, object_widetag, pop_root,
    publish_conservative_root, publish_safepoint, push_root, read_cons_word, read_object_word,
    register_layout, register_root_set, register_thread, register_thread_with_thread,
    request_safepoint, set_strict_forwarding, set_tlab, tlab_bump, weak_value, write_barrier,
    write_cons_word, write_object_word,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn word_boundaries_and_root_slot_are_observable() {
    let values = [i64::MIN / 2, -1, 0, 1, i64::MAX / 2];
    for value in values {
        let word = Word::fixnum(value);
        assert_eq!(word.as_fixnum(), Some(value));
        assert!(word.is_fixnum());
    }
    let character = Word::character('λ' as u32);
    assert!(character.is_character());
    assert_eq!(character.lowtag(), LowTag::Character as u8);
    assert!(Word::NIL.is_list());
    assert!(!Word::NIL.is_cons());
    let pointer = Word::pointer(0x1000, LowTag::OtherPointer);
    assert_eq!(pointer.address(), 0x1000);
    assert_eq!(Word::from_bits(pointer.bits()), pointer);

    let cell = std::cell::Cell::new(Word::fixnum(9));
    let slot = ncl_sys::RootSlot::new(&cell);
    assert_eq!(*slot, Word::fixnum(9));
}

#[test]
fn thread_state_transitions_and_root_stack_have_contracts() {
    let mut thread = Thread::default();
    assert_eq!(thread.native_state(), NativeState::Lisp);
    assert_eq!(thread.safepoint_state(), SafepointState::Running);
    let mut first = Word::fixnum(1);
    let first_token = push_root(&mut thread, &mut first);
    let mut second = Word::fixnum(2);
    let second_token = push_root(&mut thread, &mut second);
    assert!(!pop_root(&mut thread, first_token));
    assert!(pop_root(&mut thread, second_token));
    assert!(pop_root(&mut thread, first_token));
    assert!(!pop_root(&mut thread, first_token));

    publish_conservative_root(&mut thread, Word::TRUE);
    thread.request_poll();
    assert_eq!(thread.safepoint_state(), SafepointState::PollRequested);
    publish_safepoint(&mut thread);
    assert_eq!(thread.safepoint_state(), SafepointState::Running);
    request_safepoint(&mut thread);
    assert_eq!(thread.safepoint_state(), SafepointState::PollRequested);
    thread.clear_safepoint_request();
    thread.request_interrupt();
    assert!(thread.take_interrupt());
    assert!(!thread.take_interrupt());
    enter_native(&mut thread);
    assert_eq!(thread.native_state(), NativeState::Native);
    assert_eq!(thread.safepoint_state(), SafepointState::Safe);
    leave_native(&mut thread);
    assert_eq!(thread.native_state(), NativeState::Lisp);
    assert_eq!(thread.safepoint_state(), SafepointState::Running);

    set_tlab(&mut thread, 12, 48);
    assert_eq!(tlab_bump(&thread), 12);
    let layout = ncl_sys::thread_layout();
    assert!(layout.tlab_limit > layout.tlab_bump);
    assert!(layout.pending > layout.safepoint_request);
}

#[test]
fn heap_wrappers_cover_registration_allocation_and_access() {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 16384,
        bytes_considered_between_gcs: 128,
    });
    assert_eq!(heap.dynamic_space_size(), 16384);
    assert_eq!(heap.bytes_considered_between_gcs(), 128);
    assert_eq!(heap_epoch(&Thread::new()), 0);
    let mut unregistered = Thread::new();
    assert_eq!(
        alloc(&mut unregistered, &heap, TypeTag { widetag: 3 }, 1),
        Err(ncl_sys::StorageCondition::ThreadNotRegistered)
    );
    let mut thread = Thread::new();
    assert!(register_thread(&heap, &mut thread).is_ok());
    assert_eq!(
        register_thread(&heap, &mut thread),
        Err(ncl_sys::StorageCondition::ThreadNotRegistered)
    );
    let mut sibling = Thread::new();
    assert!(register_thread_with_thread(&thread, &mut sibling).is_ok());
    assert!(register_thread_with_thread(&unregistered, &mut Thread::new()).is_err());

    let object = alloc(&mut thread, &heap, TypeTag { widetag: 7 }, 2).unwrap_or(Word::NIL);
    assert_eq!(object_widetag(&thread, object), Some(7));
    assert_eq!(
        read_object_word(&thread, object, 0),
        Some(Word::from_bits(0))
    );
    assert!(write_object_word(&mut thread, object, 0, Word::fixnum(8)));
    assert_eq!(read_object_word(&thread, object, 0), Some(Word::fixnum(8)));
    assert!(!write_object_word(&mut Thread::new(), object, 0, Word::NIL));

    let cons = alloc_cons(&mut thread, &heap, Word::fixnum(3), Word::NIL).unwrap_or(Word::NIL);
    assert_eq!(read_cons_word(&thread, cons, 0), Some(Word::fixnum(3)));
    assert!(write_cons_word(&mut thread, cons, 1, Word::TRUE));
    assert_eq!(read_cons_word(&thread, cons, 1), Some(Word::TRUE));
    assert!(alloc_large(&mut thread, &heap, TypeTag { widetag: 8 }, 1024).is_ok());
    assert!(
        register_layout(
            &heap,
            7,
            ReferenceLayout {
                reference_words: vec![1],
                boxed_from: None
            }
        )
        .is_ok()
    );
    assert!(
        register_layout(
            &heap,
            7,
            ReferenceLayout {
                reference_words: vec![],
                boxed_from: None
            }
        )
        .is_err()
    );
    set_strict_forwarding(&thread, true);
    write_barrier(&mut thread, object, 1);
    assert_eq!(make_weak(&thread, object, Weakness::Value), object);
    assert_eq!(weak_value(&thread, object), Word::fixnum(8));
    ncl_sys::unregister_thread(&sibling);
    collect(&mut thread, false);
    assert!(heap_epoch(&thread) >= 1);
    ncl_sys::unregister_thread(&thread);
}

#[test]
fn direct_heap_wrappers_and_unregistered_code_errors_are_observable() {
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    let object = alloc(&mut thread, &heap, TypeTag { widetag: 1 }, 1).unwrap_or(Word::NIL);
    assert_eq!(ncl_sys::read_word(&heap, object, 0), None);
    assert!(!ncl_sys::write_word(&heap, object, 0, Word::TRUE));
    assert_eq!(ncl_sys::widetag(&heap, object), None);
    assert_eq!(ncl_sys::make_weak(&thread, object, Weakness::Key), object);
    assert_eq!(ncl_sys::weak_value(&thread, object), Word::NIL);
    ncl_sys::write_barrier(&mut thread, object, 0);
    ncl_sys::register_finalizer(&thread, object, |_| {});
    ncl_sys::run_pending_finalizers(&thread);
    let Ok(mut code) = ncl_sys::alloc_code(16) else {
        return;
    };
    assert!(ncl_sys::publish_code(&mut code).is_ok());
    assert_eq!(
        ncl_sys::register_code(
            &thread,
            &code,
            ncl_sys::CodeObjectMetadata {
                entry_offset: 0,
                size: code.len(),
                frame_words: 0,
                function_name: "unregistered".to_string(),
                source_locations: Vec::new(),
                constant_slots: Vec::new(),
                safepoint_map: ncl_sys::SafepointMap::default(),
                debug_table: Vec::new(),
            }
        ),
        Err(ncl_sys::CodeError::NotRegistered)
    );
    assert!(!ncl_sys::write_cons_word(
        &mut thread,
        object,
        0,
        Word::TRUE
    ));
}

#[test]
fn public_code_write_wrapper_reports_bounds() {
    let Ok(mut code) = ncl_sys::alloc_code(1) else {
        return;
    };
    assert_eq!(ncl_sys::write_code(&mut code, 0, &[7]), Ok(()));
    assert_eq!(
        ncl_sys::write_code(&mut code, 1, &[8]),
        Err(ncl_sys::CodeError::OutOfBounds)
    );
}

#[test]
fn heap_level_roots_and_after_gc_hooks_are_released() {
    static HOOK_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn hook() {
        HOOK_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn finalizer(_: Word) {
        HOOK_RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    let heap = Heap::new(HeapConfig::default());
    let mut thread = Thread::new();
    assert!(register_thread(&heap, &mut thread).is_ok());
    let mut values = [Word::NIL, Word::TRUE];
    let token = register_root_set(&mut thread, &mut values);
    assert!(pop_root(&mut thread, token));
    let mut value = alloc_cons(&mut thread, &heap, Word::fixnum(1), Word::NIL).unwrap_or(Word::NIL);
    let heap_token = ncl_sys::push_heap_root(&heap, &mut value);
    let mut second_value = Word::TRUE;
    let second_token = ncl_sys::push_heap_root(&heap, &mut second_value);
    assert!(!ncl_sys::pop_heap_root(&heap, heap_token));
    assert!(ncl_sys::pop_heap_root(&heap, second_token));
    assert!(ncl_sys::pop_heap_root(&heap, heap_token));
    ncl_sys::register_after_gc_hook(&heap, hook);
    collect(&mut thread, true);
    ncl_sys::register_finalizer(&thread, value, finalizer);
    ncl_sys::run_pending_finalizers(&thread);
    assert!(HOOK_RUNS.load(std::sync::atomic::Ordering::Relaxed) >= 1);
}

#[test]
fn synchronization_wrappers_wake_and_timeout() {
    let mutex = Arc::new(Mutex::new(false));
    let condvar = Arc::new(Condvar::default());
    let waiter_mutex = Arc::clone(&mutex);
    let waiter_condvar = Arc::clone(&condvar);
    let waiter = thread::spawn(move || {
        let mut guard = waiter_mutex.lock();
        while !*guard {
            guard = waiter_condvar.wait(guard);
        }
        *guard
    });
    thread::sleep(Duration::from_millis(5));
    *mutex.lock() = true;
    condvar.notify_one();
    assert!(waiter.join().is_ok_and(|value| value));
    let (guard, timed_out) = condvar.wait_timeout(mutex.lock(), Duration::from_millis(1));
    assert!(timed_out);
    drop(guard);

    let semaphore = Arc::new(Semaphore::new(0));
    let permit = Arc::clone(&semaphore);
    let worker = thread::spawn(move || {
        permit.acquire();
        true
    });
    thread::sleep(Duration::from_millis(5));
    semaphore.release();
    let result = worker.join();
    assert!(result.unwrap_or(false));

    let queue = Arc::new(WaitQueue::default());
    let queued = Arc::clone(&queue);
    let worker = thread::spawn(move || {
        queued.wait();
        1_u8
    });
    thread::sleep(Duration::from_millis(5));
    queue.wake_one();
    let result = worker.join();
    assert_eq!(result.unwrap_or(0), 1);
    queue.wake_all();
    let queue_clone = Arc::clone(&queue);
    let worker = thread::spawn(move || {
        queue_clone.wait();
        2_u8
    });
    let result = worker.join();
    assert_eq!(result.unwrap_or(0), 2);
}
