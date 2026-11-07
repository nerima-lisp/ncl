#[cfg(test)]
mod private_tests {
    use super::super::*;

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
}
