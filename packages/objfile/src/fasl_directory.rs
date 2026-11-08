use crate::error::{FaslError, FaslRejectReason};
use crate::fasl_records::SectionReader;

pub const SECTION_COUNT: usize = 8;
const DIRECTORY_ENTRY_SIZE: usize = 24;
const DIRECTORY_SIZE: usize = SECTION_COUNT * DIRECTORY_ENTRY_SIZE;
const FIRST_SECTION_OFFSET: u64 = 320;
pub const MAX_FILE_SIZE: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionId {
    Metadata,
    Symbols,
    Constants,
    Code,
    Relocations,
    StackMaps,
    DebugRecords,
    TopLevelRecords,
}

impl SectionId {
    const ALL: [Self; SECTION_COUNT] = [
        Self::Metadata,
        Self::Symbols,
        Self::Constants,
        Self::Code,
        Self::Relocations,
        Self::StackMaps,
        Self::DebugRecords,
        Self::TopLevelRecords,
    ];

    const fn wire_id(self) -> u16 {
        match self {
            Self::Metadata => 1,
            Self::Symbols => 2,
            Self::Constants => 3,
            Self::Code => 4,
            Self::Relocations => 5,
            Self::StackMaps => 6,
            Self::DebugRecords => 7,
            Self::TopLevelRecords => 8,
        }
    }

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Metadata => 0,
            Self::Symbols => 1,
            Self::Constants => 2,
            Self::Code => 3,
            Self::Relocations => 4,
            Self::StackMaps => 5,
            Self::DebugRecords => 6,
            Self::TopLevelRecords => 7,
        }
    }

    const fn alignment(self) -> u32 {
        match self {
            Self::Metadata
            | Self::Symbols
            | Self::Relocations
            | Self::StackMaps
            | Self::DebugRecords
            | Self::TopLevelRecords => 4,
            Self::Constants => 8,
            Self::Code => 16,
        }
    }

    fn from_wire_id(value: u16) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|section| section.wire_id() == value)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    pub id: SectionId,
    pub alignment: u32,
    pub file_offset: u64,
    pub byte_length: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directory {
    entries: [DirectoryEntry; SECTION_COUNT],
}

impl Directory {
    pub fn from_lengths(lengths: [u64; SECTION_COUNT]) -> Result<Self, FaslError> {
        let mut entries = [DirectoryEntry {
            id: SectionId::Metadata,
            alignment: 4,
            file_offset: FIRST_SECTION_OFFSET,
            byte_length: 0,
        }; SECTION_COUNT];
        let mut cursor = FIRST_SECTION_OFFSET;
        for ((entry, id), byte_length) in entries.iter_mut().zip(SectionId::ALL).zip(lengths) {
            if byte_length > MAX_FILE_SIZE {
                return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
            }
            let alignment = id.alignment();
            let file_offset = align_section_start(cursor, alignment);
            // The section cap and the largest alignment keep this sum far below u64::MAX.
            let end = file_offset + byte_length;
            if end > MAX_FILE_SIZE {
                return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
            }
            *entry = DirectoryEntry {
                id,
                alignment,
                file_offset,
                byte_length,
            };
            cursor = end;
        }
        Ok(Self { entries })
    }

