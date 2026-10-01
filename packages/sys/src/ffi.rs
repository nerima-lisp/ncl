//! Safe wrappers for the foreign-function boundary.

#![allow(
    clippy::chunks_exact_to_as_chunks,
    clippy::len_zero,
    clippy::many_single_char_names,
    clippy::missing_const_for_fn,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value
)]

use core::ffi::{CStr, c_int, c_void};

use crate::os::declarations;

/// Error returned by the dynamic loader.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DlError(pub String);

impl core::fmt::Display for DlError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for DlError {}

/// Opaque handle to a loaded shared object.
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct SharedObject(pub(crate) *mut c_void);

impl SharedObject {
    /// Construct a handle from a live loader pointer.
    #[must_use]
    pub const fn from_raw(pointer: *mut c_void) -> Self {
        Self(pointer)
    }

    /// Return the underlying loader pointer.
    #[must_use]
    pub const fn as_raw(&self) -> *mut c_void {
        self.0
    }
}

/// Error from unmanaged memory operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryError {
    /// The requested size or address is invalid.
    Invalid,
    /// The platform allocator failed.
    AllocationFailed,
}

/// Error from the supported scalar C call ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallError {
    /// The call has more than eight machine-word arguments.
    TooManyArguments,
    /// The requested result buffer is not one machine word.
    InvalidResult,
}

/// Open a shared object.
pub fn dlopen_shared_object(path: &str, lazy: bool) -> Result<SharedObject, DlError> {
    let path = std::ffi::CString::new(path).map_err(|_| DlError("path contains NUL".into()))?;
    let flags: c_int = if lazy { 0x1 } else { 0x2 };
    // SAFETY: `path` is a live NUL-terminated string and flags are valid POSIX loader flags.
    let handle = unsafe { declarations::dlopen(path.as_ptr(), flags) };
    if handle.is_null() {
        Err(loader_error())
    } else {
        Ok(SharedObject(handle))
    }
}

/// Close a shared object.
pub fn dlclose_shared_object(handle: SharedObject) -> Result<(), DlError> {
    // SAFETY: ownership of the live loader handle is consumed exactly once.
    let result = unsafe { declarations::dlclose(handle.0) };
    if result == 0 {
        Ok(())
    } else {
        Err(loader_error())
    }
}

/// Resolve a symbol in a shared object.
pub fn dlsym_foreign_symbol(handle: Option<&SharedObject>, name: &str) -> Result<usize, DlError> {
    let name = std::ffi::CString::new(name).map_err(|_| DlError("name contains NUL".into()))?;
    let object = handle.map_or_else(
        || {
            // SAFETY: a null path requests the current process image on POSIX.
            unsafe { declarations::dlopen(core::ptr::null(), 0x1) }
        },
        |value| value.0,
    );
    // SAFETY: `name` is NUL-terminated and `object` is either the process handle or live handle.
    let address = unsafe { declarations::dlsym(object, name.as_ptr()) };
    if address.is_null() {
        Err(loader_error())
    } else {
        Ok(address as usize)
    }
}

/// Return and clear the current loader error.
pub fn dlerror_message() -> Option<String> {
    // SAFETY: `dlerror` returns a thread-local NUL-terminated message or null.
    let error = unsafe { declarations::dlerror() };
    (!error.is_null()).then(|| {
        // SAFETY: `error` is the non-null NUL-terminated pointer returned by `dlerror`.
        unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned()
    })
}

fn loader_error() -> DlError {
    DlError(dlerror_message().unwrap_or_else(|| "dynamic loader operation failed".into()))
}

/// Allocate unmanaged memory.
pub fn allocate_system_memory(size: usize) -> Result<usize, MemoryError> {
    if size == 0 {
        return Err(MemoryError::Invalid);
    }
    // SAFETY: the allocator accepts any non-zero size and returns an owned block.
    let address = unsafe { declarations::malloc(size) };
    (!address.is_null())
        .then_some(address as usize)
        .ok_or(MemoryError::AllocationFailed)
}

/// Release unmanaged memory allocated by [`allocate_system_memory`].
pub fn deallocate_system_memory(address: usize, _size: usize) -> Result<(), MemoryError> {
    if address == 0 {
        return Err(MemoryError::Invalid);
    }
    // SAFETY: caller promises this address came from the matching allocator.
    unsafe { declarations::free(address as *mut c_void) };
    Ok(())
}

