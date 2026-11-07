#[cfg(test)]
mod private_tests {
    use super::super::*;
    use crate::fasl_records::FaslCodeLayout;

    const CODE: [u8; 8] = [0; 8];

    fn file(path: &str) -> DebugFile {
        DebugFile {
            file_id: 0,
            path: path.to_owned(),
        }
    }

    fn record() -> DebugRecord {
        DebugRecord {
            code_object_id: 0,
            pc_start: 0,
            pc_end: 4,
            file_id: 0,
            line: 1,
            column: 1,
            source_start: 0,
            source_end: 2,
            variables: Vec::new(),
        }
    }

    fn layout() -> [FaslCodeLayout<'static>; 1] {
        [FaslCodeLayout {
            id: 0,
            entry_offset: 0,
            frame_words: 4,
            constant_count: 0,
            code: &CODE,
        }]
    }

    #[test]
    fn debug_path_and_variable_record_round_trip_exact_bytes() {
        let files = [DebugFile {
            file_id: 0,
            path: "a.lisp".into(),
        }];
        let records = [DebugRecord {
            code_object_id: 0,
            pc_start: 0,
            pc_end: 4,
            file_id: 0,
            line: 1,
            column: 1,
            source_start: 0,
            source_end: 2,
            variables: vec![DebugVariable {
                name: "x".into(),
                location_kind: DebugLocationKind::StackSlot,
                location_index: 2,
            }],
        }];
        let expected = vec![
            1, 0, 0, 0, // file count
            0, 0, 0, 0, // file ID
            6, 0, 0, 0, // path length
            b'a', b'.', b'l', b'i', b's', b'p', 0, 0, // path and padding
            1, 0, 0, 0, // record count
            0, 0, 0, 0, // code-object ID
            0, 0, 0, 0, // PC start
            4, 0, 0, 0, // PC end
            0, 0, 0, 0, // file ID
            1, 0, 0, 0, // line
            1, 0, 0, 0, // column
            0, 0, 0, 0, // source start
            2, 0, 0, 0, // source end
            1, 0, 0, 0, // variable count
            1, 0, 0, 0, b'x', // variable name
            0, 0, 0, 0, // stack slot location kind
            2, 0, 0, 0, // stack slot index
            0, 0, 0, // padding
        ];
        let encoded = encode_section(&files, &records);
        assert_eq!(encoded, Ok(expected.clone()));
        let layout = [FaslCodeLayout {
            id: 0,
            entry_offset: 0,
            frame_words: 4,
            constant_count: 0,
            code: &[0; 4],
        }];
        assert_eq!(
            decode_section(&expected, 0, &layout),
            Ok((files.to_vec(), records.to_vec()))
        );
    }

    #[test]
    fn debug_rejects_parent_path_and_out_of_frame_location() {
        assert!(!is_normalized_relative_path("../outside.lisp"));
        assert!(!is_normalized_relative_path("/absolute.lisp"));
        assert!(!is_normalized_relative_path("a/./b.lisp"));
        assert!(!valid_location(DebugLocationKind::StackSlot, 4, 4));
        assert!(!valid_location(DebugLocationKind::SavedRegister, 16, 4));
    }

    #[test]
    fn debug_encoder_rejects_noncanonical_paths_records_and_variables() {
        let invalid = FaslError::new(FaslRejectReason::InvalidDebugRecord);
        for path in ["", "../outside.lisp", "a//b.lisp", "a\\b.lisp", "C:/a.lisp"] {
            assert_eq!(
                encode_section(&[file(path)], &[]),
                Err(invalid),
                "path {path:?}"
            );
        }

        let reversed_files = [
            DebugFile {
                file_id: 0,
                path: "b.lisp".to_owned(),
            },
            DebugFile {
                file_id: 1,
                path: "a.lisp".to_owned(),
            },
        ];
        assert_eq!(encode_section(&reversed_files, &[]), Err(invalid));

        let duplicate_records = [record(), record()];
        assert_eq!(
            encode_section(&[file("a.lisp")], &duplicate_records),
            Err(invalid)
        );

        let mut bad_file_id = record();
        bad_file_id.file_id = 1;
        assert_eq!(
            encode_section(&[file("a.lisp")], &[bad_file_id]),
            Err(invalid)
        );

        for name in [String::new(), "x\0y".to_owned()] {
            let mut bad_name = record();
            bad_name.variables.push(DebugVariable {
                name,
                location_kind: DebugLocationKind::StackSlot,
                location_index: 0,
            });
            assert_eq!(encode_section(&[file("a.lisp")], &[bad_name]), Err(invalid));
        }
    }

