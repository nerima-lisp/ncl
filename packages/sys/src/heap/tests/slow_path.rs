use super::*;
use std::sync::{Arc, mpsc};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

#[test]
fn slow_path_budget_collects_minor_once_and_resets_other_mutator_debt() {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 256,
        bytes_considered_between_gcs: 16,
    });
    let mut collector = Thread::new();
    let mut native = Thread::new();
    assert_eq!(heap.register_thread(&mut collector), Ok(()));
    assert_eq!(heap.register_thread(&mut native), Ok(()));
    collector.bytes_cons = 17;
    native.bytes_cons = 56;
    native.enter_native();
    let epoch = heap.gc_epoch();

    let allocation = heap.alloc(&mut collector, TypeTag { widetag: 8 }, 0);

    assert!(allocation.is_ok());
    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert_eq!(native.bytes_cons, 0);
    assert_eq!(collector.bytes_cons, 8);
    heap.unregister_thread(&native);
    heap.unregister_thread(&collector);
}

#[test]
fn slow_path_cons_resolves_rooted_inputs_before_writing_the_new_cell() {
    let heap = Arc::new(Heap::new(HeapConfig {
        dynamic_space_size: 256,
        bytes_considered_between_gcs: 4096,
    }));
    let mut roots = setup_forwarding_roots(Arc::clone(&heap));
    assert_eq!(roots.values.len(), 2);
    assert_eq!(roots.addresses.len(), 2);

    let mut mutator = Thread::new();
    assert_eq!(heap.register_thread(&mut mutator), Ok(()));
    let first_root = mutator.push_root(&mut roots.values[0]);
    let second_root = mutator.push_root(&mut roots.values[1]);
    mutator.bytes_cons = 4097;
    let epoch = heap.gc_epoch();
    let allocation = heap.alloc_cons(&mut mutator, roots.values[0], roots.values[1]);
    assert!(
        allocation.is_ok(),
        "slow-path cons allocation should succeed"
    );
    let cell = allocation.unwrap_or(Word::NIL);

    assert_eq!(heap.gc_epoch(), epoch + 1);
    let current_first = roots.values[0];
    let current_second = roots.values[1];
    let old_first_address = roots.addresses[0];
    let old_second_address = roots.addresses[1];
    let state = heap.lock_state();
    let Some(first_index) = Heap::find(&state, current_first) else {
        panic!("first rooted input remains live");
    };
    let Some(second_index) = Heap::find(&state, current_second) else {
        panic!("second rooted input remains live");
    };
    let first_pinned = state.objects[first_index].pinned;
    let second_pinned = state.objects[second_index].pinned;
    drop(state);
    if first_pinned {
        assert_eq!(current_first.address(), old_first_address);
    } else {
        assert_ne!(current_first.address(), old_first_address);
    }
    if second_pinned {
        assert_eq!(current_second.address(), old_second_address);
    } else {
        assert_ne!(current_second.address(), old_second_address);
    }

    assert_eq!(heap.read_word(current_first, 0), Some(Word::fixnum(1)));
    assert_eq!(heap.read_word(current_second, 0), Some(Word::fixnum(2)));
    assert_eq!(heap.read_word(cell, 0), Some(current_first));
    assert_eq!(heap.read_word(cell, 1), Some(current_second));
    assert!(mutator.pop_root(second_root));
    assert!(mutator.pop_root(first_root));
    heap.unregister_thread(&mutator);
}

