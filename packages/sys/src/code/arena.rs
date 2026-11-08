use crate::os::declarations;
use core::ffi::c_void;
use core::ffi::{c_int, c_long};
use core::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};

use super::CodeError;

const ARENA_SIZE: usize = 16 * 1024 * 1024;
#[cfg(target_os = "macos")]
const JIT_REGION_SIZE: usize = 1024 * 1024 * 1024;
const PROT_READ: c_int = 1;
const PROT_WRITE: c_int = 2;
#[cfg(target_os = "macos")]
const PROT_EXEC: c_int = 4;
const MAP_PRIVATE: c_int = 2;
#[cfg(target_os = "macos")]
const MAP_ANON: c_int = 0x1000;
#[cfg(target_os = "macos")]
const MAP_JIT: c_int = 0x800;
#[cfg(target_os = "linux")]
const MAP_ANON: c_int = 0x20;
#[cfg(target_os = "macos")]
const SC_PAGESIZE: c_int = 29;
#[cfg(target_os = "linux")]
const SC_PAGESIZE: c_int = 30;

unsafe extern "C" {
    fn sysconf(name: c_int) -> c_long;
}

static PAGE_SIZE: OnceLock<Result<usize, CodeError>> = OnceLock::new();
static ACTIVE_ARENA: Mutex<Option<Weak<CodeArena>>> = Mutex::new(None);
#[cfg(target_os = "macos")]
static JIT_REGION: OnceLock<Result<JitRegion, CodeError>> = OnceLock::new();

#[derive(Debug)]
pub(super) struct CodeArena {
    base: NonNull<u8>,
    len: usize,
    next: AtomicUsize,
    #[cfg(target_os = "macos")]
    backing: ArenaBacking,
}

// SAFETY: the mapping remains valid for the arena's lifetime, and its base and
// length are immutable. Reservation is coordinated by `next`; Drop runs only
// after the final Arc is released.
unsafe impl Send for CodeArena {}
// SAFETY: shared access only reads the mapping base and reserves disjoint ranges
// with the atomic cursor. The mapping is unmapped only by exclusive Drop.
unsafe impl Sync for CodeArena {}

#[cfg(target_os = "macos")]
#[derive(Debug)]
enum ArenaBacking {
    JitRegion {
        region: &'static JitRegion,
        offset: usize,
    },
}

pub(super) struct CodeExtent {
    pub(super) ptr: NonNull<u8>,
    #[cfg(not(target_os = "macos"))]
    pub(super) len: usize,
    pub(super) arena: Arc<CodeArena>,
}

impl CodeArena {
    fn new(len: usize) -> Result<Arc<Self>, CodeError> {
        #[cfg(target_os = "linux")]
        {
            let base = map_anonymous(len, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON)?;
            Ok(Arc::new(Self {
                base,
                len,
                next: AtomicUsize::new(0),
            }))
        }
        #[cfg(target_os = "macos")]
        {
            if len > JIT_REGION_SIZE {
                return Err(CodeError::CapacityExceeded);
            }
            let region = jit_region()?;
            let offset = region.reserve(len)?;
            // SAFETY: `reserve` returns an offset inside the process-wide JIT mapping.
            let base = unsafe { NonNull::new_unchecked(region.base.as_ptr().add(offset)) };
            Ok(Arc::new(Self {
                base,
                len,
                next: AtomicUsize::new(0),
                backing: ArenaBacking::JitRegion { region, offset },
            }))
        }
    }

    fn reserve(self: &Arc<Self>, len: usize) -> Option<CodeExtent> {
        let start = self
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |start| {
                if start > self.len || len > self.len - start {
                    None
                } else {
                    Some(start + len)
                }
            })
            .ok()?;
        Some(self.extent(start, len))
    }

    fn initial_extent(self: &Arc<Self>, len: usize) -> CodeExtent {
        self.next.store(len, Ordering::Relaxed);
        self.extent(0, len)
    }

    fn extent(
        self: &Arc<Self>,
        start: usize,
        #[cfg_attr(target_os = "macos", allow(unused_variables))] len: usize,
    ) -> CodeExtent {
        // SAFETY: `reserve` checks the range, and `allocate` passes a page-rounded
        // extent no larger than a new arena. This pointer arithmetic preserves
        // the mapping's provenance.
        let ptr = unsafe { NonNull::new_unchecked(self.base.as_ptr().add(start)) };
        CodeExtent {
            ptr,
            #[cfg(not(target_os = "macos"))]
            len,
            arena: Arc::clone(self),
        }
    }
}

impl Drop for CodeArena {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            let base = self.base.as_ptr().cast::<c_void>();
            // SAFETY: `base` starts this arena's mapping, and `self.len` is its complete size.
            let _ = unsafe { declarations::munmap(base, self.len) };
        }
        #[cfg(target_os = "macos")]
        let ArenaBacking::JitRegion { region, offset } = &self.backing;
        #[cfg(target_os = "macos")]
        region.release(*offset, self.len);
    }
}

