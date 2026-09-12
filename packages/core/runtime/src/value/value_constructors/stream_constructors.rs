use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use crate::value::RandomState;
use crate::{Stream, Value};

impl Value {
    pub(crate) fn string_input_stream(source: &str, start: usize, end: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::input(source, start, end))))
    }

    pub(crate) fn string_output_stream() -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::output())))
    }

    pub(crate) fn file_input_stream(source: &str) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_input(source))))
    }

    pub(crate) fn binary_input_stream(bytes: Vec<u8>) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::binary_input(bytes))))
    }

    pub(crate) fn file_probe_stream(path: PathBuf) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_probe(path))))
    }

    pub(crate) fn file_output_stream(path: PathBuf, initial: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output(path, initial))))
    }

    pub(crate) fn file_output_stream_at(path: PathBuf, initial: String, position: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output_at(
            path, initial, position,
        ))))
    }

    pub(crate) fn binary_output_stream(path: PathBuf, initial: Vec<u8>, position: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::binary_output(
            path, initial, position,
        ))))
    }

    pub(crate) fn file_io_stream(path: PathBuf, source: &str, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_io(path, source, append))))
    }

    pub(crate) fn binary_io_stream(path: PathBuf, bytes: Vec<u8>, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::binary_io(
            path, bytes, append,
        ))))
    }

    pub(crate) fn set_stream_delete_on_close(&self, path: PathBuf) -> bool {
        let Self::Stream(stream) = self else {
            return false;
        };
        stream.borrow_mut().set_delete_on_close(path);
        true
    }

    pub(crate) fn random_state(state: RandomState) -> Self {
        Self::RandomState(Rc::new(RefCell::new(state)))
    }

    pub(crate) fn random_state_reference(&self) -> Option<Rc<RefCell<RandomState>>> {
        match self {
            Self::RandomState(state) => Some(Rc::clone(state)),
            _ => None,
        }
    }
}
