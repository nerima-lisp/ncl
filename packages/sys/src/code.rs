use crate::os::declarations;
use core::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
#[path = "code/arena.rs"]
mod arena;
use arena::{CodeArena, allocate};
#[path = "code/metadata.rs"]
mod metadata;
pub use metadata::{CodeObjectMetadata, SourceLocation};
#[path = "code/registry.rs"]
mod registry;
pub use registry::CodeRegistry;
#[path = "code/frames.rs"]
mod frames;
pub use frames::{
    FrameHeader, Safepoint, SafepointMap, SafepointMapError, scan_frame, scan_frame_chain,
    scan_frame_chain_with_registry, scan_frame_with_registers, walk_frame_headers,
};

#[cfg(not(target_os = "macos"))]
const PROT_READ: i32 = 1;
#[cfg(not(target_os = "macos"))]
const PROT_EXEC: i32 = 4;

/// Errors from executable code-space operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeError {
    /// The requested mapping has zero bytes.
    EmptyAllocation,
    /// A range or address arithmetic operation exceeded the allocation.
    OutOfBounds,
    /// Publication was requested after executable publication.
    AlreadyPublished,
    /// Publication requires the code range to be sealed first.
    NotSealed,
    /// Registry activation requires instruction-cache synchronization first.
    CacheNotSynchronized,
    /// Sealing was requested more than once.
    AlreadySealed,
    /// Cache synchronization was requested more than once.
    AlreadySynchronized,
    /// Metadata is already registered for this code range.
    AlreadyRegistered,
    /// Registry activation was requested more than once.
    AlreadyActivated,
    /// The current target has no cache-synchronization implementation.
    UnsupportedTarget,
    /// A registry operation requires executable code.
    NotPublished,
    /// The operating system rejected the mapping request.
    MappingFailed,
    /// The process-wide executable code reservation has no free extent.
    CapacityExceeded,
    /// The operating system rejected executable page permissions.
    ProtectionFailed,
    /// A live registered frame still returns into the code allocation.
    CodeInUse,
    /// The code allocation is not present in the heap registry.
    NotRegistered,
}

const REGISTRY_UNREGISTERED: u8 = 0;
const REGISTRY_PREPARED: u8 = 1;
const REGISTRY_ACTIVE: u8 = 2;

/// A page-backed, non-moving code allocation.
#[derive(Debug)]
pub struct CodePtr {
    ptr: NonNull<u8>,
    len: usize,
    #[cfg(not(target_os = "macos"))]
    mapping_len: usize,
    _arena: Arc<CodeArena>,
    published: AtomicBool,
    registry_state: Arc<AtomicU8>,
    entry: AtomicUsize,
    sealed: bool,
    cache_synchronized: bool,
}

// SAFETY: CodePtr owns a stable process mapping. Writes require `&mut self` and
// become unavailable after publication; moving the owner between threads does
// not move the mapping or change its protection state.
unsafe impl Send for CodePtr {}

