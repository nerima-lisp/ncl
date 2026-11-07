use crate::error::{FaslError, FaslRejectReason};

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
            let file_offset = align_up(cursor, alignment)?;
            let end = file_offset
                .checked_add(byte_length)
                .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
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

    pub fn encode(&self) -> Result<[u8; DIRECTORY_SIZE], FaslError> {
        let mut bytes = [0; DIRECTORY_SIZE];
        for (index, entry) in self.entries.iter().enumerate() {
            let start = index * DIRECTORY_ENTRY_SIZE;
            write_range(
                &mut bytes,
                start..start + 2,
                &entry.id.wire_id().to_le_bytes(),
            )?;
            write_range(
                &mut bytes,
                start + 4..start + 8,
                &entry.alignment.to_le_bytes(),
            )?;
            write_range(
                &mut bytes,
                start + 8..start + 16,
                &entry.file_offset.to_le_bytes(),
            )?;
            write_range(
                &mut bytes,
                start + 16..start + 24,
                &entry.byte_length.to_le_bytes(),
            )?;
        }
        Ok(bytes)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, FaslError> {
        if bytes.len() != DIRECTORY_SIZE {
            return Err(FaslError::at(FaslRejectReason::MalformedDirectory, 128));
        }
        let mut entries = [DirectoryEntry {
            id: SectionId::Metadata,
            alignment: 4,
            file_offset: FIRST_SECTION_OFFSET,
            byte_length: 0,
        }; SECTION_COUNT];
        for (index, (entry, expected_id)) in entries.iter_mut().zip(SectionId::ALL).enumerate() {
            let start = index * DIRECTORY_ENTRY_SIZE;
            let id = read_u16(bytes, start)?;
            if SectionId::from_wire_id(id) != Some(expected_id) {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    128 + start,
                ));
            }
            if read_u16(bytes, start + 2)? != 0 {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    128 + start + 2,
                ));
            }
            let alignment = read_u32(bytes, start + 4)?;
            if alignment == 0
                || alignment > 4096
                || !alignment.is_power_of_two()
                || alignment != expected_id.alignment()
            {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    128 + start + 4,
                ));
            }
            let file_offset = read_u64(bytes, start + 8)?;
            let byte_length = read_u64(bytes, start + 16)?;
            if file_offset.checked_add(byte_length).is_none() {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    128 + start + 8,
                ));
            }
            if byte_length > MAX_FILE_SIZE {
                return Err(FaslError::at(
                    FaslRejectReason::ResourceLimitExceeded,
                    128 + start + 16,
                ));
            }
            *entry = DirectoryEntry {
                id: expected_id,
                alignment,
                file_offset,
                byte_length,
            };
        }
        Ok(Self { entries })
    }

    pub const fn entries(&self) -> &[DirectoryEntry; SECTION_COUNT] {
        &self.entries
    }

    pub fn validate_file(&self, file: &[u8]) -> Result<(), FaslError> {
        let file_len = u64::try_from(file.len())
            .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        if file_len > MAX_FILE_SIZE {
            return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
        }
        if file_len < FIRST_SECTION_OFFSET {
            return Err(FaslError::at(
                FaslRejectReason::MalformedDirectory,
                file.len(),
            ));
        }

        let mut cursor = FIRST_SECTION_OFFSET;
        for (index, entry) in self.entries.iter().enumerate() {
            let directory_offset = 128 + index * DIRECTORY_ENTRY_SIZE;
            let expected_offset = align_up(cursor, entry.alignment)?;
            if entry.file_offset != expected_offset {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    directory_offset + 8,
                ));
            }
            let pad_start = usize::try_from(cursor).map_err(|_| {
                FaslError::at(FaslRejectReason::MalformedDirectory, directory_offset + 8)
            })?;
            let section_start = usize::try_from(expected_offset).map_err(|_| {
                FaslError::at(FaslRejectReason::MalformedDirectory, directory_offset + 8)
            })?;
            let padding = file
                .get(pad_start..section_start)
                .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, pad_start))?;
            if let Some(nonzero) = padding.iter().position(|byte| *byte != 0) {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    pad_start + nonzero,
                ));
            }
            let end = expected_offset
                .checked_add(entry.byte_length)
                .ok_or_else(|| {
                    FaslError::at(FaslRejectReason::MalformedDirectory, directory_offset + 8)
                })?;
            if end > file_len {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    directory_offset + 16,
                ));
            }
            cursor = end;
        }
        if cursor != file_len {
            let offset = usize::try_from(cursor)
                .map_err(|_| FaslError::new(FaslRejectReason::MalformedDirectory))?;
            return Err(FaslError::at(FaslRejectReason::MalformedDirectory, offset));
        }
        Ok(())
    }

    pub fn section<'a>(&self, file: &'a [u8], id: SectionId) -> Result<&'a [u8], FaslError> {
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| FaslError::new(FaslRejectReason::MalformedDirectory))?;
        let start = usize::try_from(entry.file_offset)
            .map_err(|_| FaslError::new(FaslRejectReason::MalformedDirectory))?;
        let length = usize::try_from(entry.byte_length)
            .map_err(|_| FaslError::new(FaslRejectReason::ResourceLimitExceeded))?;
        let end = start
            .checked_add(length)
            .ok_or_else(|| FaslError::new(FaslRejectReason::MalformedDirectory))?;
        file.get(start..end)
            .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, start))
    }
}

