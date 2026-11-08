use crate::os::declarations;
use core::ffi::c_int;
use core::ptr::NonNull;

pub const LOGICAL_PAGE_SIZE: usize = 32 * 1024;

const PROT_NONE: c_int = 0;
const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
const MAP_PRIVATE: c_int = 2;
#[cfg(target_os = "linux")]
const MAP_ANONYMOUS: c_int = 0x20;
#[cfg(target_os = "macos")]
const MAP_ANONYMOUS: c_int = 0x1000;
// Linux drops anonymous backing immediately while retaining the mapping.
#[cfg(target_os = "linux")]
const MADV_DONTNEED: c_int = 4;
// Darwin keeps the address range reserved while making backing reusable by the VM.
#[cfg(target_os = "macos")]
const MADV_FREE_REUSABLE: c_int = 7;
// Darwin requires the owner to reclaim a reusable range before accessing it again.
#[cfg(target_os = "macos")]
const MADV_FREE_REUSE: c_int = 8;

/// Errors returned by the reserved managed-heap address range.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PageError {
    ZeroCapacity,
    InvalidAlignment,
    AddressSpaceOverflow,
    MetadataAllocationFailed,
    InvalidAllocationRecord,
    MappingFailed,
    ProtectionFailed,
    AdviceFailed,
    PageOutOfBounds,
    GroupOutOfBounds,
    PageNotCommitted,
}

/// A reserved virtual range divided into 32 KiB logical GC pages.
#[derive(Debug)]
pub struct PageRange {
    base: NonNull<u8>,
    capacity: usize,
    pages_per_group: usize,
    committed_groups: Vec<bool>,
    #[cfg(target_os = "macos")]
    reusable_groups: Vec<bool>,
}

// SAFETY: PageRange uniquely owns its mmap reservation and its VM tracking metadata.
// Moving that ownership does not invalidate the mapping; mutation requires `&mut self`,
// and PageRange remains !Sync so the mapping is not accessed concurrently through sharing.
unsafe impl Send for PageRange {}

impl PageRange {
    /// Reserve a contiguous range whose size satisfies logical and host VM page alignment.
    pub(crate) fn reserve(capacity: usize) -> Result<Self, PageError> {
        if capacity == 0 {
            return Err(PageError::ZeroCapacity);
        }
        let max_capacity =
            usize::try_from(isize::MAX).map_err(|_| PageError::AddressSpaceOverflow)?;
        if capacity > max_capacity {
            return Err(PageError::AddressSpaceOverflow);
        }

        // SAFETY: getpagesize has no pointer arguments or thread-state preconditions.
        let host_page_size = unsafe { declarations::getpagesize() };
        let host_page_size = usize::try_from(host_page_size)
            .ok()
            .filter(|size| *size > 0)
            .ok_or(PageError::AddressSpaceOverflow)?;
        if !capacity.is_multiple_of(LOGICAL_PAGE_SIZE) || !capacity.is_multiple_of(host_page_size) {
            return Err(PageError::InvalidAlignment);
        }

        let group_size = checked_lcm(LOGICAL_PAGE_SIZE, host_page_size)
            .ok_or(PageError::AddressSpaceOverflow)?;
        if !capacity.is_multiple_of(group_size) {
            return Err(PageError::InvalidAlignment);
        }
        let group_count = capacity
            .checked_div(group_size)
            .ok_or(PageError::AddressSpaceOverflow)?;
        let pages_per_group = group_size
            .checked_div(LOGICAL_PAGE_SIZE)
            .ok_or(PageError::AddressSpaceOverflow)?;

        let mut committed_groups = Vec::new();
        committed_groups
            .try_reserve_exact(group_count)
            .map_err(|_| PageError::MetadataAllocationFailed)?;
        committed_groups.resize(group_count, false);
        #[cfg(target_os = "macos")]
        let reusable_groups = {
            let mut groups = Vec::new();
            groups
                .try_reserve_exact(group_count)
                .map_err(|_| PageError::MetadataAllocationFailed)?;
            groups.resize(group_count, false);
            groups
        };

        #[cfg(target_os = "macos")]
        let flags = MAP_PRIVATE | MAP_ANONYMOUS;
        #[cfg(target_os = "linux")]
        let flags = MAP_PRIVATE | MAP_ANONYMOUS;

        // SAFETY: null requests an OS-selected address; capacity is positive and aligned,
        // and the flags request a private anonymous mapping with no access permissions.
        let raw =
            unsafe { declarations::mmap(core::ptr::null_mut(), capacity, PROT_NONE, flags, -1, 0) };
        if raw.addr() == usize::MAX {
            return Err(PageError::MappingFailed);
        }
        if raw.is_null() {
            // SAFETY: a null result is not usable by Rust, but mmap may have mapped address zero.
            let _ = unsafe { declarations::munmap(raw, capacity) };
            return Err(PageError::MappingFailed);
        }

        let base = NonNull::new(raw.cast::<u8>()).ok_or(PageError::MappingFailed)?;
        Ok(Self {
            base,
            capacity,
            pages_per_group,
            committed_groups,
            #[cfg(target_os = "macos")]
            reusable_groups,
        })
    }

