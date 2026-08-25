use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

pub struct Stream {
    kind: StreamKind,
    closed: bool,
}

enum StreamKind {
    Input {
        characters: Rc<Vec<char>>,
        start: usize,
        position: usize,
        pushback: Option<char>,
        file: bool,
    },
    Probe,
    Io {
        characters: Vec<char>,
        position: usize,
        pushback: Option<char>,
        at_line_start: bool,
        file_path: Rc<PathBuf>,
    },
    Output {
        buffer: Vec<char>,
        position: usize,
        at_line_start: bool,
        file_path: Option<Rc<PathBuf>>,
    },
    TwoWay {
        input: Rc<RefCell<Stream>>,
        output: Rc<RefCell<Stream>>,
    },
    Broadcast {
        streams: Vec<Rc<RefCell<Stream>>>,
    },
    Concatenated {
        streams: Vec<Rc<RefCell<Stream>>>,
        current: usize,
    },
    Echo {
        input: Rc<RefCell<Stream>>,
        output: Rc<RefCell<Stream>>,
    },
}

#[path = "stream/common.rs"]
mod common;
#[path = "stream/constructors.rs"]
mod constructors;
#[path = "stream/file.rs"]
mod file;
#[path = "stream/input_basic.rs"]
mod input_basic;
#[path = "stream/input_consume.rs"]
mod input_consume;
#[path = "stream/input_lines.rs"]
mod input_lines;
#[path = "stream/output.rs"]
mod output;
