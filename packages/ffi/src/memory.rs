//! Tracked unmanaged memory and the pinned-object runtime side.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use ncl_object::{Runtime, ThreadContext, Word};
use ncl_sys::RootSlot;
use ncl_sys::foreign::{
    AccessWidth, Alignment, ByteLength, ForeignRegion, SapAddress, move_sap, read_sap, write_sap,
};

use crate::FfiError;
use crate::alien::{AlienType, marshal_argument, unmarshal_result};
use crate::roots::with_roots;
use crate::sap::SystemAreaPointer;

static SYSTEM_ALLOCATIONS: OnceLock<Mutex<HashMap<usize, ForeignRegion>>> = OnceLock::new();

#[derive(Clone, Copy)]
pub enum SapAccessType {
    Signed8,
    Unsigned8,
    Signed16,
    Unsigned16,
    Signed32,
    Unsigned32,
    Signed64,
    Unsigned64,
    Pointer,
    SingleFloat,
    DoubleFloat,
}

/// Allocate tracked unmanaged memory and return its base SAP.
///
/// # Errors
/// Returns an error if the size is invalid, allocation fails, or the allocation
/// registry cannot retain the region.
pub fn allocate_system_memory(size: usize) -> Result<SystemAreaPointer, FfiError> {
    let length = u64::try_from(size).map_err(|_| FfiError::ValueOutOfRange {
        type_name: "system-memory-size",
    })?;
    let region = ForeignRegion::allocate(ByteLength::new(length), Alignment::new(8)?)?;
    let address = system_address(region.address().get())?;
    let mut allocations = allocation_registry()
        .lock()
        .map_err(|_| FfiError::ForeignMemory(ncl_sys::foreign::ForeignError::RegistryPoisoned))?;
    allocations
        .try_reserve(1)
        .map_err(|_| FfiError::ForeignMemory(ncl_sys::foreign::ForeignError::RegistryFull))?;
    allocations.insert(address.address(), region);
    drop(allocations);
    Ok(address)
}

/// Release a tracked allocation previously returned by `allocate_system_memory`.
///
/// # Errors
/// Returns an error if `address` is not tracked or `size` differs from the
/// allocation's recorded size.
pub fn deallocate_system_memory(address: SystemAreaPointer, size: usize) -> Result<(), FfiError> {
    let mut allocations = allocation_registry()
        .lock()
        .map_err(|_| FfiError::ForeignMemory(ncl_sys::foreign::ForeignError::RegistryPoisoned))?;
    let region = allocations
        .get(&address.address())
        .ok_or(FfiError::ForeignMemory(
            ncl_sys::foreign::ForeignError::UnknownRegion,
        ))?;
    let expected = usize::try_from(region.len().get()).map_err(|_| FfiError::ValueOutOfRange {
        type_name: "system-memory-size",
    })?;
    if expected != size {
        return Err(FfiError::AllocationSizeMismatch {
            expected,
            got: size,
        });
    }
    let released = allocations.remove(&address.address());
    drop(allocations);
    drop(released);
    Ok(())
}

/// Copy bytes between currently tracked regions, preserving overlap semantics.
///
/// # Errors
/// Returns an error if either address is outside tracked memory or `count` is
/// outside the supported range.
pub fn memmove(
    destination: SystemAreaPointer,
    source: SystemAreaPointer,
    count: usize,
) -> Result<SystemAreaPointer, FfiError> {
    let destination_address = sap_address(destination)?;
    let source_address = sap_address(source)?;
    let count = u64::try_from(count).map_err(|_| FfiError::ValueOutOfRange {
        type_name: "memory-copy-size",
    })?;
    move_sap(destination_address, source_address, ByteLength::new(count))?;
    Ok(destination)
}

/// Read a scalar value from a currently tracked SAP region at a checked offset.
///
/// # Errors
/// Returns an error if `ty` is unsupported, the address is outside tracked
/// memory, or the value cannot be represented in `ty`.
pub fn sap_ref(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    ty: &AlienType,
    base: SystemAreaPointer,
    offset: isize,
) -> Result<Word, FfiError> {
    let Some(width) = access_width(ty) else {
        return Err(FfiError::UnsupportedType(ty.label()));
    };
    let address = checked_offset(base, offset)?;
    let raw = read_sap(address, width)?;
    let bytes = raw
        .to_le_bytes()
        .into_iter()
        .take(width_bytes(width))
        .collect::<Vec<_>>();
    unmarshal_result(ctx, runtime, ty, &bytes)
}