fn write_range(
    bytes: &mut [u8],
    range: core::ops::Range<usize>,
    value: &[u8],
) -> Result<(), FaslError> {
    let offset = range.start;
    let destination = bytes
        .get_mut(range)
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, offset))?;
    if destination.len() != value.len() {
        return Err(FaslError::at(FaslRejectReason::MalformedDirectory, offset));
    }
    destination.copy_from_slice(value);
    Ok(())
}

fn align_up(value: u64, alignment: u32) -> Result<u64, FaslError> {
    let mask = u64::from(
        alignment
            .checked_sub(1)
            .ok_or_else(|| FaslError::new(FaslRejectReason::MalformedDirectory))?,
    );
    value
        .checked_add(mask)
        .map(|aligned| aligned & !mask)
        .ok_or_else(|| FaslError::new(FaslRejectReason::ResourceLimitExceeded))
}

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], FaslError> {
    let end = offset
        .checked_add(N)
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, offset))?;
    let source = bytes
        .get(offset..end)
        .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, 128 + offset))?;
    let mut value = [0; N];
    value.copy_from_slice(source);
    Ok(value)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, FaslError> {
    Ok(u16::from_le_bytes(read_array::<2>(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, FaslError> {
    Ok(u32::from_le_bytes(read_array::<4>(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, FaslError> {
    Ok(u64::from_le_bytes(read_array::<8>(bytes, offset)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn must_succeed<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("test fixture must be valid"))
    }

    fn must_exist<T>(value: Option<T>) -> T {
        value.unwrap_or_else(|| panic!("fixed directory entry must exist"))
    }

    fn fixture() -> (Directory, Vec<u8>) {
        let directory = must_succeed(Directory::from_lengths([1; SECTION_COUNT]));
        let last = must_exist(directory.entries().last());
        let last = last.file_offset.saturating_add(last.byte_length);
        let mut file = vec![0; must_succeed(usize::try_from(last))];
        file[128..320].copy_from_slice(&must_succeed(directory.encode()));
        (directory, file)
    }

    #[test]
    fn directory_round_trip_fixes_ids_alignments_empty_offsets_and_padding() {
        let (directory, file) = fixture();
        let decoded = Directory::decode(&must_succeed(directory.encode()));
        assert_eq!(decoded, Ok(directory.clone()));
        assert_eq!(directory.validate_file(&file), Ok(()));
        let entries = directory.entries();
        assert_eq!(entries[0].file_offset, 320);
        assert_eq!(entries[1].file_offset, 324);
        assert_eq!(entries[2].file_offset, 328);
        assert_eq!(entries[3].file_offset, 336);
        assert!(entries.iter().all(|entry| entry.byte_length == 1));
        assert_eq!(directory.section(&file, SectionId::Code), Ok(&[0][..]));
    }

    #[test]
    fn directory_rejects_bad_order_flags_alignment_overflow_padding_and_trailing_bytes() {
        let (directory, file) = fixture();
        let mut bytes = must_succeed(directory.encode());
        bytes[..2].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            Directory::decode(&bytes).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 128))
        );

        let mut bytes = must_succeed(directory.encode());
        bytes[2] = 1;
        assert_eq!(
            Directory::decode(&bytes).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 130))
        );

        let mut bytes = must_succeed(directory.encode());
        bytes[4..8].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(
            Directory::decode(&bytes).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 132))
        );

        let mut overflow = must_succeed(directory.encode());
        overflow[8..16].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            Directory::decode(&overflow).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 136))
        );

        let mut bad_padding = file.clone();
        bad_padding[321] = 1;
        assert_eq!(
            directory.validate_file(&bad_padding),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 321))
        );

        let mut trailing = file;
        trailing.push(0);
        assert_eq!(
            directory.validate_file(&trailing),
            Err(FaslError::at(
                FaslRejectReason::MalformedDirectory,
                trailing.len() - 1
            ))
        );
    }
}