    /// Return the number of logical 32 KiB pages in the range.
    pub(crate) const fn page_count(&self) -> usize {
        self.capacity / LOGICAL_PAGE_SIZE
    }

    /// Return the number of logical pages grouped into one host VM commit unit.
    #[cfg(test)]
    pub(crate) const fn pages_per_group(&self) -> usize {
        self.pages_per_group
    }

    /// Return the number of host VM commit groups in the range.
    pub(crate) const fn group_count(&self) -> usize {
        self.committed_groups.len()
    }

    /// Return the commit-group index for a logical page, if it is in range.
    pub(crate) fn group_for_page(&self, page: usize) -> Option<usize> {
        (page < self.page_count()).then_some(page / self.pages_per_group)
    }

    /// Return the logical-page range covered by a commit group.
    pub(crate) fn pages_in_group(&self, group: usize) -> Option<core::ops::Range<usize>> {
        if group >= self.group_count() {
            return None;
        }
        let start = group.checked_mul(self.pages_per_group)?;
        let end = start.checked_add(self.pages_per_group)?;
        Some(start..end)
    }

    /// Return the start address of a logical page without dereferencing it.
    pub(crate) fn page_address(&self, page: usize) -> Option<usize> {
        if page >= self.page_count() {
            return None;
        }
        self.base
            .as_ptr()
            .addr()
            .checked_add(page.checked_mul(LOGICAL_PAGE_SIZE)?)
    }

    /// Return the base address of the reserved range.
    pub(crate) fn base_address(&self) -> usize {
        self.base.as_ptr().addr()
    }

    /// Return a pointer to a logical page without accessing it.
    pub(crate) fn page_pointer(&self, page: usize) -> Option<NonNull<u8>> {
        if page >= self.page_count() {
            return None;
        }
        let offset = page.checked_mul(LOGICAL_PAGE_SIZE)?;
        // SAFETY: the checked offset is inside the mapping returned by mmap.
        NonNull::new(unsafe { self.base.as_ptr().add(offset) })
    }

    /// Return a word-aligned pointer to a logical page without accessing it.
    pub(crate) fn page_words_pointer(&self, page: usize) -> Option<NonNull<u64>> {
        if page >= self.page_count() {
            return None;
        }
        let offset = page.checked_mul(LOGICAL_PAGE_SIZE)?;
        let address = self.base.as_ptr().addr().checked_add(offset)?;
        NonNull::new(core::ptr::with_exposed_provenance_mut::<u64>(address))
    }

    /// Whether the page's host VM group currently has read/write backing.
    #[cfg(test)]
    pub(crate) fn is_group_committed(&self, group: usize) -> Option<bool> {
        self.committed_groups.get(group).copied()
    }

