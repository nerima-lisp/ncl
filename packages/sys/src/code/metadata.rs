use super::SafepointMap;
use crate::Word;
use crate::heap::metadata::MetadataError;
use core::alloc::Layout;

/// One source location entry in a code object's debug table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocation {
    /// Code-relative byte offset.
    pub code_offset: u32,
    /// Source file identifier.
    pub file: String,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

/// Metadata registered for one immutable code range.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeObjectMetadata {
    /// Entry byte offset within the allocation.
    pub entry_offset: usize,
    /// Immutable code byte size.
    pub size: usize,
    /// Complete native frame size in words.
    pub frame_words: u16,
    /// Lisp function name.
    pub function_name: String,
    /// Source locations indexed by code-relative ranges.
    pub source_locations: Vec<SourceLocation>,
    /// GC-managed constant slots.
    pub constant_slots: Vec<Word>,
    /// Precise safepoint map index.
    pub safepoint_map: SafepointMap,
    /// Encoded debug records.
    pub debug_table: Vec<u8>,
}

impl CodeObjectMetadata {
    pub(super) fn requested_capacity_bytes(&self) -> Result<usize, MetadataError> {
        let mut bytes = 0;
        add_capacity::<u8>(&mut bytes, self.function_name.capacity())?;
        add_capacity::<SourceLocation>(&mut bytes, self.source_locations.capacity())?;
        for location in &self.source_locations {
            add_capacity::<u8>(&mut bytes, location.file.capacity())?;
        }
        add_capacity::<Word>(&mut bytes, self.constant_slots.capacity())?;
        add_capacity::<u8>(&mut bytes, self.debug_table.capacity())?;
        add_bytes(&mut bytes, self.safepoint_map.requested_capacity_bytes()?)?;
        Ok(bytes)
    }
}

fn add_capacity<T>(total: &mut usize, capacity: usize) -> Result<(), MetadataError> {
    let bytes = Layout::array::<T>(capacity)
        .map_err(|_| MetadataError::CapacityOverflow)?
        .size();
    add_bytes(total, bytes)
}

fn add_bytes(total: &mut usize, bytes: usize) -> Result<(), MetadataError> {
    *total = total
        .checked_add(bytes)
        .ok_or(MetadataError::CapacityOverflow)?;
    Ok(())
}