#[test]
fn slow_path_cons_preserves_explicitly_pinned_inputs() {
    let heap = Arc::new(Heap::new(HeapConfig {
        dynamic_space_size: 256,
        bytes_considered_between_gcs: 4096,
    }));
    let ForwardingRoots { values, addresses } = setup_forwarding_roots(Arc::clone(&heap));
    assert_eq!(values.len(), 2);
    assert_eq!(addresses.len(), 2);
    let first = values[0];
    let second = values[1];

    let mut mutator = Thread::new();
    assert_eq!(heap.register_thread(&mut mutator), Ok(()));
    crate::publish_conservative_root(&mut mutator, first);
    crate::publish_conservative_root(&mut mutator, second);
    mutator.bytes_cons = 4097;
    let epoch = heap.gc_epoch();
    let allocation = heap.alloc_cons(&mut mutator, first, second);
    assert!(
        allocation.is_ok(),
        "slow-path cons allocation should succeed"
    );
    let cell = allocation.unwrap_or(Word::NIL);

    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert_eq!(first.address(), addresses[0]);
    assert_eq!(second.address(), addresses[1]);
    assert_eq!(heap.forwarded_word(first), Some(first));
    assert_eq!(heap.forwarded_word(second), Some(second));
    let state = heap.lock_state();
    let Some(first_index) = Heap::find(&state, first) else {
        panic!("first pinned input remains live");
    };
    let Some(second_index) = Heap::find(&state, second) else {
        panic!("second pinned input remains live");
    };
    assert!(state.objects[first_index].pinned);
    assert!(state.objects[second_index].pinned);
    drop(state);

    assert_eq!(heap.read_word(first, 0), Some(Word::fixnum(1)));
    assert_eq!(heap.read_word(second, 0), Some(Word::fixnum(2)));
    assert_eq!(heap.read_word(cell, 0), Some(first));
    assert_eq!(heap.read_word(cell, 1), Some(second));
    heap.unregister_thread(&mutator);
}

#[test]
fn native_stack_snapshot_retains_unrooted_large_input() {
    let heap = Heap::new(HeapConfig::default());
    let mut collector = Thread::new();
    let mut native = Thread::new();
    assert_eq!(heap.register_thread(&mut collector), Ok(()));
    assert_eq!(heap.register_thread(&mut native), Ok(()));
    let object = heap.alloc_large(&mut native, TypeTag { widetag: 13 }, 1023);
    assert!(object.is_ok(), "setup large object should allocate");
    let mut car = object.unwrap_or(Word::NIL);
    assert!(native.roots.is_empty());
    let car_pointer = core::ptr::addr_of_mut!(car);
    // SAFETY: the pointer addresses the live local through the collection below.
    unsafe { car_pointer.write_volatile(car) };
    native.enter_native();
    assert!(native.stack_bounds.is_some());

    heap.begin_collection(&mut collector);
    heap.collect(true);
    heap.end_collection();

    // SAFETY: the local root slot remains alive until the collection completes.
    car = unsafe { car_pointer.read_volatile() };
    assert_eq!(heap.widetag(car), Some(13));
    heap.unregister_thread(&native);
    heap.unregister_thread(&collector);
}

