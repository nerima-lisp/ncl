use super::super::{Heap, MIN_HEAP_ADDRESS, Object, PageKind, State, StorageCondition, TypeTag};
use crate::heap::pages::LOGICAL_PAGE_SIZE;
use crate::heap_state::HeapWords;
use crate::{Thread, Word};
use std::sync::atomic::Ordering;

impl Heap {
    pub(super) fn allocate_large_once(
        &self,
        thread: &mut Thread,
        words: usize,
        bytes: usize,
        tag: TypeTag,
    ) -> Result<Word, StorageCondition> {
        let mut state = self.lock_state();
        if state.pending_page_error.is_some() {
            return Err(StorageCondition::PageOperationFailed);
        }
        let (start_page, end_page) = self.commit_large_extent(&mut state, bytes)?;
        let index = state.objects.len();
        let metadata = state
            .pages
            .get_mut(start_page..end_page)
            .ok_or(StorageCondition::CapacityExceeded)?;
        for (page_offset, metadata) in metadata.iter_mut().enumerate() {
            metadata.reset();
            metadata.kind = Some(PageKind::Large);
            metadata.generation = 2;
            metadata.extent_owner = Some(index);
            metadata.used_bytes = bytes
                .saturating_sub(page_offset * LOGICAL_PAGE_SIZE)
                .min(LOGICAL_PAGE_SIZE);
        }
        let range = state
            .page_range
            .as_ref()
            .ok_or(StorageCondition::CapacityExceeded)?;
        let pointer = range
            .page_pointer(start_page)
            .ok_or(StorageCondition::CapacityExceeded)?;
        let word_pointer = pointer.cast::<u64>();
        // SAFETY: all pages in the checked extent are committed and cover `bytes`.
        unsafe { core::ptr::write_bytes(word_pointer.as_ptr(), 0, words) };
        // SAFETY: the pointer covers the complete initialized large object.
        let mut managed = unsafe { HeapWords::new(word_pointer, words) };
        let Some(header) = managed.get_mut(0) else {
            return Err(StorageCondition::InvalidSize);
        };
        *header = u64::from(tag.widetag);
        let address = word_pointer.as_ptr().addr();
        state.used = state.used.saturating_add(bytes);
        let Some(first_page) = state.pages.get_mut(start_page) else {
            return Err(StorageCondition::CapacityExceeded);
        };
        first_page.object_starts.push((0, index));
        state.objects.push(Object {
            words: managed,
            start: address,
            kind: PageKind::Large,
            generation: 2,
            survived: 0,
            pinned: false,
            weak: None,
            finalizer: None,
            alive: true,
            forwarded_to: None,
        });
        drop(state);
        thread.bytes_cons = thread.bytes_cons.saturating_add(bytes);
        Ok(Word::pointer(address, crate::LowTag::OtherPointer))
    }

    fn commit_large_extent(
        &self,
        state: &mut State,
        bytes: usize,
    ) -> Result<(usize, usize), StorageCondition> {
        if state.page_range.is_none() {
            let _ = state.page_error;
            return Err(StorageCondition::CapacityExceeded);
        }
        if state
            .page_range
            .as_ref()
            .is_some_and(|range| range.base_address() < MIN_HEAP_ADDRESS)
        {
            return Err(StorageCondition::HeapAddressTooLow);
        }
        let page_count = bytes.div_ceil(LOGICAL_PAGE_SIZE);
        let Some(start_page) = find_free_page_run(&state.pages, page_count) else {
            return Err(StorageCondition::CapacityExceeded);
        };
        let end_page = start_page
            .checked_add(page_count)
            .ok_or(StorageCondition::CapacityExceeded)?;
        if state.pages.get(start_page..end_page).is_none() {
            return Err(StorageCondition::CapacityExceeded);
        }
        if self
            .available_bytes
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |available| {
                if available >= bytes {
                    Some(available - bytes)
                } else {
                    None
                }
            })
            .is_err()
        {
            return Err(StorageCondition::CapacityExceeded);
        }
        for page in start_page..end_page {
            state.retire_page_forwardings(page);
            let result = state
                .page_range
                .as_mut()
                .ok_or(StorageCondition::CapacityExceeded)?
                .commit_page(page)
                .and_then(|()| {
                    state
                        .page_range
                        .as_mut()
                        .ok_or(crate::heap::pages::PageError::PageOutOfBounds)?
                        .clear_page(page)
                });
            if result.is_err() {
                self.available_bytes.fetch_add(bytes, Ordering::Release);
                return Err(StorageCondition::CapacityExceeded);
            }
        }
        Ok((start_page, end_page))
    }
}

fn find_free_page_run(pages: &[super::super::PageMetadata], count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    pages.windows(count).position(|run| {
        run.iter().all(|page| {
            page.kind.is_none()
                && page.owner.is_none()
                && page.extent_owner.is_none()
                && !page.pinned
        })
    })
}
