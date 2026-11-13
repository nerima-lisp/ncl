use super::{HeapWords, PageKind, State};
use crate::Thread;
use crate::heap::{LOGICAL_PAGE_SIZE, PageError};
use std::ptr::NonNull;

impl State {
    pub(crate) fn flush_pending_allocations(&mut self) -> Result<(), PageError> {
        let threads = self.threads.clone();
        for thread_pointer in threads {
            // SAFETY: registered Thread pointers remain valid until unregistered; each
            // append-only allocation log publishes complete records atomically.
            let published = unsafe { (&*thread_pointer).pending_allocation_count() };
            self.reconcile_thread_allocations(thread_pointer, published)?;
        }
        Ok(())
    }

    pub(crate) fn flush_pending_allocation_for(
        &mut self,
        address: usize,
    ) -> Result<bool, PageError> {
        for thread_index in 0..self.threads.len() {
            let Some(thread_pointer) = self.threads.get(thread_index).copied() else {
                continue;
            };
            // SAFETY: registered Thread pointers remain valid until unregistered.
            let thread = unsafe { &*thread_pointer };
            let published = thread.pending_allocation_count();
            let consumed = thread.reconciled_allocation_count().min(published);
            for record_index in consumed..published {
                let Some((pending_address, _, _)) = thread.pending_allocation(record_index) else {
                    return Err(PageError::InvalidAllocationRecord);
                };
                if pending_address == address {
                    self.reconcile_thread_allocations(thread_pointer, record_index + 1)?;
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub(crate) fn flush_thread_allocations(
        &mut self,
        thread_pointer: *mut Thread,
    ) -> Result<(), PageError> {
        if !self.threads.contains(&thread_pointer) {
            return Ok(());
        }
        // SAFETY: the caller still owns a registered Thread while holding the heap state lock.
        let published = unsafe { (&*thread_pointer).pending_allocation_count() };
        self.reconcile_thread_allocations(thread_pointer, published)
    }

    fn reconcile_thread_allocations(
        &mut self,
        thread_pointer: *mut Thread,
        limit: usize,
    ) -> Result<(), PageError> {
        // SAFETY: callers pass a registered thread whose lifetime spans the state lock.
        let thread = unsafe { &*thread_pointer };
        let published = thread.pending_allocation_count();
        let consumed = thread.reconciled_allocation_count().min(published);
        for record_index in consumed..limit.min(published) {
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
            // SAFETY: the mutator committed the page before publishing this checked record.
            let pointer = NonNull::new(unsafe { page_pointer.as_ptr().add(offset / word_size) })
                .ok_or(PageError::PageOutOfBounds)?;
            // SAFETY: the view covers initialized object words in the committed page.
            let managed = unsafe { HeapWords::new(pointer, words) };
            let index = self.objects.len();
            let page_metadata = self.pages.get_mut(page).ok_or(PageError::PageOutOfBounds)?;
            page_metadata.add_object_start(offset, index);
            page_metadata.used_bytes = page_metadata.used_bytes.max(end);
            let generation = page_metadata.generation;
            self.used = self.used.saturating_add(bytes);
            let widetag = if kind == PageKind::HeaderObject {
                u8::try_from(managed.first().copied().unwrap_or_default() & 0xff)
                    .unwrap_or_default()
            } else {
                0
            };
            self.objects.push(super::Object {
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
        thread.mark_allocations_reconciled(limit.min(published));
        Ok(())
    }
}