#[test]
fn native_alloc_cons_publishes_large_argument_before_waiting_for_collection() {
    let heap = Arc::new(Heap::new(HeapConfig::default()));
    let mut setup = Thread::new();
    assert_eq!(heap.register_thread(&mut setup), Ok(()));
    let large = heap
        .alloc_large(&mut setup, TypeTag { widetag: 13 }, 1023)
        .unwrap_or(Word::NIL);
    assert_eq!(heap.widetag(large), Some(13));
    heap.unregister_thread(&setup);

    let (blocker_ready_sender, blocker_ready_receiver) = mpsc::channel();
    let (poll_sender, poll_receiver) = mpsc::channel();
    let blocker_heap = Arc::clone(&heap);
    let blocker = thread::spawn(move || {
        let mut blocker = Thread::new();
        assert_eq!(blocker_heap.register_thread(&mut blocker), Ok(()));
        assert!(blocker_ready_sender.send(()).is_ok());
        assert!(poll_receiver.recv().is_ok());
        blocker.poll_safepoint();
        blocker_heap.unregister_thread(&blocker);
    });
    assert!(blocker_ready_receiver.recv().is_ok());

    let (native_ready_sender, native_ready_receiver) = mpsc::channel();
    let (native_pointer_sender, native_pointer_receiver) = mpsc::channel();
    let (argument_sender, argument_receiver) = mpsc::channel();
    let (allocation_sender, allocation_receiver) = mpsc::channel();
    let native_heap = Arc::clone(&heap);
    let native = thread::spawn(move || {
        let mut native = Thread::new();
        assert_eq!(native_heap.register_thread(&mut native), Ok(()));
        native.enter_native();
        assert!(
            native_pointer_sender
                .send(std::ptr::from_mut(&mut native).expose_provenance())
                .is_ok()
        );
        assert!(native_ready_sender.send(()).is_ok());
        let object = argument_receiver
            .recv()
            .unwrap_or_else(|_| panic!("test should send the Native allocation argument"));
        let allocation = native_heap.alloc_cons(&mut native, object, Word::NIL);
        assert!(allocation_sender.send(allocation).is_ok());
        native_heap.unregister_thread(&native);
    });
    assert!(native_ready_receiver.recv().is_ok());
    let native_pointer = std::ptr::with_exposed_provenance::<Thread>(
        native_pointer_receiver
            .recv()
            .unwrap_or_else(|_| panic!("Native worker should publish its thread pointer")),
    );

    let (collector_ready_sender, collector_ready_receiver) = mpsc::channel();
    let collector_heap = Arc::clone(&heap);
    let collector = thread::spawn(move || {
        let mut collector = Thread::new();
        assert_eq!(collector_heap.register_thread(&mut collector), Ok(()));
        assert!(collector_ready_sender.send(()).is_ok());
        collector_heap.collect_with_thread(&mut collector, true);
        collector_heap.unregister_thread(&collector);
    });
    assert!(collector_ready_receiver.recv().is_ok());

    wait_for_safepoint_request(&heap);

    assert!(argument_sender.send(large).is_ok());
    wait_for_native_argument_snapshot(&heap, native_pointer, large);

    assert!(matches!(
        allocation_receiver.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    assert!(poll_sender.send(()).is_ok());
    assert!(collector.join().is_ok());

    let allocation = allocation_receiver
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| panic!("Native cons allocation should resume"));
    let cell = allocation
        .unwrap_or_else(|error| panic!("Native cons allocation should succeed: {error:?}"));
    assert_eq!(heap.read_word(cell, 0), Some(large));
    assert_eq!(heap.widetag(large), Some(13));
    assert!(blocker.join().is_ok());
    assert!(native.join().is_ok());
}

fn wait_for_safepoint_request(heap: &Heap) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stop_world = heap
        .stop_world
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    while !stop_world.requested {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "collector did not request a safepoint"
        );
        let (next, wait) = heap
            .stop_world_ready
            .wait_timeout(stop_world, remaining)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world = next;
        assert!(!wait.timed_out() || stop_world.requested);
    }
    drop(stop_world);
}

fn wait_for_native_argument_snapshot(heap: &Heap, native_pointer: *const Thread, large: Word) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let remaining = deadline.saturating_duration_since(Instant::now());
    let (stop_world, _) = heap
        .stop_world_ready
        .wait_timeout_while(
            heap.stop_world
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
            remaining,
            |_| {
                // SAFETY: this wait predicate holds the lock protecting the registered thread's snapshot.
                !unsafe { (&*native_pointer).conservative_snapshot() }.contains(&large)
            },
        )
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: the returned guard still holds the lock protecting the registered thread's snapshot.
    let contains_argument = unsafe { (&*native_pointer).conservative_snapshot() }.contains(&large);
    drop(stop_world);
    assert!(
        contains_argument,
        "Native stack snapshot was not refreshed before the deadline"
    );
}

struct ForwardingRoots {
    values: Vec<Word>,
    addresses: Vec<usize>,
}

fn setup_forwarding_roots(heap: Arc<Heap>) -> ForwardingRoots {
    let setup = thread::spawn(move || {
        let mut mutator = Thread::new();
        assert_eq!(heap.register_thread(&mut mutator), Ok(()));
        let first_result = heap.alloc_cons(&mut mutator, Word::fixnum(1), Word::NIL);
        assert!(first_result.is_ok(), "first setup cons should allocate");
        let first = first_result.unwrap_or(Word::NIL);
        let second_result = heap.alloc_cons(&mut mutator, Word::fixnum(2), Word::NIL);
        assert!(second_result.is_ok(), "second setup cons should allocate");
        let second = second_result.unwrap_or(Word::NIL);
        let roots = ForwardingRoots {
            values: vec![first, second],
            addresses: vec![first.address(), second.address()],
        };
        heap.unregister_thread(&mutator);
        roots
    });
    let roots = setup.join();
    assert!(roots.is_ok(), "setup worker should complete");
    roots.unwrap_or_else(|_| ForwardingRoots {
        values: Vec::new(),
        addresses: Vec::new(),
    })
}