    #[test]
    fn debug_saved_register_round_trips_and_invalid_frame_locations_reject() {
        let files = [file("a.lisp")];
        let mut saved_register = record();
        saved_register.variables.push(DebugVariable {
            name: "x".to_owned(),
            location_kind: DebugLocationKind::SavedRegister,
            location_index: 3,
        });
        let bytes = match encode_section(&files, &[saved_register.clone()]) {
            Ok(bytes) => bytes,
            Err(error) => panic!("valid debug record rejected: {error}"),
        };
        assert_eq!(&bytes[65..69], &1_u32.to_le_bytes());
        assert_eq!(
            decode_section(&bytes, 0, &layout()),
            Ok((files.to_vec(), vec![saved_register]))
        );

        let mut bad_location = record();
        bad_location.variables.push(DebugVariable {
            name: "x".to_owned(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 4,
        });
        let bytes = match encode_section(&files, &[bad_location]) {
            Ok(bytes) => bytes,
            Err(error) => panic!("codec should defer frame validation: {error}"),
        };
        assert_eq!(
            decode_section(&bytes, 100, &layout()),
            Err(FaslError::at(FaslRejectReason::InvalidDebugRecord, 160))
        );
    }

    #[test]
    fn debug_decoder_rejects_unsorted_records_and_variable_names() {
        let files = [file("a.lisp")];
        let mut later = record();
        later.pc_start = 4;
        later.pc_end = 6;
        let mut bytes = match encode_section(&files, &[record(), later]) {
            Ok(bytes) => bytes,
            Err(error) => panic!("valid debug records rejected: {error}"),
        };
        let first = bytes[24..60].to_vec();
        let second = bytes[60..96].to_vec();
        bytes[24..60].copy_from_slice(&second);
        bytes[60..96].copy_from_slice(&first);
        assert_eq!(
            decode_section(&bytes, 200, &layout()),
            Err(FaslError::at(FaslRejectReason::InvalidDebugRecord, 260))
        );

        let mut ordered_variables = record();
        for name in ["a", "b"] {
            ordered_variables.variables.push(DebugVariable {
                name: name.to_owned(),
                location_kind: DebugLocationKind::StackSlot,
                location_index: 0,
            });
        }
        let mut bytes = match encode_section(&files, &[ordered_variables]) {
            Ok(bytes) => bytes,
            Err(error) => panic!("valid debug variables rejected: {error}"),
        };
        bytes[68] = b'z';
        bytes[84] = b'a';
        assert_eq!(
            decode_section(&bytes, 0, &layout()),
            Err(FaslError::at(FaslRejectReason::InvalidDebugRecord, 65))
        );
    }

    #[test]
    fn debug_validator_rejects_bad_paths_ranges_and_locations() {
        let invalid = Err(FaslError::new(FaslRejectReason::InvalidDebugRecord));
        assert_eq!(validate(&[file("a/../b.lisp")], &[], &layout()), invalid);

        let files = [file("a.lisp")];
        let mut bad_range = record();
        bad_range.pc_end = 9;
        assert_eq!(validate(&files, &[bad_range], &layout()), invalid);

        let mut bad_location = record();
        bad_location.variables.push(DebugVariable {
            name: "x".to_owned(),
            location_kind: DebugLocationKind::SavedRegister,
            location_index: 16,
        });
        assert_eq!(validate(&files, &[bad_location], &layout()), invalid);
    }
}
