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
}