#[test]
fn slow_path_retries_56_byte_request_after_reclaiming_with_16_bytes_free() {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 128,
        bytes_considered_between_gcs: 4096,
    });
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let mut live = heap
        .alloc_cons(&mut thread, Word::fixnum(7), Word::NIL)
        .unwrap_or(Word::NIL);
    let live_root = thread.push_root(&mut live);
    for value in 0..6 {
        let garbage = heap.alloc_cons(&mut thread, Word::fixnum(value), Word::NIL);
        assert!(garbage.is_ok());
    }
    assert_eq!(heap.lock_state().used, 112);
    let epoch = heap.gc_epoch();

    let allocation = heap.alloc(&mut thread, TypeTag { widetag: 8 }, 6);

    assert!(allocation.is_ok());
    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert!(heap.lock_state().used <= 128);
    assert!(Heap::find(&heap.lock_state(), live).is_some());
    assert!(thread.pop_root(live_root));
    heap.unregister_thread(&thread);
}

#[test]
fn slow_path_returns_capacity_error_when_full_collection_cannot_fit_live_set() {
    let heap = Heap::new(HeapConfig {
        dynamic_space_size: 128,
        bytes_considered_between_gcs: 4096,
    });
    let mut thread = Thread::new();
    assert_eq!(heap.register_thread(&mut thread), Ok(()));
    let mut live = Vec::with_capacity(7);
    for value in 0..7 {
        let object = heap.alloc_cons(&mut thread, Word::fixnum(value), Word::NIL);
        assert!(object.is_ok(), "fixture allocation should succeed");
        let object = object.unwrap_or(Word::NIL);
        live.push(object);
    }
    let mut roots = Vec::with_capacity(7);
    for value in &mut live {
        roots.push(thread.push_root(value));
    }
    assert_eq!(heap.lock_state().used, 112);
    let epoch = heap.gc_epoch();

    let allocation = heap.alloc(&mut thread, TypeTag { widetag: 8 }, 6);

    assert_eq!(allocation, Err(StorageCondition::CapacityExceeded));
    assert_eq!(heap.gc_epoch(), epoch + 2);
    assert_eq!(heap.lock_state().used, 112);
    assert_eq!(thread.bytes_cons, 0);
    for value in &live {
        assert!(Heap::find(&heap.lock_state(), *value).is_some());
    }
    for token in roots.into_iter().rev() {
        assert!(thread.pop_root(token));
    }
    heap.unregister_thread(&thread);
}

struct Blocker {
    poll: mpsc::Sender<()>,
    debt: mpsc::Receiver<usize>,
    join: JoinHandle<()>,
}

struct Waiter {
    pointer: usize,
    start: mpsc::Sender<()>,
    result: mpsc::Receiver<(bool, Word, usize, crate::NativeState, crate::SafepointState)>,
    join: JoinHandle<()>,
}

struct Collector {
    pointer: usize,
    start: mpsc::Sender<()>,
    result: mpsc::Receiver<(bool, usize)>,
    join: JoinHandle<()>,
}

fn receive_or<T>(receiver: &mpsc::Receiver<T>, fallback: T, failure: &str) -> T {
    let result = receiver.recv_timeout(Duration::from_secs(5));
    assert!(result.is_ok(), "{failure}");
    result.unwrap_or(fallback)
}

fn spawn_blocker(heap: Arc<Heap>) -> Blocker {
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (poll_sender, poll_receiver) = mpsc::channel();
    let (debt_sender, debt_receiver) = mpsc::channel();
    let join = thread::spawn(move || {
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        thread.bytes_cons = 32;
        assert!(ready_sender.send(()).is_ok());
        assert!(poll_receiver.recv().is_ok());
        crate::poll_safepoint(&mut thread);
        assert!(debt_sender.send(thread.bytes_cons).is_ok());
        heap.unregister_thread(&thread);
    });
    receive_or(&ready_receiver, (), "blocker should register");
    Blocker {
        poll: poll_sender,
        debt: debt_receiver,
        join,
    }
}

