#[cfg(target_os = "macos")]
use crate::os::declarations;
use core::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
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
#[path = "code/publish.rs"]
mod publish;
pub use publish::{
    activate_code, activate_code_registry, publish_code, seal_code, synchronize_code,
};

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
    published: bool,
    registry_state: Arc<AtomicU8>,
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
        let is_published = self.is_published();
        if is_published || self.sealed {
            return Err(if is_published {
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
    pub const fn is_published(&self) -> bool {
        self.published
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
        published: false,
        registry_state: Arc::new(AtomicU8::new(REGISTRY_UNREGISTERED)),
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
