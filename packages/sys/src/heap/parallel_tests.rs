use super::*;

#[test]
fn collector_worker_count_is_configurable() {
    let parallel = Heap::new_with_workers(HeapConfig::default(), 2);
    assert_eq!(parallel.worker_count(), 2);

    let serial = Heap::new_with_workers(HeapConfig::default(), 1);
    assert_eq!(serial.worker_count(), 1);
    let mut thread = Thread::new();
    assert_eq!(serial.register_thread(&mut thread), Ok(()));
    let mut root = serial
        .alloc_cons(&mut thread, Word::fixnum(1), Word::NIL)
        .unwrap_or(Word::NIL);
    let token = serial.push_root(&mut root);
    serial.collect(false);
    assert!(serial.read_word(root, 0).is_some());
    assert!(serial.pop_root(token));
}

#[test]
fn parallel_and_serial_scavenge_preserve_the_same_chain() {
    fn collect_chain(worker_count: usize) -> Vec<i64> {
        let storage = Heap::new_with_workers(HeapConfig::default(), worker_count);
        let mut thread = Thread::new();
        assert_eq!(storage.register_thread(&mut thread), Ok(()));
        let mut head = Word::NIL;
        for value in 0..5000 {
            head = storage
                .alloc_cons(&mut thread, Word::fixnum(value), head)
                .unwrap_or(Word::NIL);
        }
        let token = thread.push_root(&mut head);
        storage.collect(false);
        let mut values = Vec::new();
        let mut current = head;
        while current != Word::NIL {
            let car = storage.read_word(current, 0).unwrap_or(Word::NIL);
            values.push(car.as_fixnum().unwrap_or(-1));
            current = storage.read_word(current, 1).unwrap_or(Word::NIL);
        }
        assert!(thread.pop_root(token));
        values
    }

    assert_eq!(collect_chain(1), collect_chain(2));
}