    pub fn encode(&self) -> [u8; DIRECTORY_SIZE] {
        let mut bytes = [0; DIRECTORY_SIZE];
        let values = self.entries.iter().flat_map(encode_entry);
        for (destination, value) in bytes.iter_mut().zip(values) {
            *destination = value;
        }
        bytes
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FaslError> {
        if bytes.len() != DIRECTORY_SIZE {
            return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 128));
        }
        Ok(Self {
            entries: Self::decode_entries(bytes)?,
        })
    }

    fn decode_entries(bytes: &[u8]) -> Result<[DirectoryEntry; SECTION_COUNT], FaslError> {
        let mut entries = [DirectoryEntry {
            id: SectionId::Metadata,
            alignment: 4,
            file_offset: FIRST_SECTION_OFFSET,
            byte_length: 0,
        }; SECTION_COUNT];
        let mut reader = SectionReader::new(bytes, 128, FaslRejectReason::MalformedDirectory);
        for (entry, expected_id) in entries.iter_mut().zip(SectionId::ALL) {
            let entry_offset = reader.absolute_position();
            let id = reader.u16()?;
            if SectionId::from_wire_id(id) != Some(expected_id) {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    entry_offset,
                ));
            }
            let reserved_offset = reader.absolute_position();
            if reader.u16()? != 0 {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    reserved_offset,
                ));
            }
            let alignment_offset = reader.absolute_position();
            let alignment = reader.u32()?;
            if alignment == 0
                || alignment > 4096
                || !alignment.is_power_of_two()
                || alignment != expected_id.alignment()
            {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    alignment_offset,
                ));
            }
            let file_offset_offset = reader.absolute_position();
            let file_offset = reader.u64()?;
            let byte_length_offset = reader.absolute_position();
            let byte_length = reader.u64()?;
            if file_offset.checked_add(byte_length).is_none() {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    file_offset_offset,
                ));
            }
            if byte_length > MAX_FILE_SIZE {
                return Err(FaslError::at(
                    FaslRejectReason::ResourceLimitExceeded,
                    byte_length_offset,
                ));
            }
            *entry = DirectoryEntry {
                id: expected_id,
                alignment,
                file_offset,
                byte_length,
            };
        }
        reader.finish()?;
        Ok(entries)
    }

    pub const fn entries(&self) -> &[DirectoryEntry; SECTION_COUNT] {
        &self.entries
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "cursor and aligned offsets are bounded by the validated input slice"
    )]
    pub fn validate_file(&self, file: &[u8]) -> Result<(), FaslError> {
        validate_file_size(file.len())?;
        // check-added-lines: allow(as-cast) Rust usize is no wider than the FASL u64 size field.
        let file_len = file.len() as u64;
        if file_len < FIRST_SECTION_OFFSET {
            return Err(FaslError::at(
                FaslRejectReason::MalformedDirectory,
                file.len(),
            ));
        }

        let mut cursor = FIRST_SECTION_OFFSET;
        for (index, entry) in self.entries.iter().enumerate() {
            let directory_offset = 128 + index * DIRECTORY_ENTRY_SIZE;
            let expected_offset = align_section_start(cursor, entry.alignment);
            if entry.file_offset != expected_offset {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    directory_offset + 8,
                ));
            }
            if expected_offset > file_len {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    directory_offset + 8,
                ));
            }
            // The bounded cursor and expected section offset are no greater than file_len.
            // check-added-lines: allow(as-cast) the file length is already a usize.
            let pad_start = cursor as usize;
            // check-added-lines: allow(as-cast) the expected offset is within the input slice.
            let section_start = expected_offset as usize;
            let padding = file
                .get(pad_start..section_start)
                .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, pad_start))?;
            if let Some(nonzero) = padding.iter().position(|byte| *byte != 0) {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    pad_start + nonzero,
                ));
            }
            // The length is capped, and alignment adds at most 15 to the bounded cursor.
            let end = expected_offset + entry.byte_length;
            if end > file_len {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    directory_offset + 16,
                ));
            }
            cursor = end;
        }
        if cursor != file_len {
            // check-added-lines: allow(as-cast) cursor never exceeds the validated file length.
            let offset = cursor as usize;
            return Err(FaslError::at(FaslRejectReason::MalformedDirectory, offset));
        }
        Ok(())
    }
}

const fn validate_file_size(file_size: usize) -> Result<(), FaslError> {
    // check-added-lines: allow(as-cast) Rust usize is no wider than the FASL u64 size field.
    let file_size = file_size as u64;
    if file_size > MAX_FILE_SIZE {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
    Ok(())
}

fn align_section_start(value: u64, alignment: u32) -> u64 {
    let mask = u64::from(alignment - 1);
    (value + mask) & !mask
}

fn encode_entry(entry: &DirectoryEntry) -> impl Iterator<Item = u8> {
    entry
        .id
        .wire_id()
        .to_le_bytes()
        .into_iter()
        .chain([0; 2])
        .chain(entry.alignment.to_le_bytes())
        .chain(entry.file_offset.to_le_bytes())
        .chain(entry.byte_length.to_le_bytes())
}

#[cfg(test)]
#[path = "tests/fasl_directory.rs"]
mod tests;
