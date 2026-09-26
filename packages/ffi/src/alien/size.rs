//! Size, alignment, and field offsets of alien types.
//!
//! Sizes follow the LP64 data model shared by the supported targets (x86-64 and
//! `AArch64` on Linux and macOS): `long`, `size_t`, and pointers are eight bytes,
//! `int` is four, `short` is two, and `char` is one.

use super::{AlienRecord, AlienType, FieldOffset};

/// Round `value` up to a multiple of `align`.
const fn align_up(value: usize, align: usize) -> usize {
    if align <= 1 {
        return value;
    }
    let remainder = value % align;
    if remainder == 0 {
        value
    } else {
        value + (align - remainder)
    }
}

/// The size in bytes of `ty`.
#[must_use]
pub fn size_of(ty: &AlienType) -> usize {
    match ty {
        AlienType::Void | AlienType::Boolean | AlienType::Char | AlienType::UnsignedChar => 1,
        AlienType::Short | AlienType::UnsignedShort => 2,
        AlienType::Int
        | AlienType::UnsignedInt
        | AlienType::SingleFloat
        | AlienType::Enumeration(_) => 4,
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
        | AlienType::Function(_) => 8,
        AlienType::LongFloat => 16,
        AlienType::Array(element, length) => size_of(element).saturating_mul(length.get()),
        AlienType::Structure(record) => record_size(record),
        AlienType::Union(record) => union_size(record),
    }
}

/// The alignment in bytes of `ty`.
#[must_use]
pub fn align_of(ty: &AlienType) -> usize {
    match ty {
        AlienType::Void | AlienType::Boolean | AlienType::Char | AlienType::UnsignedChar => 1,
        AlienType::Short | AlienType::UnsignedShort => 2,
        AlienType::Int
        | AlienType::UnsignedInt
        | AlienType::SingleFloat
        | AlienType::Enumeration(_) => 4,
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
        | AlienType::Function(_) => 8,
        AlienType::LongFloat => 16,
        AlienType::Array(element, _) => align_of(element),
        AlienType::Structure(record) | AlienType::Union(record) => record_align(record),
    }
}

/// The offset in bytes of the field at `index` within `record`.
#[must_use]
pub fn offset_of(record: &AlienRecord, index: usize) -> Option<usize> {
    field_offset(record, index).map(FieldOffset::get)
}

/// Return the typed byte offset of a field within a structure.
#[must_use]
pub fn field_offset(record: &AlienRecord, index: usize) -> Option<FieldOffset> {
    record_offsets(record).get(index).copied()
}

/// The padded size in bytes of a structure.
#[must_use]
pub fn record_size(record: &AlienRecord) -> usize {
    let mut offset = 0;
    let mut align = 1;
    for (_, field) in &record.fields {
        let field_align = align_of(field);
        align = align.max(field_align);
        offset = align_up(offset, field_align);
        offset = offset.saturating_add(size_of(field));
    }
    align_up(offset, align)
}

/// The size in bytes of a union: the largest member, padded to the alignment.
#[must_use]
pub fn union_size(record: &AlienRecord) -> usize {
    let mut size = 0;
    let mut align = 1;
    for (_, field) in &record.fields {
        size = size.max(size_of(field));
        align = align.max(align_of(field));
    }
    align_up(size, align)
}

/// The alignment of a record: the largest field alignment.
fn record_align(record: &AlienRecord) -> usize {
    record
        .fields
        .iter()
        .map(|(_, field)| align_of(field))
        .max()
        .unwrap_or(1)
}

/// The byte offset of every field of a structure, in declaration order.
fn record_offsets(record: &AlienRecord) -> Vec<FieldOffset> {
    let mut offset = 0;
    let mut offsets = Vec::with_capacity(record.fields.len());
    for (_, field) in &record.fields {
        let field_align = align_of(field);
        offset = align_up(offset, field_align);
        offsets.push(FieldOffset::new(offset));
        offset = offset.saturating_add(size_of(field));
    }
    offsets
}
