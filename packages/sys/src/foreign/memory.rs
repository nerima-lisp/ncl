use super::{AccessWidth, Alignment, ByteLength, ForeignError, SapAddress};
use core::alloc::Layout;
use core::ptr::NonNull;
use std::alloc::{alloc_zeroed, dealloc};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

static REGIONS: OnceLock<Mutex<Vec<RegionRecord>>> = OnceLock::new();
static NEXT_REGION_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug)]
struct RegionRecord {
    id: u64,
    start: u64,
    end: u64,
    allocation: Arc<Allocation>,
}

#[derive(Debug)]
struct Allocation {
    base: NonNull<u8>,
    layout: Layout,
}

// SAFETY: moving or sharing this owner preserves the allocation address and layout.
// SAP accesses serialize through the registry; foreign calls retain an owner lease,
// and the caller is responsible for synchronizing C access to the pointed-to bytes.
unsafe impl Send for Allocation {}
// SAFETY: shared references expose only immutable allocation metadata. Raw memory
// access is confined to checked SAP operations or the caller's foreign-call contract.
unsafe impl Sync for Allocation {}

impl Drop for Allocation {
    fn drop(&mut self) {
        // SAFETY: `base` was allocated with this exact layout in `ForeignRegion::allocate`.
        unsafe { dealloc(self.base.as_ptr(), self.layout) };
    }
}

/// An owned allocation registered for checked SAP access.
#[derive(Debug)]
pub struct ForeignRegion {
    id: u64,
    address: SapAddress,
    length: ByteLength,
    allocation: Arc<Allocation>,
}

/// Copy bytes between currently registered regions, with overlap semantics.
///
/// # Errors
///
/// Returns an error if either address is null, untracked, or out of bounds, if the
/// length does not fit the platform, or if the region registry is unavailable.
pub fn move_sap(
    destination: SapAddress,
    source: SapAddress,
    length: ByteLength,
) -> Result<(), ForeignError> {
    let count = usize::try_from(length.get()).map_err(|_| ForeignError::LengthOutOfRange)?;
    if count == 0 {
        return Ok(());
    }
    let regions = lock_registry()?;
    let source_region = range_record(&regions, source, length)?;
    let destination_region = range_record(&regions, destination, length)?;
    let source_pointer = access_pointer(source_region, source)?;
    let destination_pointer = access_pointer(destination_region, destination)?;
    // SAFETY: both complete byte ranges were checked against live registered
    // allocations while the registry lock is held; `copy` permits overlap.
    unsafe { core::ptr::copy(source_pointer, destination_pointer, count) };
    drop(regions);
    Ok(())
}

impl ForeignRegion {
    /// Allocate and register a region with the requested size and alignment.
    ///
    /// # Errors
    ///
    /// Returns an error if the size is zero or out of range, the allocation layout
    /// is invalid, allocation fails, or the region registry cannot register it.
    pub fn allocate(length: ByteLength, alignment: Alignment) -> Result<Self, ForeignError> {
        let byte_count =
            usize::try_from(length.get()).map_err(|_| ForeignError::LengthOutOfRange)?;
        if byte_count == 0 {
            return Err(ForeignError::EmptyRegion);
        }
        let layout = Layout::from_size_align(byte_count, alignment.as_usize()?)
            .map_err(|_| ForeignError::InvalidLayout)?;
        let id = NEXT_REGION_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| ForeignError::RegionIdExhausted)?;
        // SAFETY: `layout` was created from a nonzero size and valid alignment;
        // zero-initialization makes every integer-width read valid immediately.
        let Some(base) = NonNull::new(unsafe { alloc_zeroed(layout) }) else {
            return Err(ForeignError::AllocationFailed);
        };
        let allocation = Arc::new(Allocation { base, layout });
        let Ok(start) = u64::try_from(allocation.base.as_ptr().expose_provenance()) else {
            return Err(ForeignError::AddressOutOfRange);
        };
        let address = SapAddress::new(start)?;
        let Some(end) = start.checked_add(length.get()) else {
            return Err(ForeignError::AddressOverflow);
        };
        let record = RegionRecord {
            id,
            start,
            end,
            allocation: Arc::clone(&allocation),
        };
        register(record)?;
        Ok(Self {
            id,
            address,
            length,
            allocation,
        })
    }

    /// The integer SAP at this allocation's base address.
    #[must_use]
    pub const fn address(&self) -> SapAddress {
        self.address
    }

    /// The number of bytes that SAP accesses may cover.
    #[must_use]
    pub const fn len(&self) -> ByteLength {
        self.length
    }

    /// A tracked region is never empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Read from an address inside this region.
    ///
    /// # Errors
    ///
    /// Returns an error if the address is null, outside this region, out of bounds,
    /// misaligned, or the region registry is unavailable.
    pub fn read(&self, address: SapAddress, width: AccessWidth) -> Result<u64, ForeignError> {
        read_access(address, width, Some(self))
    }

    /// Write to an address inside this region.
    ///
    /// # Errors
    ///
    /// Returns an error if the address is null, outside this region, out of bounds,
    /// misaligned, the value does not fit the requested width, or the registry is
    /// unavailable.
    pub fn write(
        &self,
        address: SapAddress,
        width: AccessWidth,
        value: u64,
    ) -> Result<(), ForeignError> {
        write_access(address, width, value, Some(self))
    }
}

