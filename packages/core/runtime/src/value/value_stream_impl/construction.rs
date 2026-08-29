use std::path::PathBuf;
use std::rc::Rc;

use crate::Stream;
use crate::value::value_stream::StreamKind;

impl Stream {
    pub(in crate::value) fn input(source: &str, start: usize, end: usize) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().skip(start).take(end - start).collect()),
                position: 0,
                pushback: None,
                file: false,
            },
            closed: false,
        }
    }

    pub(in crate::value) fn file_input(source: &str) -> Self {
        Self {
            kind: StreamKind::Input {
                characters: Rc::new(source.chars().collect()),
                position: 0,
                pushback: None,
                file: true,
            },
            closed: false,
        }
    }

    pub(in crate::value) fn file_io(path: PathBuf, source: &str, append: bool) -> Self {
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

    pub(in crate::value) const fn output() -> Self {
        Self {
            kind: StreamKind::Output {
                buffer: String::new(),
                at_line_start: true,
                file_path: None,
            },
            closed: false,
        }
    }

    pub(in crate::value) fn file_output(path: PathBuf, initial: String) -> Self {
        let at_line_start = initial.ends_with('\n');
        Self {
            kind: StreamKind::Output {
                buffer: initial,
                at_line_start,
                file_path: Some(Rc::new(path)),
            },
            closed: false,
        }
    }
}
