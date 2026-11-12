use crate::heap::{LOGICAL_PAGE_SIZE, PageError, PageRange};
use crate::{CodeRegistry, Finalizer, PageKind, ReferenceLayout, Thread, Weakness, Word};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

pub(crate) const MIN_OBJECTS_BEFORE_COMPACTION: usize = 1_024;

#[derive(Debug)]
pub struct HeapWords {
    pointer: NonNull<u64>,
    len: usize,
}

// SAFETY: HeapWords is only stored in State, which serializes its access through
// Heap::state. Its PageRange outlives every nonempty view, and HeapWords remains
// non-Sync so a slice cannot be shared across threads without the State lock.
unsafe impl Send for HeapWords {}

impl HeapWords {
    /// Construct a view over words in a committed page range.
    ///
    /// # Safety
    /// `pointer` must point to `len` initialized, writable words in a committed
    /// `PageRange` owned by the State that will contain this view. That range must
    /// remain committed while `len` is nonzero.
    pub(crate) const unsafe fn new(pointer: NonNull<u64>, len: usize) -> Self {
        Self { pointer, len }
    }

    pub(crate) const fn as_ptr(&self) -> *const u64 {
        self.pointer.as_ptr()
    }

    pub(crate) const fn retire(&mut self) {
        self.pointer = NonNull::dangling();
        self.len = 0;
    }
}

impl Deref for HeapWords {
    type Target = [u64];

    fn deref(&self) -> &Self::Target {
        // SAFETY: construction requires a live committed range for len words;
        // retire clears len before the owning page is decommitted.
        unsafe { std::slice::from_raw_parts(self.pointer.as_ptr(), self.len) }
    }
}

impl DerefMut for HeapWords {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: construction requires a live writable committed range for len
        // words, and mutable access is serialized by Heap::state.
        unsafe { std::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.len) }
    }
}

#[derive(Debug)]
pub struct Object {
    pub(crate) words: HeapWords,
    pub(crate) start: usize,
    pub(crate) kind: PageKind,
    pub(crate) widetag: u8,
    pub(crate) generation: u8,
    pub(crate) survived: u8,
    pub(crate) pinned: bool,
    pub(crate) weak: Option<Weakness>,
    pub(crate) finalizer: Option<(Finalizer, bool)>,
    pub(crate) alive: bool,
    pub(crate) forwarded_to: Option<usize>,
}

#[derive(Debug, Default)]
pub struct PageMetadata {
    pub(crate) kind: Option<PageKind>,
    pub(crate) generation: u8,
    pub(crate) pinned: bool,
    pub(crate) used_bytes: usize,
    pub(crate) owner: Option<usize>,
    pub(crate) extent_owner: Option<usize>,
    pub(crate) evacuation: bool,
    pub(crate) object_starts: Vec<(usize, usize)>,
}

impl PageMetadata {
    pub(crate) fn reset(&mut self) {
        self.kind = None;
        self.generation = 0;
        self.pinned = false;
        self.used_bytes = 0;
        self.owner = None;
        self.extent_owner = None;
        self.evacuation = false;
        self.object_starts.clear();
    }
}

#[derive(Debug)]
pub struct State {
    pub(crate) used: usize,
    pub(crate) gc_epoch: u64,
    pub(crate) objects: Vec<Object>,
    pub(crate) compact_at_object_count: usize,
    pub(crate) page_range: Option<PageRange>,
    pub(crate) page_error: Option<PageError>,
    pub(crate) pending_page_error: Option<PageError>,
    pub(crate) pages: Vec<PageMetadata>,
    pub(crate) layouts: HashMap<u8, ReferenceLayout>,
    pub(crate) threads: Vec<*mut Thread>,
    pub(crate) dirty_cards: HashSet<(usize, usize)>,
    pub(crate) roots: Vec<*mut Word>,
    pub(crate) root_generations: Vec<u128>,
    pub(crate) next_root_generation: u128,
    pub(crate) finalizers: Vec<(Word, Finalizer)>,
    pub(crate) after_gc_hooks: Vec<fn()>,
    pub(crate) code_registry: CodeRegistry,
}

impl State {
    pub(crate) fn compact_objects_if_needed(&mut self) -> Result<(), PageError> {
        if self.objects.len() < self.compact_at_object_count {
            return Ok(());
        }
        self.compact_objects()
    }