    /// Commit the host VM group containing a logical page.
    pub(crate) fn commit_page(&mut self, page: usize) -> Result<(), PageError> {
        let group = self
            .group_for_page(page)
            .ok_or(PageError::PageOutOfBounds)?;
        let address = self.group_pointer(group)?;
        let group_bytes = self
            .pages_per_group
            .checked_mul(LOGICAL_PAGE_SIZE)
            .ok_or(PageError::AddressSpaceOverflow)?;

        #[cfg(target_os = "macos")]
        if self
            .reusable_groups
            .get(group)
            .copied()
            .ok_or(PageError::GroupOutOfBounds)?
        {
            // SAFETY: address and group_bytes identify an aligned range previously advised
            // MADV_FREE_REUSABLE while still owned by this mapping.
            let result = unsafe {
                declarations::madvise(address.as_ptr().cast(), group_bytes, MADV_FREE_REUSE)
            };
            if result != 0 {
                return Err(PageError::AdviceFailed);
            }
            *self
                .reusable_groups
                .get_mut(group)
                .ok_or(PageError::GroupOutOfBounds)? = false;
            if self
                .committed_groups
                .get(group)
                .copied()
                .ok_or(PageError::GroupOutOfBounds)?
            {
                // SAFETY: a failed decommit may have left this group accessible after it was
                // marked reusable; reclaim ownership before clearing the still-mapped pages.
                unsafe { core::ptr::write_bytes(address.as_ptr(), 0, group_bytes) };
                return Ok(());
            }
        }
        if self
            .committed_groups
            .get(group)
            .copied()
            .ok_or(PageError::GroupOutOfBounds)?
        {
            return Ok(());
        }

        // SAFETY: the checked range is inside this reserved mapping and aligned to host pages.
        let result = unsafe {
            declarations::mprotect(address.as_ptr().cast(), group_bytes, PROT_READ | PROT_WRITE)
        };
        if result != 0 {
            return Err(PageError::ProtectionFailed);
        }

        // SAFETY: mprotect made the entire group writable, and the checked range lies in it.
        unsafe { core::ptr::write_bytes(address.as_ptr(), 0, group_bytes) };
        *self
            .committed_groups
            .get_mut(group)
            .ok_or(PageError::GroupOutOfBounds)? = true;
        Ok(())
    }

    /// Zero a logical page that is already committed, for safe page reuse.
    pub(crate) fn clear_page(&self, page: usize) -> Result<(), PageError> {
        let group = self
            .group_for_page(page)
            .ok_or(PageError::PageOutOfBounds)?;
        if !self
            .committed_groups
            .get(group)
            .copied()
            .ok_or(PageError::GroupOutOfBounds)?
        {
            return Err(PageError::PageNotCommitted);
        }
        let address = self
            .page_pointer(page)
            .ok_or(PageError::AddressSpaceOverflow)?;
        // SAFETY: the containing group is read/write committed and the logical page is in range.
        unsafe { core::ptr::write_bytes(address.as_ptr(), 0, LOGICAL_PAGE_SIZE) };
        Ok(())
    }

    /// Decommit a group whose logical pages are all free while retaining its virtual addresses.
    pub(crate) fn decommit_group(&mut self, group: usize) -> Result<(), PageError> {
        let pages = self
            .pages_in_group(group)
            .ok_or(PageError::GroupOutOfBounds)?;
        if !self
            .committed_groups
            .get(group)
            .copied()
            .ok_or(PageError::GroupOutOfBounds)?
        {
            return Ok(());
        }
        let address = self.group_pointer(group)?;
        let group_bytes = pages
            .len()
            .checked_mul(LOGICAL_PAGE_SIZE)
            .ok_or(PageError::AddressSpaceOverflow)?;

        #[cfg(target_os = "linux")]
        let advice = MADV_DONTNEED;
        #[cfg(target_os = "macos")]
        let advice = MADV_FREE_REUSABLE;
        // SAFETY: address and group_bytes identify the writable, page-aligned part of this mapping.
        let result = unsafe { declarations::madvise(address.as_ptr().cast(), group_bytes, advice) };
        if result != 0 {
            return Err(PageError::AdviceFailed);
        }

        #[cfg(target_os = "macos")]
        {
            *self
                .reusable_groups
                .get_mut(group)
                .ok_or(PageError::GroupOutOfBounds)? = true;
        }

        // SAFETY: the checked group belongs to this live mapping and is host-page aligned.
        let result =
            unsafe { declarations::mprotect(address.as_ptr().cast(), group_bytes, PROT_NONE) };
        if result != 0 {
            #[cfg(target_os = "macos")]
            {
                // SAFETY: cancel MADV_FREE_REUSABLE while the mapping remains accessible.
                if unsafe {
                    declarations::madvise(address.as_ptr().cast(), group_bytes, MADV_FREE_REUSE)
                } == 0
                {
                    *self
                        .reusable_groups
                        .get_mut(group)
                        .ok_or(PageError::GroupOutOfBounds)? = false;
                }
            }
            return Err(PageError::ProtectionFailed);
        }

        *self
            .committed_groups
            .get_mut(group)
            .ok_or(PageError::GroupOutOfBounds)? = false;
        Ok(())
    }

