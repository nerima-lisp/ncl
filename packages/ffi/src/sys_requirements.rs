//! Primitives `ncl-ffi` needs from `ncl-sys` that do not exist yet.
//!
//! `ncl-ffi` stays within Rust's safe subset, and `ncl-sys` currently exposes
//! only raw `extern "C"` declarations for dynamic loading and for reading or
//! writing an arbitrary address. Every operation that needs one of those
//! declarations fails with
//! [`FfiError::MissingSysPrimitive`](crate::FfiError::MissingSysPrimitive)
//! carrying the matching constant below, so the gap is observable in tests
//! rather than silently absent.
//!
//! Each constant is the exact signature `ncl-ffi` requires, including the
//! safety condition the wrapper must establish.

/// Open a shared object and return an opaque, owned handle.
///
/// Safety condition: the wrapper owns the handle and closes it exactly once.
pub const DLOPEN_SHARED_OBJECT: &str =
    "ncl_sys::dlopen_shared_object(path: &str, lazy: bool) -> Result<SharedObject, DlError>";

/// Close a shared object handle opened by [`DLOPEN_SHARED_OBJECT`].
///
/// Safety condition: the handle is live and not closed twice.
pub const DLCLOSE_SHARED_OBJECT: &str =
    "ncl_sys::dlclose_shared_object(handle: SharedObject) -> Result<(), DlError>";

/// Resolve a symbol address within an open shared object.
///
/// Safety condition: the address is only passed to [`CALL_FOREIGN_FUNCTION`]
/// with a matching signature.
pub const DLSYM_FOREIGN_SYMBOL: &str =
    "ncl_sys::dlsym_foreign_symbol(handle: SharedObject, name: &str) -> Result<usize, DlError>";

/// Return the most recent dynamic-loader error message, if any.
pub const DLERROR_MESSAGE: &str = "ncl_sys::dlerror_message() -> Option<String>";

/// Call a C function pointer with a marshalled argument buffer.
///
/// Safety condition: `address` is a live function pointer whose C signature
/// matches the layout `ncl-ffi` marshalled into `arguments`, and `result` is
/// exactly `size_of(result_type)` bytes.
pub const CALL_FOREIGN_FUNCTION: &str = "ncl_sys::call_foreign_function(address: usize, arguments: &[u8], result: &mut [u8]) -> Result<(), CallError>";

/// Copy `size` bytes from an arbitrary address into `out`.
///
/// Safety condition: `address .. address + size` is a readable range.
pub const READ_SYSTEM_MEMORY: &str = "ncl_sys::read_system_memory(address: usize, size: usize, out: &mut [u8]) -> Result<(), MemoryError>";

/// Copy `bytes` to an arbitrary address.
///
/// Safety condition: `address .. address + bytes.len()` is a writable range.
pub const WRITE_SYSTEM_MEMORY: &str =
    "ncl_sys::write_system_memory(address: usize, bytes: &[u8]) -> Result<(), MemoryError>";

/// Allocate `size` bytes of unmanaged memory and return its address.
pub const ALLOCATE_SYSTEM_MEMORY: &str =
    "ncl_sys::allocate_system_memory(size: usize) -> Result<usize, MemoryError>";

/// Release unmanaged memory previously returned by [`ALLOCATE_SYSTEM_MEMORY`].
pub const DEALLOCATE_SYSTEM_MEMORY: &str =
    "ncl_sys::deallocate_system_memory(address: usize, size: usize) -> Result<(), MemoryError>";

/// Copy `count` bytes between two possibly overlapping addresses.
pub const MEMMOVE_SYSTEM_MEMORY: &str = "ncl_sys::memmove_system_memory(destination: usize, source: usize, count: usize) -> Result<(), MemoryError>";

/// Pin an object so a collection cannot move it while an alien view is live.
pub const PIN_OBJECT: &str =
    "ncl_sys::pin_object(thread: &mut Thread, object: Word) -> Result<PinToken, StorageCondition>";

/// Release a pin acquired by [`PIN_OBJECT`].
pub const UNPIN_OBJECT: &str =
    "ncl_sys::unpin_object(thread: &mut Thread, token: PinToken) -> bool";

/// Return the payload address of a heap object for an alien view.
///
/// Safety condition: the address is only valid while the object stays pinned.
pub const OBJECT_ADDRESS: &str =
    "ncl_sys::object_address(thread: &Thread, object: Word) -> Option<usize>";