    pub(crate) fn retire_page_forwardings(&mut self, page: usize) {
        let Some(base) = self
            .page_range
            .as_ref()
            .and_then(|range| range.page_address(page))
        else {
            return;
        };
        let Some(end) = base.checked_add(LOGICAL_PAGE_SIZE) else {
            return;
        };
        for index in 0..self.objects.len() {
            let rewrite = {
                let Some(source) = self.objects.get(index) else {
                    continue;
                };
                if source.alive
                    || source.forwarded_to.is_none()
                    || !Self::is_forwarder_on_page(source, base, end)
                {
                    let mut next = source.forwarded_to;
                    let mut crosses_retiring_page = false;
                    let mut live_target = None;
                    for _ in 0..self.objects.len() {
                        let Some(next_index) = next else {
                            break;
                        };
                        let Some(target) = self.objects.get(next_index) else {
                            break;
                        };
                        crosses_retiring_page |= Self::is_forwarder_on_page(target, base, end);
                        if target.alive {
                            live_target = Some(next_index);
                            break;
                        }
                        next = target.forwarded_to;
                    }
                    crosses_retiring_page.then_some(live_target)
                } else {
                    None
                }
            };
            if let Some(target) = rewrite
                && let Some(source) = self.objects.get_mut(index)
            {
                source.forwarded_to = target;
                if target.is_none() {
                    source.words.retire();
                }
            }
        }
        for object in &mut self.objects {
            if Self::is_forwarder_on_page(object, base, end) {
                object.forwarded_to = None;
                object.words.retire();
            }
        }
    }

    const fn is_forwarder_on_page(object: &Object, base: usize, end: usize) -> bool {
        !object.alive && object.forwarded_to.is_some() && object.start >= base && object.start < end
    }

    pub(crate) fn flush_pending_allocations(&mut self) -> Result<(), PageError> {
        let threads = self.threads.clone();
        for thread_pointer in threads {
            // SAFETY: registered Thread pointers remain valid until unregistered; its
            // append-only allocation log publishes each complete record atomically.
            let thread = unsafe { &*thread_pointer };
            let published = thread.pending_allocation_count();
            let consumed = thread.reconciled_allocation_count().min(published);
            for record_index in consumed..published {
                let Some((address, words, kind)) = thread.pending_allocation(record_index) else {
                    return Err(PageError::InvalidAllocationRecord);
                };
                let page = self.page_index(address).ok_or(PageError::PageOutOfBounds)?;
                let range = self
                    .page_range
                    .as_ref()
                    .ok_or(PageError::InvalidAllocationRecord)?;
                let page_base = range.page_address(page).ok_or(PageError::PageOutOfBounds)?;
                let offset = address
                    .checked_sub(page_base)
                    .ok_or(PageError::InvalidAllocationRecord)?;
                let word_size = std::mem::size_of::<u64>();
                let bytes = words
                    .checked_mul(word_size)
                    .ok_or(PageError::AddressSpaceOverflow)?;
                let end = offset
                    .checked_add(bytes)
                    .filter(|end| *end <= LOGICAL_PAGE_SIZE)
                    .ok_or(PageError::InvalidAllocationRecord)?;
                if words == 0 || !offset.is_multiple_of(word_size) {
                    return Err(PageError::InvalidAllocationRecord);
                }
                let page_pointer = range
                    .page_words_pointer(page)
                    .ok_or(PageError::PageOutOfBounds)?;
                // SAFETY: the mutator committed this page before publishing the record;
                // the checked, word-aligned extent fits within its logical page.
                let pointer =
                    NonNull::new(unsafe { page_pointer.as_ptr().add(offset / word_size) })
                        .ok_or(PageError::PageOutOfBounds)?;
                // SAFETY: this view covers the initialized object words in the committed page.
                let managed = unsafe { HeapWords::new(pointer, words) };
                let index = self.objects.len();
                let page_metadata = self.pages.get_mut(page).ok_or(PageError::PageOutOfBounds)?;
                page_metadata.object_starts.push((offset, index));
                page_metadata.used_bytes = page_metadata.used_bytes.max(end);
                let generation = page_metadata.generation;
                self.used = self.used.saturating_add(bytes);
                let widetag = if kind == PageKind::HeaderObject {
                    managed.first().copied().unwrap_or_default() as u8
                } else {
                    0
                };
                self.objects.push(Object {
                    words: managed,
                    start: address,
                    kind,
                    widetag,
                    generation,
                    survived: 0,
                    pinned: false,
                    weak: None,
                    finalizer: None,
                    alive: true,
                    forwarded_to: None,
                });
                thread.mark_allocations_reconciled(record_index + 1);
            }
            thread.mark_allocations_reconciled(published);
        }
        Ok(())
    }