impl Drop for ForeignRegion {
    fn drop(&mut self) {
        let mut regions = match registry().lock() {
            Ok(regions) => regions,
            Err(poisoned) => poisoned.into_inner(),
        };
        regions.retain(|region| region.id != self.id);
        drop(regions);
        // Dropping the record can release its owner, but call leases keep the
        // allocation alive until foreign code returns.
    }
}

/// Read a value through a currently registered address.
pub(super) fn read_sap(address: SapAddress, width: AccessWidth) -> Result<u64, ForeignError> {
    read_access(address, width, None)
}

/// Write a value through a currently registered address.
pub(super) fn write_sap(
    address: SapAddress,
    width: AccessWidth,
    value: u64,
) -> Result<(), ForeignError> {
    write_access(address, width, value, None)
}

fn read_access(
    address: SapAddress,
    width: AccessWidth,
    expected_region: Option<&ForeignRegion>,
) -> Result<u64, ForeignError> {
    let regions = lock_registry()?;
    let record = access_record(&regions, address, width, expected_region)?;
    let pointer = access_pointer(record, address)?;
    // SAFETY: the registry lock keeps the owning region registered and alive; the
    // requested range and natural alignment were checked by `access_record`.
    let value = unsafe {
        match width {
            AccessWidth::U8 => u64::from(pointer.read()),
            AccessWidth::U16 => {
                let mut bytes = [0_u8; 2];
                core::ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), bytes.len());
                u64::from(u16::from_ne_bytes(bytes))
            }
            AccessWidth::U32 => {
                let mut bytes = [0_u8; 4];
                core::ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), bytes.len());
                u64::from(u32::from_ne_bytes(bytes))
            }
            AccessWidth::U64 => {
                let mut bytes = [0_u8; 8];
                core::ptr::copy_nonoverlapping(pointer, bytes.as_mut_ptr(), bytes.len());
                u64::from_ne_bytes(bytes)
            }
        }
    };
    drop(regions);
    Ok(value)
}

fn write_access(
    address: SapAddress,
    width: AccessWidth,
    value: u64,
    expected_region: Option<&ForeignRegion>,
) -> Result<(), ForeignError> {
    if !fits_width(value, width) {
        return Err(ForeignError::ValueOutOfRange);
    }
    let regions = lock_registry()?;
    let record = access_record(&regions, address, width, expected_region)?;
    let pointer = access_pointer(record, address)?;
    // SAFETY: the registry lock keeps the owning region registered and alive; the
    // requested range and natural alignment were checked by `access_record`.
    unsafe {
        match width {
            AccessWidth::U8 => {
                pointer.write(u8::try_from(value).map_err(|_| ForeignError::ValueOutOfRange)?);
            }
            AccessWidth::U16 => {
                let bytes = u16::try_from(value)
                    .map_err(|_| ForeignError::ValueOutOfRange)?
                    .to_ne_bytes();
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
            }
            AccessWidth::U32 => {
                let bytes = u32::try_from(value)
                    .map_err(|_| ForeignError::ValueOutOfRange)?
                    .to_ne_bytes();
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
            }
            AccessWidth::U64 => {
                let bytes = value.to_ne_bytes();
                core::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer, bytes.len());
            }
        }
    }
    drop(regions);
    Ok(())
}

fn fits_width(value: u64, width: AccessWidth) -> bool {
    match width {
        AccessWidth::U8 => u8::try_from(value).is_ok(),
        AccessWidth::U16 => u16::try_from(value).is_ok(),
        AccessWidth::U32 => u32::try_from(value).is_ok(),
        AccessWidth::U64 => true,
    }
}

