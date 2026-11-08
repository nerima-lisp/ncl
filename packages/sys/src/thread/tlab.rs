use crate::{PageKind, Thread};
use core::sync::atomic::{AtomicUsize, Ordering};

const TLAB_ALLOCATION_RECORD_CAPACITY: usize = 4096;

#[derive(Debug)]
pub struct TlabAllocationRecord {
    address: AtomicUsize,
    descriptor: AtomicUsize,
}

impl TlabAllocationRecord {
    const fn new() -> Self {
        Self {
            address: AtomicUsize::new(0),
            descriptor: AtomicUsize::new(0),
        }
    }

    fn store(&self, address: usize, words: usize, kind: PageKind) -> bool {
        if words == 0 {
            return false;
        }
        let kind_bits = match kind {
            PageKind::Cons => 0,
            PageKind::HeaderObject => 1,
            PageKind::Large | PageKind::Code | PageKind::Static => return false,
        };
        let Some(word_bits) = words.checked_mul(2) else {
            return false;
        };
        let Some(descriptor) = word_bits.checked_add(kind_bits) else {
            return false;
        };
        if descriptor == 0 {
            return false;
        }
        self.address.store(address, Ordering::Relaxed);
        self.descriptor.store(descriptor, Ordering::Relaxed);
        true
    }

    fn load(&self) -> Option<(usize, usize, PageKind)> {
        let descriptor = self.descriptor.load(Ordering::Acquire);
        if descriptor == 0 {
            return None;
        }
        let kind = if descriptor & 1 == 0 {
            PageKind::Cons
        } else {
            PageKind::HeaderObject
        };
        let words = descriptor >> 1;
        if words == 0 {
            return None;
        }
        let address = self.address.load(Ordering::Relaxed);
        Some((address, words, kind))
    }
}

impl Thread {
    pub(crate) fn initialize_allocation_log(&mut self) -> bool {
        if self.pending_allocations.len() == TLAB_ALLOCATION_RECORD_CAPACITY {
            return true;
        }
        if self
            .pending_allocations
            .try_reserve_exact(TLAB_ALLOCATION_RECORD_CAPACITY)
            .is_err()
        {
            return false;
        }
        self.pending_allocations
            .resize_with(TLAB_ALLOCATION_RECORD_CAPACITY, TlabAllocationRecord::new);
        true
    }

    pub(crate) fn record_tlab_allocation(
        &self,
        address: usize,
        words: usize,
        kind: PageKind,
    ) -> bool {
        let index = self.pending_allocation_count.load(Ordering::Relaxed);
        let Some(record) = self.pending_allocations.get(index) else {
            return false;
        };
        if !record.store(address, words, kind) {
            return false;
        }
        self.pending_allocation_count
            .store(index + 1, Ordering::Release);
        true
    }

    pub(crate) fn pending_allocation_count(&self) -> usize {
        self.pending_allocation_count.load(Ordering::Acquire)
    }

    pub(crate) fn reconciled_allocation_count(&self) -> usize {
        self.reconciled_allocation_count.load(Ordering::Relaxed)
    }

    pub(crate) fn pending_allocation(&self, index: usize) -> Option<(usize, usize, PageKind)> {
        self.pending_allocations.get(index)?.load()
    }

    pub(crate) fn mark_allocations_reconciled(&self, count: usize) {
        self.reconciled_allocation_count
            .store(count, Ordering::Release);
    }

    pub(crate) fn reset_allocation_log(&self) {
        self.reconciled_allocation_count.store(0, Ordering::Relaxed);
        self.pending_allocation_count.store(0, Ordering::Release);
    }
}
