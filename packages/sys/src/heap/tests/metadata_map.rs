use crate::heap::metadata::{MetadataBudget, MetadataError};
use crate::heap::metadata_map::{Bucket, MetadataMap, MetadataSet};
use core::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CollisionKey(usize);

impl Hash for CollisionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0_u8.hash(state);
    }
}

#[test]
fn collision_chains_survive_removal_and_tombstone_reuse() {
    let mut map = MetadataMap::new(MetadataBudget::default());
    for id in 0..5 {
        assert!(map.insert(CollisionKey(id), id * 10).is_ok());
    }

    assert_eq!(map.remove(&CollisionKey(2)), Some(20));
    assert_eq!(map.get(&CollisionKey(3)), Some(&30));
    assert!(map.insert(CollisionKey(5), 50).is_ok());
    assert_eq!(map.get(&CollisionKey(5)), Some(&50));
    assert_eq!(map.get(&CollisionKey(4)), Some(&40));
}

#[test]
fn growth_preserves_entries() {
    let mut map = MetadataMap::new(MetadataBudget::default());
    for key in 0..128 {
        assert!(map.insert(key, key * 3).is_ok());
    }

    assert_eq!(map.len(), 128);
    for key in 0..128 {
        assert_eq!(map.get(&key), Some(&(key * 3)));
    }
}

#[test]
fn replacement_and_removal_return_expected_values() {
    let mut map = MetadataMap::new(MetadataBudget::default());
    assert!(matches!(map.insert("name", 1), Ok(None)));
    assert_eq!(map.insert("name", 2).ok(), Some(Some(1)));
    assert_eq!(map.get(&"name"), Some(&2));
    assert_eq!(map.remove(&"name"), Some(2));
    assert!(!map.contains_key(&"name"));
    assert!(map.is_empty());
}

#[test]
fn peak_charge_rejection_preserves_map_and_drops_rejected_value() {
    struct CountedValue {
        drops: Arc<AtomicUsize>,
    }

    impl Drop for CountedValue {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    let bucket_bytes = core::mem::size_of::<Bucket<usize, CountedValue>>();
    let budget = MetadataBudget::new(Some(bucket_bytes * 20));
    let mut map: MetadataMap<usize, CountedValue> = MetadataMap::new(budget.clone());
    let mut expected_drops = Vec::new();
    for key in 0..6 {
        let drops = Arc::new(AtomicUsize::new(0));
        assert!(
            map.insert(
                key,
                CountedValue {
                    drops: Arc::clone(&drops),
                },
            )
            .is_ok()
        );
        expected_drops.push(drops);
    }
    let used_before = budget.used_bytes();
    assert_eq!(used_before, bucket_bytes * 8);

    let rejected_drops = Arc::new(AtomicUsize::new(0));
    let Err(error) = map.insert(
        6,
        CountedValue {
            drops: Arc::clone(&rejected_drops),
        },
    ) else {
        panic!("growth should exceed the old-plus-new table charge");
    };

    assert!(matches!(error, MetadataError::CapacityExceeded { .. }));
    assert_eq!(rejected_drops.load(Ordering::SeqCst), 1);
    assert_eq!(map.len(), 6);
    assert_eq!(budget.used_bytes(), used_before);
    for (key, drops) in expected_drops.iter().enumerate() {
        assert!(
            map.get(&key)
                .is_some_and(|value| Arc::ptr_eq(&value.drops, drops))
        );
    }
    drop(map);
    for drops in expected_drops {
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn retain_keeps_selected_entries_and_allows_mutation() {
    let budget = MetadataBudget::default();
    let mut map = MetadataMap::new(budget.clone());
    for key in 0..8 {
        assert!(map.insert(key, key).is_ok());
    }
    let used_before = budget.used_bytes();
    map.retain(|key, value| {
        *value += 10;
        key % 2 == 0
    });

    assert_eq!(map.len(), 4);
    for key in [0, 2, 4, 6] {
        assert_eq!(map.get(&key), Some(&(key + 10)));
    }
    for key in [1, 3, 5, 7] {
        assert!(!map.contains_key(&key));
    }
    assert_eq!(budget.used_bytes(), used_before);
}

#[test]
fn map_and_set_drop_release_their_table_charges() {
    let budget = MetadataBudget::default();
    let mut map = MetadataMap::new(budget.clone());
    assert!(map.insert(1, 2).is_ok());
    assert!(budget.used_bytes() > 0);
    drop(map);
    assert_eq!(budget.used_bytes(), 0);

    let mut set = MetadataSet::new(budget.clone());
    assert!(set.insert(3).is_ok());
    assert!(budget.used_bytes() > 0);
    drop(set);
    assert_eq!(budget.used_bytes(), 0);
}

#[test]
fn set_operations_match_map_membership() {
    let mut set = MetadataSet::new(MetadataBudget::default());
    assert!(matches!(set.insert(1), Ok(true)));
    assert!(matches!(set.insert(1), Ok(false)));
    assert!(set.insert(2).is_ok());
    assert!(set.contains(&1));
    assert_eq!(set.remove(&1), Some(1));
    assert!(!set.contains(&1));
    set.retain(|value| value % 2 == 0);
    assert_eq!(set.iter().copied().collect::<Vec<_>>(), [2]);
}