fn access_record<'a>(
    regions: &'a [RegionRecord],
    address: SapAddress,
    width: AccessWidth,
    expected_region: Option<&ForeignRegion>,
) -> Result<&'a RegionRecord, ForeignError> {
    let start = address.get();
    if start == 0 {
        return Err(ForeignError::NullAddress);
    }
    let record = match expected_region {
        Some(owner) => {
            let record = regions
                .iter()
                .find(|region| {
                    region.id == owner.id && Arc::ptr_eq(&region.allocation, &owner.allocation)
                })
                .ok_or(ForeignError::UnknownRegion)?;
            if start < record.start || start >= record.end {
                return Err(ForeignError::OutOfBounds);
            }
            record
        }
        None => regions
            .iter()
            .find(|region| region.start <= start && start < region.end)
            .ok_or(ForeignError::UnknownRegion)?,
    };
    let end = start
        .checked_add(width.bytes())
        .ok_or(ForeignError::AddressOverflow)?;
    if end > record.end {
        return Err(ForeignError::OutOfBounds);
    }
    if !start.is_multiple_of(width.bytes()) {
        return Err(ForeignError::Misaligned);
    }
    Ok(record)
}

fn access_pointer(record: &RegionRecord, address: SapAddress) -> Result<*mut u8, ForeignError> {
    let offset = address
        .get()
        .checked_sub(record.start)
        .ok_or(ForeignError::OutOfBounds)?;
    let offset = usize::try_from(offset).map_err(|_| ForeignError::AddressOutOfRange)?;
    // SAFETY: `access_record` or `range_record` verified the full requested range
    // lies inside this allocation; scalar callers also verified alignment.
    Ok(unsafe { record.allocation.base.as_ptr().add(offset) })
}

fn range_record(
    regions: &[RegionRecord],
    address: SapAddress,
    length: ByteLength,
) -> Result<&RegionRecord, ForeignError> {
    let start = address.get();
    if start == 0 {
        return Err(ForeignError::NullAddress);
    }
    let record = regions
        .iter()
        .find(|region| region.start <= start && start < region.end)
        .ok_or(ForeignError::UnknownRegion)?;
    let end = start
        .checked_add(length.get())
        .ok_or(ForeignError::AddressOverflow)?;
    if end > record.end {
        return Err(ForeignError::OutOfBounds);
    }
    Ok(record)
}

pub(super) fn call_with_tracked_inputs<R>(
    pointers: &[Option<SapAddress>],
    call: impl FnOnce() -> Result<R, ForeignError>,
) -> Result<R, ForeignError> {
    let has_non_null_input = pointers
        .iter()
        .any(|pointer| pointer.is_some_and(|address| address.get() != 0));
    if !has_non_null_input {
        return call();
    }
    let regions = lock_registry()?;
    let mut leases = Vec::new();
    leases
        .try_reserve(pointers.len())
        .map_err(|_| ForeignError::RegistryFull)?;
    for address in pointers.iter().flatten().copied() {
        if address.get() == 0 {
            continue;
        }
        if let Some(region) = regions
            .iter()
            .find(|region| region.start <= address.get() && address.get() < region.end)
        {
            leases.push(Arc::clone(&region.allocation));
        }
    }
    drop(regions);
    let result = call();
    drop(leases);
    result
}

fn register(record: RegionRecord) -> Result<(), ForeignError> {
    let mut regions = lock_registry()?;
    regions
        .try_reserve(1)
        .map_err(|_| ForeignError::RegistryFull)?;
    regions.push(record);
    drop(regions);
    Ok(())
}

fn lock_registry() -> Result<MutexGuard<'static, Vec<RegionRecord>>, ForeignError> {
    registry()
        .lock()
        .map_err(|_| ForeignError::RegistryPoisoned)
}

fn registry() -> &'static Mutex<Vec<RegionRecord>> {
    REGIONS.get_or_init(|| Mutex::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn call_lease_keeps_allocation_alive_after_registry_removal() -> Result<(), ForeignError> {
        let alignment = Alignment::new(8)?;
        let region = ForeignRegion::allocate(ByteLength::new(8), alignment)?;
        let address = region.address();
        let weak_after = Arc::downgrade(&region.allocation);
        let weak_inside = weak_after.clone();
        let result = call_with_tracked_inputs(&[Some(address)], move || {
            drop(region);
            Ok(weak_inside.upgrade().is_some())
        });
        assert_eq!(result, Ok(true));
        assert!(weak_after.upgrade().is_none());
        Ok(())
    }
}
