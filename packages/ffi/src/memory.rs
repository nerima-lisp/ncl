//! Unmanaged memory operations and the pinned-object runtime side.
//!
//! Raw address operations are delegated to safe wrappers in `ncl-sys`.

#![allow(clippy::redundant_closure)]

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_sys::RootSlot;

use crate::FfiError;
use crate::alien::AlienType;
use crate::roots::with_roots;
use crate::sap::SystemAreaPointer;

/// Allocate `size` bytes of unmanaged memory and return its address.
///
/// # Errors
/// Returns [`FfiError::Memory`] when the platform allocator rejects the request.
pub fn allocate_system_memory(size: usize) -> Result<SystemAreaPointer, FfiError> {
    ncl_sys::ffi::allocate_system_memory(size)
        .map(SystemAreaPointer::new)
        .map_err(FfiError::Memory)
}

/// Release unmanaged memory previously returned by `allocate-system-memory`.
///
/// # Errors
/// Returns [`FfiError::Memory`] for a null address.
pub fn deallocate_system_memory(address: SystemAreaPointer, size: usize) -> Result<(), FfiError> {
    ncl_sys::ffi::deallocate_system_memory(address.address(), size).map_err(FfiError::Memory)
}

/// Copy `count` bytes between two possibly overlapping addresses.
///
/// # Errors
/// Returns [`FfiError::Memory`] for an invalid range.
pub fn memmove(
    destination: SystemAreaPointer,
    source: SystemAreaPointer,
    count: usize,
) -> Result<SystemAreaPointer, FfiError> {
    ncl_sys::ffi::memmove_system_memory(destination.address(), source.address(), count)
        .map(|()| destination)
        .map_err(FfiError::Memory)
}

/// Read a value of `ty` from `base + offset`, the shared implementation of
/// `sap-ref-*`, `signed-sap-ref-*`, and `sap-ref-sap`.
///
/// # Errors
/// Returns [`FfiError::Memory`] for an invalid range or the type-layer error
/// for an unsupported aggregate.
pub fn sap_ref(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    ty: &AlienType,
    base: SystemAreaPointer,
    offset: isize,
) -> Result<Word, FfiError> {
    let address = base.address().wrapping_add_signed(offset);
    let mut bytes = vec![0_u8; crate::alien::size_of(ty)];
    ncl_sys::ffi::read_system_memory(address, &mut bytes).map_err(FfiError::Memory)?;
    crate::alien::unmarshal_result(ctx, runtime, ty, &bytes)
}

/// Write `value`, marshalled as `ty`, to `base + offset`, the shared
/// implementation of `(setf sap-ref-*)`.
///
/// # Errors
/// Returns [`FfiError::Memory`] for an invalid range or the type-layer error
/// for an unsupported aggregate.
pub fn sap_set(
    ctx: &ThreadContext,
    _runtime: &Runtime,
    ty: &AlienType,
    base: SystemAreaPointer,
    offset: isize,
    value: Word,
) -> Result<(), FfiError> {
    let bytes = crate::alien::marshal_argument(ctx, ty, value)?;
    let address = base.address().wrapping_add_signed(offset);
    ncl_sys::ffi::write_system_memory(address, &bytes).map_err(FfiError::Memory)
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
