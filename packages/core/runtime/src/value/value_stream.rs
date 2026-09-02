use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

/// A character stream used by the standard I/O primitives.
#[derive(Debug)]
pub struct Stream {
    pub(super) kind: StreamKind,
    pub(super) closed: bool,
    pub(super) element_type: StreamElementType,
    pub(super) byte_data: Option<ByteStreamData>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StreamElementType {
    Character,
    UnsignedByte8,
}

#[derive(Debug)]
pub(super) enum ByteStreamData {
    Input {
        bytes: Rc<Vec<u8>>,
        position: usize,
    },
    Io {
        bytes: Vec<u8>,
        position: usize,
        file_path: Rc<PathBuf>,
    },
    Output {
        bytes: Vec<u8>,
        position: usize,
        file_path: Rc<PathBuf>,
    },
}

#[derive(Debug)]
pub(super) enum StreamKind {
    Input {
        characters: Rc<Vec<char>>,
        position: usize,
        pushback: Option<char>,
        file: bool,
    },
    Io {
        characters: Vec<char>,
        position: usize,
        pushback: Option<char>,
        at_line_start: bool,
        file_path: Rc<PathBuf>,
    },
    Output {
        buffer: String,
        position: usize,
        destination: Option<Rc<RefCell<String>>>,
        at_line_start: bool,
        file_path: Option<Rc<PathBuf>>,
    },
}
