//! Open-addressed hash tables used by the object layer.

use ncl_sys::Word;

/// Hash comparison required by a Common Lisp hash table.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HashTest {
    Eq,
    Eql,
    Equal,
    Equalp,
}

/// Weak reference policy for table entries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Weakness {
    None,
    Key,
    Value,
    KeyAndValue,
    KeyOrValue,
}

#[derive(Clone, Copy, Debug)]
struct Entry {
    key: Word,
    value: Word,
    hash: u64,
    live: bool,
}

/// A synchronized-by-owner open-addressed table with a power-of-two capacity.
#[derive(Debug)]
pub struct HashTable {
    slots: Vec<Option<Entry>>,
    len: usize,
    test: HashTest,
    weakness: Weakness,
}

impl HashTable {
    /// Create an empty table. Capacity is rounded up to a power of two.
    #[must_use]
    pub fn new(test: HashTest, weakness: Weakness, capacity: usize) -> Self {
        let mut size = capacity.max(2).next_power_of_two();
        while size < 8 {
            size *= 2;
        }
        Self {
            slots: vec![None; size],
            len: 0,
            test,
            weakness,
        }
    }
    /// Return the comparison mode.
    #[must_use]
    pub const fn test(&self) -> HashTest {
        self.test
    }
    /// Return the weakness mode.
    #[must_use]
    pub const fn weakness(&self) -> Weakness {
        self.weakness
    }
    /// Return the number of live entries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }
    /// Return whether the table has no live entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Return the current capacity.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.slots.len()
    }
    /// Insert or replace an entry.
    pub fn insert(&mut self, key: Word, value: Word) {
        if (self.len + 1) * 8 >= self.capacity() * 7 {
            self.rehash(self.capacity() * 2);
        }
        self.insert_hashed(key, value, sxhash(key));
    }
    /// Find a value using the table's comparison mode.
    #[must_use]
    pub fn get(&self, key: Word) -> Option<Word> {
        self.find(key, sxhash(key)).map(|entry| entry.value)
    }
    /// Remove an entry and return its value.
    pub fn remove(&mut self, key: Word) -> Option<Word> {
        let hash = sxhash(key);
        let index = self.find_index(key, hash)?;
        let value = self.slots[index].take()?.value;
        self.len -= 1;
        self.rehash(self.capacity());
        Some(value)
    }
    /// Notify the table that a weak referent was cleared by GC.
    pub fn gc_cleared(&mut self, key: Option<Word>, value: Option<Word>) {
        let remove = self
            .slots
            .iter()
            .flatten()
            .find(|entry| match self.weakness {
                Weakness::None => false,
                Weakness::Key => key == Some(entry.key),
                Weakness::Value => value == Some(entry.value),
                Weakness::KeyAndValue => key == Some(entry.key) && value == Some(entry.value),
                Weakness::KeyOrValue => key == Some(entry.key) || value == Some(entry.value),
            })
            .map(|entry| entry.key);
        if let Some(key) = remove {
            let _ = self.remove(key);
        }
    }
    /// Rehash entries after an object address changes.
    pub fn rehash_after_gc(&mut self) {
        self.rehash(self.capacity());
    }
    fn insert_hashed(&mut self, key: Word, value: Word, hash: u64) {
        let mut index = usize::try_from(hash).unwrap_or(0) & (self.capacity() - 1);
        loop {
            match self.slots[index] {
                Some(entry) if entry.live && equal(self.test, entry.key, key) => {
                    self.slots[index] = Some(Entry {
                        key,
                        value,
                        hash,
                        live: true,
                    });
                    return;
                }
                None => {
                    self.slots[index] = Some(Entry {
                        key,
                        value,
                        hash,
                        live: true,
                    });
                    self.len += 1;
                    return;
                }
                _ => {
                    index = (index + 1) & (self.capacity() - 1);
                }
            }
        }
    }
    fn find(&self, key: Word, hash: u64) -> Option<Entry> {
        self.find_index(key, hash)
            .and_then(|index| self.slots[index])
    }
    fn find_index(&self, key: Word, hash: u64) -> Option<usize> {
        let mut index = usize::try_from(hash).unwrap_or(0) & (self.capacity() - 1);
        for _ in 0..self.capacity() {
            match self.slots[index] {
                Some(entry)
                    if entry.live && entry.hash == hash && equal(self.test, entry.key, key) =>
                {
                    return Some(index);
                }
                None => return None,
                _ => index = (index + 1) & (self.capacity() - 1),
            }
        }
        None
    }
    fn rehash(&mut self, capacity: usize) {
        let old = std::mem::replace(
            &mut self.slots,
            vec![None; capacity.max(2).next_power_of_two()],
        );
        self.len = 0;
        for entry in old.into_iter().flatten() {
            self.insert_hashed(entry.key, entry.value, entry.hash);
        }
    }
}

fn equal(test: HashTest, left: Word, right: Word) -> bool {
    match test {
        HashTest::Eq | HashTest::Eql | HashTest::Equal | HashTest::Equalp => left == right,
    }
}

/// Compute a stable hash for immediate values and object identities.
#[must_use]
pub const fn sxhash(word: Word) -> u64 {
    let mut x = word.bits();
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inserts_and_rehashes() {
        let mut table = HashTable::new(HashTest::Eq, Weakness::None, 2);
        for value in 0..32 {
            table.insert(Word::fixnum(value), Word::fixnum(value + 1));
        }
        assert_eq!(table.len(), 32);
        assert!(table.capacity() >= 64);
        assert_eq!(table.get(Word::fixnum(7)), Some(Word::fixnum(8)));
        assert_eq!(table.remove(Word::fixnum(7)), Some(Word::fixnum(8)));
    }
    #[test]
    fn weakness_notification_removes_entry() {
        let mut table = HashTable::new(HashTest::Eq, Weakness::Key, 8);
        table.insert(Word::fixnum(1), Word::fixnum(2));
        table.gc_cleared(Some(Word::fixnum(1)), None);
        assert_eq!(table.len(), 0);
    }
}
