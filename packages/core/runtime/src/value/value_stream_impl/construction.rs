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
            delete_on_close: None,
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
            delete_on_close: None,
        }
    }

    pub(in crate::value) fn binary_input(bytes: Vec<u8>) -> Self {
        Self {
            kind: StreamKind::BinaryInput {
                bytes: Rc::new(bytes),
                position: 0,
                pushback: None,
            },
            closed: false,
            delete_on_close: None,
        }
    }

    pub(in crate::value) fn file_byte_input(bytes: Vec<u8>) -> Self {
        Self::binary_input(bytes)
    }

    pub(in crate::value) fn file_probe(_path: PathBuf) -> Self {
        Self {
            kind: StreamKind::Probe,
            closed: false,
            delete_on_close: None,
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
                committed_characters: characters.clone(),
                characters,
                position,
                committed_position: position,
                pushback: None,
                committed_pushback: None,
                at_line_start,
                committed_at_line_start: at_line_start,
                output_dirty: false,
                file_path: Rc::new(path),
            },
            closed: false,
            delete_on_close: None,
        }
    }

    pub(in crate::value) const fn output() -> Self {
        Self {
            kind: StreamKind::Output {
                buffer: String::new(),
                committed_buffer: String::new(),
                position: 0,
                committed_position: 0,
                at_line_start: true,
                committed_at_line_start: true,
                output_dirty: false,
                file_path: None,
            },
            closed: false,
            delete_on_close: None,
        }
    }

    pub(in crate::value) fn file_output(path: PathBuf, initial: String) -> Self {
        let position = initial.chars().count();
        Self::file_output_at(path, initial, position)
    }

    pub(in crate::value) fn file_output_at(
        path: PathBuf,
        initial: String,
        position: usize,
    ) -> Self {
        let characters: Vec<char> = initial.chars().collect();
        let at_line_start = if position == 0 {
            true
        } else {
            characters.get(position - 1) == Some(&'\n')
        };
        Self {
            kind: StreamKind::Output {
                committed_buffer: initial.clone(),
                buffer: initial,
                position,
                committed_position: position,
                at_line_start,
                committed_at_line_start: at_line_start,
                output_dirty: false,
                file_path: Some(Rc::new(path)),
            },
            closed: false,
            delete_on_close: None,
        }
    }

    pub(in crate::value) fn binary_output(path: PathBuf, bytes: Vec<u8>, position: usize) -> Self {
        Self {
            kind: StreamKind::BinaryOutput {
                committed_bytes: bytes.clone(),
                bytes,
                position,
                committed_position: position,
                output_dirty: false,
                file_path: Rc::new(path),
            },
            closed: false,
            delete_on_close: None,
        }
    }

    pub(in crate::value) fn file_byte_output(path: PathBuf, bytes: Vec<u8>) -> Self {
        let position = bytes.len();
        Self::binary_output(path, bytes, position)
    }

    pub(in crate::value) fn binary_io(path: PathBuf, bytes: Vec<u8>, append: bool) -> Self {
        let position = if append { bytes.len() } else { 0 };
        Self {
            kind: StreamKind::BinaryIo {
                committed_bytes: bytes.clone(),
                bytes,
                position,
                committed_position: position,
                pushback: None,
                committed_pushback: None,
                output_dirty: false,
                file_path: Rc::new(path),
            },
            closed: false,
            delete_on_close: None,
        }
    }

    pub(in crate::value) fn file_byte_io(path: PathBuf, bytes: Vec<u8>, append: bool) -> Self {
        Self::binary_io(path, bytes, append)
    }
}
