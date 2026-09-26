use crate::os::declarations;
use core::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[path = "code/metadata.rs"]
mod metadata;
pub use metadata::{CodeObjectMetadata, SourceLocation};
#[path = "code/registry.rs"]
mod registry;
pub use registry::CodeRegistry;
#[path = "code/frames.rs"]
mod frames;
pub use frames::{
    FrameHeader, Safepoint, SafepointMap, scan_frame, scan_frame_chain,
    scan_frame_chain_with_registry, scan_frame_with_registers, walk_frame_headers,
};

const PAGE_SIZE: usize = 4096;
const PROT_READ: i32 = 1;
const PROT_WRITE: i32 = 2;
const PROT_EXEC: i32 = 4;
const MAP_PRIVATE: i32 = 2;
#[cfg(target_os = "macos")]
const MAP_ANON: i32 = 0x1000;
#[cfg(target_os = "macos")]
const MAP_JIT: i32 = 0x800;
#[cfg(target_os = "linux")]
const MAP_ANON: i32 = 0x20;

/// Errors from executable code-space operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CodeError {
    /// The requested mapping has zero bytes.
    EmptyAllocation,
    /// A range or address arithmetic operation exceeded the allocation.
    OutOfBounds,
    /// Publication was requested after executable publication.
    AlreadyPublished,
    /// A registry operation requires published code.
    NotPublished,
    /// The operating system rejected the mapping request.
    MappingFailed,
    /// The operating system rejected executable page permissions.
    ProtectionFailed,
    /// A live registered frame still returns into the code allocation.
    CodeInUse,
    /// The code allocation is not present in the heap registry.
    NotRegistered,
}

/// A page-backed, non-moving code allocation.
#[derive(Debug)]
pub struct CodePtr {
    ptr: NonNull<u8>,
    len: usize,
    mapping_len: usize,
    published: AtomicBool,
    entry: AtomicUsize,
}

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
        if self.is_published() {
            return Err(CodeError::AlreadyPublished);
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
    fn publish(&mut self) -> Result<(), CodeError> {
        if self.is_published() {
            return Err(CodeError::AlreadyPublished);
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
        // SAFETY: the range is a live instruction mapping owned by self.
        unsafe {
            declarations::pthread_jit_write_protect_np(1);
            declarations::sys_icache_invalidate(self.ptr.as_ptr().cast(), self.len);
        }
        self.entry.store(self.address(), Ordering::Release);
        self.published.store(true, Ordering::Release);
        Ok(())
    }
}

impl Drop for CodePtr {
    fn drop(&mut self) {
        // SAFETY: ptr and mapping_len came from the successful mmap owned by self.
        let _ = unsafe { declarations::munmap(self.ptr.as_ptr().cast(), self.mapping_len) };
    }
}

/// Allocate writable, non-moving code storage.
///
/// # Errors
///
/// Returns an error for an empty allocation or when the OS cannot map pages.
pub fn alloc_code(bytes: usize) -> Result<CodePtr, CodeError> {
    if bytes == 0 {
        return Err(CodeError::EmptyAllocation);
    }
    let mapping_len = bytes
        .checked_next_multiple_of(PAGE_SIZE)
        .ok_or(CodeError::MappingFailed)?;
    #[cfg(target_os = "macos")]
    let flags = MAP_PRIVATE | MAP_ANON | MAP_JIT;
    #[cfg(not(target_os = "macos"))]
    let flags = MAP_PRIVATE | MAP_ANON;
    #[cfg(target_os = "macos")]
    let initial_prot = PROT_READ | PROT_WRITE | PROT_EXEC;
    #[cfg(not(target_os = "macos"))]
    let initial_prot = PROT_READ | PROT_WRITE;
    // SAFETY: null requests an OS-selected page-aligned anonymous mapping.
    let raw = unsafe {
        declarations::mmap(
            core::ptr::null_mut(),
            mapping_len,
            initial_prot,
            flags,
            -1,
            0,
        )
    };
    let ptr = NonNull::new(raw.cast()).ok_or(CodeError::MappingFailed)?;
    Ok(CodePtr {
        ptr,
        len: bytes,
        mapping_len,
        published: AtomicBool::new(false),
        entry: AtomicUsize::new(0),
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
    code.publish()
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