    pub(crate) fn compact_objects(&mut self) -> Result<(), PageError> {
        const DROPPED: usize = usize::MAX;
        const RETAINED: usize = usize::MAX - 1;

        let mut old_to_new = Vec::new();
        old_to_new
            .try_reserve_exact(self.objects.len())
            .map_err(|_| PageError::MetadataAllocationFailed)?;
        old_to_new.resize(self.objects.len(), DROPPED);
        for (index, object) in self.objects.iter().enumerate() {
            if object.alive || Self::live_forwarding_target(&self.objects, index).is_some() {
                let mapping = old_to_new
                    .get_mut(index)
                    .ok_or(PageError::PageOutOfBounds)?;
                *mapping = RETAINED;
            }
        }
        for page in &self.pages {
            if let Some(owner) = page.extent_owner {
                let mapping = old_to_new
                    .get_mut(owner)
                    .ok_or(PageError::PageOutOfBounds)?;
                *mapping = RETAINED;
            }
        }

        let mut compacted_count = 0_usize;
        for mapping in &mut old_to_new {
            if *mapping == RETAINED {
                *mapping = compacted_count;
                compacted_count = compacted_count
                    .checked_add(1)
                    .ok_or(PageError::AddressSpaceOverflow)?;
            }
        }

        let mut compacted = Vec::new();
        compacted
            .try_reserve_exact(compacted_count)
            .map_err(|_| PageError::MetadataAllocationFailed)?;
        let mut dirty_cards = HashSet::new();
        dirty_cards
            .try_reserve(self.dirty_cards.len())
            .map_err(|_| PageError::MetadataAllocationFailed)?;
        for (old_index, card) in self.dirty_cards.iter().copied() {
            if let Some(new_index) = old_to_new
                .get(old_index)
                .copied()
                .filter(|index| *index != DROPPED)
            {
                dirty_cards.insert((new_index, card));
            }
        }

        let old_objects = std::mem::take(&mut self.objects);
        for (new_index, mut object) in old_to_new.iter().copied().zip(old_objects) {
            if new_index == DROPPED {
                continue;
            }
            if let Some(old_target) = object.forwarded_to {
                object.forwarded_to = old_to_new
                    .get(old_target)
                    .copied()
                    .filter(|index| *index != DROPPED);
            }
            compacted.push(object);
        }
        self.objects = compacted;
        self.compact_at_object_count = self
            .objects
            .len()
            .saturating_mul(2)
            .max(MIN_OBJECTS_BEFORE_COMPACTION);
        self.dirty_cards = dirty_cards;
        for page in &mut self.pages {
            if let Some(old_owner) = page.extent_owner {
                page.extent_owner = old_to_new
                    .get(old_owner)
                    .copied()
                    .filter(|index| *index != DROPPED);
            }
        }
        self.rebuild_object_starts();
        Ok(())
    }

    fn live_forwarding_target(objects: &[Object], source: usize) -> Option<usize> {
        let mut index = objects.get(source)?.forwarded_to?;
        let mut remaining = objects.len();
        while remaining > 0 {
            let object = objects.get(index)?;
            if object.alive {
                return Some(index);
            }
            index = object.forwarded_to?;
            remaining -= 1;
        }
        None
    }

    pub(crate) fn page_index(&self, address: usize) -> Option<usize> {
        let range = self.page_range.as_ref()?;
        let base = range.base_address();
        let offset = address.checked_sub(base)?;
        let index = offset / LOGICAL_PAGE_SIZE;
        (index < self.pages.len()).then_some(index)
    }

    pub(crate) fn object_index_at(&self, address: usize) -> Option<usize> {
        let range = self.page_range.as_ref()?;
        let page = self.page_index(address)?;
        let page_base = range.page_address(page)?;
        let offset = address.checked_sub(page_base)?;
        let object_starts = &self.pages.get(page)?.object_starts;
        let position = object_starts
            .binary_search_by_key(&offset, |(start, _)| *start)
            .ok()?;
        object_starts.get(position).map(|(_, index)| *index)
    }

    pub(crate) fn rebuild_object_starts(&mut self) {
        for page in &mut self.pages {
            page.object_starts.clear();
        }
        let Some(range) = self.page_range.as_ref() else {
            return;
        };
        let base = range.base_address();
        let mut starts = Vec::new();
        for (index, object) in self.objects.iter().enumerate() {
            if !object.alive && object.forwarded_to.is_none() {
                continue;
            }
            let Some(offset) = object.start.checked_sub(base) else {
                continue;
            };
            let page = offset / LOGICAL_PAGE_SIZE;
            if page < self.pages.len() {
                starts.push((page, offset % LOGICAL_PAGE_SIZE, index));
            }
        }
        starts.sort_unstable_by_key(|(page, offset, _)| (*page, *offset));
        for (page, offset, index) in starts {
            if let Some(page) = self.pages.get_mut(page) {
                page.object_starts.push((offset, index));
            }
        }
    }
}
