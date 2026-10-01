//! Debugger observations over the runtime's stable `ncl-sys` boundary.

use ncl_sys::{CodeRegistry, FrameHeader, SourceLocation, Word, walk_frame_headers};

/// A source location resolved for a native program counter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceLocationSnapshot {
    /// Code-relative offset represented by the location entry.
    pub code_offset: u32,
    /// Source file identifier.
    pub file: String,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

impl From<&SourceLocation> for SourceLocationSnapshot {
    fn from(location: &SourceLocation) -> Self {
        Self {
            code_offset: location.code_offset,
            file: location.file.clone(),
            line: location.line,
            column: location.column,
        }
    }
}

/// One immutable frame observation captured from a frame chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrameSnapshot {
    /// The raw frame header captured at the observation boundary.
    pub header: FrameHeader,
    /// Name from the code metadata, when the return PC belongs to published code.
    pub function_name: Option<String>,
    /// Code-relative return-PC offset, when the return PC belongs to published code.
    pub code_offset: Option<u32>,
    /// Nearest source location at or before the return-PC offset.
    pub source_location: Option<SourceLocationSnapshot>,
}

/// A bounded debugger snapshot of a native frame chain.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebuggerSnapshot {
    /// Frames in innermost-to-outermost order.
    pub frames: Vec<FrameSnapshot>,
}

/// Capture a bounded frame snapshot from caller-provided frame words.
#[must_use]
pub fn snapshot_frames(
    words: &[Word],
    first: usize,
    limit: usize,
    registry: &CodeRegistry,
) -> DebuggerSnapshot {
    let frames = walk_frame_headers(words, first, limit)
        .into_iter()
        .map(|header| {
            let resolved = registry.find(header.return_pc);
            let (function_name, code_offset, source_location) = match resolved {
                Some((metadata, offset)) => (
                    Some(metadata.function_name.clone()),
                    Some(offset),
                    metadata
                        .source_locations
                        .iter()
                        .filter(|location| location.code_offset <= offset)
                        .max_by_key(|location| location.code_offset)
                        .map(SourceLocationSnapshot::from),
                ),
                None => (None, None, None),
            };
            FrameSnapshot {
                header,
                function_name,
                code_offset,
                source_location,
            }
        })
        .collect();
    DebuggerSnapshot { frames }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use super::snapshot_frames;
    use ncl_sys::{CodeObjectMetadata, SourceLocation, Word, alloc_code, publish_code};

    #[test]
    fn snapshot_resolves_function_and_nearest_source_location() {
        let mut code = alloc_code(16).expect("code allocation");
        publish_code(&mut code).expect("publish");
        let mut registry = ncl_sys::CodeRegistry::default();
        registry
            .register(
                &code,
                CodeObjectMetadata {
                    entry_offset: 0,
                    size: 16,
                    frame_words: 4,
                    function_name: "WORK".to_owned(),
                    source_locations: vec![
                        SourceLocation {
                            code_offset: 0,
                            file: "demo.lisp".to_owned(),
                            line: 3,
                            column: 1,
                        },
                        SourceLocation {
                            code_offset: 8,
                            file: "demo.lisp".to_owned(),
                            line: 4,
                            column: 5,
                        },
                    ],
                    constant_slots: Vec::new(),
                    safepoint_map: ncl_sys::SafepointMap::default(),
                    debug_table: Vec::new(),
                },
            )
            .expect("register");
        let words = [
            Word::from_bits(0),
            Word::from_bits((code.address() + 9) as u64),
            Word::NIL,
            Word::from_bits(0),
        ];
        let snapshot = snapshot_frames(&words, 0, 1, &registry);
        assert_eq!(snapshot.frames[0].function_name.as_deref(), Some("WORK"));
        assert_eq!(snapshot.frames[0].code_offset, Some(9));
        assert_eq!(
            snapshot.frames[0].source_location.as_ref().map(|x| x.line),
            Some(4)
        );
    }

    #[test]
    fn snapshot_keeps_unknown_pc_and_bounds_malformed_chain() {
        let words = [
            Word::from_bits(0),
            Word::from_bits(u64::MAX),
            Word::NIL,
            Word::from_bits(0),
        ];
        let snapshot = snapshot_frames(&words, 0, 4, &ncl_sys::CodeRegistry::default());
        assert_eq!(snapshot.frames.len(), 1);
        assert_eq!(snapshot.frames[0].function_name, None);
        assert_eq!(snapshot.frames[0].code_offset, None);
    }
}
