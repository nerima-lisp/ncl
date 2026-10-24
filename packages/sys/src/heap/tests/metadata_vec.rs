use super::{MetadataBudget, MetadataError, MetadataVec};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn rejected_reserve_preserves_contents_and_charge() {
    let item_bytes = core::mem::size_of::<usize>();
    let budget = MetadataBudget::new(Some(item_bytes * 2));
    let mut values = MetadataVec::<usize>::new(budget.clone());
    assert!(values.push(10).is_ok());
    assert!(values.push(20).is_ok());
    let used_before = budget.used_bytes();
    let capacity_before = values.capacity();

    let error = match values.reserve(1) {
        Ok(()) => panic!("reserve unexpectedly exceeded the configured capacity"),
        Err(error) => error,
    };

    assert!(matches!(error, MetadataError::CapacityExceeded { .. }));
    assert_eq!(values.as_slice(), &[10, 20]);
    assert_eq!(values.capacity(), capacity_before);
    assert_eq!(budget.used_bytes(), used_before);
}

#[test]
fn drop_releases_the_requested_capacity_charge() {
    let budget = MetadataBudget::new(None);
    let mut values = MetadataVec::<usize>::new(budget.clone());
    assert!(values.reserve(3).is_ok());
    let Ok(layout) = core::alloc::Layout::array::<usize>(values.capacity()) else {
        panic!("vector capacity must have a valid layout");
    };
    let expected_charge = layout.size();
    assert_eq!(budget.used_bytes(), expected_charge);

    drop(values);

    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn growth_preserves_values_and_indexing() {
    let mut values = MetadataVec::new(MetadataBudget::default());
    for value in 0..32 {
        assert!(values.push(value).is_ok());
    }

    assert_eq!(values.len(), 32);
    assert!(values.capacity() >= values.len());
    assert_eq!(values[0], 0);
    assert_eq!(values[31], 31);
    assert_eq!(
        values.iter().copied().collect::<Vec<_>>(),
        (0..32).collect::<Vec<_>>()
    );
}

#[test]
fn pop_remove_and_clear_drop_each_removed_value_once() {
    struct Counted {
        id: usize,
        drops: Arc<AtomicUsize>,
    }

    impl Drop for Counted {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let budget = MetadataBudget::default();
    let mut values = MetadataVec::new(budget.clone());
    for id in 0..4 {
        assert!(
            values
                .push(Counted {
                    id,
                    drops: Arc::clone(&drops),
                })
                .is_ok()
        );
    }
    let charge_before = budget.used_bytes();

    let Some(removed) = values.remove(1) else {
        panic!("index exists");
    };
    assert_eq!(removed.id, 1);
    assert_eq!(budget.used_bytes(), charge_before);
    assert_eq!(
        values.iter().map(|value| value.id).collect::<Vec<_>>(),
        [0, 2, 3]
    );
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(removed);
    assert_eq!(drops.load(Ordering::SeqCst), 1);

    let Some(popped) = values.pop() else {
        panic!("the vector should contain a last element");
    };
    assert_eq!(popped.id, 3);
    drop(popped);
    assert_eq!(drops.load(Ordering::SeqCst), 2);

    values.clear();
    assert!(values.is_empty());
    assert_eq!(budget.used_bytes(), charge_before);
    assert_eq!(drops.load(Ordering::SeqCst), 4);
    assert!(values.pop().is_none());
    drop(values);
    assert_eq!(drops.load(Ordering::SeqCst), 4);
}

#[test]
fn retain_preserves_order_and_remains_valid_if_predicate_panics() {
    let budget = MetadataBudget::default();
    let mut values = MetadataVec::new(budget.clone());
    for value in 0..6 {
        assert!(values.push(value).is_ok());
    }
    let charge_before = budget.used_bytes();

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        values.retain(|value| match *value {
            1 => false,
            3 => panic!("predicate panic"),
            _ => true,
        });
    }));

    assert!(outcome.is_err());
    assert_eq!(values.as_slice(), &[0, 2, 3, 4, 5]);
    assert_eq!(budget.used_bytes(), charge_before);
    values.retain(|value| value % 2 == 0);
    assert_eq!(values.as_slice(), &[0, 2, 4]);
    assert_eq!(budget.used_bytes(), charge_before);
}

#[test]
fn retain_moves_values_across_interleaved_deletions() {
    let mut values = MetadataVec::new(MetadataBudget::default());
    for value in 0..4 {
        assert!(values.push(value).is_ok());
    }

    values.retain(|value| *value % 2 == 1);

    assert_eq!(values.as_slice(), &[1, 3]);
}

#[test]
fn retain_does_not_redrop_an_element_when_its_destructor_panics() {
    struct PanicOnDrop {
        id: usize,
        drops: Arc<AtomicUsize>,
    }

    impl Drop for PanicOnDrop {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
            assert_ne!(self.id, 1, "removed element destructor panic");
        }
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let mut values = MetadataVec::new(MetadataBudget::default());
    for id in 0..4 {
        assert!(
            values
                .push(PanicOnDrop {
                    id,
                    drops: Arc::clone(&drops),
                })
                .is_ok()
        );
    }

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        values.retain(|value| value.id != 1);
    }));

    assert!(outcome.is_err());
    assert_eq!(
        values.iter().map(|value| value.id).collect::<Vec<_>>(),
        [0, 2, 3]
    );
    drop(values);
    assert_eq!(drops.load(Ordering::SeqCst), 4);
}

#[test]
fn references_into_iterate_over_initialized_elements() {
    let mut values = MetadataVec::new(MetadataBudget::default());
    assert!(values.push(2).is_ok());
    assert!(values.push(4).is_ok());
    assert_eq!((&values).into_iter().copied().collect::<Vec<_>>(), [2, 4]);

    for value in &mut values {
        *value += 1;
    }
    assert_eq!(values.as_slice(), &[3, 5]);
}

#[test]
fn dropping_a_vector_releases_only_its_owned_charge() {
    let item_bytes = core::mem::size_of::<usize>();
    let budget = MetadataBudget::default();
    let Ok(mut separate_charge) = budget.reserve(item_bytes) else {
        panic!("unbounded budget should accept the reservation");
    };
    let mut values = MetadataVec::<usize>::new(budget.clone());
    assert!(values.push(7).is_ok());
    assert_eq!(budget.used_bytes(), item_bytes * 2);

    assert!(matches!(
        separate_charge.release(item_bytes * 2),
        Err(MetadataError::ReleaseUnderflow { .. })
    ));
    assert_eq!(budget.used_bytes(), item_bytes * 2);

    drop(values);
    assert_eq!(budget.used_bytes(), item_bytes);
    drop(separate_charge);
    assert_eq!(budget.used_bytes(), 0);
}