    fn group_pointer(&self, group: usize) -> Result<NonNull<u8>, PageError> {
        if group >= self.group_count() {
            return Err(PageError::GroupOutOfBounds);
        }
        let offset = group
            .checked_mul(self.pages_per_group)
            .and_then(|start| start.checked_mul(LOGICAL_PAGE_SIZE))
            .ok_or(PageError::AddressSpaceOverflow)?;
        // SAFETY: group bounds and checked multiplication keep the pointer in the mapping.
        NonNull::new(unsafe { self.base.as_ptr().add(offset) })
            .ok_or(PageError::AddressSpaceOverflow)
    }
}

impl Drop for PageRange {
    fn drop(&mut self) {
        // SAFETY: base and capacity are the exact range returned by the successful mmap call.
        let _ = unsafe { declarations::munmap(self.base.as_ptr().cast(), self.capacity) };
    }
}

const fn checked_lcm(left: usize, right: usize) -> Option<usize> {
    let divisor = gcd(left, right);
    (left / divisor).checked_mul(right)
}

const fn gcd(mut left: usize, mut right: usize) -> usize {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[cfg(test)]
mod tests {
    use super::{LOGICAL_PAGE_SIZE, PageError, PageRange, checked_lcm};

    fn test_ok<T, E: core::fmt::Debug>(result: Result<T, E>, context: &str) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("{context}: {error:?}"),
        }
    }

    fn test_some<T>(value: Option<T>, context: &str) -> T {
        value.unwrap_or_else(|| panic!("{context}"))
    }

    fn two_group_capacity() -> usize {
        // SAFETY: getpagesize has no pointer arguments or thread-state preconditions.
        let host_page_size = test_ok(
            usize::try_from(unsafe { crate::os::declarations::getpagesize() }),
            "host page size must be nonnegative",
        );
        test_ok(
            checked_lcm(LOGICAL_PAGE_SIZE, host_page_size)
                .and_then(|group_size| group_size.checked_mul(2))
                .ok_or("test mapping size is not representable"),
            "test mapping size is representable",
        )
    }

    #[test]
    fn reserve_checks_nonzero_page_alignment_and_exposes_groups() {
        assert!(matches!(
            PageRange::reserve(0),
            Err(PageError::ZeroCapacity)
        ));
        assert!(matches!(
            PageRange::reserve(1),
            Err(PageError::InvalidAlignment)
        ));

        let range = test_ok(
            PageRange::reserve(two_group_capacity()),
            "aligned mapping reserves",
        );
        assert_eq!(range.group_count(), 2);
        assert_eq!(range.group_for_page(0), Some(0));
        assert_eq!(range.group_for_page(range.pages_per_group()), Some(1));
        assert_eq!(range.group_for_page(range.page_count()), None);
        assert_eq!(range.pages_in_group(2), None);
    }

    #[test]
    fn committing_and_clearing_pages_zeroes_memory() {
        let mut range = test_ok(
            PageRange::reserve(two_group_capacity()),
            "aligned mapping reserves",
        );
        let pointer = test_some(range.page_pointer(0), "first page exists");
        assert_eq!(range.is_group_committed(0), Some(false));
        test_ok(range.commit_page(0), "first page commits");
        assert_eq!(range.is_group_committed(0), Some(true));
        // SAFETY: commit_page made the full logical page readable and writable.
        unsafe { pointer.as_ptr().write(0xa5) };
        test_ok(range.clear_page(0), "committed page clears");
        // SAFETY: the page remains committed and the pointer still targets its first byte.
        assert_eq!(unsafe { pointer.as_ptr().read() }, 0);
    }

    #[test]
    fn decommit_and_recommit_preserves_addresses_and_returns_zeroed_pages() {
        let mut range = test_ok(
            PageRange::reserve(two_group_capacity()),
            "aligned mapping reserves",
        );
        let pointer = test_some(range.page_pointer(0), "first page exists");
        let address = pointer.as_ptr().addr();
        test_ok(range.commit_page(0), "first page commits");
        // SAFETY: commit_page made the address readable and writable.
        unsafe { pointer.as_ptr().write(0x5a) };
        test_ok(range.decommit_group(0), "empty group decommits");
        assert_eq!(range.is_group_committed(0), Some(false));
        assert_eq!(range.page_address(0), Some(address));

        test_ok(range.commit_page(0), "first page recommits");
        assert_eq!(range.is_group_committed(0), Some(true));
        // SAFETY: commit_page made the page readable and zeroed the complete group.
        assert_eq!(unsafe { pointer.as_ptr().read() }, 0);
    }
}
