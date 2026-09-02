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

    pub(crate) fn string_output_stream_to(&self) -> Option<Self> {
        let Self::MutableString(destination) = self else {
            return None;
        };
        Some(Self::Stream(Rc::new(RefCell::new(Stream::output_to(
            Rc::clone(destination),
        )))))
    }

    pub(crate) fn attach_string_output_destination(&self, destination: &Self) -> bool {
        let (Self::Stream(stream), Self::MutableString(destination)) = (self, destination) else {
            return false;
        };
        stream
            .borrow_mut()
            .attach_destination(Rc::clone(destination));
        true
    }

    pub(crate) fn file_input_stream(source: &str) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_input(source))))
    }

    pub(crate) fn file_byte_input_stream(source: Vec<u8>) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_byte_input(source))))
    }

    pub(crate) fn file_byte_output_stream(path: PathBuf, initial: Vec<u8>) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_byte_output(
            path, initial,
        ))))
    }

    pub(crate) fn file_byte_io_stream(path: PathBuf, initial: Vec<u8>, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_byte_io(
            path, initial, append,
        ))))
    }

    pub(crate) fn file_output_stream(path: PathBuf, initial: String) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output(path, initial))))
    }

    pub(crate) fn file_output_stream_at(path: PathBuf, initial: String, position: usize) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_output_at(
            path, initial, position,
        ))))
    }

    pub(crate) fn delete_stream_file_on_close(&self, path: PathBuf) -> bool {
        let Self::Stream(stream) = self else {
            return false;
        };
        stream.borrow_mut().delete_on_close(path);
        true
    }

    pub(crate) fn file_io_stream(path: PathBuf, source: &str, append: bool) -> Self {
        Self::Stream(Rc::new(RefCell::new(Stream::file_io(path, source, append))))
    }

    pub(crate) fn random_state(state: RandomState) -> Self {
        Self::RandomState(Rc::new(RefCell::new(state)))
    }

    pub(crate) fn random_state_from_reference(state: Rc<RefCell<RandomState>>) -> Self {
        Self::RandomState(state)
    }

    pub(crate) fn random_state_reference(&self) -> Option<Rc<RefCell<RandomState>>> {
        match self {
            Self::RandomState(state) => Some(Rc::clone(state)),
            _ => None,
        }
    }
}
