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

    fn must_encode(files: &[DebugFile], records: &[DebugRecord]) -> Vec<u8> {
        encode_section(files, records)
            .unwrap_or_else(|error| panic!("invalid debug fixture: {error}"))
    }

    fn assert_rejected(bytes: &[u8], base_offset: usize, reason: FaslRejectReason) {
        assert!(matches!(
            decode_section(bytes, base_offset, &layout()),
            Err(FaslError { reason: actual, .. }) if actual == reason
        ));
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
    fn debug_decoder_rejects_malformed_file_tables() {
        let files = [file("a.lisp")];
        let valid = must_encode(&files, &[]);
        for (offset, value) in [(0, 2_u32), (4, 1), (8, 0)] {
            let mut bytes = valid.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert_rejected(&bytes, 100, FaslRejectReason::InvalidDebugRecord);
        }

        let mut invalid_utf8 = valid.clone();
        invalid_utf8[12] = 0xff;
        assert_rejected(&invalid_utf8, 100, FaslRejectReason::InvalidDebugRecord);

        let mut nonzero_padding = valid;
        nonzero_padding[18] = 1;
        assert_rejected(&nonzero_padding, 100, FaslRejectReason::InvalidDebugRecord);

        let sorted = [
            file("a"),
            DebugFile {
                file_id: 1,
                path: "b".into(),
            },
        ];
        let mut unsorted = must_encode(&sorted, &[]);
        unsorted.swap(12, 24);
        assert_rejected(&unsorted, 0, FaslRejectReason::InvalidDebugRecord);

        let mut excessive_records = must_encode(&[], &[]);
        excessive_records[4..8].copy_from_slice(&1_u32.to_le_bytes());
        assert_rejected(&excessive_records, 20, FaslRejectReason::InvalidDebugRecord);
    }

    #[test]
    fn debug_decoder_rejects_invalid_record_references_and_fields() {
        let files = [file("a.lisp")];
        let valid = must_encode(&files, &[record()]);
        let mut bad_reference = valid.clone();
        bad_reference[24..28].copy_from_slice(&1_u32.to_le_bytes());
        assert_rejected(&bad_reference, 100, FaslRejectReason::InvalidReference);

        for (offset, value) in [(28, 4_u32), (32, 9), (36, 1), (40, 0), (44, 0), (48, 3)] {
            let mut bytes = valid.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert_rejected(&bytes, 100, FaslRejectReason::InvalidDebugRecord);
        }

        let mut excessive_variables = valid;
        excessive_variables[56..60].copy_from_slice(&1_u32.to_le_bytes());
        assert_rejected(
            &excessive_variables,
            100,
            FaslRejectReason::InvalidDebugRecord,
        );

        let invalid_layout_id = must_encode(&files, &[record()]);
        assert_eq!(
            decode_section(
                &invalid_layout_id,
                100,
                &[FaslCodeLayout {
                    id: 1,
                    ..layout()[0]
                }]
            ),
            Err(FaslError::at(FaslRejectReason::InvalidReference, 124))
        );
    }

    #[test]
    fn debug_decoder_rejects_invalid_variable_wire_fields() {
        let files = [file("a.lisp")];
        let mut source = record();
        source.variables.push(DebugVariable {
            name: "x".into(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 0,
        });
        let valid = must_encode(&files, &[source]);
        for (offset, value) in [(60, 0_u32), (65, 2), (69, 4)] {
            let mut bytes = valid.clone();
            bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            assert_rejected(&bytes, 100, FaslRejectReason::InvalidDebugRecord);
        }
        let mut invalid_utf8 = valid.clone();
        invalid_utf8[64] = 0xff;
        assert_rejected(&invalid_utf8, 100, FaslRejectReason::InvalidDebugRecord);
        let mut nul_name = valid.clone();
        nul_name[64] = 0;
        assert_rejected(&nul_name, 100, FaslRejectReason::InvalidDebugRecord);
        let mut nonzero_padding = valid;
        nonzero_padding[73] = 1;
        assert_rejected(&nonzero_padding, 100, FaslRejectReason::InvalidDebugRecord);
    }

    #[test]
    fn debug_decoder_rejects_every_strict_prefix_of_a_complete_record() {
        let files = [file("a.lisp")];
        let mut source = record();
        source.variables.push(DebugVariable {
            name: "x".into(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 0,
        });
        let bytes = must_encode(&files, &[source]);
        for end in 0..bytes.len() {
            assert_rejected(&bytes[..end], 500, FaslRejectReason::InvalidDebugRecord);
        }
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

        let duplicate_files = [
            file("a.lisp"),
            DebugFile {
                file_id: 1,
                path: "a.lisp".into(),
            },
        ];
        assert_eq!(validate(&duplicate_files, &[], &layout()), invalid);
        assert_eq!(validate(&files, &[record(), record()], &layout()), invalid);

        for bad in [
            DebugRecord {
                line: 0,
                ..record()
            },
            DebugRecord {
                column: 0,
                ..record()
            },
            DebugRecord {
                source_start: 3,
                ..record()
            },
        ] {
            assert_eq!(validate(&files, &[bad], &layout()), invalid);
        }

        for name in [String::new(), "x\0y".into()] {
            let mut bad_variable = record();
            bad_variable.variables.push(DebugVariable {
                name,
                location_kind: DebugLocationKind::StackSlot,
                location_index: 0,
            });
            assert_eq!(validate(&files, &[bad_variable], &layout()), invalid);
        }

        let mut reverse_names = record();
        for name in ["b", "a"] {
            reverse_names.variables.push(DebugVariable {
                name: name.into(),
                location_kind: DebugLocationKind::StackSlot,
                location_index: 0,
            });
        }
        assert_eq!(validate(&files, &[reverse_names], &layout()), invalid);

        let mut bad_stack_slot = record();
        bad_stack_slot.variables.push(DebugVariable {
            name: "x".into(),
            location_kind: DebugLocationKind::StackSlot,
            location_index: 4,
        });
        assert_eq!(validate(&files, &[bad_stack_slot], &layout()), invalid);
    }

    mod edge_tests {
        include!("fasl_debug_edge_tests.rs");
    }
}
