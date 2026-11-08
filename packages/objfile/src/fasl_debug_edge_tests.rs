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
