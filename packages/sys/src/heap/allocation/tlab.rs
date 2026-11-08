use super::super::{Heap, MIN_HEAP_ADDRESS, PageKind, StorageCondition};
use crate::heap::pages::LOGICAL_PAGE_SIZE;
use crate::{Thread, Word};
use std::sync::atomic::Ordering;

pub(super) enum TlabAttempt {
    Allocated(Word),
    NeedsRefill,
    LogFull,
}

impl Heap {
    pub(super) fn allocate_fast(
        &self,
        thread: &mut Thread,
        kind: PageKind,
        tag: super::super::TypeTag,
        words: usize,
        under_slow_path_gate: bool,
    ) -> Result<TlabAttempt, StorageCondition> {
        let bytes = words.checked_mul(8).ok_or(StorageCondition::InvalidSize)?;
        if kind == PageKind::Large || bytes >= super::super::LARGE_OBJECT {
            return Ok(TlabAttempt::NeedsRefill);
        }
        let native_snapshot_is_safe = under_slow_path_gate
            && thread.native_state() == crate::NativeState::Native
            && thread.state == crate::SafepointState::Safe;
        if thread.safepoint_request != 0
            || (thread.state != crate::SafepointState::Running && !native_snapshot_is_safe)
            || thread.pending_allocation_count() >= thread.pending_allocations.len()
        {
            return Ok(
                if thread.pending_allocation_count() >= thread.pending_allocations.len() {
                    TlabAttempt::LogFull
                } else {
                    TlabAttempt::NeedsRefill
                },
            );
        }
        let (page, base, bump, limit) = if kind == PageKind::Cons {
            (
                thread.cons_tlab_page,
                thread.cons_tlab_start,
                thread.cons_tlab_bump,
                thread.cons_tlab_limit,
            )
        } else {
            (
                thread.header_tlab_page,
                thread.header_tlab_start,
                thread.tlab_bump,
                thread.tlab_limit,
            )
        };
        let (Some(_page), Some(base)) = (page, base) else {
            return Ok(TlabAttempt::NeedsRefill);
        };
        let Some(end) = bump.checked_add(bytes).filter(|end| *end <= limit) else {
            return Ok(TlabAttempt::NeedsRefill);
        };
        let Some(offset) = bump.checked_sub(base.as_ptr().addr()) else {
            return Ok(TlabAttempt::NeedsRefill);
        };
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
        // SAFETY: the page base is u64-aligned, allocation offsets are word multiples,
        // and the checked bump extent lies below the committed page limit.
        let pointer = unsafe {
            base.cast::<u64>()
                .as_ptr()
                .add(offset / core::mem::size_of::<u64>())
        };
        // SAFETY: the bump range is writable and contains exactly `words` words.
        unsafe { core::ptr::write_bytes(pointer, 0, words) };
        if kind != PageKind::Cons {
            // SAFETY: every allocated object has at least one word.
            unsafe { pointer.write(u64::from(tag.widetag)) };
        }
        let address = pointer.addr();
        if !thread.record_tlab_allocation(address, words, kind) {
            self.available_bytes.fetch_add(bytes, Ordering::Release);
            return Err(StorageCondition::InvalidSize);
        }
        if kind == PageKind::Cons {
            thread.cons_tlab_bump = end;
        } else {
            thread.tlab_bump = end;
        }
        thread.bytes_cons = thread.bytes_cons.saturating_add(bytes);
        Ok(TlabAttempt::Allocated(Word::pointer(
            address,
            if kind == PageKind::Cons {
                crate::LowTag::List
            } else {
                crate::LowTag::OtherPointer
            },
        )))
    }

    pub(super) fn refill_tlab(
        &self,
        thread: &mut Thread,
        kind: PageKind,
    ) -> Result<(), StorageCondition> {
        let mut state = self.lock_state();
        if state.pending_page_error.is_some() {
            return Err(StorageCondition::PageOperationFailed);
        }
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
        let owner = std::ptr::from_mut(thread).addr();
        let (old_page, bump_ref, limit_ref, base_ref) = if kind == PageKind::Cons {
            (
                thread.cons_tlab_page,
                &mut thread.cons_tlab_bump,
                &mut thread.cons_tlab_limit,
                &mut thread.cons_tlab_start,
            )
        } else {
            (
                thread.header_tlab_page,
                &mut thread.tlab_bump,
                &mut thread.tlab_limit,
                &mut thread.header_tlab_start,
            )
        };
        if let Some(page) = old_page
            && let Some(metadata) = state.pages.get_mut(page)
            && metadata.owner == Some(owner)
        {
            metadata.owner = None;
        }
        *bump_ref = 0;
        *limit_ref = 0;
        *base_ref = None;
        if kind == PageKind::Cons {
            thread.cons_tlab_page = None;
        } else {
            thread.header_tlab_page = None;
        }
        let Some(page) = state.pages.iter().position(|metadata| {
            metadata.kind.is_none()
                && metadata.owner.is_none()
                && metadata.extent_owner.is_none()
                && !metadata.pinned
        }) else {
            return Err(StorageCondition::CapacityExceeded);
        };
        state.retire_page_forwardings(page);
        let range = state
            .page_range
            .as_mut()
            .ok_or(StorageCondition::CapacityExceeded)?;
        range
            .commit_page(page)
            .and_then(|()| range.clear_page(page))
            .map_err(|_| StorageCondition::CapacityExceeded)?;
        let base = range
            .page_address(page)
            .ok_or(StorageCondition::CapacityExceeded)?;
        let limit = base
            .checked_add(LOGICAL_PAGE_SIZE)
            .ok_or(StorageCondition::CapacityExceeded)?;
        let pointer = range
            .page_pointer(page)
            .ok_or(StorageCondition::CapacityExceeded)?;
        let metadata = state
            .pages
            .get_mut(page)
            .ok_or(StorageCondition::CapacityExceeded)?;
        metadata.reset();
        metadata.kind = Some(kind);
        metadata.owner = Some(owner);
        drop(state);
        if kind == PageKind::Cons {
            thread.cons_tlab_page = Some(page);
            thread.cons_tlab_start = Some(pointer);
            thread.cons_tlab_bump = base;
            thread.cons_tlab_limit = limit;
        } else {
            thread.header_tlab_page = Some(page);
            thread.header_tlab_start = Some(pointer);
            thread.tlab_bump = base;
            thread.tlab_limit = limit;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Heap, PageKind, Thread, TlabAttempt};
    use crate::{HeapConfig, TypeTag};

    #[test]
    fn pending_poll_request_blocks_native_safe_tlab_allocation() {
        let heap = Heap::new(HeapConfig::default());
        let mut thread = Thread::new();
        assert_eq!(heap.register_thread(&mut thread), Ok(()));
        assert_eq!(
            heap.refill_tlab(&mut thread, PageKind::HeaderObject),
            Ok(())
        );
        thread.enter_native();
        thread.request_poll();
        // Keep the published native snapshot safe to exercise the gated exception itself.
        thread.state = crate::SafepointState::Safe;
        let bump_before = thread.tlab_bump;
        let records_before = thread.pending_allocation_count();

        let result = heap.allocate_fast(
            &mut thread,
            PageKind::HeaderObject,
            TypeTag { widetag: 8 },
            1,
            true,
        );

        assert!(matches!(result, Ok(TlabAttempt::NeedsRefill)));
        assert_eq!(thread.tlab_bump, bump_before);
        assert_eq!(thread.pending_allocation_count(), records_before);
        heap.unregister_thread(&thread);
    }
}