pub(super) fn allocate(bytes: usize) -> Result<CodeExtent, CodeError> {
    let page = page_size()?;
    let extent_len = round_up(bytes, page)?;
    let arena_len = round_up(ARENA_SIZE, page)?;
    let extent = if extent_len > arena_len {
        let arena = CodeArena::new(extent_len)?;
        arena.initial_extent(extent_len)
    } else {
        let mut active = ACTIVE_ARENA
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(arena) = active.as_ref().and_then(Weak::upgrade)
            && let Some(extent) = arena.reserve(extent_len)
        {
            drop(active);
            extent
        } else {
            let arena = CodeArena::new(arena_len)?;
            let extent = arena.initial_extent(extent_len);
            *active = Some(Arc::downgrade(&arena));
            drop(active);
            extent
        }
    };

    #[cfg(target_os = "macos")]
    // SAFETY: the thread owns the new writable JIT extent while it is initialized.
    unsafe {
        declarations::pthread_jit_write_protect_np(0);
    }
    // SAFETY: the allocation contains at least `bytes` writable bytes.
    unsafe { extent.ptr.as_ptr().write_bytes(0, bytes) };
    #[cfg(target_os = "macos")]
    // SAFETY: restore the JIT write-protection state after initialization.
    unsafe {
        declarations::pthread_jit_write_protect_np(1);
    }
    Ok(extent)
}

fn page_size() -> Result<usize, CodeError> {
    *PAGE_SIZE.get_or_init(|| {
        // SAFETY: SC_PAGESIZE is the target libc's _SC_PAGESIZE selector.
        let size = unsafe { sysconf(SC_PAGESIZE) };
        usize::try_from(size)
            .ok()
            .filter(|size| *size > 0)
            .ok_or(CodeError::MappingFailed)
    })
}

fn round_up(value: usize, alignment: usize) -> Result<usize, CodeError> {
    let remainder = value % alignment;
    if remainder == 0 {
        Ok(value)
    } else {
        value
            .checked_add(alignment - remainder)
            .ok_or(CodeError::OutOfBounds)
    }
}

fn map_anonymous(len: usize, protection: c_int, flags: c_int) -> Result<NonNull<u8>, CodeError> {
    // SAFETY: null requests an OS-selected page-aligned anonymous mapping.
    let raw = unsafe { declarations::mmap(core::ptr::null_mut(), len, protection, flags, -1, 0) };
    mapped_address(raw, len)
}

fn mapped_address(raw: *mut c_void, len: usize) -> Result<NonNull<u8>, CodeError> {
    if raw.addr() == usize::MAX {
        return Err(CodeError::MappingFailed);
    }
    let Some(ptr) = NonNull::new(raw.cast::<u8>()) else {
        // SAFETY: `raw` is the successful zero-address result from mmap.
        let _ = unsafe { declarations::munmap(raw, len) };
        return Err(CodeError::MappingFailed);
    };
    Ok(ptr)
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct FreeExtent {
    offset: usize,
    len: usize,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct JitRegion {
    base: NonNull<u8>,
    free: Mutex<Vec<FreeExtent>>,
}

#[cfg(target_os = "macos")]
// SAFETY: the process-wide JIT mapping remains valid for the process lifetime;
// its base is immutable and free extents are protected by the mutex.
unsafe impl Send for JitRegion {}
#[cfg(target_os = "macos")]
// SAFETY: shared access reads the immutable base and mutates free extents only
// while holding the mutex.
unsafe impl Sync for JitRegion {}

#[cfg(target_os = "macos")]
impl JitRegion {
    fn new() -> Result<Self, CodeError> {
        let page = page_size()?;
        let len = round_up(JIT_REGION_SIZE, page)?;
        let base = map_anonymous(
            len,
            PROT_READ | PROT_WRITE | PROT_EXEC,
            MAP_PRIVATE | MAP_ANON | MAP_JIT,
        )?;
        let mut free = Vec::with_capacity(64);
        free.push(FreeExtent { offset: 0, len });
        Ok(Self {
            base,
            free: Mutex::new(free),
        })
    }

    fn reserve(&self, len: usize) -> Result<usize, CodeError> {
        let mut free = self
            .free
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut reservation = None;
        for extent in &mut *free {
            if extent.len >= len {
                let offset = extent.offset;
                let consumed = extent.len == len;
                if !consumed {
                    extent.offset += len;
                    extent.len -= len;
                }
                reservation = Some((offset, consumed));
                break;
            }
        }
        let Some((offset, consumed)) = reservation else {
            return Err(CodeError::CapacityExceeded);
        };
        if consumed {
            free.retain(|extent| extent.offset != offset);
        }
        drop(free);
        Ok(offset)
    }

    fn release(&self, offset: usize, len: usize) {
        let mut free = self
            .free
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        free.push(FreeExtent { offset, len });
        free.sort_unstable_by_key(|extent| extent.offset);
        let mut merged: Vec<FreeExtent> = Vec::with_capacity(free.len());
        for extent in free.drain(..) {
            if let Some(last) = merged.last_mut()
                && last.offset + last.len == extent.offset
            {
                last.len += extent.len;
            } else {
                merged.push(extent);
            }
        }
        *free = merged;
        drop(free);
    }
}

#[cfg(target_os = "macos")]
fn jit_region() -> Result<&'static JitRegion, CodeError> {
    match JIT_REGION.get_or_init(JitRegion::new) {
        Ok(region) => Ok(region),
        Err(error) => Err(*error),
    }
}
