use std::path::PathBuf;
use std::rc::Rc;

/// A character stream used by the standard I/O primitives.
#[derive(Debug)]
pub struct Stream {
    pub(super) kind: StreamKind,
    pub(super) closed: bool,
    pub(super) delete_on_close: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) enum StreamKind {
    Input {
        characters: Rc<Vec<char>>,
        position: usize,
        pushback: Option<char>,
        file: bool,
    },
    BinaryInput {
        bytes: Rc<Vec<u8>>,
        position: usize,
        pushback: Option<u8>,
    },
    Probe,
    Io {
        characters: Vec<char>,
        committed_characters: Vec<char>,
        position: usize,
        committed_position: usize,
        pushback: Option<char>,
        committed_pushback: Option<char>,
        at_line_start: bool,
        committed_at_line_start: bool,
        output_dirty: bool,
        file_path: Rc<PathBuf>,
    },
    Output {
        buffer: String,
        committed_buffer: String,
        position: usize,
        committed_position: usize,
        at_line_start: bool,
        committed_at_line_start: bool,
        output_dirty: bool,
        file_path: Option<Rc<PathBuf>>,
    },
    BinaryOutput {
        bytes: Vec<u8>,
        committed_bytes: Vec<u8>,
        position: usize,
        committed_position: usize,
        output_dirty: bool,
        file_path: Rc<PathBuf>,
    },
    BinaryIo {
        bytes: Vec<u8>,
        committed_bytes: Vec<u8>,
        position: usize,
        committed_position: usize,
        pushback: Option<u8>,
        committed_pushback: Option<u8>,
        output_dirty: bool,
        file_path: Rc<PathBuf>,
    },
}