/// Copy bytes between unmanaged memory ranges.
pub fn memmove_system_memory(
    destination: usize,
    source: usize,
    count: usize,
) -> Result<(), MemoryError> {
    if count != 0 && (destination == 0 || source == 0) {
        return Err(MemoryError::Invalid);
    }
    // SAFETY: the caller supplies live ranges of `count` bytes; libc handles overlap.
    unsafe { declarations::memmove(destination as *mut c_void, source as *const c_void, count) };
    Ok(())
}

/// Read bytes from unmanaged memory.
pub fn read_system_memory(address: usize, out: &mut [u8]) -> Result<(), MemoryError> {
    if !out.is_empty() && address == 0 {
        return Err(MemoryError::Invalid);
    }
    // SAFETY: caller supplies a readable range; `out` is valid for its length.
    unsafe { core::ptr::copy_nonoverlapping(address as *const u8, out.as_mut_ptr(), out.len()) };
    Ok(())
}

/// Write bytes to unmanaged memory.
pub fn write_system_memory(address: usize, bytes: &[u8]) -> Result<(), MemoryError> {
    if !bytes.is_empty() && address == 0 {
        return Err(MemoryError::Invalid);
    }
    // SAFETY: caller supplies a writable range; `bytes` is valid for its length.
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), address as *mut u8, bytes.len()) };
    Ok(())
}

/// Call a scalar C function with up to eight machine-word arguments.
pub fn call_foreign_function(
    address: usize,
    arguments: &[u8],
    result: &mut [u8],
) -> Result<(), CallError> {
    if result.len() > 8 || result.len() == 0 && arguments.len() > 64 {
        return Err(CallError::InvalidResult);
    }
    if !arguments.len().is_multiple_of(8) {
        return Err(CallError::InvalidResult);
    }
    let words: Vec<u64> = arguments
        .chunks_exact(8)
        .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap_or([0; 8])))
        .collect();
    if words.len() > 8 {
        return Err(CallError::TooManyArguments);
    }
    // SAFETY: the caller establishes that `address` has the scalar C signature represented here.
    let value = unsafe { call_words(address, &words) };
    if !result.is_empty() {
        result.copy_from_slice(&value.to_le_bytes()[..result.len()]);
    }
    Ok(())
}

unsafe fn call_words(address: usize, words: &[u64]) -> u64 {
    type F0 = unsafe extern "C" fn() -> u64;
    type F1 = unsafe extern "C" fn(u64) -> u64;
    type F2 = unsafe extern "C" fn(u64, u64) -> u64;
    type F3 = unsafe extern "C" fn(u64, u64, u64) -> u64;
    type F4 = unsafe extern "C" fn(u64, u64, u64, u64) -> u64;
    type F5 = unsafe extern "C" fn(u64, u64, u64, u64, u64) -> u64;
    type F6 = unsafe extern "C" fn(u64, u64, u64, u64, u64, u64) -> u64;
    type F7 = unsafe extern "C" fn(u64, u64, u64, u64, u64, u64, u64) -> u64;
    type F8 = unsafe extern "C" fn(u64, u64, u64, u64, u64, u64, u64, u64) -> u64;
    // SAFETY: the caller establishes that `address` has the scalar C signature represented here.
    unsafe {
        match words {
            [] => core::mem::transmute::<usize, F0>(address)(),
            [a] => core::mem::transmute::<usize, F1>(address)(*a),
            [a, b] => core::mem::transmute::<usize, F2>(address)(*a, *b),
            [a, b, c] => core::mem::transmute::<usize, F3>(address)(*a, *b, *c),
            [a, b, c, d] => core::mem::transmute::<usize, F4>(address)(*a, *b, *c, *d),
            [a, b, c, d, e] => core::mem::transmute::<usize, F5>(address)(*a, *b, *c, *d, *e),
            [a, b, c, d, e, f] => {
                core::mem::transmute::<usize, F6>(address)(*a, *b, *c, *d, *e, *f)
            }
            [a, b, c, d, e, f, g] => {
                core::mem::transmute::<usize, F7>(address)(*a, *b, *c, *d, *e, *f, *g)
            }
            [a, b, c, d, e, f, g, h] => {
                core::mem::transmute::<usize, F8>(address)(*a, *b, *c, *d, *e, *f, *g, *h)
            }
            _ => 0,
        }
    }
}
