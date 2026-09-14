use crate::{Thread, Word};
use std::collections::HashMap;
use std::sync::Mutex;

/// Allocation failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageCondition {
    CapacityExceeded,
    InvalidSize,
    ThreadNotRegistered,
}
/// Physical page category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageKind {
    Cons,
    HeaderObject,
    Large,
    Code,
    Static,
}
/// Registered object type metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeTag {
    pub widetag: u8,
}
/// Ranges of payload words containing references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReferenceLayout {
    pub reference_words: Vec<usize>,
}
#[derive(Debug)]
pub struct LayoutError;
#[derive(Clone, Copy, Debug)]
pub struct HeapConfig {
    pub dynamic_space_size: usize,
    pub bytes_considered_between_gcs: usize,
}
#[derive(Debug)]
struct State {
    used: usize,
    pages: Vec<Box<[u64]>>,
    layouts: HashMap<u8, ReferenceLayout>,
    threads: Vec<*const Thread>,
}
/// Shared heap and allocation policy.
#[derive(Debug)]
pub struct Heap {
    config: HeapConfig,
    state: Mutex<State>,
}

impl Default for HeapConfig {
    fn default() -> Self {
        Self {
            dynamic_space_size: 64 * 1024 * 1024,
            bytes_considered_between_gcs: 4 * 1024 * 1024,
        }
    }
}
impl Heap {
    /// Reserve a heap with a hard byte limit.
    pub fn new(config: HeapConfig) -> Self {
        Self {
            config,
            state: Mutex::new(State {
                used: 0,
                pages: Vec::new(),
                layouts: HashMap::new(),
                threads: Vec::new(),
            }),
        }
    }
    /// Return the configured dynamic-space limit.
    pub const fn dynamic_space_size(&self) -> usize {
        self.config.dynamic_space_size
    }
    /// Return the GC byte threshold.
    pub const fn bytes_considered_between_gcs(&self) -> usize {
        self.config.bytes_considered_between_gcs
    }
    /// Register a layout once per widetag.
    pub fn register_layout(&self, widetag: u8, layout: ReferenceLayout) -> Result<(), LayoutError> {
        let mut state = self.lock_state();
        if state.layouts.insert(widetag, layout).is_some() {
            return Err(LayoutError);
        }
        Ok(())
    }
    pub(crate) fn register_thread(&self, thread: &mut Thread) -> Result<(), StorageCondition> {
        let mut state = self.lock_state();
        let ptr = std::ptr::from_mut(thread).cast_const();
        if state.threads.contains(&ptr) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        state.threads.push(ptr);
        thread.heap = Some(self);
        Ok(())
    }
    pub(crate) fn unregister_thread(&self, thread: &Thread) {
        let mut state = self.lock_state();
        let ptr = std::ptr::from_ref(thread);
        state.threads.retain(|item| *item != ptr);
    }
    pub(crate) fn alloc(
        &self,
        thread: &mut Thread,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        self.allocate(thread, PageKind::HeaderObject, tag, words + 1)
    }
    pub(crate) fn alloc_large(
        &self,
        thread: &mut Thread,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        self.allocate(thread, PageKind::Large, tag, words + 1)
    }
    pub(crate) fn alloc_cons(
        &self,
        thread: &mut Thread,
        car: Word,
        cdr: Word,
    ) -> Result<Word, StorageCondition> {
        let word = self.allocate(thread, PageKind::Cons, TypeTag { widetag: 0 }, 2)?;
        let address = word.address() as *mut u64;
        unsafe {
            /* SAFETY: allocate returned an aligned live two-word page slot. */
            address.write(car.bits());
            address.add(1).write(cdr.bits());
        }
        Ok(word)
    }
    fn allocate(
        &self,
        thread: &mut Thread,
        kind: PageKind,
        tag: TypeTag,
        words: usize,
    ) -> Result<Word, StorageCondition> {
        if words == 0 || words > (1 << 20) {
            return Err(StorageCondition::InvalidSize);
        }
        if thread.heap != Some(self) {
            return Err(StorageCondition::ThreadNotRegistered);
        }
        let bytes = words.checked_mul(8).ok_or(StorageCondition::InvalidSize)?;
        let mut state = self.lock_state();
        if state
            .used
            .checked_add(bytes)
            .is_none_or(|n| n > self.config.dynamic_space_size)
        {
            return Err(StorageCondition::CapacityExceeded);
        }
        let mut page = vec![0_u64; words].into_boxed_slice();
        let address = page.as_mut_ptr() as usize;
        if kind != PageKind::Cons {
            page[0] = u64::from(tag.widetag);
        }
        state.used += bytes;
        state.pages.push(page);
        Ok(Word::pointer(
            address,
            if kind == PageKind::Cons {
                crate::LowTag::List
            } else {
                crate::LowTag::OtherPointer
            },
        ))
    }
    fn lock_state(&self) -> std::sync::MutexGuard<'_, State> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

unsafe impl Send for State {}
unsafe impl Sync for State {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tags_round_trip() {
        let value = Word::fixnum(-42);
        assert_eq!(value.as_fixnum(), Some(-42));
        assert!(Word::NIL.is_list());
        assert!(Word::NIL.is_list());
    }
    #[test]
    fn allocation_and_limit() {
        let heap = Heap::new(HeapConfig {
            dynamic_space_size: 8,
            ..HeapConfig::default()
        });
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        assert!(heap.alloc_cons(&mut thread, Word::NIL, Word::NIL).is_err());
    }
    #[test]
    fn duplicate_layout_rejected() {
        let heap = Heap::new(HeapConfig::default());
        let layout = ReferenceLayout {
            reference_words: vec![0],
        };
        assert!(heap.register_layout(9, layout.clone()).is_ok());
        assert!(heap.register_layout(9, layout).is_err());
    }
    #[test]
    fn roots_are_lifo() {
        let mut thread = Thread::new();
        let mut value = Word::NIL;
        let token = thread.push_root(&mut value);
        assert!(thread.pop_root(token));
    }
}
