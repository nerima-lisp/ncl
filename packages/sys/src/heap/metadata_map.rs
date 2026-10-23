use super::metadata::{MetadataBudget, MetadataError, MetadataVec};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hash};
use std::mem;

const MIN_CAPACITY: usize = 8;

#[derive(Debug)]
pub(super) enum Bucket<K, V> {
    Empty,
    Tombstone,
    Occupied { hash: u64, key: K, value: V },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Probe {
    Found(usize),
    Vacant(usize),
    Full,
}

/// A capacity-accounted open-addressing map.
///
/// The budget covers requested bucket-array storage, including inline key and
/// value slots. Allocations owned by keys or values are outside the budget.
/// Rehashing holds charges for both arrays until all entries have moved and the
/// old table drops.
#[derive(Debug)]
pub struct MetadataMap<K, V> {
    budget: MetadataBudget,
    slots: MetadataVec<Bucket<K, V>>,
    len: usize,
    tombstones: usize,
    hasher: RandomState,
}

impl<K: Eq + Hash, V> MetadataMap<K, V> {
    /// Create an empty map charged to `budget` when its table grows.
    #[must_use]
    pub fn new(budget: MetadataBudget) -> Self {
        Self {
            slots: MetadataVec::new(budget.clone()),
            budget,
            len: 0,
            tombstones: 0,
            hasher: RandomState::new(),
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    #[cfg(test)]
    const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Remove all entries while retaining the table capacity.
    pub fn clear(&mut self) {
        self.retain(|_, _| false);
        for bucket in &mut self.slots {
            if matches!(bucket, Bucket::Tombstone) {
                *bucket = Bucket::Empty;
            }
        }
        self.tombstones = 0;
    }

    /// Ensure table capacity for `additional` distinct entries.
    pub fn reserve(&mut self, additional: usize) -> Result<(), MetadataError> {
        let entries = self
            .len
            .checked_add(additional)
            .ok_or(MetadataError::CapacityOverflow)?;
        let required_capacity = table_capacity(entries)?;
        if required_capacity <= self.slots.capacity() {
            return Ok(());
        }
        self.rehash(required_capacity)
    }

    /// Insert a key/value pair, returning the previous value for an equal key.
    ///
    /// An equal existing key is preserved when replacing a value. The key and
    /// value are dropped if the insertion fails.
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, MetadataError> {
        let hash = self.hash(&key);
        if let Probe::Found(index) = self.probe(&key, hash) {
            let Some(Bucket::Occupied { value: current, .. }) = self.slots.get_mut(index) else {
                return Err(MetadataError::InvalidTableState);
            };
            return Ok(Some(mem::replace(current, value)));
        }

        self.reserve(1)?;

        let index = match self.probe(&key, hash) {
            Probe::Vacant(index) => index,
            Probe::Found(index) => {
                let Some(Bucket::Occupied { value: current, .. }) = self.slots.get_mut(index)
                else {
                    return Err(MetadataError::InvalidTableState);
                };
                return Ok(Some(mem::replace(current, value)));
            }
            Probe::Full => return Err(MetadataError::InvalidTableState),
        };
        let Some(slot) = self.slots.get_mut(index) else {
            return Err(MetadataError::InvalidTableState);
        };
        let new_tombstones = if matches!(slot, Bucket::Tombstone) {
            let Some(tombstones) = self.tombstones.checked_sub(1) else {
                return Err(MetadataError::InvalidTableState);
            };
            tombstones
        } else {
            self.tombstones
        };
        let Some(new_len) = self.len.checked_add(1) else {
            return Err(MetadataError::CapacityOverflow);
        };
        *slot = Bucket::Occupied { hash, key, value };
        self.tombstones = new_tombstones;
        self.len = new_len;
        Ok(None)
    }

    #[must_use]
    pub fn get(&self, key: &K) -> Option<&V> {
        let hash = self.hash(key);
        match self.probe(key, hash) {
            Probe::Found(index) => match self.slots.get(index)? {
                Bucket::Occupied { value, .. } => Some(value),
                Bucket::Empty | Bucket::Tombstone => None,
            },
            Probe::Vacant(_) | Probe::Full => None,
        }
    }

    #[must_use]
    pub fn contains_key(&self, key: &K) -> bool {
        self.get(key).is_some()
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        self.remove_entry(key).map(|(_, value)| value)
    }

    /// Iterate over the key/value pairs in table order.
    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> + '_ {
        self.slots.iter().filter_map(|bucket| match bucket {
            Bucket::Occupied { key, value, .. } => Some((key, value)),
            Bucket::Empty | Bucket::Tombstone => None,
        })
    }

    /// Keep pairs accepted by `predicate`, preserving their table positions.
    ///
    /// If the predicate or a removed pair's destructor panics, all buckets
    /// remain valid. Changes to values made before a predicate panic are kept.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        for bucket in &mut self.slots {
            let keep = match bucket {
                Bucket::Occupied { key, value, .. } => predicate(key, value),
                Bucket::Empty | Bucket::Tombstone => true,
            };
            if !keep {
                let removed = mem::replace(bucket, Bucket::Tombstone);
                self.len -= 1;
                self.tombstones += 1;
                drop(removed);
            }
        }
    }

    fn remove_entry(&mut self, key: &K) -> Option<(K, V)> {
        let hash = self.hash(key);
        let Probe::Found(index) = self.probe(key, hash) else {
            return None;
        };
        let bucket = self.slots.get_mut(index)?;
        if !matches!(bucket, Bucket::Occupied { .. }) {
            return None;
        }
        let removed = mem::replace(bucket, Bucket::Tombstone);
        self.len -= 1;
        self.tombstones += 1;
        match removed {
            Bucket::Occupied { key, value, .. } => Some((key, value)),
            Bucket::Empty | Bucket::Tombstone => None,
        }
    }