impl CodePtr {
    /// Return the initialized code bytes.
    #[must_use]
    pub const fn as_slice(&self) -> &[u8] {
        // SAFETY: ptr is a live mmap allocation owned by self.
        unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }
    /// Copy bytes into the allocation at a checked offset.
    ///
    /// # Errors
    ///
    /// Returns an error for a published allocation or an out-of-bounds range.
    pub fn write_code(&mut self, offset: usize, bytes: &[u8]) -> Result<(), CodeError> {
        if self.is_published() || self.sealed {
            return Err(if self.is_published() {
                CodeError::AlreadyPublished
            } else {
                CodeError::AlreadySealed
            });
        }
        let end = offset
            .checked_add(bytes.len())
            .ok_or(CodeError::OutOfBounds)?;
        if end > self.len {
            return Err(CodeError::OutOfBounds);
        }
        #[cfg(target_os = "macos")]
        // SAFETY: this thread owns the JIT mapping and brackets its write.
        unsafe {
            declarations::pthread_jit_write_protect_np(0);
        };
        // SAFETY: the allocation is writable until publication and the checked range is in-bounds.
        unsafe {
            core::slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len)[offset..end]
                .copy_from_slice(bytes);
        }
        #[cfg(target_os = "macos")]
        // SAFETY: restore the JIT write-protection state after the copy.
        unsafe {
            declarations::pthread_jit_write_protect_np(1);
        };
        Ok(())
    }
    /// Return the address of the code allocation.
    #[must_use]
    pub fn address(&self) -> usize {
        self.ptr.as_ptr() as usize
    }
    /// Return the requested byte length.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }
    /// Whether the allocation has no requested bytes.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Whether this allocation has been published.
    #[must_use]
    pub fn is_published(&self) -> bool {
        self.published.load(Ordering::Acquire)
    }
    /// Load the published entry address with acquire ordering.
    #[must_use]
    pub fn entry(&self) -> usize {
        self.entry.load(Ordering::Acquire)
    }
    pub(super) fn prepare_registry(&self) -> Result<Arc<AtomicU8>, CodeError> {
        self.registry_state
            .compare_exchange(
                REGISTRY_UNREGISTERED,
                REGISTRY_PREPARED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map_err(|_| CodeError::AlreadyRegistered)?;
        Ok(Arc::clone(&self.registry_state))
    }
    fn activate_registry(&self) -> Result<(), CodeError> {
        if !self.is_published() {
            return Err(CodeError::NotPublished);
        }
        match self.registry_state.compare_exchange(
            REGISTRY_PREPARED,
            REGISTRY_ACTIVE,
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            Ok(_) => Ok(()),
            Err(REGISTRY_ACTIVE) => Err(CodeError::AlreadyActivated),
            Err(_) => Err(CodeError::NotRegistered),
        }
    }
    fn seal(&mut self) -> Result<(), CodeError> {
        if self.is_published() {
            return Err(CodeError::AlreadyPublished);
        }
        if self.sealed {
            return Err(CodeError::AlreadySealed);
        }
        #[cfg(not(target_os = "macos"))]
        {
            // SAFETY: the mapping is page-aligned and mapping_len is its exact length.
            let result = unsafe {
                declarations::mprotect(
                    self.ptr.as_ptr().cast(),
                    self.mapping_len,
                    PROT_READ | PROT_EXEC,
                )
            };
            if result != 0 {
                return Err(CodeError::ProtectionFailed);
            }
        }
        #[cfg(target_os = "macos")]
        // SAFETY: the current thread owns this JIT mapping and ends its write scope.
        unsafe {
            declarations::pthread_jit_write_protect_np(1);
        }
        self.sealed = true;
        Ok(())
    }
    const fn validate_cache_synchronization(&self) -> Result<(), CodeError> {
        if !self.sealed {
            return Err(CodeError::NotSealed);
        }
        if self.cache_synchronized {
            return Err(CodeError::AlreadySynchronized);
        }
        Ok(())
    }

    #[cfg(target_arch = "x86_64")]
    const fn synchronize_cache(&mut self) -> Result<(), CodeError> {
        match self.validate_cache_synchronization() {
            Ok(()) => {}
            Err(error) => return Err(error),
        }
        self.cache_synchronized = true;
        Ok(())
    }

    #[cfg(not(target_arch = "x86_64"))]
    fn synchronize_cache(&mut self) -> Result<(), CodeError> {
        self.validate_cache_synchronization()?;
        #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
        synchronize_aarch64_linux(self.ptr.as_ptr(), self.len)?;
        #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
        // SAFETY: the sealed JIT range is live and contains `len` initialized bytes.
        unsafe {
            declarations::sys_icache_invalidate(self.ptr.as_ptr().cast(), self.len);
        }
        #[cfg(not(any(
            target_arch = "x86_64",
            all(target_arch = "aarch64", any(target_os = "linux", target_os = "macos"))
        )))]
        return Err(CodeError::UnsupportedTarget);
        self.cache_synchronized = true;
        Ok(())
    }
    fn activate(&self) -> Result<(), CodeError> {
        if self.is_published() {
            return Err(CodeError::AlreadyPublished);
        }
        if !self.sealed {
            return Err(CodeError::NotSealed);
        }
        if !self.cache_synchronized {
            return Err(CodeError::CacheNotSynchronized);
        }
        self.entry.store(self.address(), Ordering::Release);
        self.published.store(true, Ordering::Release);
        Ok(())
    }
}

