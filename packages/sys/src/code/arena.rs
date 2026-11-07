use crate::os::declarations;
use core::ffi::{c_int, c_long, c_void};
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
    base: usize,
    len: usize,
    next: AtomicUsize,
    #[cfg(target_os = "macos")]
    backing: ArenaBacking,
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
enum ArenaBacking {
    JitRegion { offset: usize },
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
            let base = region
                .base
                .checked_add(offset)
                .ok_or(CodeError::CapacityExceeded)?;
            Ok(Arc::new(Self {
                base,
                len,
                next: AtomicUsize::new(0),
                backing: ArenaBacking::JitRegion { offset },
            }))
        }
    }

    fn reserve(self: &Arc<Self>, len: usize) -> Result<Option<CodeExtent>, CodeError> {
        let mut start = self.next.load(Ordering::Relaxed);
        loop {
            let end = start.checked_add(len).ok_or(CodeError::OutOfBounds)?;
            if end > self.len {
                return Ok(None);
            }
            match self
                .next
                .compare_exchange_weak(start, end, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => {
                    let address = self.base.checked_add(start).ok_or(CodeError::OutOfBounds)?;
                    let ptr = NonNull::new(address as *mut u8).ok_or(CodeError::MappingFailed)?;
                    return Ok(Some(CodeExtent {
                        ptr,
                        #[cfg(not(target_os = "macos"))]
                        len,
                        arena: Arc::clone(self),
                    }));
                }
                Err(actual) => start = actual,
            }
        }
    }
}

impl Drop for CodeArena {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        {
            // SAFETY: this arena owns the complete mmap reservation.
            let _ = unsafe { declarations::munmap(self.base as *mut c_void, self.len) };
        }
        #[cfg(target_os = "macos")]
        let ArenaBacking::JitRegion { offset } = &self.backing;
        #[cfg(target_os = "macos")]
        if let Ok(region) = jit_region() {
            region.release(*offset, self.len);
        }
    }
}

pub(super) fn allocate(bytes: usize) -> Result<CodeExtent, CodeError> {
    let page = page_size()?;
    let extent_len = round_up(bytes, page)?;
    let arena_len = round_up(ARENA_SIZE, page)?;
    if extent_len > arena_len {
        let arena = CodeArena::new(extent_len)?;
        return CodeArena::reserve(&arena, extent_len)?.ok_or(CodeError::CapacityExceeded);
    }

    let mut active = ACTIVE_ARENA
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(arena) = active.as_ref().and_then(Weak::upgrade)
        && let Some(extent) = arena.reserve(extent_len)?
    {
        drop(active);
        return Ok(extent);
    }

    let arena = CodeArena::new(arena_len)?;
    let extent = arena
        .reserve(extent_len)?
        .ok_or(CodeError::CapacityExceeded)?;
    *active = Some(Arc::downgrade(&arena));
    drop(active);
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

fn map_anonymous(len: usize, protection: c_int, flags: c_int) -> Result<usize, CodeError> {
    // SAFETY: null requests an OS-selected page-aligned anonymous mapping.
    let raw = unsafe { declarations::mmap(core::ptr::null_mut(), len, protection, flags, -1, 0) };
    if raw == (-1_isize as *mut c_void) {
        return Err(CodeError::MappingFailed);
    }
    let Some(ptr) = NonNull::new(raw.cast::<u8>()) else {
        // SAFETY: a zero-address mapping is not representable by CodePtr.
        let _ = unsafe { declarations::munmap(raw, len) };
        return Err(CodeError::MappingFailed);
    };
    Ok(ptr.as_ptr() as usize)
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
    base: usize,
    free: Mutex<Vec<FreeExtent>>,
}

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
        let Some(index) = free.iter().position(|extent| extent.len >= len) else {
            return Err(CodeError::CapacityExceeded);
        };
        let offset = free[index].offset;
        if free[index].len == len {
            free.remove(index);
        } else {
            free[index].offset += len;
            free[index].len -= len;
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
        let mut index = 0;
        while index + 1 < free.len() {
            let end = free[index].offset + free[index].len;
            if end == free[index + 1].offset {
                let next = free.remove(index + 1);
                free[index].len += next.len;
            } else {
                index += 1;
            }
        }
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