    fn hash(&self, key: &K) -> u64 {
        self.hasher.hash_one(key)
    }

    fn probe(&self, key: &K, hash: u64) -> Probe {
        let capacity = self.slots.len();
        if capacity == 0 {
            return Probe::Full;
        }

        let mask = capacity - 1;
        let start = platform_hash_index(hash) & mask;
        let mut first_tombstone = None;
        for distance in 0..capacity {
            let index = start.wrapping_add(distance) & mask;
            match self.slots.get(index) {
                None => return Probe::Full,
                Some(Bucket::Empty) => {
                    return Probe::Vacant(first_tombstone.unwrap_or(index));
                }
                Some(Bucket::Tombstone) => {
                    if first_tombstone.is_none() {
                        first_tombstone = Some(index);
                    }
                }
                Some(Bucket::Occupied {
                    hash: existing_hash,
                    key: existing_key,
                    ..
                }) if *existing_hash == hash && existing_key == key => {
                    return Probe::Found(index);
                }
                Some(Bucket::Occupied { .. }) => {}
            }
        }

        first_tombstone.map_or(Probe::Full, Probe::Vacant)
    }

    fn rehash(&mut self, capacity: usize) -> Result<(), MetadataError> {
        let mut new_slots = empty_slots(self.budget.clone(), capacity)?;
        if new_slots.len() <= self.len {
            return Err(MetadataError::InvalidTableState);
        }
        for bucket in &mut self.slots {
            let bucket = mem::replace(bucket, Bucket::Tombstone);
            if let Bucket::Occupied { hash, key, value } = bucket {
                insert_into_empty_table(&mut new_slots, hash, key, value)?;
            }
        }
        self.slots = new_slots;
        self.tombstones = 0;
        Ok(())
    }
}

/// A capacity-accounted open-addressing set.
#[derive(Debug)]
pub struct MetadataSet<T> {
    map: MetadataMap<T, ()>,
}

impl<T: Eq + Hash> MetadataSet<T> {
    /// Create an empty set charged to `budget` when its table grows.
    #[must_use]
    pub fn new(budget: MetadataBudget) -> Self {
        Self {
            map: MetadataMap::new(budget),
        }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.map.len()
    }

    /// Remove all values while retaining the table capacity.
    pub fn clear(&mut self) {
        self.map.clear();
    }

    /// Ensure table capacity for `additional` distinct values.
    pub fn reserve(&mut self, additional: usize) -> Result<(), MetadataError> {
        self.map.reserve(additional)
    }

    /// Insert `value`, returning whether it was newly added.
    ///
    /// The value is dropped if insertion fails. Inserting an equal value
    /// preserves the existing set element.
    pub fn insert(&mut self, value: T) -> Result<bool, MetadataError> {
        match self.map.insert(value, ()) {
            Ok(previous) => Ok(previous.is_none()),
            Err(error) => Err(error),
        }
    }

    #[must_use]
    pub fn contains(&self, value: &T) -> bool {
        self.map.contains_key(value)
    }

    /// Remove `value` and return the stored element, if present.
    pub fn remove(&mut self, value: &T) -> Option<T> {
        self.map.remove_entry(value).map(|(stored, ())| stored)
    }

    /// Iterate over the values in table order.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        self.map.iter().map(|(value, ())| value)
    }

    /// Keep values accepted by `predicate`.
    ///
    /// If the predicate or a removed value's destructor panics, the set remains
    /// valid and contains every value not yet removed.
    pub fn retain<F>(&mut self, mut predicate: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.map.retain(|value, ()| predicate(value));
    }
}

fn table_capacity(entries: usize) -> Result<usize, MetadataError> {
    if entries == 0 {
        return Ok(0);
    }
    let extra = entries
        .checked_add(2)
        .ok_or(MetadataError::CapacityOverflow)?
        / 3;
    entries
        .checked_add(extra)
        .ok_or(MetadataError::CapacityOverflow)?
        .max(MIN_CAPACITY)
        .checked_next_power_of_two()
        .ok_or(MetadataError::CapacityOverflow)
}

fn platform_hash_index(hash: u64) -> usize {
    let mut bytes = [0; core::mem::size_of::<usize>()];
    for (byte, hash_byte) in bytes.iter_mut().zip(hash.to_le_bytes().iter()) {
        *byte = *hash_byte;
    }
    usize::from_le_bytes(bytes)
}

fn empty_slots<K, V>(
    budget: MetadataBudget,
    capacity: usize,
) -> Result<MetadataVec<Bucket<K, V>>, MetadataError> {
    let mut slots = MetadataVec::new(budget);
    slots.reserve(capacity)?;
    for _ in 0..capacity {
        if let Err(error) = slots.push(Bucket::Empty) {
            return Err(error.error);
        }
    }
    Ok(slots)
}

fn insert_into_empty_table<K, V>(
    slots: &mut MetadataVec<Bucket<K, V>>,
    hash: u64,
    key: K,
    value: V,
) -> Result<(), MetadataError> {
    let Some(mask) = slots.len().checked_sub(1) else {
        return Err(MetadataError::InvalidTableState);
    };
    let mut index = platform_hash_index(hash) & mask;
    for _ in 0..slots.len() {
        let Some(slot) = slots.get_mut(index) else {
            return Err(MetadataError::InvalidTableState);
        };
        if matches!(slot, Bucket::Empty) {
            *slot = Bucket::Occupied { hash, key, value };
            return Ok(());
        }
        index = index.wrapping_add(1) & mask;
    }
    Err(MetadataError::InvalidTableState)
}

#[cfg(test)]
#[path = "tests/metadata_map.rs"]
mod tests;