#[cfg(all(target_arch = "aarch64", target_os = "linux"))]
fn synchronize_aarch64_linux(start: *mut u8, len: usize) -> Result<(), CodeError> {
    use core::arch::asm;

    let start = start.addr();
    let end = start.checked_add(len).ok_or(CodeError::OutOfBounds)?;
    let ctr: u64;
    // SAFETY: reading CTR_EL0 is permitted at EL0 on the supported Linux AArch64 targets.
    unsafe {
        asm!("mrs {ctr}, ctr_el0", ctr = out(reg) ctr, options(nostack));
    }
    let d_line = 4_usize << ((ctr >> 16) & 0xf);
    let i_line = 4_usize << (ctr & 0xf);
    let mut address = start & !(d_line - 1);
    while address < end {
        // SAFETY: each address is a cache-line-aligned point within the owned code range.
        unsafe {
            asm!("dc cvau, {address}", address = in(reg) address, options(nostack));
        }
        address = address.checked_add(d_line).ok_or(CodeError::OutOfBounds)?;
    }
    // SAFETY: orders the data-cache clean before instruction-cache invalidation.
    unsafe {
        asm!("dsb ish", options(nostack));
    }
    address = start & !(i_line - 1);
    while address < end {
        // SAFETY: each address is a cache-line-aligned point within the owned code range.
        unsafe {
            asm!("ic ivau, {address}", address = in(reg) address, options(nostack));
        }
        address = address.checked_add(i_line).ok_or(CodeError::OutOfBounds)?;
    }
    // SAFETY: completes instruction-cache invalidation before subsequent execution.
    unsafe {
        asm!("dsb ish", "isb sy", options(nostack));
    }
    Ok(())
}

/// Allocate writable, non-moving code storage.
///
/// # Errors
///
/// Returns an error for an empty allocation, arithmetic overflow, unavailable
/// mapping capacity, or an operating-system mapping failure.
pub fn alloc_code(bytes: usize) -> Result<CodePtr, CodeError> {
    if bytes == 0 {
        return Err(CodeError::EmptyAllocation);
    }
    let extent = allocate(bytes)?;
    Ok(CodePtr {
        ptr: extent.ptr,
        len: bytes,
        #[cfg(not(target_os = "macos"))]
        mapping_len: extent.len,
        _arena: extent.arena,
        published: AtomicBool::new(false),
        registry_state: Arc::new(AtomicU8::new(REGISTRY_UNREGISTERED)),
        entry: AtomicUsize::new(0),
        sealed: false,
        cache_synchronized: false,
    })
}

/// Release code storage that was never registered with [`crate::Heap`].
///
/// Registered code must be released through [`crate::Heap::release_code`], which
/// performs the stop-the-world quiescence check before dropping the mapping.
pub fn free_code(code: CodePtr) {
    drop(code);
}

/// Publish code after relocations and metadata have been installed.
///
/// # Errors
///
/// Returns an error when the allocation was already published or its permissions cannot be changed.
pub fn publish_code(code: &mut CodePtr) -> Result<(), CodeError> {
    seal_code(code)?;
    synchronize_code(code)?;
    activate_code(code)
}

/// Seal a code range read-execute, or restore macOS JIT write protection.
///
/// # Errors
/// Returns an error when the range was already published or the operating system rejected protection changes.
pub fn seal_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.seal()
}

/// Synchronize instruction caches after the final write and protection transition.
///
/// # Errors
/// Returns an error when the range is not sealed or cache synchronization cannot be performed.
#[cfg(target_arch = "x86_64")]
pub const fn synchronize_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.synchronize_cache()
}

/// Synchronize instruction caches after the final write and protection transition.
///
/// # Errors
/// Returns an error when the range is not sealed or cache synchronization cannot be performed.
#[cfg(not(target_arch = "x86_64"))]
pub fn synchronize_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.synchronize_cache()
}

/// Activate a sealed, synchronized code range for registry and function-cell publication.
///
/// # Errors
/// Returns an error when sealing or cache synchronization has not completed.
pub fn activate_code(code: &mut CodePtr) -> Result<(), CodeError> {
    code.activate()
}

/// Activate prepared metadata for PC lookup after executable publication.
///
/// # Errors
/// Returns an error when the code is not published, has no prepared registry entry, or is already active.
pub fn activate_code_registry(code: &CodePtr) -> Result<(), CodeError> {
    code.activate_registry()
}

/// Copy bytes into a writable code allocation.
///
/// # Errors
///
/// Returns an error for a published allocation or an out-of-bounds range.
pub fn write_code(code: &mut CodePtr, offset: usize, bytes: &[u8]) -> Result<(), CodeError> {
    code.write_code(offset, bytes)
}

#[cfg(test)]
mod tests;
