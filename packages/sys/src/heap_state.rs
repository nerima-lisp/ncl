use crate::heap::{LOGICAL_PAGE_SIZE, PageError, PageRange};
use crate::{CodeRegistry, Finalizer, PageKind, ReferenceLayout, Thread, Weakness, Word};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

#[path = "heap_state/allocations.rs"]
mod allocations;
#[path = "heap_state/object_index.rs"]
mod object_index;
use object_index::ObjectStartBitmap;

pub const MIN_OBJECTS_BEFORE_COMPACTION: usize = 1_024;

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
    object_start_bitmap: Option<Box<ObjectStartBitmap>>,
    pub(crate) incoming_forwarders: Vec<usize>,
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
        self.clear_object_starts();
        self.incoming_forwarders.clear();
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
        let has_forwarders = self.pages.get(page).is_some_and(|metadata| {
            metadata.object_starts.iter().any(|(_, index)| {
                self.objects
                    .get(*index)
                    .is_some_and(|object| Self::is_forwarder_on_page(object, base, end))
            })
        });
        if !has_forwarders {
            if let Some(metadata) = self.pages.get_mut(page) {
                metadata.incoming_forwarders.clear();
            }
            return;
        }
        let incoming_count = self
            .pages
            .get(page)
            .map_or(0, |metadata| metadata.incoming_forwarders.len());
        for position in 0..incoming_count {
            let Some(source_index) = self
                .pages
                .get(page)
                .and_then(|metadata| metadata.incoming_forwarders.get(position))
                .copied()
            else {
                continue;
            };
            let Some(target_index) = self
                .objects
                .get(source_index)
                .and_then(|source| source.forwarded_to)
            else {
                continue;
            };
            if !self
                .objects
                .get(target_index)
                .is_some_and(|target| Self::is_forwarder_on_page(target, base, end))
                || self
                    .objects
                    .get(source_index)
                    .is_some_and(|source| Self::is_forwarder_on_page(source, base, end))
            {
                continue;
            }

            let target = self.forwarding_target_after_page(target_index, base, end);
            let target_page = target.and_then(|target_index| {
                self.objects
                    .get(target_index)
                    .and_then(|object| self.page_index(object.start))
                    .filter(|target_page| *target_page != page)
            });
            let target = target_page.and(target);
            if let Some(source) = self.objects.get_mut(source_index) {
                source.forwarded_to = target;
                if target.is_none() {
                    source.words.retire();
                }
            }
            if let (Some(target_page), Some(_)) = (target_page, target)
                && let Some(metadata) = self.pages.get_mut(target_page)
            {
                metadata.incoming_forwarders.push(source_index);
            }
        }

        let object_count = self
            .pages
            .get(page)
            .map_or(0, |metadata| metadata.object_starts.len());
        for position in 0..object_count {
            let Some(index) = self
                .pages
                .get(page)
                .and_then(|metadata| metadata.object_starts.get(position))
                .map(|(_, index)| *index)
            else {
                continue;
            };
            if let Some(object) = self.objects.get_mut(index)
                && Self::is_forwarder_on_page(object, base, end)
            {
                object.forwarded_to = None;
                object.words.retire();
            }
        }
        if let Some(metadata) = self.pages.get_mut(page) {
            metadata.incoming_forwarders.clear();
        }
    }

    const fn is_forwarder_on_page(object: &Object, base: usize, end: usize) -> bool {
        !object.alive && object.forwarded_to.is_some() && object.start >= base && object.start < end
    }

    fn forwarding_target_after_page(
        &self,
        mut index: usize,
        base: usize,
        end: usize,
    ) -> Option<usize> {
        for _ in 0..self.objects.len() {
            let object = self.objects.get(index)?;
            if Self::is_forwarder_on_page(object, base, end) {
                index = object.forwarded_to?;
            } else {
                return (object.alive || object.forwarded_to.is_some()).then_some(index);
            }
        }
        None
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
}
