//! Unmanaged memory operations and the pinned-object runtime side.
//!
//! `ncl-sys` exposes no safe primitive to read, write, allocate, or copy an
//! arbitrary address, so every operation that needs one returns
//! [`FfiError::MissingSysPrimitive`] naming the required signature. Rooting is
//! implemented here because it only needs the public `push_root` / `pop_root`.

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_sys::RootSlot;

use crate::FfiError;
use crate::alien::AlienType;
use crate::roots::with_roots;
use crate::sap::SystemAreaPointer;
use crate::sys_requirements::{
    ALLOCATE_SYSTEM_MEMORY, DEALLOCATE_SYSTEM_MEMORY, MEMMOVE_SYSTEM_MEMORY, READ_SYSTEM_MEMORY,
    WRITE_SYSTEM_MEMORY,
};

/// Allocate `size` bytes of unmanaged memory and return its address.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// allocation wrapper.
pub const fn allocate_system_memory(_size: usize) -> Result<SystemAreaPointer, FfiError> {
    Err(FfiError::MissingSysPrimitive(ALLOCATE_SYSTEM_MEMORY))
}

/// Release unmanaged memory previously returned by `allocate-system-memory`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// release wrapper.
pub const fn deallocate_system_memory(
    _address: SystemAreaPointer,
    _size: usize,
) -> Result<(), FfiError> {
    Err(FfiError::MissingSysPrimitive(DEALLOCATE_SYSTEM_MEMORY))
}

/// Copy `count` bytes between two possibly overlapping addresses.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// copy wrapper.
pub const fn memmove(
    _destination: SystemAreaPointer,
    _source: SystemAreaPointer,
    _count: usize,
) -> Result<SystemAreaPointer, FfiError> {
    Err(FfiError::MissingSysPrimitive(MEMMOVE_SYSTEM_MEMORY))
}

/// Read a value of `ty` from `base + offset`, the shared implementation of
/// `sap-ref-*`, `signed-sap-ref-*`, and `sap-ref-sap`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// arbitrary-address read.
pub const fn sap_ref(
    _ctx: &mut ThreadContext,
    _runtime: &Runtime,
    _ty: &AlienType,
    _base: SystemAreaPointer,
    _offset: isize,
) -> Result<Word, FfiError> {
    Err(FfiError::MissingSysPrimitive(READ_SYSTEM_MEMORY))
}

/// Write `value`, marshalled as `ty`, to `base + offset`, the shared
/// implementation of `(setf sap-ref-*)`.
///
/// # Errors
/// Returns [`FfiError::MissingSysPrimitive`] until `ncl-sys` exposes a safe
/// arbitrary-address write.
pub const fn sap_set(
    _ctx: &ThreadContext,
    _runtime: &Runtime,
    _ty: &AlienType,
    _base: SystemAreaPointer,
    _offset: isize,
    _value: Word,
) -> Result<(), FfiError> {
    Err(FfiError::MissingSysPrimitive(WRITE_SYSTEM_MEMORY))
}

/// Root every value in `values` across a block that builds alien views of them.
///
/// This is the runtime side of `with-alien` / `with-pinned-objects`: precise
/// roots keep the objects live and rewrite their slots in place after a
/// collection. A true non-moving pin is a separate `ncl-sys` requirement
/// (`sys_requirements::PIN_OBJECT`), because a root does not stop the collector
/// from copying a young object.
///
/// # Errors
/// Returns [`FfiError::RootStackCorrupt`] when a token does not pop in stack
/// order, and any error the closure returns.
pub fn with_rooted_objects<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &[RootSlot<'_>]) -> Result<T, FfiError>,
) -> Result<T, FfiError> {
    with_roots(ctx, values, f)
}