fn spawn_waiter(heap: Arc<Heap>, root_value: Word, native: bool) -> Waiter {
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (start_sender, start_receiver) = mpsc::channel();
    let (result_sender, result_receiver) = mpsc::channel();
    let join = thread::spawn(move || {
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        thread.bytes_cons = 17;
        let mut root = root_value;
        let token = thread.push_root(&mut root);
        if native {
            thread.enter_native();
        }
        assert!(
            ready_sender
                .send(std::ptr::from_mut(&mut thread).addr())
                .is_ok()
        );
        assert!(start_receiver.recv().is_ok());
        let result = heap
            .allocate_with_coordinator(
                &mut thread,
                PageKind::HeaderObject,
                TypeTag { widetag: 8 },
                1,
            )
            .map(|allocation| allocation.value);
        let root_after_collection = root;
        let debt = thread.bytes_cons;
        let native_state = thread.native_state();
        let safepoint_state = thread.safepoint_state();
        assert!(thread.pop_root(token));
        heap.unregister_thread(&thread);
        assert!(
            result_sender
                .send((
                    result.is_ok(),
                    root_after_collection,
                    debt,
                    native_state,
                    safepoint_state,
                ))
                .is_ok()
        );
    });
    let pointer = receive_or(&ready_receiver, 0, "waiter should register");
    Waiter {
        pointer,
        start: start_sender,
        result: result_receiver,
        join,
    }
}

fn spawn_collector(heap: Arc<Heap>) -> Collector {
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (start_sender, start_receiver) = mpsc::channel();
    let (result_sender, result_receiver) = mpsc::channel();
    let join = thread::spawn(move || {
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        thread.bytes_cons = 24;
        assert!(
            ready_sender
                .send(std::ptr::from_mut(&mut thread).addr())
                .is_ok()
        );
        assert!(start_receiver.recv().is_ok());
        let result = heap
            .allocate_with_coordinator(
                &mut thread,
                PageKind::HeaderObject,
                TypeTag { widetag: 8 },
                1,
            )
            .map(|allocation| allocation.value);
        let debt = thread.bytes_cons;
        heap.unregister_thread(&thread);
        assert!(result_sender.send((result.is_ok(), debt)).is_ok());
    });
    let pointer = receive_or(&ready_receiver, 0, "collector should register");
    Collector {
        pointer,
        start: start_sender,
        result: result_receiver,
        join,
    }
}

fn wait_for_waiter_parked(
    heap: &Heap,
    collector_pointer: usize,
    waiter_pointer: usize,
    native_waiter: bool,
    waiter_start: &mpsc::Sender<()>,
    deadline: Instant,
) {
    let mut stop_world = heap
        .stop_world
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    while !stop_world.requested {
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            !remaining.is_zero(),
            "collector did not request a safepoint"
        );
        let (next, wait) = heap
            .stop_world_ready
            .wait_timeout(stop_world, remaining)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world = next;
        assert!(!wait.timed_out() || stop_world.requested);
    }
    // SAFETY: the stop-world lock serializes state reads while the collector owns the gate.
    let collector_state = unsafe { (*(collector_pointer as *const Thread)).state };
    assert_eq!(collector_state, crate::SafepointState::Collector);
    assert!(waiter_start.send(()).is_ok());
    loop {
        // SAFETY: the stop-world lock serializes the collector state read.
        let collector_state = unsafe { (*(collector_pointer as *const Thread)).state };
        // SAFETY: the stop-world lock serializes these waiter state reads.
        let waiter = unsafe { &*(waiter_pointer as *const Thread) };
        let waiter_is_parked = if native_waiter {
            waiter.native_state() == crate::NativeState::Native
                && waiter.state == crate::SafepointState::Safe
                && !stop_world.parked.contains(&waiter_pointer)
        } else {
            waiter.native_state() != crate::NativeState::Native
                && waiter.state == crate::SafepointState::Parked
                && stop_world.parked.contains(&waiter_pointer)
        };
        if collector_state == crate::SafepointState::Collector
            && waiter_is_parked
            && stop_world.gate_parked.contains(&waiter_pointer)
        {
            assert!(stop_world.parked.len() < heap.active_mutators());
            break;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(!remaining.is_zero(), "slow path waiter did not park");
        let (next, wait) = heap
            .stop_world_ready
            .wait_timeout(stop_world, remaining)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        stop_world = next;
        assert!(!wait.timed_out());
    }
    // SAFETY: the waiter remains parked until the collector releases the slow-path gate.
    let root_count = unsafe { (*(waiter_pointer as *const Thread)).roots.len() };
    assert_eq!(root_count, 1);
    drop(stop_world);
}