/// Write a scalar value to a currently tracked SAP region at a checked offset.
///
/// # Errors
/// Returns an error if `ty` is unsupported, the address is outside tracked
/// memory, or `value` cannot be represented in `ty`.
pub fn sap_set(
    ctx: &ThreadContext,
    _runtime: &Runtime,
    ty: &AlienType,
    base: SystemAreaPointer,
    offset: isize,
    value: Word,
) -> Result<(), FfiError> {
    let Some(width) = access_width(ty) else {
        return Err(FfiError::UnsupportedType(ty.label()));
    };
    let address = checked_offset(base, offset)?;
    let bytes = marshal_argument(ctx, ty, value)?;
    if bytes.len() != width_bytes(width) {
        return Err(FfiError::UnsupportedType(ty.label()));
    }
    let mut raw = [0_u8; 8];
    for (destination, source) in raw.iter_mut().zip(bytes) {
        *destination = source;
    }
    write_sap(address, width, u64::from_le_bytes(raw))?;
    Ok(())
}

pub fn sap_ref_typed(
    ctx: &mut ThreadContext,
    runtime: &Runtime,
    ty: SapAccessType,
    base: SystemAreaPointer,
    offset: isize,
) -> Result<Word, FfiError> {
    match ty {
        SapAccessType::Signed8 | SapAccessType::Unsigned8 => {
            let address = checked_offset(base, offset)?;
            let value = u8::try_from(read_sap(address, AccessWidth::U8)?).map_err(|_| {
                FfiError::ValueOutOfRange {
                    type_name: "sap-ref-8",
                }
            })?;
            let value = if matches!(ty, SapAccessType::Signed8) {
                i64::from(i8::from_ne_bytes([value]))
            } else {
                i64::from(value)
            };
            Ok(Word::fixnum(value))
        }
        SapAccessType::Pointer => {
            let address = checked_offset(base, offset)?;
            let value = read_sap(address, AccessWidth::U64)?;
            let address = usize::try_from(value).map_err(|_| FfiError::ValueOutOfRange {
                type_name: "system-area-pointer",
            })?;
            SystemAreaPointer::new(address).to_word(ctx, runtime)
        }
        SapAccessType::Signed16
        | SapAccessType::Unsigned16
        | SapAccessType::Signed32
        | SapAccessType::Unsigned32
        | SapAccessType::Signed64
        | SapAccessType::Unsigned64
        | SapAccessType::SingleFloat
        | SapAccessType::DoubleFloat => {
            let alien_type = alien_type(ty).ok_or(FfiError::UnsupportedType("sap-ref-8"))?;
            sap_ref(ctx, runtime, &alien_type, base, offset)
        }
    }
}

pub fn sap_set_typed(
    ctx: &ThreadContext,
    runtime: &Runtime,
    ty: SapAccessType,
    base: SystemAreaPointer,
    offset: isize,
    value: Word,
) -> Result<(), FfiError> {
    match ty {
        SapAccessType::Signed8 | SapAccessType::Unsigned8 => {
            let integer_type = if matches!(ty, SapAccessType::Signed8) {
                AlienType::Int
            } else {
                AlienType::UnsignedInt
            };
            let bytes = marshal_argument(ctx, &integer_type, value)?;
            let bytes: [u8; 4] = bytes.try_into().map_err(|_| FfiError::ValueOutOfRange {
                type_name: "sap-ref-8",
            })?;
            let byte = if matches!(ty, SapAccessType::Signed8) {
                let value = i32::from_le_bytes(bytes);
                let value = i8::try_from(value).map_err(|_| FfiError::ValueOutOfRange {
                    type_name: "signed-sap-ref-8",
                })?;
                let [byte] = value.to_ne_bytes();
                byte
            } else {
                u8::try_from(u32::from_le_bytes(bytes)).map_err(|_| FfiError::ValueOutOfRange {
                    type_name: "sap-ref-8",
                })?
            };
            let address = checked_offset(base, offset)?;
            write_sap(address, AccessWidth::U8, u64::from(byte))?;
            Ok(())
        }
        SapAccessType::Signed16
        | SapAccessType::Unsigned16
        | SapAccessType::Signed32
        | SapAccessType::Unsigned32
        | SapAccessType::Signed64
        | SapAccessType::Unsigned64
        | SapAccessType::Pointer
        | SapAccessType::SingleFloat
        | SapAccessType::DoubleFloat => {
            let alien_type = alien_type(ty).ok_or(FfiError::UnsupportedType("sap-ref-8"))?;
            sap_set(ctx, runtime, &alien_type, base, offset, value)
        }
    }
}

