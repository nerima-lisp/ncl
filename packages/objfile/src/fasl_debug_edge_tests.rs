use super::super::super::MAX_RECORDS;
use super::*;

#[test]
fn debug_encoder_rejects_each_record_limit_before_record_validation() {
    let too_many_files = vec![file(""); MAX_RECORDS + 1];
    assert_eq!(
        encode_section(&too_many_files, &[]),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
    drop(too_many_files);

    let too_many_records = vec![record(); MAX_RECORDS + 1];
    assert_eq!(
        encode_section(&[], &too_many_records),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
    drop(too_many_records);

    let mut oversized_record = record();
    oversized_record.variables = vec![
        DebugVariable {
            name: String::new(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 0,
        };
        MAX_RECORDS + 1
    ];
    assert_eq!(
        encode_section(&[file("a.lisp")], &[oversized_record]),
        Err(FaslError::new(FaslRejectReason::ResourceLimitExceeded))
    );
}

#[test]
fn debug_decoder_rejects_bounded_counts_and_trailing_bytes() {
    let empty = must_encode(&[], &[]);
    let limit = FaslRejectReason::ResourceLimitExceeded;
    let mut excessive_files = empty.clone();
    excessive_files[..4].copy_from_slice(&1_048_577_u32.to_le_bytes());
    assert_rejected(&excessive_files, 100, limit);
    let mut excessive_records = empty.clone();
    excessive_records[4..8].copy_from_slice(&1_048_577_u32.to_le_bytes());
    assert_rejected(&excessive_records, 100, limit);
    let mut excessive_path = must_encode(&[file("a")], &[]);
    excessive_path[8..12].copy_from_slice(&1_048_577_u32.to_le_bytes());
    assert_rejected(&excessive_path, 100, limit);
    let mut excessive_name = record();
    excessive_name.variables.push(DebugVariable {
        name: "x".into(),
        location_kind: DebugLocationKind::StackSlot,
        location_index: 0,
    });
    let mut excessive_name = must_encode(&[file("a.lisp")], &[excessive_name]);
    excessive_name[60..64].copy_from_slice(&1_048_577_u32.to_le_bytes());
    assert_rejected(&excessive_name, 100, limit);
    let mut excessive_variables = must_encode(&[file("a.lisp")], &[record()]);
    excessive_variables[56..60].copy_from_slice(&1_048_577_u32.to_le_bytes());
    assert_rejected(&excessive_variables, 100, limit);
    let mut trailing = empty;
    trailing.push(0);
    assert_rejected(&trailing, 100, FaslRejectReason::InvalidDebugRecord);
}

#[test]
fn debug_encoder_and_validator_enforce_string_size_limits() {
    let long = "x".repeat(1_048_577);
    let invalid_section: Result<Vec<u8>, FaslError> =
        Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
    let invalid_validation: Result<(), FaslError> =
        Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
    assert_eq!(encode_section(&[file(&long)], &[]), invalid_section);
    assert_eq!(validate(&[file(&long)], &[], &layout()), invalid_validation);

    let mut source = record();
    source.variables.push(DebugVariable {
        name: long,
        location_kind: DebugLocationKind::StackSlot,
        location_index: 0,
    });
    assert_eq!(
        encode_section(&[file("a")], &[source.clone()]),
        invalid_section
    );
    assert_eq!(
        validate(&[file("a")], &[source], &layout()),
        invalid_validation
    );
}

#[test]
fn debug_file_reader_reaches_truncated_following_headers() {
    let long_path = [b'a'; 13];
    let short_path = [b'a'; 9];
    for (path, path_length, second_id) in [(&long_path[..], 13, false), (&short_path[..], 9, true)]
    {
        let mut writer = crate::fasl_records::SectionWriter::new();
        writer.put_u32(2);
        writer.put_u32(0);
        writer.put_u32(path_length);
        writer.put_bytes(path);
        writer.zero_padding(4);
        if second_id {
            writer.put_u32(1);
        }
        assert_eq!(
            decode_section(&writer.into_bytes(), 500, &layout()),
            Err(FaslError::at(FaslRejectReason::InvalidDebugRecord, 528))
        );
    }
}

#[test]
fn debug_record_reader_reaches_truncated_fixed_fields_after_variable_data() {
    for (prefix, name_length) in [
        (0, 21),
        (4, 17),
        (8, 13),
        (12, 9),
        (16, 5),
        (20, 1),
        (24, 1),
        (28, 1),
        (32, 1),
    ] {
        let mut first = record();
        first.variables.push(DebugVariable {
            name: "x".repeat(name_length),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 0,
        });
        let mut second = record();
        second.pc_start = 4;
        second.pc_end = 5;
        let mut bytes = must_encode(&[file("a.lisp")], &[first, second]);
        let first_record_length = 48 + ((name_length + 3) & !3);
        let end = 24 + first_record_length + prefix;
        bytes.truncate(end);

        assert_eq!(
            decode_section(&bytes, 500, &layout()),
            Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                500 + end
            )),
            "second record prefix length {prefix}"
        );
    }
}

#[test]
fn debug_variable_reader_reaches_truncated_following_fields() {
    for (prefix, first_name_length) in [(0, 17), (4, 13), (5, 13), (9, 9), (13, 5)] {
        let mut source = record();
        source.variables = vec![
            DebugVariable {
                name: "a".repeat(first_name_length),
                location_kind: DebugLocationKind::StackSlot,
                location_index: 0,
            },
            DebugVariable {
                name: "z".into(),
                location_kind: DebugLocationKind::StackSlot,
                location_index: 1,
            },
        ];
        let mut bytes = must_encode(&[file("a.lisp")], &[source]);
        let first_variable_length = 12 + ((first_name_length + 3) & !3);
        let end = 60 + first_variable_length + prefix;
        bytes.truncate(end);
        assert_eq!(
            decode_section(&bytes, 500, &layout()),
            Err(FaslError::at(
                FaslRejectReason::InvalidDebugRecord,
                500 + end
            )),
            "second variable prefix length {prefix}"
        );
    }

    let mut source = record();
    source.variables = vec![
        DebugVariable {
            name: "a".into(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 0,
        },
        DebugVariable {
            name: "z".into(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 1,
        },
    ];
    let bytes = must_encode(&[file("a.lisp")], &[source.clone()]);
    assert_eq!(
        decode_section(&bytes, 500, &layout()),
        Ok((vec![file("a.lisp")], vec![source]))
    );
}

#[test]
fn debug_encoder_rejects_duplicate_variable_names() {
    let mut source = record();
    source.variables = vec![
        DebugVariable {
            name: "x".into(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 0,
        },
        DebugVariable {
            name: "x".into(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 1,
        },
    ];
    assert_eq!(
        encode_section(&[file("a.lisp")], &[source]),
        Err(FaslError::new(FaslRejectReason::InvalidDebugRecord))
    );
}