fn assert_heap_state_lock_available(heap: &Heap) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match heap.state.try_lock() {
            Ok(state) => {
                drop(state);
                return;
            }
            Err(std::sync::TryLockError::Poisoned(error)) => {
                drop(error.into_inner());
                return;
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                assert!(Instant::now() < deadline, "heap state lock remained held");
                thread::yield_now();
            }
        }
    }
}

#[test]
fn concurrent_slow_path_waiter_publishes_roots_and_retries_after_collector() {
    run_concurrent_slow_path_waiter_publishes_roots_and_retries_after_collector(false);
}

#[test]
fn native_gate_waiter_does_not_satisfy_running_mutator_quorum() {
    run_concurrent_slow_path_waiter_publishes_roots_and_retries_after_collector(true);
}

fn run_concurrent_slow_path_waiter_publishes_roots_and_retries_after_collector(
    native_waiter: bool,
) {
    let heap = Arc::new(Heap::new(HeapConfig {
        dynamic_space_size: 4096,
        bytes_considered_between_gcs: 16,
    }));
    let mut setup = Thread::new();
    assert_eq!(heap.register_thread(&mut setup), Ok(()));
    let root_result = heap.alloc_cons(&mut setup, Word::fixnum(1), Word::NIL);
    assert!(root_result.is_ok());
    let root_value = root_result.unwrap_or(Word::NIL);
    heap.unregister_thread(&setup);

    let blocker = spawn_blocker(Arc::clone(&heap));
    let waiter = spawn_waiter(Arc::clone(&heap), root_value, native_waiter);
    let collector = spawn_collector(Arc::clone(&heap));
    let epoch = heap.gc_epoch();
    assert!(collector.start.send(()).is_ok());

    let deadline = Instant::now() + Duration::from_secs(5);
    wait_for_waiter_parked(
        &heap,
        collector.pointer,
        waiter.pointer,
        native_waiter,
        &waiter.start,
        deadline,
    );
    assert_heap_state_lock_available(&heap);
    assert!(blocker.poll.send(()).is_ok());

    assert!(blocker.join.join().is_ok());
    assert!(collector.join.join().is_ok());
    assert!(waiter.join.join().is_ok());
    let (collector_ok, collector_debt) =
        receive_or(&collector.result, (false, 0), "collector should return");
    let (waiter_ok, root_after_collection, waiter_debt, native_state, safepoint_state) = receive_or(
        &waiter.result,
        (
            false,
            Word::NIL,
            0,
            crate::NativeState::Lisp,
            crate::SafepointState::Running,
        ),
        "waiter should return",
    );
    let blocker_debt = receive_or(&blocker.debt, 0, "blocker should return");
    assert!(collector_ok);
    assert!(waiter_ok);
    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert_eq!(collector_debt, 8);
    assert_eq!(waiter_debt, 8);
    assert_eq!(
        native_state,
        if native_waiter {
            crate::NativeState::Native
        } else {
            crate::NativeState::Lisp
        }
    );
    assert_eq!(
        safepoint_state,
        if native_waiter {
            crate::SafepointState::Safe
        } else {
            crate::SafepointState::Running
        }
    );
    assert_eq!(blocker_debt, 0);
    assert!(heap.forwarded_word(root_after_collection).is_some());
}
