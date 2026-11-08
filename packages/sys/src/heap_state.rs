use crate::heap::{LOGICAL_PAGE_SIZE, PageError, PageRange};
use crate::{CodeRegistry, Finalizer, PageKind, ReferenceLayout, Thread, Weakness, Word};
use std::collections::{HashMap, HashSet};
use std::ops::{Deref, DerefMut};
use std::ptr::NonNull;

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
        for object in &mut self.objects {
            if !object.alive
                && object.forwarded_to.is_some()
                && object.start >= base
                && object.start < end
            {
                object.forwarded_to = None;
                object.words.retire();
            }
        }
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
                self.objects.push(Object {
                    words: managed,
                    start: address,
                    kind,
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
