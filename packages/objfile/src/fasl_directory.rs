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
            let file_offset = align_up(cursor, alignment)?;
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
        for (index, entry) in self.entries.iter().enumerate() {
            let start = index * DIRECTORY_ENTRY_SIZE;
            bytes[start..start + 2].copy_from_slice(&entry.id.wire_id().to_le_bytes());
            bytes[start + 4..start + 8].copy_from_slice(&entry.alignment.to_le_bytes());
            bytes[start + 8..start + 16].copy_from_slice(&entry.file_offset.to_le_bytes());
            bytes[start + 16..start + 24].copy_from_slice(&entry.byte_length.to_le_bytes());
        }
        bytes
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
            let id = read_u16(bytes, start);
            if SectionId::from_wire_id(id) != Some(expected_id) {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    128 + start,
                ));
            }
            if read_u16(bytes, start + 2) != 0 {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    128 + start + 2,
                ));
            }
            let alignment = read_u32(bytes, start + 4);
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
            let file_offset = read_u64(bytes, start + 8);
            let byte_length = read_u64(bytes, start + 16);
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
        validate_file_size(file.len())?;
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
            let expected_offset = align_up(cursor, entry.alignment)?;
            if entry.file_offset != expected_offset {
                return Err(FaslError::at(
                    FaslRejectReason::MalformedDirectory,
                    directory_offset + 8,
                ));
            }
            let pad_start = cursor as usize;
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
            return Err(FaslError::at(
                FaslRejectReason::MalformedDirectory,
                cursor as usize,
            ));
        }
        Ok(())
    }

    pub fn section<'a>(&self, file: &'a [u8], id: SectionId) -> Result<&'a [u8], FaslError> {
        let entry = &self.entries[id.index()];
        let start = usize::try_from(entry.file_offset)
            .map_err(|_| FaslError::new(FaslRejectReason::MalformedDirectory))?;
        let length = entry.byte_length as usize;
        let end = start
            .checked_add(length)
            .ok_or_else(|| FaslError::new(FaslRejectReason::MalformedDirectory))?;
        file.get(start..end)
            .ok_or_else(|| FaslError::at(FaslRejectReason::MalformedDirectory, start))
    }
}

fn validate_file_size(file_size: usize) -> Result<(), FaslError> {
    if file_size > MAX_FILE_SIZE as usize {
        return Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded));
    }
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

fn read_array<const N: usize>(bytes: &[u8], offset: usize) -> [u8; N] {
    let mut value = [0; N];
    value.copy_from_slice(&bytes[offset..offset + N]);
    value
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(read_array::<2>(bytes, offset))
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(read_array::<4>(bytes, offset))
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(read_array::<8>(bytes, offset))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn must_succeed<T, E>(result: Result<T, E>) -> T {
        result.unwrap_or_else(|_| panic!("test fixture must be valid"))
    }

    fn fixture() -> (Directory, Vec<u8>) {
        let directory = must_succeed(Directory::from_lengths([1; SECTION_COUNT]));
        let last = directory.entries()[SECTION_COUNT - 1];
        let last = last.file_offset.saturating_add(last.byte_length);
        let mut file = vec![0; must_succeed(usize::try_from(last))];
        file[128..320].copy_from_slice(&directory.encode());
        (directory, file)
    }

    #[test]
    fn directory_round_trip_fixes_ids_alignments_empty_offsets_and_padding() {
        let (directory, file) = fixture();
        let decoded = Directory::decode(&directory.encode());
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
    fn directory_rejects_bad_order_flags_alignment_padding_and_trailing_bytes() {
        let (directory, file) = fixture();
        let mut bytes = directory.encode();
        bytes[..2].copy_from_slice(&2u16.to_le_bytes());
        assert_eq!(
            Directory::decode(&bytes).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 128))
        );

        let mut bytes = directory.encode();
        bytes[2] = 1;
        assert_eq!(
            Directory::decode(&bytes).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 130))
        );

        let mut bytes = directory.encode();
        bytes[4..8].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(
            Directory::decode(&bytes).map(|_| ()),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 132))
        );

        let mut overflow = directory.encode();
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

    #[test]
    fn directory_bounds_check_section_lengths_without_allocating_file_sized_buffers() {
        let mut over_limit = [0; SECTION_COUNT];
        over_limit[0] = MAX_FILE_SIZE + 1;
        assert_eq!(
            Directory::from_lengths(over_limit),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );

        let mut end_over_limit = [0; SECTION_COUNT];
        end_over_limit[0] = MAX_FILE_SIZE;
        assert_eq!(
            Directory::from_lengths(end_over_limit),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );

        let mut exact_limit = [0; SECTION_COUNT];
        exact_limit[0] = MAX_FILE_SIZE - 320;
        assert!(Directory::from_lengths(exact_limit).is_ok());
        assert_eq!(validate_file_size(MAX_FILE_SIZE as usize), Ok(()));
        assert_eq!(
            validate_file_size(MAX_FILE_SIZE as usize + 1),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );
    }

    #[test]
    fn directory_decode_checks_exact_size_alignment_and_section_size_limits() {
        for size in [DIRECTORY_SIZE - 1, DIRECTORY_SIZE + 1] {
            assert_eq!(
                Directory::decode(&vec![0; size]),
                Err(FaslError::at(FaslRejectReason::MalformedDirectory, 128))
            );
        }

        let (directory, _) = fixture();
        for alignment in [0_u32, 8192, 8] {
            let mut bytes = directory.encode();
            bytes[4..8].copy_from_slice(&alignment.to_le_bytes());
            assert_eq!(
                Directory::decode(&bytes),
                Err(FaslError::at(FaslRejectReason::MalformedDirectory, 132)),
                "invalid alignment {alignment}"
            );
        }

        let mut oversized_section = directory.encode();
        oversized_section[16..24].copy_from_slice(&(MAX_FILE_SIZE + 1).to_le_bytes());
        assert_eq!(
            Directory::decode(&oversized_section),
            Err(FaslError::at(FaslRejectReason::ResourceLimitExceeded, 144))
        );
    }

    #[test]
    fn directory_file_validation_reports_short_ranges_misalignment_and_overflow() {
        let (directory, file) = fixture();
        assert_eq!(
            directory.validate_file(&[]),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 0))
        );
        assert_eq!(
            directory.validate_file(&file[..321]),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 321))
        );

        let mut bad_offset = directory.encode();
        bad_offset[8..16].copy_from_slice(&321_u64.to_le_bytes());
        let bad_directory = must_succeed(Directory::decode(&bad_offset));
        assert_eq!(
            bad_directory.validate_file(&file),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 136))
        );

        let section = must_succeed(Directory::from_lengths([4, 0, 0, 0, 0, 0, 0, 0]));
        assert_eq!(
            section.validate_file(&[0; 323]),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 144))
        );
        assert_eq!(
            directory.section(&[], SectionId::Code),
            Err(FaslError::at(FaslRejectReason::MalformedDirectory, 336))
        );
        assert_eq!(
            align_up(0, 0),
            Err(FaslError::new(FaslRejectReason::MalformedDirectory))
        );
        assert_eq!(
            align_up(u64::MAX, 4),
            Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
        );
    }
}
