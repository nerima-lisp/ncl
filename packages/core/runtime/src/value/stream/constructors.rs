use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use super::{Stream, StreamKind};

impl Stream {
    pub(crate) fn input(source: &str, start: usize, end: usize) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().skip(start).take(end - start).collect()),
                start,
                position: 0,
                pushback: None,
                file: false,
            },
            closed: false,
        }
    }

    pub(crate) fn file_input(source: String) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().collect()),
                start: 0,
                position: 0,
                pushback: None,
                file: true,
            },
            closed: false,
        }
    }

    pub(crate) fn file_probe() -> Self {
        Self {
            kind: StreamKind::Probe,
            closed: true,
        }
    }

    pub(crate) fn file_io(path: PathBuf, source: String, append: bool) -> Self {
        let characters: Vec<char> = source.chars().collect();
        let position = if append { characters.len() } else { 0 };
        let at_line_start = if position == 0 {
            true
        } else {
            characters.get(position - 1) == Some(&'\n')
        };
        Self {
            kind: StreamKind::Io {
                characters,
                position,
                pushback: None,
                at_line_start,
                file_path: Rc::new(path),
            },
            closed: false,
        }
    }

    pub(crate) fn output() -> Self {
        Self {
            kind: StreamKind::Output {
                buffer: Vec::new(),
                position: 0,
                at_line_start: true,
                file_path: None,
            },
            closed: false,
        }
    }

    pub(crate) fn file_output(path: PathBuf, initial: String, append: bool) -> Self {
        let buffer: Vec<char> = initial.chars().collect();
        let position = if append { buffer.len() } else { 0 };
        let at_line_start = position == 0 || buffer[position - 1] == '\n';
        Self {
            kind: StreamKind::Output {
                buffer,
                position,
                at_line_start,
                file_path: Some(Rc::new(path)),
            },
            closed: false,
        }
    }

    pub(crate) fn two_way(input: Rc<RefCell<Stream>>, output: Rc<RefCell<Stream>>) -> Self {
        Self {
            kind: StreamKind::TwoWay { input, output },
            closed: false,
        }
    }

    pub(crate) fn broadcast(streams: Vec<Rc<RefCell<Stream>>>) -> Self {
        Self {
            kind: StreamKind::Broadcast { streams },
            closed: false,
        }
    }

    pub(crate) fn concatenated(streams: Vec<Rc<RefCell<Stream>>>) -> Self {
        Self {
            kind: StreamKind::Concatenated {
                streams,
                current: 0,
            },
            closed: false,
        }
    }

    pub(crate) fn echo(input: Rc<RefCell<Stream>>, output: Rc<RefCell<Stream>>) -> Self {
        Self {
            kind: StreamKind::Echo { input, output },
            closed: false,
        }
    }
}