/// Root every value in `values` across a block that builds alien views of them.
///
/// A precise root keeps an object reachable but does not prevent the collector
/// from moving it. A non-moving pin remains a separate system requirement.
///
/// # Errors
/// Returns an error if root-stack cleanup fails or the closure returns an
/// error.
pub fn with_rooted_objects<T>(
    ctx: &mut ThreadContext,
    values: &[Word],
    f: impl FnOnce(&mut ThreadContext, &[RootSlot<'_>]) -> Result<T, FfiError>,
) -> Result<T, FfiError> {
    with_roots(ctx, values, f)
}

fn allocation_registry() -> &'static Mutex<HashMap<usize, ForeignRegion>> {
    SYSTEM_ALLOCATIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn system_address(address: u64) -> Result<SystemAreaPointer, FfiError> {
    usize::try_from(address)
        .map(SystemAreaPointer::new)
        .map_err(|_| FfiError::ValueOutOfRange {
            type_name: "system-area-pointer",
        })
}

fn sap_address(address: SystemAreaPointer) -> Result<SapAddress, FfiError> {
    let address = u64::try_from(address.address()).map_err(|_| FfiError::ValueOutOfRange {
        type_name: "system-area-pointer",
    })?;
    SapAddress::new(address).map_err(FfiError::from)
}

fn checked_offset(
    base: SystemAreaPointer,
    offset: isize,
) -> Result<ncl_sys::foreign::SapAddress, FfiError> {
    let address = if offset >= 0 {
        base.address()
            .checked_add(
                usize::try_from(offset).map_err(|_| FfiError::ValueOutOfRange {
                    type_name: "system-area-pointer-offset",
                })?,
            )
    } else {
        base.address().checked_sub(offset.unsigned_abs())
    }
    .ok_or(FfiError::ValueOutOfRange {
        type_name: "system-area-pointer-offset",
    })?;
    let address = u64::try_from(address).map_err(|_| FfiError::ValueOutOfRange {
        type_name: "system-area-pointer",
    })?;
    ncl_sys::foreign::SapAddress::new(address).map_err(FfiError::from)
}

const fn access_width(ty: &AlienType) -> Option<AccessWidth> {
    match ty {
        AlienType::Void
        | AlienType::LongFloat
        | AlienType::Array(_, _)
        | AlienType::Structure(_)
        | AlienType::Union(_) => None,
        AlienType::Boolean | AlienType::Char | AlienType::UnsignedChar => Some(AccessWidth::U8),
        AlienType::Short | AlienType::UnsignedShort => Some(AccessWidth::U16),
        AlienType::Int
        | AlienType::UnsignedInt
        | AlienType::SingleFloat
        | AlienType::Enumeration(_) => Some(AccessWidth::U32),
        AlienType::Long
        | AlienType::UnsignedLong
        | AlienType::LongLong
        | AlienType::UnsignedLongLong
        | AlienType::SizeT
        | AlienType::SSizeT
        | AlienType::DoubleFloat
        | AlienType::Pointer(_)
        | AlienType::CString
        | AlienType::Utf8String
        | AlienType::SystemAreaPointer
        | AlienType::Function(_) => Some(AccessWidth::U64),
    }
}

const fn alien_type(ty: SapAccessType) -> Option<AlienType> {
    match ty {
        SapAccessType::Signed8 | SapAccessType::Unsigned8 => None,
        SapAccessType::Signed16 => Some(AlienType::Short),
        SapAccessType::Unsigned16 => Some(AlienType::UnsignedShort),
        SapAccessType::Signed32 => Some(AlienType::Int),
        SapAccessType::Unsigned32 => Some(AlienType::UnsignedInt),
        SapAccessType::Signed64 => Some(AlienType::LongLong),
        SapAccessType::Unsigned64 => Some(AlienType::UnsignedLongLong),
        SapAccessType::Pointer => Some(AlienType::SystemAreaPointer),
        SapAccessType::SingleFloat => Some(AlienType::SingleFloat),
        SapAccessType::DoubleFloat => Some(AlienType::DoubleFloat),
    }
}

const fn width_bytes(width: AccessWidth) -> usize {
    match width {
        AccessWidth::U8 => 1,
        AccessWidth::U16 => 2,
        AccessWidth::U32 => 4,
        AccessWidth::U64 => 8,
    }
}
