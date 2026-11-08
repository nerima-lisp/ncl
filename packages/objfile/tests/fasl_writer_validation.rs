//! Writer-level cross-section rejection coverage.

use ncl_objfile::{
    Architecture, DebugFile, DebugRecord, FaslCodeObject, FaslConstantGroup, FaslError,
    FaslMetadata, FaslRejectReason, FaslV1, FaslV1Writer, SafepointEntry, SafepointGroup,
};

fn fixture() -> FaslV1<'static> {
    FaslV1 {
        architecture: Architecture::X86_64,
        runtime_abi_version: 1,
        metadata: FaslMetadata {
            semantic_version: "1".to_owned(),
            compiler_build_identity: [0; 32],
            codegen_abi_revision: 1,
            target_triple: "x86_64-unknown-linux-gnu".to_owned(),
            source_sha256: [0; 32],
            policies: Vec::new(),
            features: Vec::new(),
        },
        symbols: Vec::new(),
        constants: vec![FaslConstantGroup {
            code_object_id: 0,
            constants: Vec::new(),
        }],
        code: vec![FaslCodeObject {
            id: 0,
            entry_offset: 0,
            frame_words: 3,
            constant_count: 0,
            stack_map_count: 0,
            debug_record_count: 0,
            code: &[0x90],
        }],
        relocations: Vec::new(),
        stack_maps: vec![SafepointGroup {
            code_object_id: 0,
            entries: Vec::new(),
        }],
        debug_files: Vec::new(),
        debug_records: Vec::new(),
        top_level: Vec::new(),
    }
}

#[test]
fn native_writer_rejects_stack_map_frame_mismatch_across_sections() {
    let mut fasl = fixture();
    fasl.code[0].stack_map_count = 1;
    fasl.stack_maps[0].entries.push(SafepointEntry {
        pc_offset: 0,
        frame_words: 4,
        slot_words: 3,
        word_slot_count: 1,
        register_mask: 0,
        map_flags: 0,
        slot_bitmap: vec![0b0000_0100],
        register_ids: Vec::new(),
    });
    assert_eq!(
        FaslV1Writer::write(&fasl),
        Err(FaslError::new(FaslRejectReason::InvalidSafepointMap))
    );
}

#[test]
fn native_writer_rejects_debug_ranges_outside_the_code_object() {
    let mut fasl = fixture();
    fasl.code[0].debug_record_count = 1;
    fasl.debug_files.push(DebugFile {
        file_id: 0,
        path: "source.lisp".to_owned(),
    });
    fasl.debug_records.push(DebugRecord {
        code_object_id: 0,
        pc_start: 0,
        pc_end: 2,
        file_id: 0,
        line: 1,
        column: 1,
        source_start: 0,
        source_end: 1,
        variables: Vec::new(),
    });
    assert_eq!(
        FaslV1Writer::write(&fasl),
        Err(FaslError::new(FaslRejectReason::InvalidDebugRecord))
    );
}
