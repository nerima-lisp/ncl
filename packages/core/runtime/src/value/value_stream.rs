use std::path::PathBuf;
use std::rc::Rc;
use std::cell::RefCell;

/// A character stream used by the standard I/O primitives.
#[derive(Debug)]
pub struct Stream {
    pub(super) kind: StreamKind,
    pub(super) closed: bool,
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
        destination: Option<Rc<RefCell<String>>>,
        at_line_start: bool,
        file_path: Option<Rc<PathBuf>>,
    },
}
