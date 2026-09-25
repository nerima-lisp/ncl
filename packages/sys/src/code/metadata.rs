use super::SafepointMap;
use crate::Word;

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
