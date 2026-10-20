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

    let allocation = heap
        .allocate_with_coordinator(
            &mut collector,
            PageKind::HeaderObject,
            TypeTag { widetag: 8 },
            1,
            true,
        )
        .map(|allocation| allocation.value);

    assert!(allocation.is_ok());
    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert_eq!(native.bytes_cons, 0);
    assert_eq!(collector.bytes_cons, 8);
    heap.unregister_thread(&native);
    heap.unregister_thread(&collector);
}

#[test]
fn slow_path_cons_forwards_rooted_inputs_before_writing_the_new_cell() {
    let heap = Arc::new(Heap::new(HeapConfig {
        dynamic_space_size: 256,
        bytes_considered_between_gcs: 16,
    }));
    let roots = setup_forwarding_roots(Arc::clone(&heap));
    let collection_heap = Arc::clone(&heap);
    let collection = thread::spawn(move || {
        let ForwardingRoots {
            mut values,
            addresses,
        } = roots;
        assert_eq!(values.len(), 2);
        assert_eq!(addresses.len(), 2);
        let (first_values, remaining_values) = values.split_at_mut(1);
        let first_root = first_values.first_mut();
        let second_root = remaining_values.first_mut();
        assert!(first_root.is_some());
        assert!(second_root.is_some());
        let Some(first_root) = first_root else {
            return;
        };
        let Some(second_root) = second_root else {
            return;
        };

        let mut mutator = Thread::new();
        assert_eq!(collection_heap.register_thread(&mut mutator), Ok(()));
        mutator.bytes_cons = 17;
        let epoch = collection_heap.gc_epoch();
        let allocation_result = collection_heap.allocate_cons_with_coordinator(
            &mut mutator,
            first_root,
            second_root,
            true,
        );
        assert!(
            allocation_result.is_ok(),
            "slow-path cons allocation should succeed"
        );
        let cell = allocation_result.unwrap_or(Word::NIL);

        assert_eq!(collection_heap.gc_epoch(), epoch + 1);
        let forwarded_first = values.first().copied().unwrap_or(Word::NIL);
        let forwarded_second = values.get(1).copied().unwrap_or(Word::NIL);
        let old_first_address = addresses.first().copied().unwrap_or(0);
        let old_second_address = addresses.get(1).copied().unwrap_or(0);
        assert_ne!(forwarded_first.address(), old_first_address);
        assert_ne!(forwarded_second.address(), old_second_address);
        assert_eq!(
            collection_heap.read_word(forwarded_first, 0),
            Some(Word::fixnum(1))
        );
        assert_eq!(
            collection_heap.read_word(forwarded_second, 0),
            Some(Word::fixnum(2))
        );

        assert_eq!(collection_heap.read_word(cell, 0), Some(forwarded_first));
        assert_eq!(collection_heap.read_word(cell, 1), Some(forwarded_second));
        collection_heap.unregister_thread(&mutator);
    });
    let result = collection.join();
    assert!(result.is_ok(), "collection worker should complete");
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

    let allocation = heap
        .allocate_with_coordinator(
            &mut thread,
            PageKind::HeaderObject,
            TypeTag { widetag: 8 },
            7,
            true,
        )
        .map(|allocation| allocation.value);

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

    let allocation = heap.allocate_with_coordinator(
        &mut thread,
        PageKind::HeaderObject,
        TypeTag { widetag: 8 },
        7,
        true,
    );

    assert_eq!(
        allocation.map(|allocation| allocation.value),
        Err(StorageCondition::CapacityExceeded)
    );
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
    result: mpsc::Receiver<(bool, Word, usize)>,
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

fn spawn_waiter(heap: Arc<Heap>, root_value: Word) -> Waiter {
    let (ready_sender, ready_receiver) = mpsc::channel();
    let (start_sender, start_receiver) = mpsc::channel();
    let (result_sender, result_receiver) = mpsc::channel();
    let join = thread::spawn(move || {
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        let mut root = root_value;
        let token = thread.push_root(&mut root);
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
                true,
            )
            .map(|allocation| allocation.value);
        let root_after_collection = root;
        let debt = thread.bytes_cons;
        assert!(thread.pop_root(token));
        heap.unregister_thread(&thread);
        assert!(
            result_sender
                .send((result.is_ok(), root_after_collection, debt))
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
                true,
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
        // SAFETY: the stop-world lock serializes the waiter state read.
        let waiter_state = unsafe { (*(waiter_pointer as *const Thread)).state };
        if collector_state == crate::SafepointState::Collector
            && waiter_state == crate::SafepointState::Parked
            && stop_world.gate_parked.contains(&waiter_pointer)
        {
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
    let waiter = spawn_waiter(Arc::clone(&heap), root_value);
    let collector = spawn_collector(Arc::clone(&heap));
    let epoch = heap.gc_epoch();
    assert!(collector.start.send(()).is_ok());

    let deadline = Instant::now() + Duration::from_secs(5);
    wait_for_waiter_parked(
        &heap,
        collector.pointer,
        waiter.pointer,
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
    let (waiter_ok, root_after_collection, waiter_debt) = receive_or(
        &waiter.result,
        (false, Word::NIL, 0),
        "waiter should return",
    );
    let blocker_debt = receive_or(&blocker.debt, 0, "blocker should return");
    assert!(collector_ok);
    assert!(waiter_ok);
    assert_eq!(heap.gc_epoch(), epoch + 1);
    assert_eq!(collector_debt, 8);
    assert_eq!(waiter_debt, 8);
    assert_eq!(blocker_debt, 0);
    assert!(heap.forwarded_word(root_after_collection).is_some());
}
